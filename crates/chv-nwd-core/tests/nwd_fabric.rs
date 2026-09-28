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
use chv_nwd_core::{NetworkServer, TopologyState};
use chv_observability::Metrics;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::net::UnixStream;
use tonic::transport::{Endpoint, Uri};
use tower::service_fn;

const TEST_PUBLIC_KEY: &str = "K7XbF9cV2mQpT3nZ8sL4dW6yH1jR5uA0eG9iO2pS7kM=";
const PEER_PUBLIC_KEY: &str = "Q8YcG0dW3nRqU4oA9tM5eX7zI2kS6vB1fH0jP3qT8lN=";
const DEFAULT_TENANT_MTU: u32 = 1380;

/// Recording mock mirroring the `LinuxExecutor` fabric contract.
#[derive(Clone)]
struct FabricExecutor {
    calls: Arc<StdMutex<Vec<String>>>,
    fabric_enabled: bool,
}

impl FabricExecutor {
    fn enabled() -> Self {
        Self {
            calls: Arc::new(StdMutex::new(Vec::new())),
            fabric_enabled: true,
        }
    }

    fn disabled() -> Self {
        Self {
            calls: Arc::new(StdMutex::new(Vec::new())),
            fabric_enabled: false,
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
        }
        Ok(TopologyApplyResult {
            namespace_handle: spec.namespace_name.clone(),
            bridge_handle: spec.bridge_name.clone(),
            tenant_mtu,
            fabric_plan_generation,
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
        Ok(AppliedFabric {
            report: ApplyReport::default(),
            plan_generation: plan.plan_generation,
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
        Ok(())
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
        Ok(OverlayStatusInfo {
            vxlan_interface_up: true,
            fdb_entry_count: 1,
        })
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
    Some(FabricPlan {
        fabric_domain_id: "fabric-1".to_string(),
        local_host_id: "host-01".to_string(),
        local_fabric_ip: "100.100.0.1".to_string(),
        tenant_mtu,
        fabric_mtu: 0,
        binding_generation: 1,
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
