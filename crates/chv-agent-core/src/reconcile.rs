use crate::cache::{NodeCache, VmNicAttachment};
use crate::daemon_clients::{NwdClient, StordClient};
use crate::state_machine::NodeState;
use crate::vm_runtime::VmRuntime;
use chv_agent_runtime_ch::adapter::{VmConfig, VmDiskConfig, VmNicConfig};
use chv_errors::ChvError;
use std::collections::{BTreeSet, HashSet};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

pub struct Reconciler {
    pub cache: Arc<tokio::sync::Mutex<NodeCache>>,
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
        Self {
            cache,
            vm_runtime,
            stord_socket,
            nwd_socket,
        }
    }

    pub async fn current_state(&self) -> NodeState {
        self.cache.lock().await.current_node_state()
    }

    pub async fn transition_state(&self, to: NodeState) -> Result<NodeState, ChvError> {
        let mut cache = self.cache.lock().await;
        cache.transition_node_state(to)
    }

    pub async fn run_once(&mut self) -> Result<(), ChvError> {
        info!(
            state = %self.current_state().await.as_str(),
            "reconcile tick"
        );

        match self.current_state().await {
            NodeState::Discovered => {
                self.transition_state(NodeState::Bootstrapping).await?;
            }
            NodeState::Bootstrapping => {
                // Probe stord: if reachable, advance to HostReady
                match StordClient::connect(&self.stord_socket).await {
                    Ok(_) => {
                        self.transition_state(NodeState::HostReady).await?;
                    }
                    Err(e) => {
                        warn!(error = %e, "stord not reachable, staying in Bootstrapping");
                    }
                }
            }
            NodeState::HostReady => {
                // Verify stord can serve volume sessions, then advance to StorageReady
                match StordClient::connect(&self.stord_socket).await {
                    Ok(mut stord) => match stord.health_probe().await {
                        Ok(_) => {
                            self.transition_state(NodeState::StorageReady).await?;
                        }
                        Err(e) => {
                            warn!(error = %e, "stord health_probe failed, staying in HostReady");
                        }
                    },
                    Err(e) => {
                        warn!(error = %e, "stord not reachable, staying in HostReady");
                    }
                }
            }
            NodeState::StorageReady => {
                // Verify nwd can respond, then advance to NetworkReady
                match NwdClient::connect(&self.nwd_socket).await {
                    Ok(mut nwd) => match nwd.list_namespace_state().await {
                        Ok(_) => {
                            self.transition_state(NodeState::NetworkReady).await?;
                        }
                        Err(e) => {
                            warn!(error = %e, "nwd list_namespace_state failed, staying in StorageReady");
                        }
                    },
                    Err(e) => {
                        warn!(error = %e, "nwd not reachable, staying in StorageReady");
                    }
                }
            }
            NodeState::NetworkReady => {
                self.transition_state(NodeState::TenantReady).await?;
            }
            NodeState::TenantReady => {
                self.reconcile_networks().await?;
                self.reconcile_volumes().await?;
                self.reconcile_vms().await?;
            }
            NodeState::Degraded | NodeState::Draining | NodeState::Maintenance | NodeState::Failed => {}
        }

        Ok(())
    }

    async fn reconcile_networks(&mut self) -> Result<(), ChvError> {
        // Build a map of network_id -> cidr from network fragments (spec_json).
        // Falls back to the hardcoded default if the fragment has no subnet_cidr.
        const DEFAULT_CIDR: &str = "10.0.0.0/24";

        let (desired_networks, network_cidrs) = {
            let cache = self.cache.lock().await;
            let mut desired_networks: BTreeSet<String> =
                cache.vm_network_ids().into_iter().collect();
            desired_networks.extend(cache.network_fragments.keys().cloned());

            let mut network_cidrs: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for (net_id, frag) in &cache.network_fragments {
                let cidr = serde_json::from_slice::<serde_json::Value>(&frag.spec_json)
                    .ok()
                    .and_then(|v| {
                        v.get("subnet_cidr")
                            .and_then(|c| c.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| DEFAULT_CIDR.to_string());
                network_cidrs.insert(net_id.clone(), cidr);
            }

            (desired_networks, network_cidrs)
        };
        // Cache lock is dropped here — all subsequent operations are lock-free async I/O.

        let mut nwd = NwdClient::connect(&self.nwd_socket).await?;
        for net_id in &desired_networks {
            let bridge = format!("br-{}", net_id);
            let cidr = network_cidrs
                .get(net_id)
                .map(|s| s.as_str())
                .unwrap_or(DEFAULT_CIDR);
            let op_id = format!("reconcile-network-ensure-{}", net_id);
            if let Err(e) = nwd
                .ensure_network_topology(net_id, &bridge, cidr, Some(&op_id))
                .await
            {
                warn!(network_id = %net_id, error = %e, "failed to ensure network topology");
            }
        }

        let actual = nwd.list_namespace_state().await?;
        for state in actual.items {
            if !desired_networks.contains(&state.network_id) {
                let op_id = format!("reconcile-network-delete-{}", state.network_id);
                if let Err(e) = nwd
                    .delete_network_topology(&state.network_id, Some(&op_id))
                    .await
                {
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
            let op_id = format!("reconcile-volume-attach-{}-{}", vm_id, volume_id);
            match stord
                .open_volume(&volume_id, "local", &locator, Some(&op_id))
                .await
            {
                Ok((_, handle, _)) => {
                    if let Err(e) = stord
                        .attach_volume_to_vm(&volume_id, &vm_id, &handle, Some(&op_id))
                        .await
                    {
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

    async fn cleanup_vm(&mut self, vm_id: &str) -> Result<(), ChvError> {
        let op_id = format!("reconcile-vm-cleanup-{}", vm_id);
        cleanup_vm_resources(
            &self.cache,
            &self.stord_socket,
            &self.nwd_socket,
            vm_id,
            Some(&op_id),
        )
        .await
    }

    async fn prepare_vm(
        &mut self,
        stord: &mut StordClient,
        nwd: &mut NwdClient,
        vm_id: &str,
        vm_spec: &crate::spec::VmSpec,
        operation_id: &str,
    ) -> Result<VmConfig, ChvError> {
        let mut disks = Vec::new();
        let mut volume_ids = Vec::new();
        for disk in &vm_spec.disks {
            let open_op_id = format!("{}-open-volume-{}", operation_id, disk.volume_id);
            let (_volume_id, handle, export_path) = stord
                .open_volume(
                    &disk.volume_id,
                    "local",
                    &format!("{}.img", disk.volume_id),
                    Some(&open_op_id),
                )
                .await?;
            stord
                .attach_volume_to_vm(&disk.volume_id, vm_id, &handle, Some(&open_op_id))
                .await?;
            disks.push(VmDiskConfig {
                path: PathBuf::from(export_path),
                read_only: disk.read_only,
            });
            volume_ids.push(disk.volume_id.clone());
            let mut cache = self.cache.lock().await;
            cache.volume_handles.insert(disk.volume_id.clone(), handle);
        }

        const DEFAULT_NIC_CIDR: &str = "10.0.0.0/24";
        let mut nics = Vec::new();
        let mut nic_attachments = Vec::new();
        for nic in &vm_spec.nics {
            let nic_id = format!("{}-{}", vm_id, nic.network_id);
            let nic_op_id = format!("{}-attach-nic-{}", operation_id, nic_id);
            // Read subnet_cidr from the network fragment if available; fall back to default.
            let nic_cidr = {
                let cache = self.cache.lock().await;
                cache
                    .network_fragments
                    .get(&nic.network_id)
                    .and_then(|frag| {
                        serde_json::from_slice::<serde_json::Value>(&frag.spec_json)
                            .ok()
                            .and_then(|v| {
                                v.get("subnet_cidr")
                                    .and_then(|c| c.as_str())
                                    .map(|s| s.to_string())
                            })
                    })
                    .unwrap_or_else(|| DEFAULT_NIC_CIDR.to_string())
            };
            if let Err(e) = nwd
                .ensure_network_topology(
                    &nic.network_id,
                    &format!("br-{}", nic.network_id),
                    &nic_cidr,
                    Some(&nic_op_id),
                )
                .await
            {
                warn!(network_id = %nic.network_id, error = %e, "failed to ensure network topology");
            }
            let (_namespace_handle, tap_handle) = nwd
                .attach_vm_nic(
                    &nic_id,
                    vm_id,
                    &nic.network_id,
                    &nic.mac_address,
                    &nic.ip_address,
                    Some(&nic_op_id),
                )
                .await?;
            nics.push(VmNicConfig {
                network_id: nic.network_id.clone(),
                mac_address: nic.mac_address.clone(),
                ip_address: nic.ip_address.clone(),
                tap_name: tap_handle,
            });
            nic_attachments.push(VmNicAttachment {
                nic_id,
                network_id: nic.network_id.clone(),
            });
        }

        if !volume_ids.is_empty() || !nic_attachments.is_empty() {
            let mut cache = self.cache.lock().await;
            cache.observe_vm_attachment(vm_id, &volume_ids, &nic_attachments);
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
            let actual: BTreeSet<String> = self
                .vm_runtime
                .list()
                .into_iter()
                .map(|r| r.vm_id)
                .collect();
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
            let op_id = format!("reconcile-vm-create-{}", vm_id);
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
            let config = match self
                .prepare_vm(&mut stord, &mut nwd, vm_id, &spec, &op_id)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    warn!(vm_id = %vm_id, error = %e, "failed to prepare vm");
                    continue;
                }
            };
            if let Err(e) = self
                .vm_runtime
                .create_vm(vm_id, &generation, &config, Some(&op_id))
                .await
            {
                warn!(vm_id = %vm_id, error = %e, "failed to create vm");
                continue;
            }
            if spec.desired_state == "Running" {
                let start_op_id = format!("{}-start", op_id);
                if let Err(e) = self.vm_runtime.start_vm(vm_id, Some(&start_op_id)).await {
                    warn!(vm_id = %vm_id, error = %e, "failed to start vm");
                    continue;
                }
            }
        }

        // Delete extra VMs
        for vm_id in actual.difference(&desired) {
            let op_id = format!("reconcile-vm-delete-{}", vm_id);
            if let Err(e) = self.cleanup_vm(vm_id).await {
                warn!(vm_id = %vm_id, error = %e, "cleanup vm failed");
            }
            if let Err(e) = self.vm_runtime.stop_vm(vm_id, false, Some(&op_id)).await {
                warn!(vm_id = %vm_id, error = %e, "failed to stop vm before delete");
            }
            if let Err(e) = self.vm_runtime.delete_vm(vm_id, Some(&op_id)).await {
                warn!(vm_id = %vm_id, error = %e, "failed to delete vm");
                continue;
            }
            let mut cache = self.cache.lock().await;
            cache.remove_vm_state(vm_id);
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
                let op_id = format!("reconcile-vm-start-{}", vm_id);
                if let Err(e) = self.vm_runtime.start_vm(vm_id, Some(&op_id)).await {
                    warn!(vm_id = %vm_id, error = %e, "failed to start vm");
                    continue;
                }
            } else if spec.desired_state == "Stopped" && record.runtime_status == "Running" {
                let op_id = format!("reconcile-vm-stop-{}", vm_id);
                if let Err(e) = self.vm_runtime.stop_vm(vm_id, false, Some(&op_id)).await {
                    warn!(vm_id = %vm_id, error = %e, "failed to stop vm");
                    continue;
                }
            }
        }

        Ok(())
    }
}

pub(crate) async fn cleanup_vm_resources(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    stord_socket: &Path,
    nwd_socket: &Path,
    vm_id: &str,
    operation_id: Option<&str>,
) -> Result<(), ChvError> {
    let (volumes, nics) = {
        let cache = cache.lock().await;
        let derived_attachments = cache
            .vm_fragments
            .get(vm_id)
            .and_then(|fragment| std::str::from_utf8(&fragment.spec_json).ok())
            .and_then(|raw| crate::spec::VmSpec::from_json(raw).ok())
            .map(|spec| {
                let volume_ids = spec
                    .disks
                    .into_iter()
                    .map(|disk| disk.volume_id)
                    .collect::<Vec<_>>();
                let nics = spec
                    .nics
                    .into_iter()
                    .map(|nic| VmNicAttachment {
                        nic_id: format!("{}-{}", vm_id, nic.network_id),
                        network_id: nic.network_id,
                    })
                    .collect::<Vec<_>>();
                (volume_ids, nics)
            })
            .unwrap_or_default();

        let volume_ids = cache
            .vm_attachment_state(vm_id)
            .map(|state| state.volume_ids.clone())
            .unwrap_or(derived_attachments.0);
        let nics = cache
            .vm_attachment_state(vm_id)
            .map(|state| {
                state
                    .nics
                    .iter()
                    .map(|nic| (nic.nic_id.clone(), nic.network_id.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                derived_attachments
                    .1
                    .into_iter()
                    .map(|nic| (nic.nic_id, nic.network_id))
                    .collect::<Vec<_>>()
            });

        let volumes = volume_ids
            .into_iter()
            .map(|volume_id| {
                let handle = cache.volume_handles.get(&volume_id).cloned();
                (volume_id, handle)
            })
            .collect::<Vec<_>>();

        (volumes, nics)
    };

    if !volumes.is_empty() {
        let mut stord = StordClient::connect(stord_socket).await?;
        for (volume_id, handle) in &volumes {
            stord
                .detach_volume_from_vm(volume_id, vm_id, false, operation_id)
                .await?;
            if let Some(handle) = handle {
                stord.close_volume(volume_id, handle, operation_id).await?;
            }
        }
    }

    if !nics.is_empty() {
        let mut nwd = NwdClient::connect(nwd_socket).await?;
        for (nic_id, network_id) in &nics {
            nwd.detach_vm_nic(nic_id, vm_id, network_id, operation_id)
                .await?;
        }
    }

    let mut cache = cache.lock().await;
    for (volume_id, _) in volumes {
        cache.volume_handles.remove(&volume_id);
    }
    cache.vm_attachments.remove(vm_id);
    Ok(())
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
            VmRuntime::new(std::sync::Arc::new(
                chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default(),
            )),
            PathBuf::from("/tmp/fake-stord.sock"),
            PathBuf::from("/tmp/fake-nwd.sock"),
        )
        .await;
        assert!(rec.run_once().await.is_ok());
    }

    #[tokio::test]
    async fn reconciler_advances_from_discovered_to_bootstrapping() {
        let cache = NodeCache {
            node_state: "Discovered".to_string(),
            ..Default::default()
        };
        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(cache)),
            VmRuntime::new(std::sync::Arc::new(
                chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default(),
            )),
            PathBuf::from("/tmp/fake-stord-discovered.sock"),
            PathBuf::from("/tmp/fake-nwd-discovered.sock"),
        )
        .await;
        assert!(rec.run_once().await.is_ok());
        assert_eq!(rec.current_state().await, NodeState::Bootstrapping);
    }

    #[tokio::test]
    async fn reconciler_uses_latest_cached_node_state() {
        let cache = Arc::new(tokio::sync::Mutex::new(test_cache()));
        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let mut rec = Reconciler::new(
            cache.clone(),
            VmRuntime::new(mock.clone()),
            PathBuf::from("/tmp/fake-stord.sock"),
            PathBuf::from("/tmp/fake-nwd.sock"),
        )
        .await;

        {
            let mut cache = cache.lock().await;
            cache.transition_node_state(NodeState::Draining).unwrap();
        }

        assert!(rec.run_once().await.is_ok());
        assert!(mock.vms.lock().unwrap().is_empty());
    }

    // ------------------------------------------------------------------
    // Mock gRPC servers for stord/nwd (used by reconciler_creates_missing_vm)
    // ------------------------------------------------------------------
    use chv_nwd_api::chv_nwd_api::network_service_server::NetworkService;
    use chv_stord_api::chv_stord_api::storage_service_server::StorageService;
    use tonic::{Request, Response, Status};

    #[allow(clippy::result_large_err)]
    fn stord_operation_id(
        meta: Option<chv_stord_api::chv_stord_api::Meta>,
    ) -> Result<String, Status> {
        let operation_id = meta.map(|m| m.operation_id).unwrap_or_default();
        if operation_id.is_empty() {
            Err(Status::invalid_argument("missing operation_id"))
        } else {
            Ok(operation_id)
        }
    }

    #[allow(clippy::result_large_err)]
    fn nwd_operation_id(meta: Option<chv_nwd_api::chv_nwd_api::Meta>) -> Result<String, Status> {
        let operation_id = meta.map(|m| m.operation_id).unwrap_or_default();
        if operation_id.is_empty() {
            Err(Status::invalid_argument("missing operation_id"))
        } else {
            Ok(operation_id)
        }
    }

    struct MockStordOk;
    #[tonic::async_trait]
    impl StorageService for MockStordOk {
        async fn list_volume_sessions(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::ListVolumeSessionsRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::ListVolumeSessionsResponse>, Status>
        {
            Ok(Response::new(
                chv_stord_api::chv_stord_api::ListVolumeSessionsResponse { sessions: vec![] },
            ))
        }
        async fn open_volume(
            &self,
            req: Request<chv_stord_api::chv_stord_api::OpenVolumeRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::OpenVolumeResponse>, Status> {
            let inner = req.into_inner();
            stord_operation_id(inner.meta.clone())?;
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
            req: Request<chv_stord_api::chv_stord_api::CloseVolumeRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            stord_operation_id(req.into_inner().meta)?;
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
        ) -> Result<Response<chv_stord_api::chv_stord_api::AttachVolumeToVmResponse>, Status>
        {
            let inner = req.into_inner();
            stord_operation_id(inner.meta.clone())?;
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
            req: Request<chv_stord_api::chv_stord_api::DetachVolumeFromVmRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            stord_operation_id(req.into_inner().meta)?;
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
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::ListNamespaceStateResponse>, Status>
        {
            Ok(Response::new(
                chv_nwd_api::chv_nwd_api::ListNamespaceStateResponse { items: vec![] },
            ))
        }
        async fn ensure_network_topology(
            &self,
            req: Request<chv_nwd_api::chv_nwd_api::EnsureNetworkTopologyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            nwd_operation_id(req.into_inner().meta)?;
            Ok(Response::new(chv_nwd_api::chv_nwd_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn delete_network_topology(
            &self,
            req: Request<chv_nwd_api::chv_nwd_api::DeleteNetworkTopologyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            nwd_operation_id(req.into_inner().meta)?;
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
            nwd_operation_id(inner.meta.clone())?;
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
            req: Request<chv_nwd_api::chv_nwd_api::DetachVmNicRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            nwd_operation_id(req.into_inner().meta)?;
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

        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(test_cache())),
            VmRuntime::new(mock.clone()),
            stord_socket,
            nwd_socket,
        )
        .await;
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

        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
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
        runtime
            .create_vm("vm-orphan", "1", &config, None)
            .await
            .unwrap();

        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(empty_cache())),
            runtime,
            stord_socket,
            nwd_socket,
        )
        .await;
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

        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
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
        )
        .await;
        rec.reconcile_vms().await.unwrap();

        assert_eq!(
            rec.vm_runtime.get("vm-1").unwrap().runtime_status,
            "Running"
        );
    }
}
