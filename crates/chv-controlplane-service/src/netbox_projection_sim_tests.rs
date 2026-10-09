//! NetBox projection composed suite — PR 3 of the #586 plan
//! (`docs/plans/2026-10-09-netbox-test-scenarios.md`), running the
//! five merged composed scenarios against the in-process
//! [`chv_netbox_sim`] simulator (ADR-024, lane 2).
//!
//! This suite replaces the stateful wiremock composed suite
//! (`netbox_projection_e2e_tests.rs`, PR 8 of the #239 campaign): the
//! same worker/runner/store composition —
//! [`crate::NetboxProjectionWorker::tick`] (reclaim → post-apply sweep
//! → claim) over a real in-memory store — but the remote NetBox is a
//! stateful double with NetBox's own semantics (server-side
//! natural-key filtering, id assignment, custom fields, NetBox-shaped
//! errors) instead of hand-scripted per-test mocks. Assertions are
//! **state-based** (the backend's object dump) rather than wiremock
//! request counting: they prove the resulting NetBox state, not just
//! the calls that were made against a mock.
//!
//! # Backend abstraction
//!
//! [`NetboxBackend`] is the seam the scenarios drive. It has two
//! variants:
//!
//! - the in-process simulator (ADR-024 lane 2), the default for the
//!   always-on `#[tokio::test]` scenarios below, and
//! - `Real`, a live NetBox REST API behind
//!   `NETBOX_QUALIFICATION_URL`/`NETBOX_QUALIFICATION_TOKEN`
//!   (ADR-024 lane 3, the qualification lane): `seed` is API-driven
//!   creation, `state` a list-via-API dump normalized to the sim's
//!   dump shape, `reset` deletion of every object in the six
//!   families, and fault injection is unsupported — the outage
//!   scenarios skip their fault leg against a real NetBox, which is
//!   why [`NetboxBackend::inject_fault`] reports support instead of
//!   being infallible.
//!
//! The real arm runs through the five `qualification_*` wrappers
//! (ignored by default; `scripts/netbox-qualify.sh` runs them with
//! `-- --ignored` against a disposable compose-hosted NetBox). The
//! wrappers serialize on a static async mutex and reset the instance
//! between scenarios, because a real NetBox — unlike a fresh
//! per-test simulator — is shared state.
//!
//! # Ported semantics — and what the state-based swap revealed
//!
//! The five scenarios keep the wiremock suite's semantics exactly,
//! with three places where a wiremock assertion had been pinning
//! **mock-side** behavior rather than NetBox-side behavior:
//!
//! - **Partial failure mid-plan**: the wiremock suite fabricated a
//!   POST-only 500 (prefix create fails, prefix GETs succeed). Real
//!   NetBox — and the simulator — cannot fault a single HTTP method,
//!   so a per-kind 5xx fails the run during the plan's remote-state
//!   fetch, before any mutation. The sim port therefore reconstructs
//!   the post-failure world of a half-executed attempt — as if its
//!   VLAN create had landed — via the `/__seed` control plane and
//!   keeps the
//!   scenario's real substance: transient failure → auto-requeue →
//!   resume resolves the half-created object by natural key → final
//!   state has exactly one object per natural key. The wiremock
//!   partial ledger (`succeeded: 1, not_attempted: 4`) is not
//!   reproducible against a NetBox-shaped backend and is dropped.
//! - **IP-assignment fix-up**: the wiremock suite asserted the fix-up
//!   re-sends the address body because "the mock's read side does not
//!   reflect the interface created moments earlier in the same run
//!   (real NetBox would)". The simulator's read side does reflect
//!   same-run writes, so the suite now asserts the fix-up's real
//!   outcome directly: the address is bound to the interface's NetBox
//!   id in state.
//! - **Auth**: the wiremock suite checked `Authorization` headers on
//!   recorded requests. The simulator validates tokens itself (401
//!   otherwise), so a successful run proves authenticated mutations
//!   without inspecting the wire.
//!
//! One narrowing the state-based swap cannot avoid: state counting
//! observes object counts and content, not the wire, so a hypothetical
//! delete-and-recreate under `MarkStale` retention would keep counts
//! identical and go unremarked here (the foreign-object scenario pins
//! the seeded id as a partial guard). The wire-level no-DELETE
//! guarantee remains covered at the worker-suite level.
//!
//! Documented deviation (inherited from the wiremock suite, PR 6 of
//! the #239 plan): the apply-run terminal transition site does not
//! exist — the "apply" steps seed a `succeeded` apply run through the
//! worker suite's [`add_succeeded_apply_run`] fixture; the post-apply
//! sweep fires on the very next tick either way.

use chv_architecture_validate::model::{CHVArchitecture, InstanceResources, Network, NetworkType};
use chv_controlplane_store::{
    is_active_run_conflict, test_util::TestDb, ApplyRunRepository, NetboxProjectionRunCreateInput,
    NetboxProjectionRunRepository, VersionCreateInput, VersionRepository,
};
use chv_controlplane_types::architecture::{
    NetboxProjectionMode, NetboxProjectionRunId, NetboxProjectionRunStatus,
    NetboxProjectionTrigger, NetboxRetentionPolicy, RunStatus,
};
use chv_netbox_adapter::ownership::CustomFieldNames;
use chv_netbox_adapter::plan::RetentionPolicy;
use chv_netbox_adapter::{
    NetBoxClient, NetBoxKind, NetBoxObject, NetBoxToken, NetboxEntryStatus, NetboxPlanAction,
    NetboxProjectionInput, NetboxProjectionRunner,
};
use chv_netbox_sim::capture::normalize_object;
use chv_netbox_sim::{
    FaultConfig, LiveNetBox, NetboxSim, NetboxSimConfig, SeedPayload, SimKind,
    QUALIFICATION_TOKEN_ENV, QUALIFICATION_URL_ENV,
};
use serde_json::{json, Value};

use crate::netbox_projection_worker_tests::{
    add_succeeded_apply_run, aid, appid, audit_events, backdate_apply_run, desired_objects,
    fixture_architecture, get_run, model_json, outcome_of, post_apply_runs,
    setup_post_apply_config_with_token, setup_projection_with_token, setup_topology_and_version,
    vid, worker_for, SITE, TOKEN,
};

// ---------------------------------------------------------------------------
// Backend abstraction (simulator + real-NetBox qualification variant)
// ---------------------------------------------------------------------------

/// The NetBox backend the composed scenarios run against.
enum NetboxBackend {
    /// The in-process stateful simulator (ADR-024 lane 2).
    Sim(NetboxSim),
    /// A live NetBox REST API (ADR-024 lane 3, the qualification
    /// lane) — see the module docs.
    Real(LiveNetBox),
}

impl NetboxBackend {
    /// Start the suite's default backend: an in-process simulator on
    /// an ephemeral port that accepts exactly the projection config's
    /// token (so a successful run also proves authenticated writes).
    async fn start() -> Self {
        Self::Sim(
            NetboxSim::start(NetboxSimConfig::new(TOKEN))
                .await
                .expect("simulator starts"),
        )
    }

    /// Start the qualification backend from
    /// `NETBOX_QUALIFICATION_URL` + `NETBOX_QUALIFICATION_TOKEN`.
    /// Fails loudly (panics naming both variables) when either is
    /// unset: the qualification wrappers are `#[ignore]`d precisely so
    /// a default `cargo test` never reaches this, and a run without
    /// the pair is a harness misconfiguration, not a skip.
    async fn start_qualification() -> Self {
        let url = std::env::var(QUALIFICATION_URL_ENV).unwrap_or_else(|_| {
            panic!(
                "the real-NetBox qualification lane requires ${QUALIFICATION_URL_ENV} \
                 (and ${QUALIFICATION_TOKEN_ENV}) — run it through \
                 scripts/netbox-qualify.sh or set both variables"
            )
        });
        let token = std::env::var(QUALIFICATION_TOKEN_ENV).unwrap_or_else(|_| {
            panic!(
                "the real-NetBox qualification lane requires ${QUALIFICATION_TOKEN_ENV} \
                 (and ${QUALIFICATION_URL_ENV}) — run it through \
                 scripts/netbox-qualify.sh or set both variables"
            )
        });
        Self::Real(LiveNetBox::new(&url, &token))
    }

    /// The endpoint the projection config stores.
    fn base_url(&self) -> &str {
        match self {
            Self::Sim(sim) => sim.base_url(),
            Self::Real(live) => live.base_url(),
        }
    }

    /// The token the projection config stores for this backend.
    fn token(&self) -> String {
        match self {
            Self::Sim(_) => TOKEN.to_string(),
            Self::Real(live) => live.token().to_string(),
        }
    }

    /// Bulk-load objects at caller-chosen natural keys and ids (sim:
    /// the `/__seed` control plane; real backend: API-driven
    /// creation — the caller-chosen id is NetBox's to assign, and
    /// the scenarios only ever use `seed` on freshly-reset state).
    async fn seed(&self, payload: &Value) {
        match self {
            Self::Sim(sim) => {
                let parsed: SeedPayload =
                    serde_json::from_value(payload.clone()).expect("seed payload parses");
                sim.seed(&parsed).expect("seed succeeds");
            }
            Self::Real(live) => {
                for kind in SimKind::ALL {
                    for entry in payload[kind.collection()]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                    {
                        let mut body = entry.clone();
                        // The sim's seed format allows caller-chosen
                        // ids and timestamps; a live NetBox assigns
                        // its own.
                        if let Some(map) = body.as_object_mut() {
                            for field in ["id", "created", "last_updated"] {
                                map.remove(field);
                            }
                        }
                        live.create(kind, &body)
                            .await
                            .expect("qualification seed create succeeds");
                    }
                }
            }
        }
    }

    /// The backend's full object state, in the `/__state` dump shape
    /// (sim: the control-plane dump; real backend: a list-via-API
    /// dump normalized to the same shape).
    ///
    /// The shape is a load-bearing contract for the qualification
    /// lane: a top-level `objects` map keyed by collection name
    /// (`devices`, `virtual_machines`, `interfaces`, `prefixes`,
    /// `vlans`, `ip_addresses`), each collection an array in
    /// **deterministic ascending-id order** (the sim dumps in id
    /// order; the real backend sorts likewise). Assertions index
    /// this shape directly, and [`assert_state_unchanged`] byte-
    /// compares full dumps — the deterministic ordering is what makes
    /// that comparison meaningful.
    ///
    /// The real arm reduces every live row through
    /// [`normalize_object`] — the drift-sensitive core of the lane:
    /// everything that survives it (field names, nested-relation
    /// shapes, choice labels) is exactly what the scenarios pin, and
    /// a real NetBox that outgrows the simulator's shapes fails here
    /// rather than silently drifting. `faults`/`next_id` are
    /// simulator-only dump keys (`null` on the real arm).
    async fn state(&self) -> Value {
        match self {
            Self::Sim(sim) => {
                let state = sim.shared().lock();
                state.dump(sim.base_url())
            }
            Self::Real(live) => {
                let mut objects = serde_json::Map::new();
                for kind in SimKind::ALL {
                    let mut rows = Vec::new();
                    for row in live.list(kind).await.expect("qualification list succeeds") {
                        rows.push(normalize_object(kind, &row, live.base_url()));
                    }
                    rows.sort_by_key(|row| row["id"].as_i64().unwrap_or_default());
                    objects.insert(kind.collection().to_string(), Value::Array(rows));
                }
                json!({
                    "objects": objects,
                    "faults": Value::Null,
                    "next_id": Value::Null,
                })
            }
        }
    }

    /// Clear objects and faults (sim: `/__reset`; real backend: delete
    /// every object in the six families, children first).
    async fn reset(&self) {
        match self {
            Self::Sim(sim) => sim.reset(),
            Self::Real(live) => {
                live.delete_all()
                    .await
                    .expect("qualification reset deletes everything");
            }
        }
    }

    /// Inject a fault, globally or for one kind. Returns whether the
    /// backend supports fault injection at all — a real NetBox has no
    /// control plane, so the qualification lane skips the outage
    /// scenarios' fault legs when this returns `false`.
    fn inject_fault(&self, scope: Option<SimKind>, fault: FaultConfig) -> bool {
        match self {
            Self::Sim(sim) => {
                sim.shared().lock().set_fault(scope, fault);
                true
            }
            Self::Real(_) => false,
        }
    }

    /// Clear every injected fault, preserving objects (an all-faults-off
    /// configuration per scope is the simulator's supported neutral
    /// state; a real backend never has faults to clear).
    fn clear_faults(&self) {
        match self {
            Self::Sim(sim) => {
                let mut state = sim.shared().lock();
                state.set_fault(None, FaultConfig::default());
                for kind in SimKind::ALL {
                    state.set_fault(Some(kind), FaultConfig::default());
                }
            }
            Self::Real(_) => {}
        }
    }
}

/// Serializes the qualification wrappers: the scenarios assume an
/// empty, exclusively-held NetBox, which a fresh per-test simulator
/// provides by construction but a shared real instance does not.
static QUALIFICATION_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// ---------------------------------------------------------------------------
// State-dump helpers (the request-counting assertions' replacements)
// ---------------------------------------------------------------------------

/// One kind's rows in the state dump.
fn collection<'a>(state: &'a Value, name: &str) -> &'a [Value] {
    state["objects"][name].as_array().expect("collection array")
}

/// The first row of a collection whose `field` equals `value`.
fn find<'a>(state: &'a Value, coll: &str, field: &str, value: &str) -> Option<&'a Value> {
    collection(state, coll)
        .iter()
        .find(|row| row[field].as_str() == Some(value))
}

/// The first VLAN with the given vid.
fn vlan_with_vid(state: &Value, vid: i64) -> Option<&Value> {
    collection(state, "vlans")
        .iter()
        .find(|row| row["vid"].as_i64() == Some(vid))
}

/// Total objects across the six kinds.
fn total_objects(state: &Value) -> usize {
    SimKind::ALL
        .iter()
        .map(|kind| collection(state, kind.collection()).len())
        .sum()
}

/// Exactly one object per mapped kind — the "no natural key is
/// occupied twice" shape of this suite's one-object-per-kind fixture.
fn assert_one_object_per_kind(state: &Value) {
    for kind in SimKind::ALL {
        assert_eq!(
            collection(state, kind.collection()).len(),
            1,
            "exactly one {} in NetBox state",
            kind
        );
    }
}

/// The all-`no_op` property, state-based: two full dumps compare
/// byte-equal (ids, `last_updated`, custom fields — any write, however
/// small, would move the serialization).
fn assert_state_unchanged(before: &Value, after: &Value, context: &str) {
    assert_eq!(
        serde_json::to_string(before).expect("dump serializes"),
        serde_json::to_string(after).expect("dump serializes"),
        "NetBox state must be byte-unchanged: {context}"
    );
}

/// Enqueue a manual export run — the same repository call the BFF's
/// export handler makes.
async fn enqueue_manual_export(db: &TestDb, run_id: &str, topo_id: &str, version_id: &str) {
    NetboxProjectionRunRepository::new(db.pool.clone())
        .create(NetboxProjectionRunCreateInput {
            id: NetboxProjectionRunId::new(run_id).expect("valid run id"),
            architecture_id: aid(topo_id),
            architecture_version_id: vid(version_id),
            trigger_kind: NetboxProjectionTrigger::Manual,
            mode: NetboxProjectionMode::Export,
            plan_json: None,
            requested_by: Some("senol".to_string()),
        })
        .await
        .expect("manual run enqueued");
}

/// The NetBox write body for one desired VLAN — byte-identical to what
/// the adapter client POSTs (mirrors the client's private body builder
/// for the one kind this suite seeds), so a seeded row is exactly the
/// row a partially-executed attempt's create would have written.
fn vlan_seed_body(object: &NetBoxObject) -> Value {
    match object {
        NetBoxObject::Vlan(v) => json!({
            "vid": v.vid,
            "name": v.name,
            "tags": v.tags,
            "custom_fields": v.custom_fields,
        }),
        other => panic!("vlan seed body expects a vlan, got {:?}", other.kind()),
    }
}

/// The v2 topology: `vm-01`'s resources are MODIFIED, the server
/// `chv-node-01` is REMOVED (its device goes stale under `mark_stale`
/// retention), and the network `frontend` is ADDED. The VM's
/// `placement.server` no longer resolves, so its `device` reference
/// drops to unset — a realistic consequence of the removal.
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

/// A projection runner pointed at the backend, for the scenario's
/// dry-run legs (built exactly the way the worker builds its client).
fn runner_for(backend: &NetboxBackend) -> NetboxProjectionRunner {
    NetboxProjectionRunner::new(
        NetBoxClient::new_unchecked_for_tests(
            backend.base_url(),
            NetBoxToken::new(backend.token()),
        )
        .expect("test client"),
    )
}

// ---------------------------------------------------------------------------
// Test A — the full lifecycle, as one flowing scenario
// ---------------------------------------------------------------------------

/// Happy path + idempotency + update + removal, in numbered phases
/// mirroring the plan's scenario lines — every wiremock request-count
/// assertion replaced by a state assertion on the backend's dump.
async fn full_lifecycle_apply_to_projection_to_reapply_inner(backend: &NetboxBackend) {
    let db = TestDb::new().await;
    let model_v1 = model_json();

    // -----------------------------------------------------------------
    // Phase 1 — topology + version v1 + a SUCCEEDED apply run + a
    // post-apply-enabled config (mark_stale retention), pointed at
    // the simulator. The succeeded apply run is seeded via the
    // fixture (the inherited PR-6 deviation: the orchestrator owns
    // the terminal `Succeeded` transition and is not implemented).
    // -----------------------------------------------------------------
    setup_topology_and_version(&db, "topo-sim", "v-1", &model_v1, 1).await;
    add_succeeded_apply_run(&db, "apply-1", "topo-sim", "v-1").await;
    backdate_apply_run(&db, "apply-1", "2020-01-01T00:00:00Z").await;
    setup_post_apply_config_with_token(&db, backend.base_url(), "topo-sim", &backend.token()).await;

    // -----------------------------------------------------------------
    // Phase 2 — one worker tick: the post-apply sweep enqueues a
    // post_apply export run for v1 AND the same tick's claim loop
    // executes it to `succeeded` against the simulator.
    // -----------------------------------------------------------------
    worker_for(&db).tick().await.expect("tick succeeds");

    let runs = post_apply_runs(&db, "topo-sim").await;
    assert_eq!(runs.len(), 1, "exactly one post_apply run: {runs:?}");
    let first = &runs[0];
    assert_eq!(first.trigger_kind, NetboxProjectionTrigger::PostApply);
    assert_eq!(first.status, NetboxProjectionRunStatus::Succeeded);
    assert!(
        first.finished_at.is_some(),
        "the run executed to terminal state"
    );

    // The projection-side ledger (unchanged from the wiremock suite:
    // one create per mapped kind, no failures).
    let outcome = outcome_of(first);
    assert!(outcome.error.is_none());
    assert_eq!(outcome.summary.succeeded, 6, "one create per kind");
    assert_eq!(outcome.summary.failed, 0);
    assert_eq!(outcome.summary.skipped, 0);
    assert_eq!(outcome.summary.not_attempted, 0);
    assert_eq!(outcome.plan.summary.create, 6);

    // State: the simulator holds exactly one object per mapped kind
    // (the wiremock suite's "6 POSTs, 1 fix-up PATCH, 0 DELETEs")
    // with the expected natural keys, ownership markers, site, and
    // enriched custom fields. A successful run against the simulator
    // also proves every request was authenticated: the sim rejects
    // unauthenticated requests with 401, and it accepts exactly the
    // configured token.
    let state = backend.state().await;
    assert_one_object_per_kind(&state);

    let vm = find(&state, "virtual_machines", "name", "vm-01").expect("vm in state");
    assert_eq!(vm["vcpus"], json!(2));
    assert_eq!(vm["memory"], json!(2048));
    assert_eq!(vm["status"]["value"], json!("active"));
    assert_eq!(
        vm["device"]["name"],
        json!("chv-node-01"),
        "VM placed on its declared server"
    );
    let vm_fields = &vm["custom_fields"];
    assert_eq!(
        vm_fields["chv_external_id"],
        json!("arch:topo-sim:instance/vm-01:1")
    );
    assert_eq!(
        vm_fields["chv_managed_by"],
        json!("chv"),
        "the ownership write-guard marker"
    );
    assert_eq!(vm_fields["chv_architecture_id"], json!("topo-sim"));
    assert_eq!(vm_fields["chv_architecture_version"], json!("1"));
    assert_eq!(vm_fields["chv_mapping_version"], json!("v1"));
    assert_eq!(vm_fields["chv_managed_state"], json!("active"));

    let device = find(&state, "devices", "name", "chv-node-01").expect("device in state");
    assert_eq!(
        device["site"]["name"],
        json!(SITE),
        "site comes from the projection config"
    );
    let device_fields = &device["custom_fields"];
    assert_eq!(
        device_fields["chv_external_id"],
        json!("arch:topo-sim:server/chv-node-01:1")
    );
    assert_eq!(
        device_fields["chv_cpu_cores"],
        json!("4"),
        "declared server resources project as enrichment custom fields"
    );
    assert_eq!(device_fields["chv_memory_gb"], json!("8"));
    assert_eq!(
        device_fields["chv_owner"],
        json!("alice"),
        "metadata.owner projects as the chv_owner custom field"
    );

    let vlan = vlan_with_vid(&state, 42).expect("backend vlan in state");
    assert_eq!(vlan["name"], json!("backend"));

    // The post-loop IP-assignment fix-up, asserted on its outcome for
    // the first time: the simulator's read side reflects the interface
    // created moments earlier in the same run (the wiremock suite
    // could only assert the weaker re-send form and deferred the
    // id-binding assertion to a hand-mirrored phase). The address is
    // bound to the interface's NetBox id.
    let ip = find(&state, "ip_addresses", "address", "10.42.0.5/32")
        .expect("ip address in state (maskless create normalized to /32)");
    let interface = find(&state, "interfaces", "name", "backend").expect("interface in state");
    assert_eq!(
        ip["assigned_object_type"],
        json!("virtualization.vminterface")
    );
    assert_eq!(ip["assigned_object_id"], interface["id"]);
    assert_eq!(ip["assigned_object"]["name"], json!("backend"));
    assert_eq!(
        ip["assigned_object"]["virtual_machine"]["name"],
        json!("vm-01")
    );

    // -----------------------------------------------------------------
    // Phase 3 — re-export over the converged state: the plan is a
    // full no-op and the simulator's state is byte-unchanged (the
    // all-`no_op` property, currently proven only in the worker suite
    // against a hand-mirrored remote, here as a state-based
    // assertion against what the projection itself wrote).
    // -----------------------------------------------------------------
    let before = backend.state().await;
    enqueue_manual_export(&db, "netrun-reexport", "topo-sim", "v-1").await;

    worker_for(&db).tick().await.expect("tick succeeds");

    let run = get_run(&db, "netrun-reexport").await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Succeeded);
    // Secret-freedom: the recorded outcome (provenance envelope +
    // executed plan, the document the BFF later serves) carries no
    // token material.
    assert!(
        !run.result_json
            .as_deref()
            .expect("result json")
            .contains(backend.token().as_str()),
        "the recorded outcome must be secret-free"
    );
    let outcome = outcome_of(&run);
    assert!(outcome.error.is_none());
    assert_eq!(outcome.summary.skipped, 6, "all no_op");
    assert_eq!(outcome.summary.succeeded, 0);
    assert_eq!(outcome.summary.failed, 0);
    assert_eq!(outcome.plan.summary.create, 0);
    assert_state_unchanged(&before, &backend.state().await, "re-export is all no_op");

    // -----------------------------------------------------------------
    // Phase 4 — topology update at v2 (one modified, one removed, one
    // added) + a second SUCCEEDED apply run, backdated after v1's.
    // The next tick enqueues a fresh post_apply run for v2 and
    // executes it: update entries PATCH the objects created in phase
    // 2, the added network creates, and the removed server's device
    // is handled per mark_stale retention.
    // -----------------------------------------------------------------
    let model_v2 = serde_json::to_string(&v2_architecture()).expect("model v2 serializes");
    VersionRepository::new(db.pool.clone())
        .create(VersionCreateInput {
            id: vid("v-2"),
            architecture_id: aid("topo-sim"),
            version_number: 2,
            yaml_content: "x".to_string(),
            design_graph_json: None,
            normalized_model_json: Some(model_v2),
            change_summary: None,
            created_by: None,
        })
        .await
        .expect("version v-2 created");
    add_succeeded_apply_run(&db, "apply-2", "topo-sim", "v-2").await;

    worker_for(&db).tick().await.expect("tick succeeds");

    let runs = post_apply_runs(&db, "topo-sim").await;
    assert_eq!(
        runs.len(),
        2,
        "one post_apply run per applied version: {runs:?}"
    );
    let second = runs
        .iter()
        .find(|run| run.architecture_version_id == vid("v-2"))
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

    // State: the two creates landed, the five updates are visible on
    // the phase-2 objects, and the stale mark is visible on the
    // removed server's device — which is still present (mark_stale
    // retention never deletes).
    let state = backend.state().await;
    assert_eq!(
        total_objects(&state),
        8,
        "6 phase-2 objects + 2 creates; nothing deleted"
    );
    let frontend_vlan = vlan_with_vid(&state, 7).expect("the added frontend vlan");
    assert_eq!(frontend_vlan["name"], json!("frontend"));
    let frontend_prefix =
        find(&state, "prefixes", "prefix", "10.7.0.0/24").expect("the added frontend prefix");
    assert_eq!(frontend_prefix["vlan"]["vid"], json!(7));

    let vm = find(&state, "virtual_machines", "name", "vm-01").expect("vm in state");
    assert_eq!(
        vm["vcpus"],
        json!(4),
        "the modified VM carries v2 resources"
    );
    assert_eq!(vm["memory"], json!(4096));
    assert_eq!(
        vm["custom_fields"]["chv_architecture_version"],
        json!("2"),
        "the update refreshes the version provenance custom field"
    );
    assert!(
        vm["device"].is_null(),
        "the removed server drops the VM's device reference"
    );

    let device = find(&state, "devices", "name", "chv-node-01").expect("device in state");
    assert_eq!(
        device["custom_fields"]["chv_managed_state"],
        json!("stale"),
        "mark_stale retention: the removed resource's device is stale-marked"
    );
    assert_eq!(
        device["status"]["value"],
        json!("decommissioning"),
        "NetBox's decommissioning status accompanies the stale mark"
    );

    // -----------------------------------------------------------------
    // Phase 5 — idempotency: another tick with no new apply enqueues
    // nothing and leaves the simulator state byte-unchanged.
    // -----------------------------------------------------------------
    let before = backend.state().await;
    worker_for(&db)
        .tick()
        .await
        .expect("idempotency tick succeeds");
    assert_eq!(
        post_apply_runs(&db, "topo-sim").await.len(),
        2,
        "no additional post_apply run"
    );
    assert_state_unchanged(
        &before,
        &backend.state().await,
        "the idempotency tick writes nothing",
    );

    // -----------------------------------------------------------------
    // Phase 6 — audit trail: both executions' export_succeeded events
    // exist in the events table, correlated by run id.
    // -----------------------------------------------------------------
    for run in post_apply_runs(&db, "topo-sim").await {
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

#[tokio::test]
async fn full_lifecycle_apply_to_projection_to_reapply() {
    full_lifecycle_apply_to_projection_to_reapply_inner(&NetboxBackend::start().await).await;
}

/// The same lifecycle against a live NetBox (ADR-024 lane 3). Ignored
/// by default — a real instance is provisioned and serialized by the
/// qualification harness (`scripts/netbox-qualify.sh`), never by a
/// bare `cargo test`.
#[tokio::test]
#[ignore = "real-NetBox qualification: set NETBOX_QUALIFICATION_URL + NETBOX_QUALIFICATION_TOKEN (scripts/netbox-qualify.sh)"]
async fn qualification_full_lifecycle_apply_to_projection_to_reapply() {
    let backend = NetboxBackend::start_qualification().await;
    let _guard = QUALIFICATION_MUTEX.lock().await;
    backend.reset().await;
    full_lifecycle_apply_to_projection_to_reapply_inner(&backend).await;
}

// ---------------------------------------------------------------------------
// Test B — foreign objects are never modified (state-based)
// ---------------------------------------------------------------------------

/// An object in the NetBox state that matches a desired natural key
/// but carries no CHV ownership marker is a `conflict`: the plan
/// records it, the export proceeds with the remaining entries, and
/// the foreign object is byte-unchanged in the state dump afterwards —
/// not even its `last_updated` moved, so no write of any method
/// touched it — and no chv ownership fields were added. The unoccupied
/// kinds still create.
async fn foreign_object_at_natural_key_is_never_written_inner(backend: &NetboxBackend) {
    let db = TestDb::new().await;

    setup_projection_with_token(
        &db,
        backend.base_url(),
        "topo-foreign",
        "v-1",
        "apply-foreign",
        "netrun-foreign",
        NetboxRetentionPolicy::MarkStale,
        &backend.token(),
    )
    .await;

    // A foreign VLAN (no ownership marker at all) squatting on the
    // desired vid, seeded through the control plane at a chosen id.
    backend
        .seed(&json!({
            "vlans": [
                { "id": 55, "vid": 42, "name": "netops-backend" }
            ]
        }))
        .await;
    let before = backend.state().await;
    let foreign_before = vlan_with_vid(&before, 42)
        .expect("the seeded foreign vlan")
        .clone();

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

    // State: the foreign object is byte-unchanged (a full-object
    // comparison — any PATCH would have moved `last_updated` or a
    // custom field), still carries no chv ownership fields, and is
    // the ONLY vlan: the projection never created over the occupied
    // natural key. The five unoccupied kinds created exactly one
    // object each.
    let state = backend.state().await;
    let vlans = collection(&state, "vlans");
    assert_eq!(
        vlans.len(),
        1,
        "the foreign-occupied natural key must never be created over"
    );
    assert_eq!(
        serde_json::to_string(&vlans[0]).expect("serializes"),
        serde_json::to_string(&foreign_before).expect("serializes"),
        "the foreign object must be byte-unchanged in NetBox state"
    );
    assert!(
        vlans[0]["custom_fields"]
            .as_object()
            .expect("custom fields object")
            .is_empty(),
        "no chv ownership fields may be added to the foreign object"
    );
    assert_eq!(
        total_objects(&state),
        6,
        "the five unoccupied kinds created"
    );
    assert_eq!(
        vlan_with_vid(&state, 42).unwrap()["id"],
        foreign_before["id"],
        "the foreign object keeps its original id (never deleted and recreated)"
    );
}

#[tokio::test]
async fn foreign_object_at_natural_key_is_never_written() {
    foreign_object_at_natural_key_is_never_written_inner(&NetboxBackend::start().await).await;
}

/// The foreign-object guarantee against a live NetBox (ADR-024
/// lane 3). Ignored by default — see
/// [`qualification_full_lifecycle_apply_to_projection_to_reapply`].
#[tokio::test]
#[ignore = "real-NetBox qualification: set NETBOX_QUALIFICATION_URL + NETBOX_QUALIFICATION_TOKEN (scripts/netbox-qualify.sh)"]
async fn qualification_foreign_object_at_natural_key_is_never_written() {
    let backend = NetboxBackend::start_qualification().await;
    let _guard = QUALIFICATION_MUTEX.lock().await;
    backend.reset().await;
    foreign_object_at_natural_key_is_never_written_inner(&backend).await;
}

// ---------------------------------------------------------------------------
// Test C — a NetBox outage never changes the apply result
// ---------------------------------------------------------------------------

/// A NetBox outage during the post-apply projection fails the
/// projection run (retryable, backoff-gated) and leaves the apply run
/// that triggered it untouched: still `Succeeded`, `finished_at`
/// exactly as seeded. The sweep stays idempotent across ticks, the
/// outage writes nothing to NetBox, and once the fault clears the
/// requeued run retries to success.
///
/// The outage is injected through the simulator's fault control plane
/// (a global 5xx), replacing the wiremock suite's hand-mounted outage
/// stubs and its dead-endpoint fixture (`http://127.0.0.1:1`).
async fn netbox_outage_never_changes_the_apply_result_inner(backend: &NetboxBackend) {
    let db = TestDb::new().await;
    let model = model_json();

    // Phase 1 — topology + v1 + a SUCCEEDED apply run whose terminal
    // timestamps are the isolation canary, + a post-apply config
    // pointed at the simulator.
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
    setup_post_apply_config_with_token(&db, backend.base_url(), "topo-outage", &backend.token())
        .await;

    // Phase 2 — inject the outage (every request answers 5xx) and
    // tick: the sweep enqueues the post_apply run and the claim loop
    // executes it against the faulted NetBox: the run fails with the
    // transient (retryable) 5xx classification and is auto-requeued
    // with a backoff.
    if !backend.inject_fault(
        None,
        FaultConfig {
            server_error: Some(500),
            ..FaultConfig::default()
        },
    ) {
        // A real NetBox has no fault control plane: the outage leg is
        // simulator-only. PR 5's qualification mode skips this
        // scenario rather than faking an outage (see module docs). A
        // simulator that cannot inject faults is a regression, not a
        // skip.
        assert!(
            !matches!(backend, NetboxBackend::Sim(_)),
            "the simulator must support fault injection"
        );
        return;
    }

    worker_for(&db)
        .tick()
        .await
        .expect("outage never kills the worker");

    let runs = post_apply_runs(&db, "topo-outage").await;
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
        error.contains("status 500"),
        "5xx outage classification: {error}"
    );
    assert!(!error.contains(TOKEN), "the error is token-free");
    assert!(
        run.next_attempt_at
            .is_some_and(|at| at > chrono::Utc::now()),
        "retry backoff scheduled in the future: {:?}",
        run.next_attempt_at
    );
    assert_eq!(
        total_objects(&backend.state().await),
        0,
        "the faulted attempt wrote nothing to NetBox"
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
    let runs = post_apply_runs(&db, "topo-outage").await;
    assert_eq!(runs.len(), 1, "no re-enqueue on the second tick");
    assert_eq!(
        runs[0].attempt_count, 1,
        "the backoff prevented an early retry"
    );
    assert_eq!(runs[0].status, NetboxProjectionRunStatus::Queued);

    // Phase 5 — the outage heals: clear the fault, let the backoff
    // elapse (backdated), and the requeued run retries to success —
    // the full projection lands in the NetBox state.
    backend.clear_faults();
    sqlx::query(
        "UPDATE netbox_projection_runs SET next_attempt_at = '2020-01-01T00:00:00Z' WHERE id = ?",
    )
    .bind(runs[0].id.to_string())
    .execute(&db.pool)
    .await
    .expect("backdate retry backoff");

    worker_for(&db)
        .tick()
        .await
        .expect("recovery tick succeeds");

    let run = get_run(&db, &runs[0].id.to_string()).await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Succeeded);
    assert_eq!(
        run.attempt_count, 1,
        "the successful retry did not consume an attempt"
    );
    assert_one_object_per_kind(&backend.state().await);
}

#[tokio::test]
async fn netbox_outage_never_changes_the_apply_result() {
    netbox_outage_never_changes_the_apply_result_inner(&NetboxBackend::start().await).await;
}

/// The outage isolation against a live NetBox (ADR-024 lane 3).
/// Ignored by default — see
/// [`qualification_full_lifecycle_apply_to_projection_to_reapply`].
/// A real NetBox has no fault control plane, so this wrapper asserts
/// the scenario's early-return path: the fault leg reports itself
/// unsupported and the scenario skips without touching the instance
/// (the outage semantics themselves stay simulator-owned — a real
/// outage cannot be scheduled from a test).
#[tokio::test]
#[ignore = "real-NetBox qualification: set NETBOX_QUALIFICATION_URL + NETBOX_QUALIFICATION_TOKEN (scripts/netbox-qualify.sh)"]
async fn qualification_netbox_outage_never_changes_the_apply_result() {
    let backend = NetboxBackend::start_qualification().await;
    let _guard = QUALIFICATION_MUTEX.lock().await;
    backend.reset().await;
    netbox_outage_never_changes_the_apply_result_inner(&backend).await;
    // The real arm must have taken the unsupported-fault early return;
    // a simulator reaching this wrapper would be a harness bug.
    assert!(
        !matches!(&backend, NetboxBackend::Sim(_)),
        "the simulator arm must not run through the qualification wrapper"
    );
}

// ---------------------------------------------------------------------------
// Test D — partial failure mid-plan: requeue, resume, no duplicate
// ---------------------------------------------------------------------------

/// A transient failure during the run auto-requeues it with a backoff;
/// once the backoff elapses, the resumed attempt finds the
/// half-created object through its natural key and resolves to a no-op
/// — THE key assertion, state-based: the final NetBox state holds
/// exactly one object per natural key, so no duplicate object was ever
/// created, and a dry-run over the converged state is a full no-op.
///
/// How the half-executed world is reconstructed (see the module docs):
/// the wiremock suite fabricated a POST-only 500 to fail the run
/// mid-plan after the VLAN create had landed; the simulator — like
/// real NetBox — cannot fault a single HTTP method, so a per-kind 5xx
/// fails the run during the plan's remote-state fetch instead, and the
/// half-created VLAN is seeded through the control plane at exactly
/// the body the projection's own create would have sent.
async fn partial_failure_requeues_and_retry_resumes_without_duplicate_create_inner(
    backend: &NetboxBackend,
) {
    let db = TestDb::new().await;
    let model = model_json();

    // Phase 1 — topology + v1 + a SUCCEEDED apply run + a
    // post-apply-enabled config.
    setup_topology_and_version(&db, "topo-partial", "v-1", &model, 1).await;
    add_succeeded_apply_run(&db, "apply-partial", "topo-partial", "v-1").await;
    setup_post_apply_config_with_token(&db, backend.base_url(), "topo-partial", &backend.token())
        .await;

    // The transient failure: every prefix request answers 5xx. Check
    // fault support BEFORE seeding so a backend without a fault
    // control plane skips without leaving residue.
    if !backend.inject_fault(
        Some(SimKind::Prefix),
        FaultConfig {
            server_error: Some(500),
            ..FaultConfig::default()
        },
    ) {
        // A real NetBox has no fault control plane: the failure leg
        // is simulator-only. PR 5's qualification mode skips this
        // scenario rather than faking a mid-plan failure (see module
        // docs). A simulator that cannot inject faults is a
        // regression, not a skip.
        assert!(
            !matches!(backend, NetboxBackend::Sim(_)),
            "the simulator must support fault injection"
        );
        return;
    }

    // The half-created state of a partially-executed attempt: its VLAN
    // create landed (chv-owned, desired content) before the run hit
    // its mid-plan failure.
    let vlan = desired_objects("topo-partial")
        .into_iter()
        .find(|object| matches!(object, NetBoxObject::Vlan(_)))
        .expect("desired vlan");
    backend
        .seed(&json!({ "vlans": [vlan_seed_body(&vlan)] }))
        .await;

    worker_for(&db).tick().await.expect("tick succeeds");

    let runs = post_apply_runs(&db, "topo-partial").await;
    assert_eq!(runs.len(), 1, "exactly one post_apply run: {runs:?}");
    let run_id = runs[0].id.to_string();
    let run = &runs[0];
    assert_eq!(
        run.status,
        NetboxProjectionRunStatus::Queued,
        "transient 5xx auto-requeued"
    );
    assert_eq!(
        run.attempt_count, 1,
        "the failed execution consumed an attempt"
    );
    let error = run.error_message.as_deref().expect("error message");
    assert!(error.contains("status 500"), "5xx classification: {error}");
    assert!(!error.contains(TOKEN), "the error is token-free");
    assert!(
        run.next_attempt_at
            .is_some_and(|at| at > chrono::Utc::now()),
        "retry backoff scheduled in the future: {:?}",
        run.next_attempt_at
    );

    // State after the failed attempt: still exactly the seeded VLAN —
    // the faulted attempt created nothing.
    let state = backend.state().await;
    assert_eq!(total_objects(&state), 1);
    assert!(
        vlan_with_vid(&state, 42).is_some(),
        "the seeded vlan survives"
    );

    // Phase 2 — while the backoff is pending, a further tick neither
    // re-claims the run (the backoff gates the claim) nor re-enqueues
    // one (a post_apply run of any status counts as already
    // attempted); the state is unchanged.
    let before = backend.state().await;
    worker_for(&db).tick().await.expect("backoff tick succeeds");
    assert_eq!(
        post_apply_runs(&db, "topo-partial").await.len(),
        1,
        "no re-enqueue on the backoff tick"
    );
    assert_state_unchanged(
        &before,
        &backend.state().await,
        "the backoff prevented an early retry",
    );

    // Phase 3 — heal NetBox (clear the fault) and let the backoff
    // elapse (backdated): the resumed attempt resolves the
    // half-created VLAN through its natural key and completes the
    // projection.
    backend.clear_faults();
    sqlx::query(
        "UPDATE netbox_projection_runs SET next_attempt_at = '2020-01-01T00:00:00Z' WHERE id = ?",
    )
    .bind(&run_id)
    .execute(&db.pool)
    .await
    .expect("backdate retry backoff");

    worker_for(&db).tick().await.expect("tick succeeds");

    let run = get_run(&db, &run_id).await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Succeeded);
    assert_eq!(
        run.attempt_count, 1,
        "the successful resume did not consume an attempt"
    );
    let outcome = outcome_of(&run);
    assert!(outcome.error.is_none());
    assert_eq!(outcome.summary.skipped, 1, "the written VLAN is a no_op");
    assert_eq!(
        outcome.summary.succeeded, 5,
        "only the missing kinds create"
    );

    // THE key assertion, state-based: the final NetBox state has
    // exactly one object per natural key — the resume resolved the
    // half-created VLAN by its natural key instead of duplicating it
    // (the wiremock suite proved this as a POST count of 1 on the
    // vlan endpoint; here the resulting state itself is the proof).
    let state = backend.state().await;
    assert_one_object_per_kind(&state);
    let vlans = collection(&state, "vlans");
    assert_eq!(
        vlans.len(),
        1,
        "resume must not duplicate the already-written object"
    );
    assert_eq!(vlans[0]["vid"], json!(42));

    // And the converged state is a full no-op for the desired
    // projection (the input built exactly the way the worker builds
    // it: the applied v1 model + the config's pure slice).
    let architecture: CHVArchitecture = serde_json::from_str(&model).expect("applied model parses");
    let input = NetboxProjectionInput {
        architecture: &architecture,
        architecture_id: "topo-partial",
        architecture_version: 1,
        snapshot: None,
        site_name: Some(SITE),
        retention: RetentionPolicy::MarkStale,
        names: CustomFieldNames::new("chv_"),
    };
    let plan = runner_for(backend)
        .dry_run(&input)
        .await
        .expect("dry run computes");
    assert_eq!(plan.summary.create, 0, "nothing left to create: {plan:?}");
    assert_eq!(plan.summary.update, 0);
    assert_eq!(
        plan.summary.no_op, 6,
        "every entry matched the converged remote"
    );
    assert_eq!(plan.summary.conflict, 0);
    assert_eq!(plan.summary.stale, 0);
}

#[tokio::test]
async fn partial_failure_requeues_and_retry_resumes_without_duplicate_create() {
    partial_failure_requeues_and_retry_resumes_without_duplicate_create_inner(
        &NetboxBackend::start().await,
    )
    .await;
}

/// The resume-without-duplicate guarantee against a live NetBox
/// (ADR-024 lane 3). Ignored by default — see
/// [`qualification_full_lifecycle_apply_to_projection_to_reapply`].
/// Like the outage scenario, the mid-plan failure leg is
/// simulator-only; the real arm asserts the early-return path.
#[tokio::test]
#[ignore = "real-NetBox qualification: set NETBOX_QUALIFICATION_URL + NETBOX_QUALIFICATION_TOKEN (scripts/netbox-qualify.sh)"]
async fn qualification_partial_failure_requeues_and_retry_resumes_without_duplicate_create() {
    let backend = NetboxBackend::start_qualification().await;
    let _guard = QUALIFICATION_MUTEX.lock().await;
    backend.reset().await;
    partial_failure_requeues_and_retry_resumes_without_duplicate_create_inner(&backend).await;
    assert!(
        !matches!(&backend, NetboxBackend::Sim(_)),
        "the simulator arm must not run through the qualification wrapper"
    );
}

// ---------------------------------------------------------------------------
// Test E — manual-export double enqueue coalesces onto the one active run
// ---------------------------------------------------------------------------

/// After a manual export run is queued, a second manual enqueue attempt
/// — via `NetboxProjectionRunRepository::create` with trigger `manual`,
/// exactly what the BFF's export handler does — returns the one-active
/// conflict (classified with the shared `is_active_run_conflict`
/// helper), and the runs table still holds exactly one active run. The
/// subsequent tick composes the coalescing at the sweep level: the
/// post-apply sweep does not enqueue a second run on top of the active
/// manual one, and the claim loop executes the single run to
/// completion — one execution, one object per mapped kind in the
/// NetBox state.
async fn manual_double_enqueue_coalesces_to_one_active_run_inner(backend: &NetboxBackend) {
    let db = TestDb::new().await;
    let model = model_json();

    setup_topology_and_version(&db, "topo-coalesce", "v-1", &model, 1).await;
    add_succeeded_apply_run(&db, "apply-coalesce", "topo-coalesce", "v-1").await;
    setup_post_apply_config_with_token(&db, backend.base_url(), "topo-coalesce", &backend.token())
        .await;

    // The first manual export — the same repository call the BFF's
    // export handler makes.
    let run_repo = NetboxProjectionRunRepository::new(db.pool.clone());
    let first_id = NetboxProjectionRunId::new("netrun-coalesce-1").expect("valid run id");
    enqueue_manual_export(&db, "netrun-coalesce-1", "topo-coalesce", "v-1").await;

    // The second manual enqueue while the first is queued: the store's
    // one-active partial unique index refuses it, and the conflict is
    // classified through the same shared helper the BFF's export
    // handler uses to map it onto 409 NETBOX_RUN_ACTIVE.
    let err = run_repo
        .create(NetboxProjectionRunCreateInput {
            id: NetboxProjectionRunId::new("netrun-coalesce-2").expect("valid run id"),
            architecture_id: aid("topo-coalesce"),
            architecture_version_id: vid("v-1"),
            trigger_kind: NetboxProjectionTrigger::Manual,
            mode: NetboxProjectionMode::Export,
            plan_json: None,
            requested_by: Some("senol".to_string()),
        })
        .await
        .expect_err("the one-active index must refuse a second active run");
    assert!(is_active_run_conflict(&err), "one-active conflict: {err}");

    // The runs table still holds exactly one (active) run.
    let runs = run_repo
        .list_by_architecture(&aid("topo-coalesce"), 50)
        .await
        .expect("runs list");
    assert_eq!(runs.len(), 1, "exactly one run row: {runs:?}");
    assert_eq!(runs[0].status, NetboxProjectionRunStatus::Queued);

    // One tick composes the whole coalescing: the post-apply sweep sees
    // the active manual run holding the architecture's one-active slot
    // and coalesces (no post_apply row is enqueued), then the claim
    // loop executes the single active run to completion.
    worker_for(&db).tick().await.expect("tick succeeds");

    let runs = run_repo
        .list_by_architecture(&aid("topo-coalesce"), 50)
        .await
        .expect("runs list");
    assert_eq!(
        runs.len(),
        1,
        "the sweep coalesced — still exactly one run: {runs:?}"
    );
    assert_eq!(runs[0].id, first_id);
    assert_eq!(runs[0].trigger_kind, NetboxProjectionTrigger::Manual);
    assert_eq!(runs[0].status, NetboxProjectionRunStatus::Succeeded);
    assert!(
        post_apply_runs(&db, "topo-coalesce").await.is_empty(),
        "the sweep must coalesce behind the active manual run"
    );

    // One execution, one create per mapped kind (the wiremock suite's
    // POST count of 6, as the resulting NetBox state).
    assert_one_object_per_kind(&backend.state().await);
}

#[tokio::test]
async fn manual_double_enqueue_coalesces_to_one_active_run() {
    manual_double_enqueue_coalesces_to_one_active_run_inner(&NetboxBackend::start().await).await;
}

/// The coalescing guarantee against a live NetBox (ADR-024 lane 3).
/// Ignored by default — see
/// [`qualification_full_lifecycle_apply_to_projection_to_reapply`].
#[tokio::test]
#[ignore = "real-NetBox qualification: set NETBOX_QUALIFICATION_URL + NETBOX_QUALIFICATION_TOKEN (scripts/netbox-qualify.sh)"]
async fn qualification_manual_double_enqueue_coalesces_to_one_active_run() {
    let backend = NetboxBackend::start_qualification().await;
    let _guard = QUALIFICATION_MUTEX.lock().await;
    backend.reset().await;
    manual_double_enqueue_coalesces_to_one_active_run_inner(&backend).await;
}
