//! Handler-level integration tests for the ADR-021 stretched-L2 fabric path.
//!
//! These tests drive the real tonic UDS server (`NetworkServer`) with a
//! recording mock `NetworkExecutor` that mirrors the `LinuxExecutor`
//! contract: fabric apply during topology ensure, fabric-first teardown,
//! generation fencing in the handlers (which own the topology table),
//! tenant-MTU propagation into NIC attach, and the fail-closed behavior
//! when the fabric provider is disabled.
//!
//! The sandbox is unprivileged, so the real `ip`/`wg`-backed executor
//! cannot be driven here; its RecordingRunner-backed provider coverage
//! lives in `chv-nwd-core/src/fabric.rs` unit tests, and the shared
//! provider contract is gated in `tests/fabric_conformance.rs`.

use async_trait::async_trait;
use chv_errors::ChvError;
use chv_nwd_api::chv_nwd_api::{
    network_service_client::NetworkServiceClient, AttachVmNicRequest, DeleteNetworkTopologyRequest,
    EnsureNetworkTopologyRequest, FabricPeer, FabricPlan, GetFabricIdentityRequest,
    GetOverlayStatusRequest, NicSpec, OverlayType, TopologySpec, UpdateOverlayRequest,
};
use chv_nwd_core::executor::{NetworkExecutor, OverlayStatusInfo, TopologyApplyResult};
use chv_nwd_core::fabric::{AppliedFabric, ApplyReport, FabricIdentity};
use chv_nwd_core::handlers::NetworkServiceImpl;
use chv_nwd_core::{NetworkServer, TopologyState, TopologyTable};
use chv_observability::Metrics;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::net::UnixStream;
use tonic::transport::{Endpoint, Uri};
use tonic::Request as TonicRequest;
use tower::service_fn;

const TEST_PUBLIC_KEY: &str = "K7XbF9cV2mQpT3nZ8sL4dW6yH1jR5uA0eG9iO2pS7kM=";
const PEER_PUBLIC_KEY: &str = "Q8YcG0dW3nRqU4oA9tM5eX7zI2kS6vB1fH0jP3qT8lN=";
const DEFAULT_TENANT_MTU: u32 = 1380;

/// Recording mock mirroring the `LinuxExecutor` fabric contract.
#[derive(Clone)]
struct FabricExecutor {
    calls: Arc<StdMutex<Vec<String>>>,
    fabric_enabled: bool,
    /// Stand-in for the provider's durable ownership journal: survives a
    /// "restart" (clearing the TopologyTable) in tests.
    owned: Arc<StdMutex<HashSet<String>>>,
    /// When set, `fabric_overlay_status` fails (m9 error path).
    overlay_status_error: bool,
    /// When set, fabric removal fails persistently (m5 handler semantics).
    remove_fails: bool,
}

impl FabricExecutor {
    fn enabled() -> Self {
        Self {
            calls: Arc::new(StdMutex::new(Vec::new())),
            fabric_enabled: true,
            owned: Arc::new(StdMutex::new(HashSet::new())),
            overlay_status_error: false,
            remove_fails: false,
        }
    }

    fn disabled() -> Self {
        Self {
            fabric_enabled: false,
            ..Self::enabled()
        }
    }

    fn with_overlay_status_error() -> Self {
        Self {
            overlay_status_error: true,
            ..Self::enabled()
        }
    }

    fn with_failing_removal() -> Self {
        Self {
            remove_fails: true,
            ..Self::enabled()
        }
    }

    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn count(&self, prefix: &str) -> usize {
        self.calls()
            .iter()
            .filter(|c| c.starts_with(prefix))
            .count()
    }
}

#[async_trait]
impl NetworkExecutor for FabricExecutor {
    async fn ensure_topology(&self, spec: &TopologySpec) -> Result<TopologyApplyResult, ChvError> {
        self.record(format!("ensure:{}", spec.network_id));
        let mut tenant_mtu = None;
        let mut fabric_plan_generation = None;
        let mut binding_generation = None;
        if spec.vni > 0 && spec.overlay_type == OverlayType::OverlayVxlan as i32 {
            let plan = spec
                .fabric
                .as_ref()
                .ok_or_else(|| ChvError::InvalidArgument {
                    field: "fabric".to_string(),
                    reason: "VXLAN overlay requires a fabric plan".to_string(),
                })?;
            let applied = self
                .apply_fabric_overlay(&spec.network_id, spec.vni, plan, &spec.bridge_name)
                .await?;
            tenant_mtu = Some(applied.tenant_mtu);
            fabric_plan_generation = Some(applied.plan_generation);
            binding_generation = Some(applied.binding_generation);
        }
        Ok(TopologyApplyResult {
            namespace_handle: spec.namespace_name.clone(),
            bridge_handle: spec.bridge_name.clone(),
            tenant_mtu,
            fabric_plan_generation,
            binding_generation,
        })
    }

    async fn delete_topology(
        &self,
        network_id: &str,
        state: &TopologyState,
    ) -> Result<(), ChvError> {
        // Fabric-first teardown (reverse dependency order).
        if state.fabric_plan_generation.is_some() {
            self.remove_fabric_overlay(network_id).await?;
        }
        self.record(format!("delete:{network_id}"));
        Ok(())
    }

    async fn health(&self, _network_id: &str, _state: &TopologyState) -> Result<String, ChvError> {
        Ok("healthy".to_string())
    }

    async fn attach_vm_nic(
        &self,
        network_id: &str,
        nic_id: &str,
        _vm_id: &str,
        _bridge_name: &str,
        tenant_mtu: Option<u32>,
        _mac_address: &str,
        _ip_address: &str,
    ) -> Result<(String, String), ChvError> {
        let mtu = tenant_mtu
            .map(|m| m.to_string())
            .unwrap_or_else(|| "none".to_string());
        self.record(format!("attach:{nic_id}:{mtu}"));
        Ok((format!("ns-{network_id}"), format!("tap-{nic_id}")))
    }

    async fn detach_vm_nic(
        &self,
        _nic_id: &str,
        _ownership: chv_common::AttachmentOwnership,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn set_firewall_policy(
        &self,
        _network_id: &str,
        _policy_version: &str,
        _policy_json: &[u8],
        _bridge_name: &str,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn set_nat_policy(
        &self,
        _network_id: &str,
        _policy_version: &str,
        _policy_json: &[u8],
        _bridge_name: &str,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn ensure_dhcp_scope(
        &self,
        _network_id: &str,
        _cidr: &str,
        _range_start: &str,
        _range_end: &str,
        _dns_servers: &[String],
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn ensure_dns_scope(
        &self,
        _network_id: &str,
        _forwarders: &[&str],
        _static_records: &std::collections::HashMap<String, String>,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn expose_service(
        &self,
        _network_id: &str,
        _exposure_id: &str,
        _protocol: &str,
        _external_port: u32,
        _target_ip: &str,
        _target_port: u32,
        _mode: &str,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn withdraw_service_exposure(
        &self,
        _network_id: &str,
        _exposure_id: &str,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn send_gratuitous_arp(
        &self,
        _namespace: &str,
        _bridge_name: &str,
        _vm_ip: &str,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn apply_fabric_overlay(
        &self,
        network_id: &str,
        vni: u32,
        plan: &FabricPlan,
        bridge_name: &str,
    ) -> Result<AppliedFabric, ChvError> {
        if !self.fabric_enabled {
            return Err(ChvError::InvalidArgument {
                field: "fabric".to_string(),
                reason: "fabric provider is disabled".to_string(),
            });
        }
        self.record(format!(
            "apply_fabric:{network_id}:{vni}:{}:{bridge_name}",
            plan.plan_generation
        ));
        self.owned.lock().unwrap().insert(network_id.to_string());
        Ok(AppliedFabric {
            report: ApplyReport::default(),
            plan_generation: plan.plan_generation,
            binding_generation: plan.binding_generation,
            tenant_mtu: if plan.tenant_mtu == 0 {
                DEFAULT_TENANT_MTU
            } else {
                plan.tenant_mtu
            },
            consumer_veth: format!("chv-{network_id}-a"),
        })
    }

    async fn remove_fabric_overlay(&self, network_id: &str) -> Result<(), ChvError> {
        if !self.fabric_enabled {
            return Err(ChvError::InvalidArgument {
                field: "fabric".to_string(),
                reason: "fabric provider is disabled".to_string(),
            });
        }
        self.record(format!("remove_fabric:{network_id}"));
        if self.remove_fails {
            return Err(ChvError::Internal {
                reason: "fabric removal deliberately failing".to_string(),
            });
        }
        self.owned.lock().unwrap().remove(network_id);
        Ok(())
    }

    async fn fabric_owned(&self, network_id: &str) -> Result<bool, ChvError> {
        self.record(format!("fabric_owned:{network_id}"));
        if !self.fabric_enabled {
            return Ok(false);
        }
        Ok(self.owned.lock().unwrap().contains(network_id))
    }

    async fn fabric_identity(&self) -> Result<FabricIdentity, ChvError> {
        if !self.fabric_enabled {
            return Err(ChvError::InvalidArgument {
                field: "fabric".to_string(),
                reason: "fabric provider is disabled".to_string(),
            });
        }
        Ok(FabricIdentity {
            public_key: TEST_PUBLIC_KEY.to_string(),
            underlay_mtu: 1500,
        })
    }

    async fn fabric_overlay_status(&self, network_id: &str) -> Result<OverlayStatusInfo, ChvError> {
        self.record(format!("overlay_status:{network_id}"));
        if self.overlay_status_error {
            return Err(ChvError::Internal {
                reason: "fabric status deliberately failing".to_string(),
            });
        }
        if self.owned.lock().unwrap().contains(network_id) {
            Ok(OverlayStatusInfo {
                vxlan_interface_up: true,
                fdb_entry_count: 1,
            })
        } else {
            Ok(OverlayStatusInfo {
                vxlan_interface_up: false,
                fdb_entry_count: 0,
            })
        }
    }

    async fn reassert_tenant_mtu(
        &self,
        network_id: &str,
        _bridge_name: &str,
        _subnet_cidr: &str,
        _gateway_ip: &str,
        tenant_mtu: u32,
    ) -> Result<(), ChvError> {
        self.record(format!("reassert_mtu:{network_id}:{tenant_mtu}"));
        Ok(())
    }
}

async fn make_client(socket: PathBuf) -> NetworkServiceClient<tonic::transport::Channel> {
    let channel = Endpoint::try_from("http://[::]:50051")
        .unwrap()
        .connect_with_connector(service_fn(move |_: Uri| {
            let s = socket.clone();
            async move {
                let stream = UnixStream::connect(s).await?;
                Ok::<_, std::io::Error>(hyper_util::rt::tokio::TokioIo::new(stream))
            }
        }))
        .await
        .unwrap();
    NetworkServiceClient::new(channel)
}

/// Spawn a real tonic UDS server over the mock executor. The returned
/// `TempDir` keeps the socket path alive for the duration of the test.
async fn spawn_server(executor: FabricExecutor) -> (PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nwd.sock");
    let server = NetworkServer::new(executor, Metrics::new());
    let socket_clone = socket.clone();
    tokio::spawn(async move {
        server.serve(&socket_clone).await.ok();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    (socket, dir)
}

fn fabric_plan(generation: u64, tenant_mtu: u32) -> Option<FabricPlan> {
    fabric_plan_with_binding(generation, 1, tenant_mtu)
}

fn fabric_plan_with_binding(
    generation: u64,
    binding_generation: u64,
    tenant_mtu: u32,
) -> Option<FabricPlan> {
    Some(FabricPlan {
        fabric_domain_id: "fabric-1".to_string(),
        local_host_id: "host-01".to_string(),
        local_fabric_ip: "100.100.0.1".to_string(),
        tenant_mtu,
        fabric_mtu: 0,
        binding_generation,
        plan_generation: generation,
        peers: vec![FabricPeer {
            node_id: "host-02".to_string(),
            public_key: PEER_PUBLIC_KEY.to_string(),
            underlay_endpoint: "203.0.113.7:65001".to_string(),
            fabric_ip: "100.100.0.2".to_string(),
        }],
    })
}

fn fabric_topology_spec(network_id: &str, generation: u64) -> TopologySpec {
    TopologySpec {
        network_id: network_id.to_string(),
        tenant_id: "t1".to_string(),
        bridge_name: "br-fab".to_string(),
        namespace_name: "ns-fab".to_string(),
        subnet_cidr: "10.0.50.0/24".to_string(),
        gateway_ip: "10.0.50.1".to_string(),
        options: Default::default(),
        vni: 100,
        vtep_endpoints: vec![],
        overlay_type: OverlayType::OverlayVxlan as i32,
        fabric: fabric_plan(generation, 0),
    }
}

#[tokio::test]
async fn ensure_with_fabric_applies_and_persists_state() {
    let executor = FabricExecutor::enabled();
    let (socket, _dir) = spawn_server(executor.clone()).await;
    let mut client = make_client(socket).await;

    let resp = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-fab", 7)),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp.status, "OK");

    // The fabric apply must carry network, vni, plan generation and bridge.
    assert!(executor
        .calls()
        .iter()
        .any(|c| c == "apply_fabric:net-fab:100:7:br-fab"));

    // Persisted state must carry the tenant MTU (defaulted from 0) and the
    // applied generation into later operations: attach receives the MTU.
    let attach = client
        .attach_vm_nic(AttachVmNicRequest {
            meta: None,
            nic: Some(NicSpec {
                nic_id: "nic-fab".to_string(),
                vm_id: "vm-fab".to_string(),
                network_id: "net-fab".to_string(),
                mac_address: "02:00:00:00:00:01".to_string(),
                tap_name: "tap-fab".to_string(),
                ip_address: "10.0.50.10".to_string(),
            }),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(attach.result.as_ref().unwrap().status, "OK");
    assert!(executor.calls().iter().any(|c| c == "attach:nic-fab:1380"));
}

#[tokio::test]
async fn ensure_rejects_stale_fabric_generation() {
    let executor = FabricExecutor::enabled();
    let (socket, _dir) = spawn_server(executor.clone()).await;
    let mut client = make_client(socket).await;

    let first = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-stale", 5)),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(first.status, "OK");
    assert_eq!(executor.count("apply_fabric:"), 1);

    // An older plan generation is fenced off (ADR-021 §4).
    let stale = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-stale", 3)),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(stale.status, "error");
    assert_eq!(stale.error_code, "STALE_GENERATION");
    assert_eq!(executor.count("apply_fabric:"), 1, "no re-apply on stale");

    // An unchanged topology (same generation) is idempotently OK without
    // touching the executor again.
    let replay = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-stale", 5)),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(replay.status, "OK");
    assert_eq!(executor.count("apply_fabric:"), 1, "no re-apply on replay");
}

#[tokio::test]
async fn update_overlay_fabric_path_fences_and_updates_state() {
    let executor = FabricExecutor::enabled();
    let (socket, _dir) = spawn_server(executor.clone()).await;
    let mut client = make_client(socket).await;

    // Ensure with generation 2 (explicit tenant MTU 1400).
    let mut spec = fabric_topology_spec("net-upd", 2);
    spec.fabric = fabric_plan(2, 1400);
    let ensure = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(spec),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ensure.status, "OK");
    assert!(executor
        .calls()
        .iter()
        .any(|c| c == "apply_fabric:net-upd:100:2:br-fab"));

    // A newer plan via UpdateOverlay applies and persists the new
    // generation and tenant MTU.
    let update = client
        .update_overlay(UpdateOverlayRequest {
            network_id: "net-upd".to_string(),
            vni: 100,
            vtep_endpoints: vec![],
            fdb_entries: vec![],
            fabric: fabric_plan(9, 0),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(update.result.as_ref().unwrap().status, "OK");
    assert!(executor
        .calls()
        .iter()
        .any(|c| c == "apply_fabric:net-upd:100:9:br-fab"));

    // Stale generation is rejected in-band.
    let stale = client
        .update_overlay(UpdateOverlayRequest {
            network_id: "net-upd".to_string(),
            vni: 100,
            vtep_endpoints: vec![],
            fdb_entries: vec![],
            fabric: fabric_plan(4, 0),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(stale.result.as_ref().unwrap().status, "error");
    assert_eq!(
        stale.result.as_ref().unwrap().error_code,
        "STALE_GENERATION"
    );
    assert_eq!(executor.count("apply_fabric:"), 2, "stale plan not applied");

    // Replaying the same (current) plan is OK: the handler always applies
    // and the provider layer dedups (idempotent re-assert).
    let replay = client
        .update_overlay(UpdateOverlayRequest {
            network_id: "net-upd".to_string(),
            vni: 100,
            vtep_endpoints: vec![],
            fdb_entries: vec![],
            fabric: fabric_plan(9, 0),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(replay.result.as_ref().unwrap().status, "OK");

    // The persisted generation (9) now fences a later ensure with the old
    // generation 2 plan.
    let fenced = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-upd", 2)),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(fenced.status, "error");
    assert_eq!(fenced.error_code, "STALE_GENERATION");
}

#[tokio::test]
async fn update_overlay_without_fabric_plan_is_rejected_in_band() {
    let executor = FabricExecutor::enabled();
    let (socket, _dir) = spawn_server(executor.clone()).await;
    let mut client = make_client(socket).await;

    // Ensure a fabric-backed topology first so the network exists.
    let ensure = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-nofab", 1)),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ensure.status, "OK");

    // UpdateOverlay without a fabric plan is rejected in-band: the legacy
    // nolearning VXLAN/FDB datapath was retired by ADR-021.
    let update = client
        .update_overlay(UpdateOverlayRequest {
            network_id: "net-nofab".to_string(),
            vni: 100,
            vtep_endpoints: vec![],
            fdb_entries: vec![],
            fabric: None,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(update.result.as_ref().unwrap().status, "error");
    assert_eq!(
        update.result.as_ref().unwrap().error_code,
        "INVALID_ARGUMENT"
    );
    assert!(
        update
            .result
            .as_ref()
            .unwrap()
            .human_summary
            .contains("ADR-021"),
        "the rejection must name ADR-021, got: {}",
        update.result.as_ref().unwrap().human_summary
    );
    assert_eq!(
        executor.count("apply_fabric:"),
        1,
        "no fabric apply beyond the initial ensure"
    );
}

#[tokio::test]
async fn get_overlay_status_uses_fabric_branch() {
    let executor = FabricExecutor::enabled();
    let (socket, _dir) = spawn_server(executor.clone()).await;
    let mut client = make_client(socket).await;

    let ensure = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-obs", 1)),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ensure.status, "OK");

    let status = client
        .get_overlay_status(GetOverlayStatusRequest {
            network_id: "net-obs".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(status.network_id, "net-obs");
    assert_eq!(status.vni, 100);
    assert!(status.vxlan_interface_up);
    assert_eq!(status.fdb_entry_count, 1);
    assert!(executor
        .calls()
        .iter()
        .any(|c| c == "overlay_status:net-obs"));
}

#[tokio::test]
async fn get_fabric_identity_returns_public_key_only() {
    let executor = FabricExecutor::enabled();
    let (socket, _dir) = spawn_server(executor.clone()).await;
    let mut client = make_client(socket).await;

    let identity = client
        .get_fabric_identity(GetFabricIdentityRequest {})
        .await
        .unwrap()
        .into_inner();
    assert_eq!(identity.result.as_ref().unwrap().status, "OK");
    assert_eq!(identity.public_key, TEST_PUBLIC_KEY);
    assert_eq!(identity.underlay_mtu, 1500);
}

#[tokio::test]
async fn fabric_rpcs_fail_closed_when_provider_disabled() {
    let executor = FabricExecutor::disabled();
    let (socket, _dir) = spawn_server(executor.clone()).await;
    let mut client = make_client(socket).await;

    // Ensure with a VXLAN overlay + fabric plan must fail closed when the
    // provider is disabled; no topology state may be persisted.
    let ensure = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-off", 1)),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ensure.status, "error");
    assert_eq!(ensure.error_code, "INVALID_ARGUMENT");

    // GetFabricIdentity fails closed with an in-band error.
    let identity = client
        .get_fabric_identity(GetFabricIdentityRequest {})
        .await
        .unwrap()
        .into_inner();
    assert_eq!(identity.result.as_ref().unwrap().status, "error");
    assert_eq!(
        identity.result.as_ref().unwrap().error_code,
        "INVALID_ARGUMENT"
    );
    assert!(
        identity.public_key.is_empty(),
        "no identity may be invented when the provider is disabled"
    );

    // VXLAN overlay without any fabric plan is rejected as invalid.
    let mut spec = fabric_topology_spec("net-off", 1);
    spec.fabric = None;
    let no_plan = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(spec),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(no_plan.status, "error");
    assert_eq!(no_plan.error_code, "INVALID_ARGUMENT");
}

#[tokio::test]
async fn delete_topology_tears_down_fabric_first() {
    let executor = FabricExecutor::enabled();
    let (socket, _dir) = spawn_server(executor.clone()).await;
    let mut client = make_client(socket).await;

    let ensure = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-del", 1)),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ensure.status, "OK");

    let del = client
        .delete_network_topology(DeleteNetworkTopologyRequest {
            meta: None,
            network_id: "net-del".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(del.status, "OK");

    // Fabric teardown must run, and must precede local topology teardown.
    let calls = executor.calls();
    let remove_idx = calls
        .iter()
        .position(|c| c == "remove_fabric:net-del")
        .expect("fabric overlay must be removed");
    let delete_idx = calls
        .iter()
        .position(|c| c == "delete:net-del")
        .expect("local topology must be torn down");
    assert!(remove_idx < delete_idx, "fabric teardown runs first");

    // Idempotent delete: no second teardown, still OK.
    let del2 = client
        .delete_network_topology(DeleteNetworkTopologyRequest {
            meta: None,
            network_id: "net-del".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(del2.status, "OK");
    assert_eq!(executor.count("remove_fabric:"), 1);
}

// ---- M1: VNI / binding_generation in the ensure short-circuit -------------

#[tokio::test]
async fn ensure_replay_with_different_vni_does_not_short_circuit() {
    let executor = FabricExecutor::enabled();
    let (socket, _dir) = spawn_server(executor.clone()).await;
    let mut client = make_client(socket).await;

    let first = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-vni", 5)),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(first.status, "OK");
    assert_eq!(executor.count("apply_fabric:"), 1);

    // A VNI re-bind changes only the VNI (binding_generation may bump while
    // desired_generation — the fabric plan generation — stays put). The
    // replay must NOT short-circuit, or the datapath would keep the OLD
    // VNI and two networks could cross-bleed silently.
    let mut spec = fabric_topology_spec("net-vni", 5);
    spec.vni = 200;
    let second = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(spec),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(second.status, "OK");
    assert!(
        executor
            .calls()
            .iter()
            .any(|c| c == "apply_fabric:net-vni:200:5:br-fab"),
        "the new VNI must be applied, got {:?}",
        executor.calls()
    );
    assert_eq!(
        executor.count("apply_fabric:"),
        2,
        "a different VNI must not be short-circuited"
    );
}

#[tokio::test]
async fn update_overlay_rejects_lower_binding_generation() {
    let executor = FabricExecutor::enabled();
    let (socket, _dir) = spawn_server(executor.clone()).await;
    let mut client = make_client(socket).await;

    // Ensure with plan generation 2, binding generation 7.
    let mut spec = fabric_topology_spec("net-bg", 2);
    spec.fabric = fabric_plan_with_binding(2, 7, 0);
    let ensure = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(spec),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ensure.status, "OK");
    assert_eq!(executor.count("apply_fabric:"), 1);

    // A newer plan generation with a LOWER binding generation is a stale
    // binding: rejected in-band, nothing applied.
    let stale_binding = client
        .update_overlay(UpdateOverlayRequest {
            network_id: "net-bg".to_string(),
            vni: 100,
            vtep_endpoints: vec![],
            fdb_entries: vec![],
            fabric: fabric_plan_with_binding(9, 3, 0),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(stale_binding.result.as_ref().unwrap().status, "error");
    assert_eq!(
        stale_binding.result.as_ref().unwrap().error_code,
        "STALE_GENERATION"
    );
    assert_eq!(
        executor.count("apply_fabric:"),
        1,
        "a stale binding generation must not be applied"
    );

    // Equal binding generation replays fine (the update path always
    // re-applies; the provider layer dedups).
    let equal = client
        .update_overlay(UpdateOverlayRequest {
            network_id: "net-bg".to_string(),
            vni: 100,
            vtep_endpoints: vec![],
            fdb_entries: vec![],
            fabric: fabric_plan_with_binding(9, 7, 0),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(equal.result.as_ref().unwrap().status, "OK");

    // A higher binding generation (a VNI re-bind) proceeds.
    let higher = client
        .update_overlay(UpdateOverlayRequest {
            network_id: "net-bg".to_string(),
            vni: 100,
            vtep_endpoints: vec![],
            fdb_entries: vec![],
            fabric: fabric_plan_with_binding(10, 9, 0),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(higher.result.as_ref().unwrap().status, "OK");
    assert!(executor
        .calls()
        .iter()
        .any(|c| c == "apply_fabric:net-bg:100:10:br-fab"));
}

// ---- m7: fabric → bridge-only re-ensure ------------------------------------

#[tokio::test]
async fn fabric_to_bridge_only_reensure_removes_fabric_overlay() {
    let executor = FabricExecutor::enabled();
    let (socket, _dir) = spawn_server(executor.clone()).await;
    let mut client = make_client(socket).await;

    let ensure = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-m7", 1)),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ensure.status, "OK");
    assert_eq!(executor.count("apply_fabric:"), 1);

    // Re-ensure the same network as bridge-only (no fabric plan, vni 0).
    let mut spec = fabric_topology_spec("net-m7", 1);
    spec.vni = 0;
    spec.overlay_type = OverlayType::OverlayNone as i32;
    spec.fabric = None;
    let reensure = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(spec),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(reensure.status, "OK");
    assert!(
        executor.calls().iter().any(|c| c == "remove_fabric:net-m7"),
        "the applied fabric object must be removed on bridge-only re-ensure"
    );

    // The fabric fields are cleared from state: the overlay reports down.
    let status = client
        .get_overlay_status(GetOverlayStatusRequest {
            network_id: "net-m7".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(status.vni, 0);
    assert!(!status.vxlan_interface_up);

    // A second bridge-only ensure is a plain idempotent replay: no further
    // fabric removal.
    let mut spec = fabric_topology_spec("net-m7", 1);
    spec.vni = 0;
    spec.overlay_type = OverlayType::OverlayNone as i32;
    spec.fabric = None;
    let again = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(spec),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(again.status, "OK");
    assert_eq!(executor.count("remove_fabric:"), 1);
}

// ---- m8: tenant MTU change reaches ports and dnsmasq -----------------------

#[tokio::test]
async fn tenant_mtu_change_reasserts_mtu_on_running_topology() {
    let executor = FabricExecutor::enabled();
    let (socket, _dir) = spawn_server(executor.clone()).await;
    let mut client = make_client(socket).await;

    // First apply carries an explicit MTU of 1400. No prior state, so no
    // re-assert (the initial apply already sets bridge/veth/dnsmasq MTU).
    let mut spec = fabric_topology_spec("net-mtu-chg", 1);
    spec.fabric = fabric_plan(1, 1400);
    let first = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(spec),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(first.status, "OK");
    assert_eq!(executor.count("reassert_mtu:"), 0);

    // New generation, same MTU: applied, but no MTU re-assert.
    let mut spec = fabric_topology_spec("net-mtu-chg", 2);
    spec.fabric = fabric_plan(2, 1400);
    let same = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(spec),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(same.status, "OK");
    assert_eq!(
        executor.count("reassert_mtu:"),
        0,
        "an unchanged MTU must not re-assert"
    );

    // Same generation but a DIFFERENT explicit MTU: the replay must not be
    // short-circuited, and the MTU re-assert must fire with the new value
    // (bridge + enslaved ports + dnsmasq restart).
    let mut spec = fabric_topology_spec("net-mtu-chg", 2);
    spec.fabric = fabric_plan(2, 1420);
    let changed = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(spec),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(changed.status, "OK");
    assert!(
        executor
            .calls()
            .iter()
            .any(|c| c == "reassert_mtu:net-mtu-chg:1420"),
        "a changed tenant MTU must be re-asserted, got {:?}",
        executor.calls()
    );
}

// ---- M3: delete after a restart cleans orphaned fabric state ---------------

use chv_nwd_api::chv_nwd_api::network_service_server::NetworkService as _;

fn direct_service(
    executor: FabricExecutor,
) -> (NetworkServiceImpl<FabricExecutor>, Arc<TopologyTable>) {
    let table = Arc::new(TopologyTable::new());
    let service =
        NetworkServiceImpl::new(Arc::new(executor), table.clone(), Arc::new(Metrics::new()));
    (service, table)
}

#[tokio::test]
async fn delete_after_restart_cleans_orphaned_fabric_state() {
    let executor = FabricExecutor::enabled();
    let (service, table) = direct_service(executor.clone());

    let ensure = service
        .ensure_network_topology(TonicRequest::new(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-restart", 1)),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ensure.status, "OK");

    // Simulate an nwd restart: the in-memory topology table is wiped, but
    // the provider's durable ownership journal still holds the network.
    table.remove("net-restart");
    assert!(table.get("net-restart").is_none());
    assert!(
        executor.fabric_owned("net-restart").await.unwrap(),
        "the ownership journal survives the restart"
    );

    let del = service
        .delete_network_topology(TonicRequest::new(DeleteNetworkTopologyRequest {
            meta: None,
            network_id: "net-restart".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(del.status, "OK");
    assert!(
        executor
            .calls()
            .iter()
            .any(|c| c == "remove_fabric:net-restart"),
        "the orphaned fabric state must be torn down, got {:?}",
        executor.calls()
    );

    // The teardown is idempotent: a second delete finds no ownership.
    let del2 = service
        .delete_network_topology(TonicRequest::new(DeleteNetworkTopologyRequest {
            meta: None,
            network_id: "net-restart".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(del2.status, "OK");
    assert_eq!(executor.count("remove_fabric:"), 1);
}

#[tokio::test]
async fn delete_after_restart_without_fabric_ownership_is_a_plain_noop() {
    let executor = FabricExecutor::enabled();
    let (service, table) = direct_service(executor.clone());

    service
        .ensure_network_topology(TonicRequest::new(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-nojournal", 1)),
        }))
        .await
        .unwrap();

    // Restart wipes the table AND the provider holds no entry (e.g. the
    // fabric network was already removed out-of-band).
    table.remove("net-nojournal");
    executor.owned.lock().unwrap().clear();

    let del = service
        .delete_network_topology(TonicRequest::new(DeleteNetworkTopologyRequest {
            meta: None,
            network_id: "net-nojournal".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(del.status, "OK");
    assert_eq!(
        executor.count("remove_fabric:"),
        0,
        "nothing is torn down when the provider holds no entry"
    );
}

#[tokio::test]
async fn delete_after_restart_reports_fabric_teardown_failure_loudly_but_succeeds() {
    // A fabric teardown failure with no local topology must NOT turn the
    // delete into an error (nothing local failed); the failure is loudly
    // visible via the warn log and the remove-failure metric.
    let executor = FabricExecutor::with_failing_removal();
    let counters = fabric_metrics_recorder::install();
    let (service, table) = direct_service(executor.clone());

    // Seed ownership directly (the mock's removal fails, so apply-then-wipe
    // is replaced by inserting into the ownership stand-in).
    executor
        .owned
        .lock()
        .unwrap()
        .insert("net-faildel".to_string());
    assert!(table.get("net-faildel").is_none());

    let before = counters
        .lock()
        .unwrap()
        .get("nwd_fabric_remove_total{result=failure}")
        .copied()
        .unwrap_or(0);
    let del = service
        .delete_network_topology(TonicRequest::new(DeleteNetworkTopologyRequest {
            meta: None,
            network_id: "net-faildel".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        del.status, "OK",
        "a fabric teardown failure must not fail a delete with no local topology"
    );
    assert!(
        executor
            .calls()
            .iter()
            .any(|c| c == "remove_fabric:net-faildel"),
        "the removal must have been attempted"
    );
    let after = counters
        .lock()
        .unwrap()
        .get("nwd_fabric_remove_total{result=failure}")
        .copied()
        .unwrap_or(0);
    assert!(
        after > before,
        "the failed removal must be counted in nwd_fabric_remove_total{{result=failure}}"
    );
}

// ---- m9: get_overlay_status error propagation ------------------------------

#[tokio::test]
async fn get_overlay_status_propagates_fabric_errors() {
    let executor = FabricExecutor::with_overlay_status_error();
    let (socket, _dir) = spawn_server(executor.clone()).await;
    let mut client = make_client(socket).await;

    let ensure = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-obs-err", 1)),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ensure.status, "OK");

    // A provider error must surface, not be masked as a "down" status.
    let status = client
        .get_overlay_status(GetOverlayStatusRequest {
            network_id: "net-obs-err".to_string(),
        })
        .await;
    assert!(
        status.is_err(),
        "a fabric provider error must not be swallowed into a down status"
    );
    let err = status.unwrap_err();
    assert_eq!(
        err.code(),
        tonic::Code::Internal,
        "the structured ChvError must map to an internal status, got: {}",
        err.message()
    );
}

#[tokio::test]
async fn get_overlay_status_reports_truthful_down_when_not_owned() {
    // State row exists (fabric fields set) but the provider holds no
    // ownership entry: truthful "down", not an error.
    let executor = FabricExecutor::enabled();
    let (service, table) = direct_service(executor.clone());
    table.upsert(TopologyState {
        network_id: "net-ghost".to_string(),
        tenant_id: "t1".to_string(),
        bridge_name: "br-fab".to_string(),
        namespace_name: "ns-fab".to_string(),
        subnet_cidr: "10.0.50.0/24".to_string(),
        gateway_ip: "10.0.50.1".to_string(),
        runtime_status: "ensured".to_string(),
        vni: Some(100),
        tenant_mtu: Some(1380),
        fabric_plan_generation: Some(1),
        binding_generation: Some(1),
    });

    let status = service
        .get_overlay_status(TonicRequest::new(GetOverlayStatusRequest {
            network_id: "net-ghost".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(status.vni, 100);
    assert!(!status.vxlan_interface_up, "not owned must report down");
    assert_eq!(status.fdb_entry_count, 0);
}

#[tokio::test]
async fn get_overlay_status_reports_down_when_provider_disabled() {
    // The fabric provider was disabled in configuration after the overlay
    // was applied: the overlay state is unobservable, reported as down —
    // not as an error.
    let executor = FabricExecutor::disabled();
    let (service, table) = direct_service(executor.clone());
    table.upsert(TopologyState {
        network_id: "net-off-obs".to_string(),
        tenant_id: "t1".to_string(),
        bridge_name: "br-fab".to_string(),
        namespace_name: "ns-fab".to_string(),
        subnet_cidr: "10.0.50.0/24".to_string(),
        gateway_ip: "10.0.50.1".to_string(),
        runtime_status: "ensured".to_string(),
        vni: Some(100),
        tenant_mtu: Some(1380),
        fabric_plan_generation: Some(1),
        binding_generation: Some(1),
    });

    let status = service
        .get_overlay_status(TonicRequest::new(GetOverlayStatusRequest {
            network_id: "net-off-obs".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(status.vni, 100);
    assert!(
        !status.vxlan_interface_up,
        "a disabled provider must report down, not an error"
    );
}

// ---- n11: fabric datapath metrics -------------------------------------------

mod fabric_metrics_recorder {
    use metrics::{
        Counter, CounterFn, Gauge, Histogram, Key, KeyName, Metadata, Recorder, SharedString, Unit,
    };
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex as StdMutex, OnceLock};

    struct CapturingCounter {
        map: Arc<StdMutex<HashMap<String, u64>>>,
        key: String,
    }

    impl CounterFn for CapturingCounter {
        fn increment(&self, value: u64) {
            *self
                .map
                .lock()
                .unwrap()
                .entry(self.key.clone())
                .or_insert(0) += value;
        }

        fn absolute(&self, value: u64) {
            self.map.lock().unwrap().insert(self.key.clone(), value);
        }
    }

    struct CapturingRecorder {
        counters: Arc<StdMutex<HashMap<String, u64>>>,
    }

    impl Recorder for CapturingRecorder {
        fn describe_counter(&self, _key: KeyName, _unit: Option<Unit>, _desc: SharedString) {}
        fn describe_gauge(&self, _key: KeyName, _unit: Option<Unit>, _desc: SharedString) {}
        fn describe_histogram(&self, _key: KeyName, _unit: Option<Unit>, _desc: SharedString) {}

        fn register_counter(&self, key: &Key, _metadata: &Metadata<'_>) -> Counter {
            let labels = key
                .labels()
                .map(|l| format!("{}={}", l.key(), l.value()))
                .collect::<Vec<_>>()
                .join(",");
            let entry = format!("{}{{{}}}", key.name(), labels);
            Counter::from_arc(Arc::new(CapturingCounter {
                map: self.counters.clone(),
                key: entry,
            }))
        }

        fn register_gauge(&self, _key: &Key, _metadata: &Metadata<'_>) -> Gauge {
            Gauge::noop()
        }

        fn register_histogram(&self, _key: &Key, _metadata: &Metadata<'_>) -> Histogram {
            Histogram::noop()
        }
    }

    static COUNTERS: OnceLock<Arc<StdMutex<HashMap<String, u64>>>> = OnceLock::new();

    /// Install the capturing recorder once per test process and return the
    /// shared counter map. Other tests in this binary may also increment
    /// counters once the recorder is live, so assertions use >= bounds.
    pub fn install() -> Arc<StdMutex<HashMap<String, u64>>> {
        COUNTERS
            .get_or_init(|| {
                let counters: Arc<StdMutex<HashMap<String, u64>>> =
                    Arc::new(StdMutex::new(HashMap::new()));
                // A failure here means another recorder is already
                // installed in this process; the returned map is still the
                // one that recorder captures into (or stays empty, which
                // fails the assertions loudly rather than silently).
                let _ = metrics::set_global_recorder(CapturingRecorder {
                    counters: counters.clone(),
                });
                counters
            })
            .clone()
    }
}

#[tokio::test]
async fn fabric_apply_and_remove_are_counted_in_metrics() {
    let counters = fabric_metrics_recorder::install();
    let executor = FabricExecutor::enabled();
    let (socket, _dir) = spawn_server(executor.clone()).await;
    let mut client = make_client(socket).await;

    let ensure = client
        .ensure_network_topology(EnsureNetworkTopologyRequest {
            meta: None,
            topology: Some(fabric_topology_spec("net-metrics", 1)),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ensure.status, "OK");

    let del = client
        .delete_network_topology(DeleteNetworkTopologyRequest {
            meta: None,
            network_id: "net-metrics".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(del.status, "OK");

    let snapshot = counters.lock().unwrap().clone();
    let apply_success = snapshot
        .get("nwd_fabric_apply_total{result=success}")
        .copied()
        .unwrap_or(0);
    assert!(
        apply_success >= 1,
        "nwd_fabric_apply_total{{result=success}} must be counted, got {snapshot:?}"
    );
    let remove_success = snapshot
        .get("nwd_fabric_remove_total{result=success}")
        .copied()
        .unwrap_or(0);
    assert!(
        remove_success >= 1,
        "nwd_fabric_remove_total{{result=success}} must be counted, got {snapshot:?}"
    );
}
