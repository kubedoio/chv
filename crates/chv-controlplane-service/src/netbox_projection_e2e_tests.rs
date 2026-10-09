//! NetBox projection composed end-to-end suite — PR 8 of the #239
//! plan (`docs/plans/2026-10-08-netbox-projection-implementation-plan.md`,
//! "Integration leg").
//!
//! Where the worker suite (`netbox_projection_worker_tests.rs`) proves
//! each mechanism in isolation, these tests prove the **composition**
//! as one flowing scenario per the plan's PR-8 wording:
//!
//! ```text
//! topology → apply → post-apply enqueue → worker → mock NetBox
//!   (assert objects + custom fields) → dry-run no-op
//!   → topology update + re-apply → update entries
//!   → removed resource → stale entry
//!   → outage leg (mock down) → apply result still Succeeded
//! ```
//!
//! Everything is driven through the real composition root —
//! [`crate::NetboxProjectionWorker::tick`] (reclaim → post-apply sweep
//! → claim) over a real in-memory store and a real HTTP wire
//! (wiremock) — reusing the worker suite's fixtures rather than
//! duplicating them.
//!
//! Documented deviation (PR 6, see the plan's deviation note): the
//! apply-run terminal transition site does not exist —
//! `apply_plan` never writes `RunStatus::Succeeded` (the orchestrator
//! owns the terminal transitions and is not implemented yet). The
//! "apply" steps of the scenario therefore seed a `succeeded` apply
//! run through the same fixture the worker suite uses
//! ([`add_succeeded_apply_run`]); the post-apply sweep fires on the
//! very next tick either way.

use std::collections::BTreeMap;

use chv_architecture_validate::model::{CHVArchitecture, InstanceResources, Network, NetworkType};
use chv_controlplane_store::ApplyRunRepository;
use chv_controlplane_types::architecture::{
    NetboxProjectionRunStatus, NetboxProjectionTrigger, RunStatus,
};
use chv_netbox_adapter::ownership::CustomFieldNames;
use chv_netbox_adapter::plan::RetentionPolicy;
use chv_netbox_adapter::{
    NetBoxClient, NetBoxKind, NetBoxObject, NetBoxToken, NetboxEntryStatus, NetboxPlanAction,
    NetboxProjectionInput, NetboxProjectionRunner,
};
use serde_json::{json, Value};
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::netbox_projection_worker_tests::{
    add_succeeded_apply_run, appid, audit_events, backdate_apply_run, desired_objects,
    fixture_architecture, get_run, kind_path, mount_creates, mount_empty_lists,
    mount_ip_fixup_patch, outcome_of, page, remote_fixture, request_count, request_paths,
    setup_post_apply_config, setup_projection, setup_topology_and_version, worker_for,
    CREATE_MOCKS, SITE, TOKEN,
};

// ---------------------------------------------------------------------------
// Local helpers (thin generalizations of the worker suite's fixtures)
// ---------------------------------------------------------------------------

/// The desired v1 objects paired with the NetBox ids the phase-2
/// create mocks handed out (per-kind `CREATE_MOCKS`) — "what the mock
/// NetBox now holds after the first export".
fn v1_objects_at_created_ids(architecture_id: &str) -> Vec<(i64, NetBoxObject)> {
    desired_objects(architecture_id)
        .into_iter()
        .map(|object| {
            let (_, netbox_id) = CREATE_MOCKS
                .iter()
                .find(|(mock_path, _)| *mock_path == kind_path(&object))
                .expect("a create mock exists for every mapped kind");
            (*netbox_id, object)
        })
        .collect()
}

/// [`crate::netbox_projection_worker_tests::mount_mirror_remote`]
/// generalized to caller-chosen NetBox ids: the e2e asserts that the
/// re-apply's update entries PATCH the *exact objects the phase-2
/// creates returned* (ids 101–106), so the mirrored remote state must
/// reuse those ids instead of the worker suite's 400+i scheme. Like
/// the original, one path-only GET mock per kind serves both the
/// architecture-filter lists and the natural-key probes.
async fn mount_remote_state(server: &MockServer, objects: &[(i64, NetBoxObject)]) {
    let mut by_path: BTreeMap<&'static str, Vec<Value>> = BTreeMap::new();
    for (netbox_id, object) in objects {
        by_path
            .entry(kind_path(object))
            .or_default()
            .push(remote_fixture(*netbox_id, object));
    }
    for (kind_path, results) in by_path {
        Mock::given(method("GET"))
            .and(path(kind_path))
            .respond_with(ResponseTemplate::new(200).set_body_json(page(results)))
            .mount(server)
            .await;
    }
}

/// The v2 topology: `vm-01`'s resources are MODIFIED, the server
/// `chv-node-01` is REMOVED (its device goes stale under
/// `mark_stale` retention), and the network `frontend` is ADDED.
/// The VM's `placement.server` no longer resolves, so its `device`
/// reference drops to unset — a realistic consequence of the removal.
fn v2_architecture() -> CHVArchitecture {
    let mut arch = fixture_architecture();
    arch.instances[0].resources = Some(InstanceResources {
        cpu: Some(4),
        memory_mb: Some(4096),
    });
    arch.servers.clear();
    arch.networks.push(Network {
        name: "frontend".to_string(),
        network_type: NetworkType::Vlan,
        bridge: None,
        vlan_id: Some(7),
        cidr: Some("10.7.0.0/24".to_string()),
        gateway: None,
        dns: Vec::new(),
        dhcp: None,
    });
    arch
}

/// Parse one recorded request's JSON body.
fn body_of(request: &wiremock::Request) -> Value {
    serde_json::from_slice(&request.body).expect("request body is JSON")
}

/// All recorded requests for one method + exact path.
async fn requests_at(server: &MockServer, http_method: &str, exact_path: &str) -> Vec<Value> {
    server
        .received_requests()
        .await
        .expect("request recording enabled")
        .into_iter()
        .filter(|r| r.method.as_str() == http_method && r.url.path() == exact_path)
        .map(|r| body_of(&r))
        .collect()
}

// ---------------------------------------------------------------------------
// Test A — the full lifecycle, as one flowing scenario
// ---------------------------------------------------------------------------

/// Happy path + idempotency + update + removal, in numbered phases
/// mirroring the plan's PR-8 scenario lines.
#[tokio::test]
async fn full_lifecycle_apply_to_projection_to_reapply() {
    let db = chv_controlplane_store::test_util::TestDb::new().await;
    let server = MockServer::start().await;
    let model_v1 = crate::netbox_projection_worker_tests::model_json();

    // -----------------------------------------------------------------
    // Phase 1 — topology + version v1 + a SUCCEEDED apply run + a
    // post-apply-enabled config (mark_stale retention).
    //
    // Deviation note: the apply terminal transition site does not
    // exist yet (PR 6's documented deviation — the orchestrator owns
    // the terminal `Succeeded` transition and is not implemented), so
    // the succeeded apply run is seeded via the fixture rather than
    // driven through `apply_plan`.
    // -----------------------------------------------------------------
    setup_topology_and_version(&db, "topo-e2e", "v-1", &model_v1, 1).await;
    add_succeeded_apply_run(&db, "apply-1", "topo-e2e", "v-1").await;
    backdate_apply_run(&db, "apply-1", "2020-01-01T00:00:00Z").await;
    setup_post_apply_config(&db, &server.uri(), "topo-e2e").await;

    // Phase 2 mocks: NetBox holds nothing of ours; every kind creates.
    crate::netbox_projection_worker_tests::mount_empty_remote(&server).await;
    mount_creates(&server, &CREATE_MOCKS).await;
    mount_ip_fixup_patch(&server).await;

    // -----------------------------------------------------------------
    // Phase 2 — one worker tick: the post-apply sweep enqueues a
    // post_apply export run for v1 AND the same tick's claim loop
    // executes it to `succeeded` against the mock NetBox.
    // -----------------------------------------------------------------
    worker_for(&db).tick().await.expect("tick succeeds");

    let runs = crate::netbox_projection_worker_tests::post_apply_runs(&db, "topo-e2e").await;
    assert_eq!(runs.len(), 1, "exactly one post_apply run: {runs:?}");
    let first = &runs[0];
    assert_eq!(first.trigger_kind, NetboxProjectionTrigger::PostApply);
    assert_eq!(first.status, NetboxProjectionRunStatus::Succeeded);
    assert!(
        first.finished_at.is_some(),
        "the run executed to terminal state"
    );

    // Wire: one create per mapped kind, plus the post-loop IP-assignment
    // fix-up; nothing deleted, nothing else written.
    assert_eq!(request_count(&server, "POST", "/api/").await, 6);
    assert_eq!(request_count(&server, "PATCH", "/api/").await, 1);
    assert_eq!(request_count(&server, "DELETE", "/api/").await, 0);

    // The create payloads carry the ownership markers, natural keys,
    // site, and enriched custom fields of the mapping contract.
    let vm_creates = requests_at(&server, "POST", "/api/virtualization/virtual-machines/").await;
    assert_eq!(vm_creates.len(), 1, "one VM create");
    let vm = &vm_creates[0];
    assert_eq!(vm.get("name").and_then(Value::as_str), Some("vm-01"));
    assert_eq!(vm.get("status").and_then(Value::as_str), Some("active"));
    assert_eq!(vm.get("vcpus").and_then(Value::as_i64), Some(2));
    assert_eq!(vm.get("memory").and_then(Value::as_i64), Some(2048));
    assert_eq!(
        vm.get("device")
            .and_then(|d| d.get("name"))
            .and_then(Value::as_str),
        Some("chv-node-01"),
        "VM placed on its declared server"
    );
    let vm_fields = vm.get("custom_fields").expect("vm custom fields");
    assert_eq!(
        vm_fields.get("chv_external_id").and_then(Value::as_str),
        Some("arch:topo-e2e:instance/vm-01:1")
    );
    assert_eq!(
        vm_fields.get("chv_managed_by").and_then(Value::as_str),
        Some("chv"),
        "the ownership write-guard marker"
    );
    assert_eq!(
        vm_fields.get("chv_architecture_id").and_then(Value::as_str),
        Some("topo-e2e")
    );
    assert_eq!(
        vm_fields
            .get("chv_architecture_version")
            .and_then(Value::as_str),
        Some("1")
    );
    assert_eq!(
        vm_fields.get("chv_mapping_version").and_then(Value::as_str),
        Some("v1")
    );
    assert_eq!(
        vm_fields.get("chv_managed_state").and_then(Value::as_str),
        Some("active")
    );

    let device_creates = requests_at(&server, "POST", "/api/dcim/devices/").await;
    assert_eq!(device_creates.len(), 1, "one device create");
    let device = &device_creates[0];
    assert_eq!(
        device.get("name").and_then(Value::as_str),
        Some("chv-node-01")
    );
    assert_eq!(
        device
            .get("site")
            .and_then(|s| s.get("name"))
            .and_then(Value::as_str),
        Some(SITE),
        "site comes from the projection config"
    );
    let device_fields = device.get("custom_fields").expect("device custom fields");
    assert_eq!(
        device_fields.get("chv_external_id").and_then(Value::as_str),
        Some("arch:topo-e2e:server/chv-node-01:1")
    );
    assert_eq!(
        device_fields.get("chv_cpu_cores").and_then(Value::as_str),
        Some("4"),
        "declared server resources project as enrichment custom fields"
    );
    assert_eq!(
        device_fields.get("chv_memory_gb").and_then(Value::as_str),
        Some("8")
    );
    assert_eq!(
        device_fields.get("chv_owner").and_then(Value::as_str),
        Some("alice"),
        "metadata.owner projects as the chv_owner custom field"
    );

    let vlan_creates = requests_at(&server, "POST", "/api/ipam/vlans/").await;
    assert_eq!(vlan_creates.len(), 1, "one VLAN create");
    assert_eq!(vlan_creates[0].get("vid").and_then(Value::as_i64), Some(42));
    assert_eq!(
        vlan_creates[0].get("name").and_then(Value::as_str),
        Some("backend")
    );

    // Every mutation is authenticated with the configured token.
    let requests = server.received_requests().await.expect("recording");
    assert!(
        requests
            .iter()
            .filter(|r| r.method.as_str() == "POST")
            .all(|r| {
                r.headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|v| v == format!("Token {TOKEN}"))
            }),
        "all create requests carry Authorization: Token <configured token>"
    );

    // The single PATCH is the post-loop IP-assignment fix-up. The
    // assignment id itself cannot resolve yet — the mock's read side
    // does not reflect the interface created moments earlier in the
    // same run (real NetBox would) — so the fix-up re-sends the
    // address body; the id-binding form is asserted in phase 4, where
    // the mirrored remote does hold the interface.
    let fixups = requests_at(&server, "PATCH", "/api/ipam/ip-addresses/103/").await;
    assert_eq!(fixups.len(), 1, "exactly one IP fix-up PATCH");
    assert_eq!(
        fixups[0].get("address").and_then(Value::as_str),
        Some("10.42.0.5")
    );

    // -----------------------------------------------------------------
    // Phase 3 — dry-run over the now-mirrored state: the plan is a
    // no-op. The input is built exactly the way the worker (and the
    // BFF's synchronous dry-run endpoint) builds it: the applied v1
    // model + the config's pure slice.
    // -----------------------------------------------------------------
    server.reset().await;
    mount_remote_state(&server, &v1_objects_at_created_ids("topo-e2e")).await;

    let architecture: CHVArchitecture =
        serde_json::from_str(&model_v1).expect("applied model parses");
    let input = NetboxProjectionInput {
        architecture: &architecture,
        architecture_id: "topo-e2e",
        architecture_version: 1,
        snapshot: None,
        site_name: Some(SITE),
        retention: RetentionPolicy::MarkStale,
        names: CustomFieldNames::new("chv_"),
    };
    let runner = NetboxProjectionRunner::new(
        NetBoxClient::new_unchecked_for_tests(&server.uri(), NetBoxToken::new(TOKEN.to_string()))
            .expect("test client"),
    );
    let plan = runner.dry_run(&input).await.expect("dry run computes");
    assert_eq!(plan.summary.create, 0, "nothing left to create: {plan:?}");
    assert_eq!(plan.summary.update, 0);
    assert_eq!(
        plan.summary.no_op, 6,
        "every entry matched the remote mirror"
    );
    assert_eq!(plan.summary.conflict, 0);
    assert_eq!(plan.summary.stale, 0);

    // Deterministic: the same input serializes byte-identically.
    let first_json = serde_json::to_string(&plan).expect("plan serializes");
    let again = runner.dry_run(&input).await.expect("second dry run");
    assert_eq!(
        serde_json::to_string(&again).expect("plan serializes"),
        first_json,
        "dry-run output must be byte-stable for identical inputs"
    );

    // Secret-free: the plan JSON carries no token material.
    assert!(!first_json.contains(TOKEN), "the plan must be secret-free");

    // A dry run never mutates NetBox.
    assert_eq!(request_count(&server, "POST", "/api/").await, 0);
    assert_eq!(request_count(&server, "PATCH", "/api/").await, 0);
    assert_eq!(request_count(&server, "DELETE", "/api/").await, 0);

    // -----------------------------------------------------------------
    // Phase 4 — topology update at v2 (one modified, one removed, one
    // added) + a second SUCCEEDED apply run, backdated after v1's.
    // The next tick enqueues a fresh post_apply run for v2 and
    // executes it: update entries PATCH the objects created in phase
    // 2, the added network creates, and the removed server's device
    // is handled per mark_stale retention.
    // -----------------------------------------------------------------
    let model_v2 = serde_json::to_string(&v2_architecture()).expect("model v2 serializes");
    chv_controlplane_store::VersionRepository::new(db.pool.clone())
        .create(chv_controlplane_store::VersionCreateInput {
            id: crate::netbox_projection_worker_tests::vid("v-2"),
            architecture_id: crate::netbox_projection_worker_tests::aid("topo-e2e"),
            version_number: 2,
            yaml_content: "x".to_string(),
            design_graph_json: None,
            normalized_model_json: Some(model_v2),
            change_summary: None,
            created_by: None,
        })
        .await
        .expect("version v-2 created");
    add_succeeded_apply_run(&db, "apply-2", "topo-e2e", "v-2").await;

    // The re-apply needs create + PATCH mocks on top of the mirror.
    mount_creates(&server, &CREATE_MOCKS).await;
    Mock::given(method("PATCH"))
        .and(path_regex("^/api/"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let posts_before = request_count(&server, "POST", "/api/").await;
    let patches_before = request_count(&server, "PATCH", "/api/").await;

    worker_for(&db).tick().await.expect("tick succeeds");

    let runs = crate::netbox_projection_worker_tests::post_apply_runs(&db, "topo-e2e").await;
    assert_eq!(
        runs.len(),
        2,
        "one post_apply run per applied version: {runs:?}"
    );
    let second = runs
        .iter()
        .find(|run| {
            run.architecture_version_id == crate::netbox_projection_worker_tests::vid("v-2")
        })
        .expect("the v-2 post_apply run");
    assert_eq!(second.status, NetboxProjectionRunStatus::Succeeded);

    // The run projected the v2 model (provenance envelope).
    let envelope: Value = serde_json::from_str(second.result_json.as_deref().expect("result json"))
        .expect("envelope parses");
    assert_eq!(
        envelope
            .get("resolved_architecture_version_id")
            .and_then(Value::as_str),
        Some("v-2")
    );

    // Plan: 2 creates (the added frontend VLAN + prefix), 5 updates
    // (every still-desired object — a version bump rewrites the
    // external-id custom field, so even unchanged content diffs), and
    // exactly 1 stale entry (the removed server's device).
    let outcome = outcome_of(second);
    assert!(outcome.error.is_none());
    assert_eq!(outcome.plan.summary.create, 2);
    assert_eq!(outcome.plan.summary.update, 5);
    assert_eq!(outcome.plan.summary.stale, 1);
    assert_eq!(outcome.plan.summary.no_op, 0);
    assert_eq!(outcome.plan.summary.conflict, 0);
    assert_eq!(
        outcome.summary.succeeded, 8,
        "2 creates + 5 updates + 1 stale mark"
    );
    assert_eq!(outcome.summary.failed, 0);
    assert_eq!(outcome.summary.not_attempted, 0);

    // The modified VM's update carries v2's resources to the object
    // created in phase 2 (NetBox id 105).
    let vm_updates = requests_at(
        &server,
        "PATCH",
        "/api/virtualization/virtual-machines/105/",
    )
    .await;
    assert_eq!(
        vm_updates.len(),
        1,
        "the VM update PATCHes the phase-2 object"
    );
    assert_eq!(vm_updates[0].get("vcpus").and_then(Value::as_i64), Some(4));
    assert_eq!(
        vm_updates[0].get("memory").and_then(Value::as_i64),
        Some(4096)
    );
    assert_eq!(
        vm_updates[0]
            .get("custom_fields")
            .and_then(|f| f.get("chv_architecture_version"))
            .and_then(Value::as_str),
        Some("2"),
        "the update refreshes the version provenance custom field"
    );
    assert!(
        vm_updates[0]
            .get("device")
            .map(Value::is_null)
            .unwrap_or(false),
        "the removed server drops the VM's device reference"
    );

    // Wire deltas: 2 creates (the added network's VLAN + prefix) …
    assert_eq!(
        request_count(&server, "POST", "/api/").await - posts_before,
        2
    );
    assert_eq!(
        request_paths(&server, "POST").await,
        vec!["/api/ipam/vlans/", "/api/ipam/prefixes/"],
        "only the added frontend VLAN + prefix create"
    );

    // … and the PATCH set is exactly the five updates to the phase-2
    // object ids, the stale mark on the removed server's device, and
    // the post-loop IP fix-up — in the plan's deterministic order
    // (kind rank, then name, then action rank). Against the mirrored
    // remote the fix-up resolves the interface id (NetBox id 104).
    assert_eq!(
        request_count(&server, "PATCH", "/api/").await - patches_before,
        7
    );
    let mut patch_paths = request_paths(&server, "PATCH").await;
    patch_paths.drain(..patches_before);
    assert_eq!(
        patch_paths,
        vec![
            "/api/ipam/vlans/101/",
            "/api/ipam/prefixes/102/",
            "/api/ipam/ip-addresses/103/",
            "/api/virtualization/interfaces/104/",
            "/api/virtualization/virtual-machines/105/",
            "/api/dcim/devices/106/",
            "/api/ipam/ip-addresses/103/",
        ],
        "updates hit the phase-2 objects by their NetBox ids; the last \
         PATCH is the post-loop IP fix-up"
    );
    let fixups = requests_at(&server, "PATCH", "/api/ipam/ip-addresses/103/").await;
    assert_eq!(fixups.len(), 2, "the update + the fix-up");
    assert_eq!(
        fixups[1].get("assigned_object_id").and_then(Value::as_i64),
        Some(104),
        "the fix-up binds the address to the mirrored interface"
    );
    assert_eq!(
        fixups[1]
            .get("assigned_object_type")
            .and_then(Value::as_str),
        Some("virtualization.vminterface")
    );

    // mark_stale retention (mapping contract "Retention"): the removed
    // resource's device is PATCHed with the stale marker + NetBox's
    // decommissioning status — and NOTHING is ever deleted.
    let stale_marks = requests_at(&server, "PATCH", "/api/dcim/devices/106/").await;
    assert_eq!(stale_marks.len(), 1);
    assert_eq!(
        stale_marks[0]
            .get("custom_fields")
            .and_then(|f| f.get("chv_managed_state"))
            .and_then(Value::as_str),
        Some("stale")
    );
    assert_eq!(
        stale_marks[0].get("status").and_then(Value::as_str),
        Some("decommissioning")
    );
    assert_eq!(
        request_count(&server, "DELETE", "/api/").await,
        0,
        "under mark_stale retention nothing is ever deleted"
    );

    // -----------------------------------------------------------------
    // Phase 5 — idempotency: another tick with no new apply enqueues
    // nothing and writes nothing further.
    // -----------------------------------------------------------------
    let posts_after = request_count(&server, "POST", "/api/").await;
    let patches_after = request_count(&server, "PATCH", "/api/").await;
    let deletes_after = request_count(&server, "DELETE", "/api/").await;

    worker_for(&db)
        .tick()
        .await
        .expect("idempotency tick succeeds");

    assert_eq!(
        crate::netbox_projection_worker_tests::post_apply_runs(&db, "topo-e2e")
            .await
            .len(),
        2,
        "no additional post_apply run"
    );
    assert_eq!(request_count(&server, "POST", "/api/").await, posts_after);
    assert_eq!(
        request_count(&server, "PATCH", "/api/").await,
        patches_after
    );
    assert_eq!(
        request_count(&server, "DELETE", "/api/").await,
        deletes_after
    );

    // -----------------------------------------------------------------
    // Phase 6 — audit trail: both executions' export_succeeded events
    // exist in the events table, correlated by run id.
    // -----------------------------------------------------------------
    for run in crate::netbox_projection_worker_tests::post_apply_runs(&db, "topo-e2e").await {
        let events = audit_events(&db, &run.id.to_string()).await;
        assert!(
            events
                .iter()
                .any(|(message, _)| message == "architecture_netbox_export_succeeded"),
            "expected export_succeeded event for run {}: {events:?}",
            run.id
        );
    }
    // The v2 run's event carries its plan summary (2 creates, 1 stale).
    let events = audit_events(&db, &second.id.to_string()).await;
    assert!(
        events.iter().any(
            |(message, details)| message == "architecture_netbox_export_succeeded"
                && details
                    .as_deref()
                    .is_some_and(|d| d.contains("\"create\":2") && d.contains("\"stale\":1"))
        ),
        "expected the v2 summary in the audit event: {events:?}"
    );
}

// ---------------------------------------------------------------------------
// Test B — outage isolation (the issue's headline AC)
// ---------------------------------------------------------------------------

/// A NetBox outage during the post-apply projection fails the
/// projection run (retryable, backoff-gated) and leaves the apply run
/// that triggered it untouched: still `Succeeded`, `finished_at`
/// exactly as seeded. The sweep stays idempotent across ticks.
#[tokio::test]
async fn netbox_outage_never_changes_the_apply_result() {
    let db = chv_controlplane_store::test_util::TestDb::new().await;
    let model = crate::netbox_projection_worker_tests::model_json();

    // Phase 1 — topology + v1 + a SUCCEEDED apply run whose terminal
    // timestamps are the isolation canary, + a post-apply config
    // pointed at a dead endpoint (nothing listens on port 1).
    setup_topology_and_version(&db, "topo-outage", "v-1", &model, 1).await;
    add_succeeded_apply_run(&db, "apply-outage", "topo-outage", "v-1").await;
    sqlx::query(
        "UPDATE architecture_apply_runs SET \
         started_at = '2025-01-01T00:00:00Z', finished_at = '2025-01-01T00:01:00Z' \
         WHERE id = 'apply-outage'",
    )
    .execute(&db.pool)
    .await
    .expect("stamp apply-run terminal timestamps");
    setup_post_apply_config(&db, "http://127.0.0.1:1", "topo-outage").await;

    // Phase 2 — the tick enqueues the post_apply run and executes it
    // against the dead endpoint: the run fails with the transient
    // (retryable) outage classification and is auto-requeued with a
    // backoff.
    worker_for(&db)
        .tick()
        .await
        .expect("outage never kills the worker");

    let runs = crate::netbox_projection_worker_tests::post_apply_runs(&db, "topo-outage").await;
    assert_eq!(runs.len(), 1, "the sweep created exactly one run: {runs:?}");
    let run = &runs[0];
    assert_eq!(
        run.status,
        NetboxProjectionRunStatus::Queued,
        "transient failure auto-requeued"
    );
    assert_eq!(
        run.attempt_count, 1,
        "the failed execution consumed an attempt"
    );
    let error = run.error_message.as_deref().expect("error message");
    assert!(
        error.contains("unreachable"),
        "outage classification: {error}"
    );
    assert!(!error.contains(TOKEN), "the error is token-free");
    assert!(
        run.next_attempt_at
            .is_some_and(|at| at > chrono::Utc::now()),
        "retry backoff scheduled in the future: {:?}",
        run.next_attempt_at
    );

    // Phase 3 — THE assertion: the apply run is still Succeeded with
    // its terminal timestamps exactly as seeded. The projection
    // outage did not touch the apply result.
    let apply = ApplyRunRepository::new(db.pool.clone())
        .get(&appid("apply-outage"), None)
        .await
        .expect("apply run lookup");
    assert_eq!(apply.status, RunStatus::Succeeded);
    let seeded_finished_at = chrono::DateTime::parse_from_rfc3339("2025-01-01T00:01:00Z")
        .expect("seeded finished_at parses")
        .with_timezone(&chrono::Utc);
    assert_eq!(
        apply.finished_at,
        Some(seeded_finished_at),
        "the projection outage must not touch the apply run's finished_at"
    );

    // Phase 4 — idempotency across ticks: the failed post_apply run
    // is not re-enqueued (a post_apply run of any status counts as
    // already attempted), and the backoff gates the claim.
    worker_for(&db).tick().await.expect("second tick survives");
    let runs = crate::netbox_projection_worker_tests::post_apply_runs(&db, "topo-outage").await;
    assert_eq!(runs.len(), 1, "no re-enqueue on the second tick");
    assert_eq!(
        runs[0].attempt_count, 1,
        "the backoff prevented an early retry"
    );
    assert_eq!(runs[0].status, NetboxProjectionRunStatus::Queued);
}

// ---------------------------------------------------------------------------
// Test C — foreign objects are never modified (wire-level)
// ---------------------------------------------------------------------------

/// An object in the mock NetBox that matches a desired natural key
/// but carries no CHV ownership marker is a `conflict`: the plan
/// records it, the export proceeds with the remaining entries, and
/// the wire shows NO write to the foreign object — not even under
/// `delete`-adjacent pressure, and no DELETE is ever sent.
///
/// The worker suite proves the pure semantics compositionally
/// (`foreign_occupied_natural_key_is_conflict_and_never_deleted`);
/// this test pins them at the composed, wire level as the plan's
/// "Proves" list requires.
#[tokio::test]
async fn foreign_object_at_natural_key_is_never_written() {
    let db = chv_controlplane_store::test_util::TestDb::new().await;
    let server = MockServer::start().await;

    setup_projection(
        &db,
        &server.uri(),
        "topo-foreign",
        "v-1",
        "apply-foreign",
        "netrun-foreign",
        chv_controlplane_types::architecture::NetboxRetentionPolicy::MarkStale,
    )
    .await;

    // A foreign VLAN (no ownership marker at all) squatting on the
    // desired vid.
    let foreign_vlan = json!({
        "id": 55,
        "vid": 42,
        "name": "netops-backend",
        "tags": [],
        "custom_fields": {},
    });
    Mock::given(method("GET"))
        .and(path("/api/ipam/vlans/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(vec![foreign_vlan])))
        .mount(&server)
        .await;
    mount_empty_lists(
        &server,
        &[
            "/api/ipam/prefixes/",
            "/api/ipam/ip-addresses/",
            "/api/virtualization/interfaces/",
            "/api/virtualization/virtual-machines/",
            "/api/dcim/devices/",
        ],
    )
    .await;
    mount_creates(
        &server,
        &[
            ("/api/ipam/prefixes/", 102),
            ("/api/ipam/ip-addresses/", 103),
            ("/api/virtualization/interfaces/", 104),
            ("/api/virtualization/virtual-machines/", 105),
            ("/api/dcim/devices/", 106),
        ],
    )
    .await;
    mount_ip_fixup_patch(&server).await;

    worker_for(&db).tick().await.expect("tick succeeds");

    let run = get_run(&db, "netrun-foreign").await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Succeeded);

    // The VLAN entry degraded to a conflict and was skipped.
    let outcome = outcome_of(&run);
    assert!(outcome.error.is_none());
    let vlan_entry = outcome
        .entries
        .iter()
        .find(|entry| entry.kind == NetBoxKind::Vlan)
        .expect("vlan entry");
    assert_eq!(vlan_entry.action, NetboxPlanAction::Conflict);
    assert_eq!(vlan_entry.status, NetboxEntryStatus::Skipped);
    assert_eq!(outcome.plan.summary.conflict, 1);

    // Wire: the unoccupied kinds still create; the foreign object was
    // never written and never deleted.
    assert_eq!(
        request_count(&server, "POST", "/api/ipam/vlans/").await,
        0,
        "the foreign-occupied natural key must never be created over"
    );
    assert_eq!(request_count(&server, "POST", "/api/").await, 5);
    assert_eq!(request_count(&server, "DELETE", "/api/").await, 0);
    // Belt-and-braces: the foreign object's NetBox id appears in no
    // mutation path of any method.
    let requests = server.received_requests().await.expect("recording");
    assert!(
        requests
            .iter()
            .filter(|r| r.method.as_str() != "GET")
            .all(|r| !r.url.path().contains("/55")),
        "no write of any method may target the foreign object"
    );
}
