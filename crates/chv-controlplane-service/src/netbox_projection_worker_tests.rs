//! NetBox projection worker integration tests — the PR-4 composition
//! root exercised end to end against a wiremock NetBox.
//!
//! These suites drive [`crate::NetboxProjectionWorker::tick`] over a
//! real in-memory store (`TestDb`) and a real HTTP wire (`wiremock`),
//! injecting the adapter's test-only plain-HTTP client constructor via
//! the worker's [`crate::NetboxProjectionWorker::with_client_factory`]
//! seam. Production construction (`NetBoxClient::new`, HTTPS-only) is
//! covered by the adapter's own unit tests.
//!
//! The "remote already mirrors the projection" fixtures are derived
//! from the pure core itself: the test builds the desired objects with
//! [`chv_netbox_adapter::build_objects`] (the same call the worker's
//! runner performs) and renders each one back into NetBox wire JSON, so
//! a re-run produces a provably all-`no_op` plan without hand-maintained
//! fixtures drifting from the mapping contract.

use std::collections::BTreeMap;
use std::sync::Arc;

use chv_architecture_validate::model::{
    CHVArchitecture, Instance, InstanceNetwork, InstancePlacement, InstanceResources, Metadata,
    Network, NetworkType, Server, ServerResources,
};
use chv_controlplane_store::{
    test_util::TestDb, ApplyRunCreateInput, ApplyRunRepository, EventRepository,
    NetboxProjectionConfigRepository, NetboxProjectionConfigUpsertInput,
    NetboxProjectionRunCreateInput, NetboxProjectionRunRepository, TopologyCreateInput,
    TopologyRepository, VersionCreateInput, VersionRepository,
};
use chv_controlplane_types::architecture::{
    ArchitectureApplyRunId, ArchitectureId, ArchitectureStatus, ArchitectureVersionId,
    NetboxProjectionMode, NetboxProjectionRunId, NetboxProjectionRunStatus,
    NetboxProjectionTrigger, NetboxRetentionPolicy, RunStatus,
};
use chv_netbox_adapter::{
    build_objects, NetBoxClient, NetBoxKind, NetBoxObject, NetboxEntryStatus, NetboxPlanAction,
    NetboxProjectionOutcome, NetboxProjectionPlan, ProjectionConfigView, ProjectionInput,
};
use serde_json::{json, Value};
use wiremock::matchers::{method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::NetboxProjectionWorker;

/// Distinctive plaintext token; the redaction suites assert it never
/// reaches a log line, a persisted error message, or an event payload.
const TOKEN: &str = "netbox-secret-token-do-not-log-3f9a";
/// Site label used by every config fixture (must match
/// [`desired_objects`] so the mirrored device content compares equal).
const SITE: &str = "dc1";

/// Create (POST) mocks per kind path: (path, netbox id returned).
const CREATE_MOCKS: [(&str, i64); 6] = [
    ("/api/ipam/vlans/", 101),
    ("/api/ipam/prefixes/", 102),
    ("/api/ipam/ip-addresses/", 103),
    ("/api/virtualization/interfaces/", 104),
    ("/api/virtualization/virtual-machines/", 105),
    ("/api/dcim/devices/", 106),
];

// ---------------------------------------------------------------------------
// Minimal `tracing` subscriber capturing every field of INFO/WARN/ERROR
// events (mirroring the LogCollector convention in
// `chv-controlplane-store`'s `credential_crypto` tests, but recording all
// fields so redaction assertions cover structured values, not just
// messages).
// ---------------------------------------------------------------------------
mod log_capture {
    use std::sync::{Arc, Mutex as StdMutex};

    use tracing::field::Visit;
    use tracing::span::{Attributes, Id};
    use tracing::{Event, Level, Metadata};

    #[derive(Clone, Default)]
    pub struct LogCollector {
        events: Arc<StdMutex<Vec<String>>>,
    }

    impl LogCollector {
        /// Every captured event rendered as `field=value` pairs.
        pub fn messages(&self) -> Vec<String> {
            self.events.lock().unwrap().clone()
        }

        /// The process-wide collector, installed as the tracing global
        /// default exactly once.
        ///
        /// Why global rather than `set_default`: `tracing` caches
        /// per-callsite interest process-wide, and the cache is
        /// computed against the dispatchers registered when the
        /// callsite first executes. With per-thread `set_default`, a
        /// callsite first executed by a *parallel* test — with no
        /// collector installed — can be cached as never for this
        /// collector too, silently dropping the event (a known
        /// `set_default` + parallel-test race, tokio-rs/tracing#1097).
        /// A global default covers every thread, and installing it
        /// rebuilds the interest cache for callsites that already
        /// executed, so the worker's warnings are observable
        /// deterministically.
        pub fn global() -> LogCollector {
            static COLLECTOR: std::sync::OnceLock<LogCollector> = std::sync::OnceLock::new();
            COLLECTOR
                .get_or_init(|| {
                    let collector = LogCollector::default();
                    // Installing the global default also rebuilds the
                    // callsite-interest cache, so warnings that already
                    // executed under the no-op default become observable.
                    tracing::subscriber::set_global_default(collector.clone())
                        .expect("test binary owns the tracing global default");
                    collector
                })
                .clone()
        }
    }

    struct FieldVisitor(Vec<String>);

    impl Visit for FieldVisitor {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            self.0.push(format!("{}={:?}", field.name(), value));
        }
    }

    impl tracing::Subscriber for LogCollector {
        fn enabled(&self, metadata: &Metadata<'_>) -> bool {
            matches!(*metadata.level(), Level::INFO | Level::WARN | Level::ERROR)
        }

        fn new_span(&self, _span: &Attributes<'_>) -> Id {
            Id::from_u64(1)
        }

        fn record(&self, _span: &Id, _values: &tracing::span::Record<'_>) {}

        fn record_follows_from(&self, _span: &Id, _follows_from: &Id) {}

        fn event(&self, event: &Event<'_>) {
            let mut visitor = FieldVisitor(Vec::new());
            event.record(&mut visitor);
            if !visitor.0.is_empty() {
                self.events.lock().unwrap().push(visitor.0.join(" "));
            }
        }

        fn enter(&self, _span: &Id) {}

        fn exit(&self, _span: &Id) {}
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn aid(s: &str) -> ArchitectureId {
    ArchitectureId::new(s).expect("valid architecture id")
}

fn vid(s: &str) -> ArchitectureVersionId {
    ArchitectureVersionId::new(s).expect("valid version id")
}

fn nid(s: &str) -> NetboxProjectionRunId {
    NetboxProjectionRunId::new(s).expect("valid run id")
}

fn appid(s: &str) -> ArchitectureApplyRunId {
    ArchitectureApplyRunId::new(s).expect("valid apply-run id")
}

/// The projected architecture: one server, one VLAN network with a CIDR,
/// one instance with a fixed IP on that network — exactly one object of
/// each of the six mapped kinds.
fn fixture_architecture() -> CHVArchitecture {
    CHVArchitecture {
        api_version: "chv.kubedo.io/v1alpha1".to_string(),
        kind: "CHVArchitecture".to_string(),
        metadata: Metadata {
            name: "t1".to_string(),
            display_name: None,
            description: None,
            environment: Some("production".to_string()),
            owner: Some("alice".to_string()),
            labels: [("team".to_string(), "platform".to_string())].into(),
        },
        servers: vec![Server {
            name: "chv-node-01".to_string(),
            management_ip: None,
            role: None,
            labels: BTreeMap::new(),
            resources: Some(ServerResources {
                cpu_cores: Some(4),
                memory_gb: Some(8),
            }),
            networks: None,
        }],
        networks: vec![Network {
            name: "backend".to_string(),
            network_type: NetworkType::Vlan,
            bridge: None,
            vlan_id: Some(42),
            cidr: Some("10.42.0.0/24".to_string()),
            gateway: None,
            dns: Vec::new(),
            dhcp: None,
        }],
        datastores: Vec::new(),
        backup_targets: Vec::new(),
        backup_policies: Vec::new(),
        images: Vec::new(),
        templates: Vec::new(),
        instances: vec![Instance {
            name: "vm-01".to_string(),
            template: None,
            placement: Some(InstancePlacement {
                server: Some("chv-node-01".to_string()),
            }),
            resources: Some(InstanceResources {
                cpu: Some(2),
                memory_mb: Some(2048),
            }),
            disks: Vec::new(),
            networks: vec![InstanceNetwork {
                name: "backend".to_string(),
                ip: Some("10.42.0.5".to_string()),
            }],
            cloud_init: None,
            backup: None,
            tags: Vec::new(),
        }],
        ssh_keys: Vec::new(),
        instance_users: Vec::new(),
        roles: Vec::new(),
        users: Vec::new(),
        projects: Vec::new(),
    }
}

/// The model JSON stored in `architecture_versions.normalized_model_json`
/// (what the worker parses back into a `CHVArchitecture`).
fn model_json() -> String {
    serde_json::to_string(&fixture_architecture()).expect("model serializes")
}

/// The desired objects the worker's runner will build — computed here
/// with the same pure-core call so remote fixtures mirror it exactly.
fn desired_objects(architecture_id: &str) -> Vec<NetBoxObject> {
    let architecture = fixture_architecture();
    build_objects(&ProjectionInput {
        architecture: &architecture,
        architecture_id,
        architecture_version: 1,
        snapshot: None,
        config: ProjectionConfigView {
            custom_field_prefix: "chv_".to_string(),
            site_name: Some(SITE.to_string()),
        },
    })
    .expect("build_objects")
    .objects
}

/// Render one desired object back into NetBox wire JSON (the inverse of
/// the client's fail-closed `parse_remote`), so a mocked GET response
/// parses to content byte-equal to the desired projection.
fn remote_fixture(netbox_id: i64, object: &NetBoxObject) -> Value {
    let custom_fields =
        serde_json::to_value(object.custom_fields()).expect("custom fields serialize");
    match object {
        NetBoxObject::Device(d) => json!({
            "id": netbox_id,
            "name": d.name.clone(),
            "status": { "value": d.status.as_str() },
            "site": d.site.clone().map(|s| json!({ "name": s })),
            "tags": d.tags.clone(),
            "custom_fields": custom_fields,
        }),
        NetBoxObject::VirtualMachine(v) => json!({
            "id": netbox_id,
            "name": v.name.clone(),
            "status": { "value": v.status.as_str() },
            "cluster": v.cluster.clone().map(|c| json!({ "name": c })),
            "device": v.device.clone().map(|d| json!({ "name": d })),
            "vcpus": v.cpu,
            "memory": v.memory_mb,
            "tags": v.tags.clone(),
            "custom_fields": custom_fields,
        }),
        NetBoxObject::Interface(i) => json!({
            "id": netbox_id,
            "name": i.name.clone(),
            "virtual_machine": { "name": i.virtual_machine.clone() },
            "description": i.description.clone(),
            "tags": i.tags.clone(),
            "custom_fields": custom_fields,
        }),
        NetBoxObject::Prefix(p) => json!({
            "id": netbox_id,
            "prefix": p.prefix.clone(),
            "vlan": p.vlan.map(|vid| json!({ "vid": vid })),
            "description": p.description.clone(),
            "tags": p.tags.clone(),
            "custom_fields": custom_fields,
        }),
        NetBoxObject::Vlan(v) => json!({
            "id": netbox_id,
            "vid": v.vid,
            "name": v.name.clone(),
            "tags": v.tags.clone(),
            "custom_fields": custom_fields,
        }),
        NetBoxObject::IpAddress(a) => {
            // NetBox returns addresses with a mask suffix and nested
            // assignment objects; the client strips/normalizes both.
            let assigned = a.assigned_to_interface.as_deref().and_then(|value| {
                let (vm, iface) = value.split_once('/')?;
                Some(json!({ "name": iface, "virtual_machine": { "name": vm } }))
            });
            json!({
                "id": netbox_id,
                "address": format!("{}/24", a.address),
                "assigned_object": assigned,
                "tags": a.tags.clone(),
                "custom_fields": custom_fields,
            })
        }
    }
}

fn kind_path(object: &NetBoxObject) -> &'static str {
    match object.kind() {
        NetBoxKind::Vlan => "/api/ipam/vlans/",
        NetBoxKind::Prefix => "/api/ipam/prefixes/",
        NetBoxKind::IpAddress => "/api/ipam/ip-addresses/",
        NetBoxKind::Interface => "/api/virtualization/interfaces/",
        NetBoxKind::VirtualMachine => "/api/virtualization/virtual-machines/",
        NetBoxKind::Device => "/api/dcim/devices/",
    }
}

fn page(results: Vec<Value>) -> Value {
    json!({ "count": results.len(), "next": null, "results": results })
}

// ---------------------------------------------------------------------------
// Store + worker scaffolding
// ---------------------------------------------------------------------------

async fn setup_topology_and_version(db: &TestDb, topo_id: &str, version_id: &str, model: &str) {
    TopologyRepository::new(db.pool.clone())
        .create(TopologyCreateInput {
            id: aid(topo_id),
            name: format!("{topo_id}-name"),
            display_name: Some(format!("{topo_id} display")),
            description: None,
            environment: Some("test".to_string()),
            status: ArchitectureStatus::Draft,
            owner_user_id: Some("user-1".to_string()),
            design_graph_json: None,
            latest_yaml: None,
        })
        .await
        .expect("topology created");
    VersionRepository::new(db.pool.clone())
        .create(VersionCreateInput {
            id: vid(version_id),
            architecture_id: aid(topo_id),
            version_number: 1,
            yaml_content: "x".to_string(),
            design_graph_json: None,
            normalized_model_json: Some(model.to_string()),
            change_summary: None,
            created_by: None,
        })
        .await
        .expect("version created");
}

async fn add_succeeded_apply_run(db: &TestDb, apply_id: &str, topo_id: &str, version_id: &str) {
    ApplyRunRepository::new(db.pool.clone())
        .create(ApplyRunCreateInput {
            id: appid(apply_id),
            architecture_id: aid(topo_id),
            architecture_version_id: vid(version_id),
            plan_id: None,
            task_id: None,
            status: RunStatus::Succeeded,
            requested_by: None,
            started_at: Some(chrono::Utc::now()),
        })
        .await
        .expect("apply run created");
}

async fn setup_config(
    db: &TestDb,
    endpoint: &str,
    topo_id: &str,
    retention: NetboxRetentionPolicy,
) {
    NetboxProjectionConfigRepository::new(db.pool.clone())
        .upsert(NetboxProjectionConfigUpsertInput {
            architecture_id: aid(topo_id),
            endpoint: endpoint.to_string(),
            token: Some(TOKEN.to_string()),
            token_secret_ref: format!("netbox-{topo_id}"),
            retention_policy: retention,
            enable_post_apply: false,
            custom_field_prefix: "chv_".to_string(),
            site_name: Some(SITE.to_string()),
        })
        .await
        .expect("config upserted");
}

async fn enqueue_run(
    db: &TestDb,
    run_id: &str,
    topo_id: &str,
    version_id: &str,
    mode: NetboxProjectionMode,
) {
    NetboxProjectionRunRepository::new(db.pool.clone())
        .create(NetboxProjectionRunCreateInput {
            id: nid(run_id),
            architecture_id: aid(topo_id),
            architecture_version_id: vid(version_id),
            trigger_kind: NetboxProjectionTrigger::Manual,
            mode,
            plan_json: None,
            requested_by: Some("senol".to_string()),
        })
        .await
        .expect("run enqueued");
}

/// Full happy-path scaffolding: topology + version + succeeded apply run
/// + config + queued export run.
async fn setup_projection(
    db: &TestDb,
    endpoint: &str,
    topo_id: &str,
    version_id: &str,
    apply_id: &str,
    run_id: &str,
    retention: NetboxRetentionPolicy,
) {
    let model = model_json();
    setup_topology_and_version(db, topo_id, version_id, &model).await;
    add_succeeded_apply_run(db, apply_id, topo_id, version_id).await;
    setup_config(db, endpoint, topo_id, retention).await;
    enqueue_run(
        db,
        run_id,
        topo_id,
        version_id,
        NetboxProjectionMode::Export,
    )
    .await;
}

/// Worker wired to the test store with the wiremock-compatible
/// (plain-HTTP) client constructor injected through the factory seam.
fn worker_for(db: &TestDb) -> NetboxProjectionWorker {
    NetboxProjectionWorker::new(
        NetboxProjectionRunRepository::new(db.pool.clone()),
        NetboxProjectionConfigRepository::new(db.pool.clone()),
        EventRepository::new(db.pool.clone()),
        ApplyRunRepository::new(db.pool.clone()),
        VersionRepository::new(db.pool.clone()),
    )
    .with_client_factory(Arc::new(NetBoxClient::new_unchecked_for_tests))
}

async fn get_run(
    db: &TestDb,
    run_id: &str,
) -> chv_controlplane_types::architecture::NetboxProjectionRun {
    NetboxProjectionRunRepository::new(db.pool.clone())
        .get(&nid(run_id))
        .await
        .expect("run lookup")
        .expect("run exists")
}

/// Audit events appended for one run (correlation id = run id).
async fn audit_events(db: &TestDb, run_id: &str) -> Vec<(String, Option<String>)> {
    sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT message, details FROM events WHERE correlation_id = $1",
    )
    .bind(run_id)
    .fetch_all(&db.pool)
    .await
    .expect("events query")
}

// ---------------------------------------------------------------------------
// wiremock scaffolding
// ---------------------------------------------------------------------------

/// Every GET on any `/api/…` path answers with an empty page — the
/// "NetBox has nothing of ours" state (used with per-kind POST mocks).
async fn mount_empty_remote(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path_regex("^/api/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(Vec::new())))
        .mount(server)
        .await;
}

/// Empty pages for the given kind paths only (for suites that need other
/// kinds populated — a catch-all would overlap those mocks).
async fn mount_empty_lists(server: &MockServer, paths: &[&str]) {
    for kind_path in paths {
        Mock::given(method("GET"))
            .and(path(*kind_path))
            .respond_with(ResponseTemplate::new(200).set_body_json(page(Vec::new())))
            .mount(server)
            .await;
    }
}

/// 201 create responses for the given (path, id) pairs.
async fn mount_creates(server: &MockServer, mocks: &[(&str, i64)]) {
    for (kind_path, netbox_id) in mocks {
        Mock::given(method("POST"))
            .and(path(*kind_path))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "id": netbox_id })))
            .mount(server)
            .await;
    }
}

/// The runner's post-loop IP-assignment fix-up PATCH.
async fn mount_ip_fixup_patch(server: &MockServer) {
    Mock::given(method("PATCH"))
        .and(path_regex("^/api/ipam/ip-addresses/[0-9]+/$"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
}

/// The "NetBox already mirrors the projection" state: one GET mock per
/// kind path returning exactly that kind's desired objects. `except`
/// leaves one kind path unmounted for suites that need finer control.
async fn mount_mirror_remote(server: &MockServer, architecture_id: &str, except: Option<&str>) {
    let mut by_path: BTreeMap<&'static str, Vec<Value>> = BTreeMap::new();
    for (i, object) in desired_objects(architecture_id).into_iter().enumerate() {
        let fixture = remote_fixture(400 + i as i64, &object);
        by_path.entry(kind_path(&object)).or_default().push(fixture);
    }
    for (kind_path, results) in by_path {
        if Some(kind_path) == except {
            continue;
        }
        Mock::given(method("GET"))
            .and(path(kind_path))
            .respond_with(ResponseTemplate::new(200).set_body_json(page(results)))
            .mount(server)
            .await;
    }
}

/// Count received requests by method and path prefix.
async fn request_count(server: &MockServer, http_method: &str, path_prefix: &str) -> usize {
    server
        .received_requests()
        .await
        .expect("request recording enabled")
        .into_iter()
        .filter(|r| r.method.as_str() == http_method && r.url.path().starts_with(path_prefix))
        .count()
}

/// Paths of all requests with the given method.
async fn request_paths(server: &MockServer, http_method: &str) -> Vec<String> {
    server
        .received_requests()
        .await
        .expect("request recording enabled")
        .into_iter()
        .filter(|r| r.method.as_str() == http_method)
        .map(|r| r.url.path().to_string())
        .collect()
}

fn outcome_of(
    run: &chv_controlplane_types::architecture::NetboxProjectionRun,
) -> NetboxProjectionOutcome {
    serde_json::from_str(run.result_json.as_deref().expect("result json")).expect("outcome parses")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn export_creates_all_objects_then_re_run_is_all_no_op() {
    let db = TestDb::new().await;
    let server = MockServer::start().await;
    setup_projection(
        &db,
        &server.uri(),
        "topo-1",
        "v-1",
        "apply-1",
        "netrun-1",
        NetboxRetentionPolicy::MarkStale,
    )
    .await;

    // First export against an empty NetBox: everything creates.
    mount_empty_remote(&server).await;
    mount_creates(&server, &CREATE_MOCKS).await;
    mount_ip_fixup_patch(&server).await;

    worker_for(&db).tick().await.expect("tick succeeds");

    let run = get_run(&db, "netrun-1").await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Succeeded);
    assert!(run.finished_at.is_some());

    let outcome = outcome_of(&run);
    assert!(outcome.error.is_none());
    assert_eq!(outcome.summary.succeeded, 6, "one create per kind");
    assert_eq!(outcome.summary.failed, 0);
    assert_eq!(outcome.summary.skipped, 0);
    assert_eq!(outcome.summary.not_attempted, 0);
    assert_eq!(outcome.plan.summary.create, 6);

    // Wire: one POST per kind, plus the post-loop IP-assignment fix-up.
    assert_eq!(request_count(&server, "POST", "/api/").await, 6);
    assert_eq!(request_count(&server, "PATCH", "/api/").await, 1);
    assert_eq!(request_count(&server, "DELETE", "/api/").await, 0);

    // The success audit event carries the run id and the summary.
    let events = audit_events(&db, "netrun-1").await;
    assert!(
        events.iter().any(
            |(message, details)| message == "architecture_netbox_export_succeeded"
                && details
                    .as_deref()
                    .is_some_and(|d| d.contains("\"create\":6"))
        ),
        "expected export_succeeded event with summary, got {events:?}"
    );

    // Second export against the mirrored state: all no_op, zero writes.
    server.reset().await;
    mount_mirror_remote(&server, "topo-1", None).await;
    enqueue_run(
        &db,
        "netrun-2",
        "topo-1",
        "v-1",
        NetboxProjectionMode::Export,
    )
    .await;

    worker_for(&db).tick().await.expect("tick succeeds");

    let run = get_run(&db, "netrun-2").await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Succeeded);
    let outcome = outcome_of(&run);
    assert!(outcome.error.is_none());
    assert_eq!(outcome.summary.skipped, 6, "all no_op");
    assert_eq!(outcome.summary.succeeded, 0);
    assert_eq!(outcome.summary.failed, 0);
    assert_eq!(outcome.plan.summary.create, 0);

    // No mutation was sent on the re-run.
    assert_eq!(request_count(&server, "POST", "/api/").await, 0);
    assert_eq!(request_count(&server, "PATCH", "/api/").await, 0);
    assert_eq!(request_count(&server, "DELETE", "/api/").await, 0);
}

#[tokio::test]
async fn partial_failure_aborts_and_retry_resumes_without_duplicate_create() {
    let db = TestDb::new().await;
    let server = MockServer::start().await;
    setup_projection(
        &db,
        &server.uri(),
        "topo-1",
        "v-1",
        "apply-1",
        "netrun-1",
        NetboxRetentionPolicy::MarkStale,
    )
    .await;

    // First attempt: the VLAN create succeeds, the prefix create 500s —
    // abort-on-first-hard-failure leaves the run failed and retryable.
    mount_empty_remote(&server).await;
    Mock::given(method("POST"))
        .and(path("/api/ipam/vlans/"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "id": 101 })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/ipam/prefixes/"))
        .respond_with(ResponseTemplate::new(500).set_body_string("netbox exploded"))
        .mount(&server)
        .await;

    worker_for(&db).tick().await.expect("tick succeeds");

    let run = get_run(&db, "netrun-1").await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Failed);
    assert_eq!(run.attempt_count, 1, "failure consumed an attempt");
    let error = run.error_message.as_deref().expect("error message");
    assert!(error.contains("status 500"), "unexpected error: {error}");

    // Exactly one create of each attempted kind before the abort.
    assert_eq!(request_count(&server, "POST", "/api/ipam/vlans/").await, 1);
    assert_eq!(
        request_count(&server, "POST", "/api/ipam/prefixes/").await,
        1
    );
    assert_eq!(request_count(&server, "POST", "/api/").await, 2);

    // Retry: the VLAN written by the failed attempt is already remote
    // (matched by external id — resume, not duplicate create).
    let run_repo = NetboxProjectionRunRepository::new(db.pool.clone());
    run_repo.requeue(&nid("netrun-1")).await.expect("requeue");
    server.reset().await;

    let vlan_fixture = desired_objects("topo-1")
        .into_iter()
        .find(|object| matches!(object, NetBoxObject::Vlan(_)))
        .expect("desired vlan");
    Mock::given(method("GET"))
        .and(path("/api/ipam/vlans/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(page(vec![remote_fixture(101, &vlan_fixture)])),
        )
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

    let run = get_run(&db, "netrun-1").await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Succeeded);
    assert_eq!(run.attempt_count, 1, "success does not consume an attempt");
    let outcome = outcome_of(&run);
    assert!(outcome.error.is_none());
    assert_eq!(outcome.summary.skipped, 1, "the written VLAN is a no_op");
    assert_eq!(
        outcome.summary.succeeded, 5,
        "only the missing kinds create"
    );

    // The resume did not re-create the VLAN.
    assert_eq!(
        request_count(&server, "POST", "/api/ipam/vlans/").await,
        0,
        "resume must not duplicate the already-written object"
    );
    assert_eq!(request_count(&server, "POST", "/api/").await, 5);
}

#[tokio::test]
async fn auth_failure_fails_run_with_retryable_error() {
    let db = TestDb::new().await;
    let server = MockServer::start().await;
    setup_projection(
        &db,
        &server.uri(),
        "topo-1",
        "v-1",
        "apply-1",
        "netrun-1",
        NetboxRetentionPolicy::MarkStale,
    )
    .await;

    Mock::given(method("GET"))
        .and(path_regex("^/api/"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    worker_for(&db).tick().await.expect("tick succeeds");

    let run = get_run(&db, "netrun-1").await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Failed);
    assert_eq!(run.attempt_count, 1);
    let error = run.error_message.as_deref().expect("error message");
    assert!(
        error.contains("authentication failed"),
        "expected NETBOX_AUTH_FAILED classification, got: {error}"
    );
    assert!(!error.contains(TOKEN), "error message must be token-free");

    let events = audit_events(&db, "netrun-1").await;
    assert!(
        events
            .iter()
            .any(|(message, _)| message == "architecture_netbox_export_failed"),
        "expected export_failed event, got {events:?}"
    );

    // The failed run stays retryable under the attempt cap.
    NetboxProjectionRunRepository::new(db.pool.clone())
        .requeue(&nid("netrun-1"))
        .await
        .expect("failed auth run is retryable");
}

#[tokio::test]
async fn netbox_outage_fails_run_and_worker_survives() {
    let db = TestDb::new().await;
    // Nothing listens on port 1: every request is connection-refused.
    setup_projection(
        &db,
        "http://127.0.0.1:1",
        "topo-1",
        "v-1",
        "apply-1",
        "netrun-1",
        NetboxRetentionPolicy::MarkStale,
    )
    .await;

    let worker = worker_for(&db);
    worker.tick().await.expect("outage never kills the worker");

    let run = get_run(&db, "netrun-1").await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Failed);
    assert_eq!(run.attempt_count, 1);
    let error = run.error_message.as_deref().expect("error message");
    assert!(error.contains("unreachable"), "unexpected error: {error}");

    // The worker keeps ticking: requeue and fail again without bubbling.
    NetboxProjectionRunRepository::new(db.pool.clone())
        .requeue(&nid("netrun-1"))
        .await
        .expect("outage run is retryable");
    worker.tick().await.expect("second tick survives too");

    let run = get_run(&db, "netrun-1").await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Failed);
    assert_eq!(run.attempt_count, 2);
}

/// A foreign VLAN squatting on the desired VLAN's natural key (vid) is a
/// `conflict` — never a write, and never a DELETE even under `delete`
/// retention.
#[tokio::test]
async fn foreign_occupied_natural_key_is_conflict_and_never_deleted() {
    let db = TestDb::new().await;
    let server = MockServer::start().await;
    setup_projection(
        &db,
        &server.uri(),
        "topo-1",
        "v-1",
        "apply-1",
        "netrun-1",
        NetboxRetentionPolicy::Delete,
    )
    .await;

    // A foreign VLAN (no ownership marker) on the desired vid.
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

    let run = get_run(&db, "netrun-1").await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Succeeded);
    let outcome = outcome_of(&run);
    assert!(outcome.error.is_none());

    let vlan_entry = outcome
        .entries
        .iter()
        .find(|entry| entry.kind == NetBoxKind::Vlan)
        .expect("vlan entry");
    assert_eq!(vlan_entry.action, NetboxPlanAction::Conflict);
    assert_eq!(vlan_entry.status, NetboxEntryStatus::Skipped);

    // Conflicts never write and the foreign object is never deleted.
    assert_eq!(request_count(&server, "POST", "/api/ipam/vlans/").await, 0);
    assert_eq!(request_count(&server, "DELETE", "/api/").await, 0);
    // The unoccupied kinds still create.
    assert_eq!(request_count(&server, "POST", "/api/").await, 5);
}

/// Under `delete` retention a stale object of this architecture IS
/// deleted (after the runner's ownership re-verification), and only that
/// object.
#[tokio::test]
async fn delete_retention_deletes_owned_stale_object_after_reverification() {
    let db = TestDb::new().await;
    let server = MockServer::start().await;
    setup_projection(
        &db,
        &server.uri(),
        "topo-5",
        "v-5",
        "apply-5",
        "netrun-5",
        NetboxRetentionPolicy::Delete,
    )
    .await;

    // Five kinds mirror the projection; the VLAN kind needs split mocks
    // because its list (desired + stale) and its natural-key lookup
    // (desired only) must answer differently to stay unambiguous.
    mount_mirror_remote(&server, "topo-5", Some("/api/ipam/vlans/")).await;

    let desired_vlan = desired_objects("topo-5")
        .into_iter()
        .find(|object| matches!(object, NetBoxObject::Vlan(_)))
        .expect("desired vlan");
    // Id 500: the mirror mounts use 400+i for the other kinds (prefix is
    // 401), and the remote state is deduplicated by NetBox id — a
    // colliding fixture would silently clobber its kind.
    let desired_vlan_fixture = remote_fixture(500, &desired_vlan);
    // A stale-but-ours VLAN: full marker of this architecture, CHV
    // source long gone.
    let stale_vlan = json!({
        "id": 77,
        "vid": 77,
        "name": "legacy",
        "tags": [],
        "custom_fields": {
            "chv_external_id": "arch:topo-5:network/legacy#vlan:1",
            "chv_architecture_id": "topo-5",
            "chv_managed_by": "chv",
            "chv_managed_state": "active",
            "chv_architecture_version": "1",
            "chv_mapping_version": "v1"
        },
    });
    Mock::given(method("GET"))
        .and(path("/api/ipam/vlans/"))
        .and(query_param("cf_chv_architecture_id", "topo-5"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(page(vec![desired_vlan_fixture.clone(), stale_vlan])),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/ipam/vlans/"))
        .and(query_param("vid", "42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page(vec![desired_vlan_fixture])))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/ipam/vlans/77/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    worker_for(&db).tick().await.expect("tick succeeds");

    let run = get_run(&db, "netrun-5").await;
    let outcome = outcome_of(&run);
    assert_eq!(run.status, NetboxProjectionRunStatus::Succeeded);
    assert!(outcome.error.is_none());

    let stale_entry = outcome
        .entries
        .iter()
        .find(|entry| entry.action == NetboxPlanAction::Stale)
        .expect("stale entry for the removed network");
    assert_eq!(stale_entry.status, NetboxEntryStatus::Succeeded);
    assert_eq!(outcome.summary.skipped, 6, "everything desired is no_op");
    assert_eq!(outcome.summary.succeeded, 1, "exactly the stale delete");

    // Exactly one DELETE, aimed at the stale object.
    assert_eq!(
        request_paths(&server, "DELETE").await,
        vec!["/api/ipam/vlans/77/"],
        "only the owned stale object is deleted"
    );
    assert_eq!(request_count(&server, "POST", "/api/").await, 0);
}

#[tokio::test]
async fn worker_claims_one_run_per_architecture_and_tick_is_idempotent() {
    let db = TestDb::new().await;
    let server = MockServer::start().await;
    let model = model_json();
    for (topo, version, apply, run) in [
        ("topo-a", "v-a", "apply-a", "netrun-a"),
        ("topo-b", "v-b", "apply-b", "netrun-b"),
    ] {
        setup_topology_and_version(&db, topo, version, &model).await;
        add_succeeded_apply_run(&db, apply, topo, version).await;
        setup_config(&db, &server.uri(), topo, NetboxRetentionPolicy::MarkStale).await;
        enqueue_run(&db, run, topo, version, NetboxProjectionMode::Export).await;
    }

    mount_empty_remote(&server).await;
    mount_creates(&server, &CREATE_MOCKS).await;
    mount_ip_fixup_patch(&server).await;

    // One tick processes one run per architecture: both queued runs go.
    worker_for(&db).tick().await.expect("tick succeeds");

    for run in ["netrun-a", "netrun-b"] {
        let done = get_run(&db, run).await;
        assert_eq!(done.status, NetboxProjectionRunStatus::Succeeded, "{run}");
    }
    assert_eq!(request_count(&server, "POST", "/api/").await, 12);

    // A second tick finds nothing queued and writes nothing further.
    worker_for(&db).tick().await.expect("second tick succeeds");
    assert_eq!(request_count(&server, "POST", "/api/").await, 12);
}

#[tokio::test]
async fn dry_run_computes_plan_without_writes() {
    let db = TestDb::new().await;
    let server = MockServer::start().await;
    let model = model_json();
    setup_topology_and_version(&db, "topo-1", "v-1", &model).await;
    add_succeeded_apply_run(&db, "apply-1", "topo-1", "v-1").await;
    setup_config(
        &db,
        &server.uri(),
        "topo-1",
        NetboxRetentionPolicy::MarkStale,
    )
    .await;
    enqueue_run(
        &db,
        "netrun-1",
        "topo-1",
        "v-1",
        NetboxProjectionMode::DryRun,
    )
    .await;

    // Read-only mocks only: any mutation would 404 and fail the run.
    mount_empty_remote(&server).await;

    worker_for(&db).tick().await.expect("tick succeeds");

    let run = get_run(&db, "netrun-1").await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Succeeded);
    let plan: NetboxProjectionPlan =
        serde_json::from_str(run.result_json.as_deref().expect("result json"))
            .expect("dry-run result is the serialized plan");
    assert_eq!(plan.summary.create, 6);

    // A dry run never mutates NetBox.
    assert_eq!(request_count(&server, "POST", "/api/").await, 0);
    assert_eq!(request_count(&server, "PATCH", "/api/").await, 0);
    assert_eq!(request_count(&server, "DELETE", "/api/").await, 0);

    let events = audit_events(&db, "netrun-1").await;
    assert!(
        events
            .iter()
            .any(|(message, _)| message == "architecture_netbox_dry_run"),
        "expected dry_run event, got {events:?}"
    );
}

#[tokio::test]
async fn reclamation_fails_stale_running_run_and_leaves_fresh_alone() {
    let db = TestDb::new().await;
    let server = MockServer::start().await;
    let model = model_json();
    let run_repo = NetboxProjectionRunRepository::new(db.pool.clone());

    // A run claimed 20 minutes ago and never finished (worker crash).
    setup_topology_and_version(&db, "topo-1", "v-1", &model).await;
    setup_config(
        &db,
        &server.uri(),
        "topo-1",
        NetboxRetentionPolicy::MarkStale,
    )
    .await;
    enqueue_run(
        &db,
        "netrun-stale",
        "topo-1",
        "v-1",
        NetboxProjectionMode::Export,
    )
    .await;
    run_repo
        .claim_next_queued(&aid("topo-1"))
        .await
        .expect("claim")
        .expect("queued run");
    sqlx::query(
        "UPDATE netbox_projection_runs SET started_at = '2020-01-01T00:00:00Z' WHERE id = $1",
    )
    .bind("netrun-stale")
    .execute(&db.pool)
    .await
    .expect("backdate started_at");

    // A run claimed just now (inside the 15-minute lease).
    setup_topology_and_version(&db, "topo-2", "v-2", &model).await;
    setup_config(
        &db,
        &server.uri(),
        "topo-2",
        NetboxRetentionPolicy::MarkStale,
    )
    .await;
    enqueue_run(
        &db,
        "netrun-fresh",
        "topo-2",
        "v-2",
        NetboxProjectionMode::Export,
    )
    .await;
    run_repo
        .claim_next_queued(&aid("topo-2"))
        .await
        .expect("claim")
        .expect("queued run");

    worker_for(&db).tick().await.expect("tick succeeds");

    let stale = get_run(&db, "netrun-stale").await;
    assert_eq!(stale.status, NetboxProjectionRunStatus::Failed);
    assert_eq!(stale.attempt_count, 1, "reclamation consumed an attempt");
    assert!(
        stale
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("reclaimed"),
        "reclamation message: {:?}",
        stale.error_message
    );
    let events = audit_events(&db, "netrun-stale").await;
    assert!(
        events.iter().any(
            |(message, details)| message == "architecture_netbox_export_failed"
                && details.as_deref().is_some_and(|d| d.contains("reclaimed"))
        ),
        "expected reclaimed failure event, got {events:?}"
    );

    let fresh = get_run(&db, "netrun-fresh").await;
    assert_eq!(fresh.status, NetboxProjectionRunStatus::Running);

    // Idempotent: a second sweep finds nothing new to reclaim.
    worker_for(&db).tick().await.expect("second tick succeeds");
    let stale = get_run(&db, "netrun-stale").await;
    assert_eq!(stale.attempt_count, 1);
    let fresh = get_run(&db, "netrun-fresh").await;
    assert_eq!(fresh.status, NetboxProjectionRunStatus::Running);
}

/// The worker's fail-closed gates: no config → NETBOX_NOT_CONFIGURED;
/// no succeeded apply run → NETBOX_NOT_APPLIED. NetBox is never reached.
#[tokio::test]
async fn missing_config_or_succeeded_apply_fails_closed() {
    let db = TestDb::new().await;
    let model = model_json();

    // Config deleted between enqueue and execution.
    setup_topology_and_version(&db, "topo-1", "v-1", &model).await;
    add_succeeded_apply_run(&db, "apply-1", "topo-1", "v-1").await;
    enqueue_run(
        &db,
        "netrun-1",
        "topo-1",
        "v-1",
        NetboxProjectionMode::Export,
    )
    .await;

    // Config present, but the architecture was never applied.
    setup_topology_and_version(&db, "topo-2", "v-2", &model).await;
    setup_config(
        &db,
        "https://netbox.example.internal",
        "topo-2",
        NetboxRetentionPolicy::MarkStale,
    )
    .await;
    enqueue_run(
        &db,
        "netrun-2",
        "topo-2",
        "v-2",
        NetboxProjectionMode::Export,
    )
    .await;

    worker_for(&db).tick().await.expect("tick succeeds");

    let run = get_run(&db, "netrun-1").await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Failed);
    assert!(
        run.error_message
            .as_deref()
            .unwrap_or_default()
            .contains("NETBOX_NOT_CONFIGURED"),
        "unexpected error: {:?}",
        run.error_message
    );

    let run = get_run(&db, "netrun-2").await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Failed);
    assert!(
        run.error_message
            .as_deref()
            .unwrap_or_default()
            .contains("NETBOX_NOT_APPLIED"),
        "unexpected error: {:?}",
        run.error_message
    );

    // Both failures produced their audit events.
    for run_id in ["netrun-1", "netrun-2"] {
        assert!(
            audit_events(&db, run_id)
                .await
                .iter()
                .any(|(message, _)| message == "architecture_netbox_export_failed"),
            "expected failure event for {run_id}"
        );
    }
}

/// The plaintext token never reaches a log line, a persisted error
/// message, or an event payload — belt-and-braces on top of the
/// token-free-by-construction error types.
///
/// Log capture uses the process-wide global collector (see
/// [`log_capture::LogCollector::global`]): the worker's failure
/// warning is emitted through a `tracing` callsite that parallel
/// tests also execute, and a thread-local `set_default` collector
/// races with the process-wide callsite-interest cache.
#[tokio::test]
async fn token_never_appears_in_logs_or_persisted_errors() {
    let logs = log_capture::LogCollector::global();

    let db = TestDb::new().await;
    // Unique ids: the global collector also receives the warnings of
    // the other netbox tests running in parallel; the non-vacuity
    // check below must match *this* run's failure warning.
    setup_projection(
        &db,
        "http://127.0.0.1:1",
        "topo-tok",
        "v-tok",
        "apply-tok",
        "netrun-tok",
        NetboxRetentionPolicy::MarkStale,
    )
    .await;

    worker_for(&db).tick().await.expect("tick succeeds");

    let run = get_run(&db, "netrun-tok").await;
    assert_eq!(run.status, NetboxProjectionRunStatus::Failed);

    // The collector saw this run's failure (the assertion below is not
    // vacuous).
    let messages = logs.messages();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("netrun-tok") && m.contains("netbox projection run failed")),
        "expected the worker's failure warning, got {messages:?}; run error: {:?}",
        run.error_message
    );
    // ... but never the token, in any captured line of any test.
    for message in &messages {
        assert!(
            !message.contains(TOKEN),
            "token leaked into a log line: {message}"
        );
    }

    let error = run.error_message.as_deref().expect("error message");
    assert!(!error.contains(TOKEN), "token leaked into the run row");
    for (_, details) in audit_events(&db, "netrun-tok").await {
        let details = details.unwrap_or_default();
        assert!(!details.contains(TOKEN), "token leaked into an event");
    }
}
