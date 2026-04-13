use crate::cache::NodeCache;
use crate::daemon_clients::{NwdClient, StordClient};
use crate::state_machine::{NodeState, StateMachine};
use crate::vm_runtime::VmRuntime;
use chv_agent_runtime_ch::adapter::{VmConfig, VmDiskConfig, VmNicConfig};
use chv_errors::ChvError;
use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

pub struct Reconciler {
    pub cache: Arc<tokio::sync::Mutex<NodeCache>>,
    pub state_machine: StateMachine,
    pub vm_runtime: VmRuntime,
    pub stord_socket: PathBuf,
    pub nwd_socket: PathBuf,
}

impl Reconciler {
    pub async fn new(
        cache: Arc<tokio::sync::Mutex<NodeCache>>,
        vm_runtime: VmRuntime,
        stord_socket: PathBuf,
        nwd_socket: PathBuf,
    ) -> Self {
        let initial = {
            let c = cache.lock().await;
            c.node_state.parse().unwrap_or(NodeState::Bootstrapping)
        };
        Self {
            cache,
            state_machine: StateMachine::new(initial),
            vm_runtime,
            stord_socket,
            nwd_socket,
        }
    }

    pub async fn run_once(&mut self) -> Result<(), ChvError> {
        info!(
            state = %self.state_machine.current().as_str(),
            "reconcile tick"
        );

        // Only act when tenant-ready
        if !matches!(self.state_machine.current(), NodeState::TenantReady) {
            return Ok(());
        }

        self.reconcile_networks().await?;
        self.reconcile_volumes().await?;
        self.reconcile_vms().await?;
        Ok(())
    }

    async fn reconcile_networks(&mut self) -> Result<(), ChvError> {
        let cache = self.cache.lock().await;
        let mut desired_networks: BTreeSet<String> =
            cache.vm_network_ids().into_iter().collect();
        desired_networks.extend(cache.network_fragments.keys().cloned());

        let mut nwd = NwdClient::connect(&self.nwd_socket).await?;
        for net_id in &desired_networks {
            let bridge = format!("br-{}", net_id);
            let cidr = "10.0.0.0/24"; // TODO: derive from fragment when available
            if let Err(e) = nwd.ensure_network_topology(net_id, &bridge, cidr, None).await {
                warn!(network_id = %net_id, error = %e, "failed to ensure network topology");
            }
        }

        let actual = nwd.list_namespace_state().await?;
        for state in actual.items {
            if !desired_networks.contains(&state.network_id) {
                if let Err(e) = nwd.delete_network_topology(&state.network_id, None).await {
                    warn!(network_id = %state.network_id, error = %e, "failed to delete orphan network topology");
                }
            }
        }
        Ok(())
    }

    async fn reconcile_volumes(&mut self) -> Result<(), ChvError> {
        let cache = self.cache.lock().await;
        let pairs: HashSet<(String, String)> = cache.vm_volume_handles().into_iter().collect();
        if pairs.is_empty() {
            return Ok(());
        }
        let mut stord = StordClient::connect(&self.stord_socket).await?;
        for (vm_id, volume_id) in pairs {
            // Open volume if not already open (best-effort)
            let locator = format!("{}.img", volume_id);
            match stord.open_volume(&volume_id, "local", &locator, None).await {
                Ok((_, handle, _)) => {
                    if let Err(e) = stord.attach_volume_to_vm(&volume_id, &vm_id, &handle, None).await {
                        warn!(volume_id = %volume_id, vm_id = %vm_id, error = %e, "failed to attach volume");
                    }
                }
                Err(e) => {
                    warn!(volume_id = %volume_id, error = %e, "failed to open volume during reconcile");
                }
            }
        }
        Ok(())
    }

    async fn cleanup_vm(
        &mut self,
        stord: &mut StordClient,
        nwd: &mut NwdClient,
        vm_id: &str,
    ) -> Result<(), ChvError> {
        let mut cache = self.cache.lock().await;
        // Detach volumes
        for (cached_vm_id, volume_id) in cache.vm_volume_handles() {
            if cached_vm_id == vm_id {
                if let Err(e) = stord.detach_volume_from_vm(&volume_id, vm_id, false, None).await {
                    warn!(vm_id = %vm_id, volume_id = %volume_id, error = %e, "failed to detach volume");
                }
                if let Some(handle) = cache.volume_handles.remove(&volume_id) {
                    if let Err(e) = stord.close_volume(&volume_id, &handle, None).await {
                        warn!(vm_id = %vm_id, volume_id = %volume_id, error = %e, "failed to close volume");
                    }
                }
            }
        }

        // Detach NICs
        if let Some(fragment) = cache.vm_fragments.get(vm_id) {
            if let Ok(spec) = crate::spec::VmSpec::from_json(std::str::from_utf8(&fragment.spec_json).unwrap_or("")) {
                for nic in &spec.nics {
                    let nic_id = format!("{}-{}", vm_id, nic.network_id);
                    if let Err(e) = nwd.detach_vm_nic(&nic_id, vm_id, &nic.network_id, None).await {
                        warn!(vm_id = %vm_id, nic_id = %nic_id, error = %e, "failed to detach nic");
                    }
                }
            }
        }
        Ok(())
    }

    async fn prepare_vm(
        &mut self,
        stord: &mut StordClient,
        nwd: &mut NwdClient,
        vm_id: &str,
        vm_spec: &crate::spec::VmSpec,
    ) -> Result<VmConfig, ChvError> {
        let mut disks = Vec::new();
        for disk in &vm_spec.disks {
            let (_volume_id, handle, export_path) = stord
                .open_volume(&disk.volume_id, "local", &format!("{}.img", disk.volume_id), None)
                .await?;
            stord
                .attach_volume_to_vm(&disk.volume_id, vm_id, &handle, None)
                .await?;
            disks.push(VmDiskConfig {
                path: PathBuf::from(export_path),
                read_only: disk.read_only,
            });
        }

        let mut nics = Vec::new();
        for nic in &vm_spec.nics {
            let nic_id = format!("{}-{}", vm_id, nic.network_id);
            if let Err(e) = nwd
                .ensure_network_topology(&nic.network_id, &format!("br-{}", nic.network_id), "10.0.0.0/24", None)
                .await
            {
                warn!(network_id = %nic.network_id, error = %e, "failed to ensure network topology");
            }
            let (_namespace_handle, tap_handle) = nwd
                .attach_vm_nic(&nic_id, vm_id, &nic.network_id, &nic.mac_address, &nic.ip_address, None)
                .await?;
            nics.push(VmNicConfig {
                network_id: nic.network_id.clone(),
                mac_address: nic.mac_address.clone(),
                ip_address: nic.ip_address.clone(),
                tap_name: tap_handle,
            });
        }

        Ok(VmConfig {
            vm_id: vm_id.to_string(),
            cpus: vm_spec.cpus,
            memory_bytes: vm_spec.memory_bytes,
            kernel_path: PathBuf::from(&vm_spec.kernel_path),
            disks,
            nics,
            api_socket_path: PathBuf::from(format!("/run/chv/agent/vm-{}.sock", vm_id)),
        })
    }

    async fn reconcile_vms(&mut self) -> Result<(), ChvError> {
        let (desired, actual) = {
            let cache = self.cache.lock().await;
            let desired: BTreeSet<String> = cache.vm_fragments.keys().cloned().collect();
            let actual: BTreeSet<String> = self.vm_runtime.list().into_iter().map(|r| r.vm_id).collect();
            (desired, actual)
        };

        let mut stord = match StordClient::connect(&self.stord_socket).await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "failed to connect to stord, skipping vm reconcile");
                return Ok(());
            }
        };
        let mut nwd = match NwdClient::connect(&self.nwd_socket).await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "failed to connect to nwd, skipping vm reconcile");
                return Ok(());
            }
        };

        // Create missing VMs
        for vm_id in desired.difference(&actual) {
            let (generation, raw) = {
                let cache = self.cache.lock().await;
                let Some(fragment) = cache.vm_fragments.get(vm_id) else {
                    warn!(vm_id = %vm_id, "vm fragment missing during reconcile");
                    continue;
                };
                (fragment.generation.clone(), fragment.spec_json.clone())
            };
            let raw = match std::str::from_utf8(&raw) {
                Ok(r) => r,
                Err(e) => {
                    warn!(vm_id = %vm_id, error = %e, "failed to decode vm_fragment spec_json as utf-8");
                    continue;
                }
            };
            let spec = match crate::spec::VmSpec::from_json(raw) {
                Ok(s) => s,
                Err(e) => {
                    warn!(vm_id = %vm_id, error = %e, "failed to parse vm_fragment spec_json");
                    continue;
                }
            };
            let config = match self.prepare_vm(&mut stord, &mut nwd, vm_id, &spec).await {
                Ok(c) => c,
                Err(e) => {
                    warn!(vm_id = %vm_id, error = %e, "failed to prepare vm");
                    continue;
                }
            };
            if let Err(e) = self.vm_runtime.create_vm(vm_id, &generation, &config, None).await {
                warn!(vm_id = %vm_id, error = %e, "failed to create vm");
                continue;
            }
            if spec.desired_state == "Running" {
                if let Err(e) = self.vm_runtime.start_vm(vm_id, None).await {
                    warn!(vm_id = %vm_id, error = %e, "failed to start vm");
                    continue;
                }
            }
        }

        // Delete extra VMs
        for vm_id in actual.difference(&desired) {
            if let Err(e) = self.cleanup_vm(&mut stord, &mut nwd, vm_id).await {
                warn!(vm_id = %vm_id, error = %e, "cleanup vm failed");
            }
            if let Err(e) = self.vm_runtime.stop_vm(vm_id, false, None).await {
                warn!(vm_id = %vm_id, error = %e, "failed to stop vm before delete");
            }
            if let Err(e) = self.vm_runtime.delete_vm(vm_id, None).await {
                warn!(vm_id = %vm_id, error = %e, "failed to delete vm");
                continue;
            }
        }

        // Reconcile existing VMs
        for vm_id in desired.intersection(&actual) {
            let raw = {
                let cache = self.cache.lock().await;
                let Some(fragment) = cache.vm_fragments.get(vm_id) else {
                    warn!(vm_id = %vm_id, "vm fragment missing during reconcile");
                    continue;
                };
                fragment.spec_json.clone()
            };
            let raw = match std::str::from_utf8(&raw) {
                Ok(r) => r,
                Err(e) => {
                    warn!(vm_id = %vm_id, error = %e, "failed to decode vm_fragment spec_json as utf-8");
                    continue;
                }
            };
            let spec = match crate::spec::VmSpec::from_json(raw) {
                Ok(s) => s,
                Err(e) => {
                    warn!(vm_id = %vm_id, error = %e, "failed to parse vm_fragment spec_json");
                    continue;
                }
            };
            let Some(record) = self.vm_runtime.get(vm_id) else {
                warn!(vm_id = %vm_id, "vm runtime record missing during reconcile");
                continue;
            };
            if spec.desired_state == "Running" && record.runtime_status != "Running" {
                if let Err(e) = self.vm_runtime.start_vm(vm_id, None).await {
                    warn!(vm_id = %vm_id, error = %e, "failed to start vm");
                    continue;
                }
            } else if spec.desired_state == "Stopped" && record.runtime_status == "Running" {
                if let Err(e) = self.vm_runtime.stop_vm(vm_id, false, None).await {
                    warn!(vm_id = %vm_id, error = %e, "failed to stop vm");
                    continue;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;

    fn test_cache() -> NodeCache {
        use crate::cache::DesiredStateFragment;
        NodeCache {
            node_state: "TenantReady".to_string(),
            vm_fragments: {
                let mut m = HashMap::new();
                m.insert("vm-1".to_string(), DesiredStateFragment {
                    id: "vm-1".to_string(),
                    kind: "vm".to_string(),
                    generation: "1".to_string(),
                    spec_json: br#"{"name":"vm-1","cpus":1,"memory_bytes":1024,"kernel_path":"/dev/null","disks":[{"volume_id":"vol-1"}],"nics":[{"network_id":"net-1","mac_address":"00:00:00:00:00:01","ip_address":"10.0.0.2"}]}"#.to_vec(),
                    policy_json: vec![],
                    updated_at: "2024-01-01T00:00:00Z".to_string(),
                    updated_by: "cp".to_string(),
                });
                m
            },
            ..Default::default()
        }
    }

    fn empty_cache() -> NodeCache {
        NodeCache {
            node_state: "TenantReady".to_string(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn reconciler_skips_when_not_tenant_ready() {
        let mut cache = test_cache();
        cache.node_state = "Bootstrapping".to_string();
        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(cache)),
            VmRuntime::new(std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default())),
            PathBuf::from("/tmp/fake-stord.sock"),
            PathBuf::from("/tmp/fake-nwd.sock"),
        ).await;
        assert!(rec.run_once().await.is_ok());
    }

    // ------------------------------------------------------------------
    // Mock gRPC servers for stord/nwd (used by reconciler_creates_missing_vm)
    // ------------------------------------------------------------------
    use chv_nwd_api::chv_nwd_api::network_service_server::NetworkService;
    use chv_stord_api::chv_stord_api::storage_service_server::StorageService;
    use tonic::{Request, Response, Status};

    struct MockStordOk;
    #[tonic::async_trait]
    impl StorageService for MockStordOk {
        async fn list_volume_sessions(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::ListVolumeSessionsRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::ListVolumeSessionsResponse>, Status> {
            Ok(Response::new(
                chv_stord_api::chv_stord_api::ListVolumeSessionsResponse { sessions: vec![] },
            ))
        }
        async fn open_volume(
            &self,
            req: Request<chv_stord_api::chv_stord_api::OpenVolumeRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::OpenVolumeResponse>, Status> {
            let inner = req.into_inner();
            Ok(Response::new(
                chv_stord_api::chv_stord_api::OpenVolumeResponse {
                    result: Some(chv_stord_api::chv_stord_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                    volume_id: inner.volume_id.clone(),
                    attachment_handle: format!("handle-{}", inner.volume_id),
                    export_kind: "local".to_string(),
                    export_path: format!("/tmp/{}.img", inner.volume_id),
                },
            ))
        }
        async fn close_volume(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::CloseVolumeRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Ok(Response::new(chv_stord_api::chv_stord_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn get_volume_health(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::VolumeHealthRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::VolumeHealthResponse>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn attach_volume_to_vm(
            &self,
            req: Request<chv_stord_api::chv_stord_api::AttachVolumeToVmRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::AttachVolumeToVmResponse>, Status> {
            let inner = req.into_inner();
            Ok(Response::new(
                chv_stord_api::chv_stord_api::AttachVolumeToVmResponse {
                    result: Some(chv_stord_api::chv_stord_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                    volume_id: inner.volume_id.clone(),
                    vm_id: inner.vm_id.clone(),
                    export_kind: "local".to_string(),
                    export_path: format!("/tmp/{}.img", inner.volume_id),
                },
            ))
        }
        async fn detach_volume_from_vm(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::DetachVolumeFromVmRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Ok(Response::new(chv_stord_api::chv_stord_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn resize_volume(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::ResizeVolumeRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn prepare_snapshot(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::PrepareSnapshotRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn prepare_clone(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::PrepareCloneRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn set_device_policy(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::SetDevicePolicyRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
    }

    struct MockNwdOk;
    #[tonic::async_trait]
    impl NetworkService for MockNwdOk {
        async fn list_namespace_state(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::ListNamespaceStateRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::ListNamespaceStateResponse>, Status> {
            Ok(Response::new(
                chv_nwd_api::chv_nwd_api::ListNamespaceStateResponse { items: vec![] },
            ))
        }
        async fn ensure_network_topology(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::EnsureNetworkTopologyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Ok(Response::new(chv_nwd_api::chv_nwd_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn delete_network_topology(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::DeleteNetworkTopologyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Ok(Response::new(chv_nwd_api::chv_nwd_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn get_network_health(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::NetworkHealthRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::NetworkHealthResponse>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn attach_vm_nic(
            &self,
            req: Request<chv_nwd_api::chv_nwd_api::AttachVmNicRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::AttachVmNicResponse>, Status> {
            let inner = req.into_inner();
            let nic = inner.nic.unwrap();
            Ok(Response::new(
                chv_nwd_api::chv_nwd_api::AttachVmNicResponse {
                    result: Some(chv_nwd_api::chv_nwd_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                    namespace_handle: format!("ns-{}", nic.network_id),
                    tap_handle: format!("tap-{}", nic.network_id),
                },
            ))
        }
        async fn detach_vm_nic(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::DetachVmNicRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Ok(Response::new(chv_nwd_api::chv_nwd_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn set_firewall_policy(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::SetFirewallPolicyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn set_nat_policy(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::SetNatPolicyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn ensure_dhcp_scope(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::EnsureDhcpScopeRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn ensure_dns_scope(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::EnsureDnsScopeRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn expose_service(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::ExposeServiceRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
        async fn withdraw_service_exposure(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::WithdrawServiceExposureRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }
    }

    async fn start_mock_stord(socket: &std::path::Path) {
        let uds = tokio::net::UnixListener::bind(socket).unwrap();
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(
                    chv_stord_api::chv_stord_api::storage_service_server::StorageServiceServer::new(
                        MockStordOk,
                    ),
                )
                .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(uds))
                .await
                .ok();
        });
        for _ in 0..10 {
            if StordClient::connect(socket).await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn start_mock_nwd(socket: &std::path::Path) {
        let uds = tokio::net::UnixListener::bind(socket).unwrap();
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(
                    chv_nwd_api::chv_nwd_api::network_service_server::NetworkServiceServer::new(
                        MockNwdOk,
                    ),
                )
                .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(uds))
                .await
                .ok();
        });
        for _ in 0..10 {
            if NwdClient::connect(socket).await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn reconciler_creates_missing_vm() {
        let dir = tempfile::tempdir().unwrap();
        let stord_socket = dir.path().join("stord.sock");
        let nwd_socket = dir.path().join("nwd.sock");
        start_mock_stord(&stord_socket).await;
        start_mock_nwd(&nwd_socket).await;

        let mock = std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(test_cache())),
            VmRuntime::new(mock.clone()),
            stord_socket,
            nwd_socket,
        ).await;
        rec.reconcile_vms().await.unwrap();

        let vms = mock.vms.lock().unwrap();
        assert!(vms.contains_key("vm-1"));
        let config = vms.get("vm-1").unwrap();
        assert_eq!(config.cpus, 1);
        assert_eq!(config.memory_bytes, 1024);
    }

    #[tokio::test]
    async fn reconciler_deletes_orphan_vm() {
        let dir = tempfile::tempdir().unwrap();
        let stord_socket = dir.path().join("stord.sock");
        let nwd_socket = dir.path().join("nwd.sock");
        start_mock_stord(&stord_socket).await;
        start_mock_nwd(&nwd_socket).await;

        let mock = std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let runtime = VmRuntime::new(mock.clone());
        let config = VmConfig {
            vm_id: "vm-orphan".to_string(),
            cpus: 1,
            memory_bytes: 512,
            kernel_path: PathBuf::from("/dev/null"),
            disks: vec![],
            nics: vec![],
            api_socket_path: PathBuf::from("/run/chv/vm-orphan.sock"),
        };
        runtime.create_vm("vm-orphan", "1", &config, None).await.unwrap();

        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(empty_cache())),
            runtime,
            stord_socket,
            nwd_socket,
        ).await;
        rec.reconcile_vms().await.unwrap();

        assert!(mock.vms.lock().unwrap().get("vm-orphan").is_none());
    }

    #[tokio::test]
    async fn reconciler_starts_stopped_vm() {
        let dir = tempfile::tempdir().unwrap();
        let stord_socket = dir.path().join("stord.sock");
        let nwd_socket = dir.path().join("nwd.sock");
        start_mock_stord(&stord_socket).await;
        start_mock_nwd(&nwd_socket).await;

        let mock = std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let runtime = VmRuntime::new(mock.clone());
        let config = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 1,
            memory_bytes: 1024,
            kernel_path: PathBuf::from("/dev/null"),
            disks: vec![],
            nics: vec![],
            api_socket_path: PathBuf::from("/run/chv/vm-1.sock"),
        };
        runtime.create_vm("vm-1", "1", &config, None).await.unwrap();
        runtime.stop_vm("vm-1", false, None).await.unwrap();
        assert_eq!(runtime.get("vm-1").unwrap().runtime_status, "Stopped");

        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(test_cache())),
            runtime,
            stord_socket,
            nwd_socket,
        ).await;
        rec.reconcile_vms().await.unwrap();

        assert_eq!(rec.vm_runtime.get("vm-1").unwrap().runtime_status, "Running");
    }
}
