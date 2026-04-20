use crate::cache::{NodeCache, VmNicAttachment};
use crate::control_plane::ControlPlaneClient;
use crate::reconcile::{cleanup_vm_resources, vm_runtime_dir};
use crate::state_machine::NodeState;
use crate::vm_runtime::VmRuntime;
use chv_agent_runtime_ch::adapter::VmConfig;
use chv_errors::ChvError;
use control_plane_node_api::control_plane_node_api as proto;
use std::path::Path;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::{Request, Response, Status};
use tracing::warn;

#[derive(Clone)]
pub struct AgentServer {
    pub cache: Arc<tokio::sync::Mutex<NodeCache>>,
    pub vm_runtime: VmRuntime,
    pub stord_socket: std::path::PathBuf,
    pub nwd_socket: std::path::PathBuf,
    pub cache_path: Option<std::path::PathBuf>,
    pub runtime_dir: std::path::PathBuf,
}

impl AgentServer {
    pub fn new(
        cache: Arc<tokio::sync::Mutex<NodeCache>>,
        vm_runtime: VmRuntime,
        stord_socket: std::path::PathBuf,
        nwd_socket: std::path::PathBuf,
        cache_path: Option<std::path::PathBuf>,
        runtime_dir: std::path::PathBuf,
    ) -> Self {
        Self {
            cache,
            vm_runtime,
            stord_socket,
            nwd_socket,
            cache_path,
            runtime_dir,
        }
    }

    async fn persist_cache(&self, cache: &NodeCache) {
        if let Some(ref path) = self.cache_path {
            if let Err(e) = cache.save(path).await {
                tracing::warn!(error = %e, "failed to persist cache");
            }
        }
    }

    async fn open_and_attach_volume(
        &self,
        volume_id: &str,
        vm_id: &str,
        spec_json: &[u8],
        operation_id: &str,
    ) -> Result<String, Status> {
        let spec = serde_json::from_slice::<serde_json::Value>(spec_json)
            .map_err(|e| Status::invalid_argument(format!("invalid volume spec_json: {}", e)))?;
        let backend_class = spec
            .get("backend_class")
            .and_then(|v| v.as_str())
            .unwrap_or("local");
        let locator = spec
            .get("locator")
            .and_then(|v| v.as_str())
            .unwrap_or(volume_id);

        let mut stord = crate::daemon_clients::StordClient::connect(&self.stord_socket)
            .await
            .map_err(|e| Status::unavailable(format!("stord unavailable: {}", e)))?;
        let (_, handle, _) = stord
            .open_volume(volume_id, backend_class, locator, Some(operation_id))
            .await
            .map_err(|e| Status::internal(format!("open_volume failed: {}", e)))?;
        stord
            .attach_volume_to_vm(volume_id, vm_id, &handle, Some(operation_id))
            .await
            .map_err(|e| Status::internal(format!("attach_volume_to_vm failed: {}", e)))?;
        Ok(handle)
    }

    pub async fn serve(self, socket_path: &Path) -> Result<(), ChvError> {
        if let Some(parent) = socket_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ChvError::Io {
                    path: parent.to_string_lossy().to_string(),
                    source: e,
                })?;
        }
        if socket_path.exists() {
            tokio::fs::remove_file(socket_path)
                .await
                .map_err(|e| ChvError::Io {
                    path: socket_path.to_string_lossy().to_string(),
                    source: e,
                })?;
        }
        let uds = UnixListener::bind(socket_path).map_err(|e| ChvError::Io {
            path: socket_path.to_string_lossy().to_string(),
            source: e,
        })?;
        let uds_stream = UnixListenerStream::new(uds);
        tonic::transport::Server::builder()
            .add_service(proto::reconcile_service_server::ReconcileServiceServer::new(self.clone()))
            .add_service(proto::lifecycle_service_server::LifecycleServiceServer::new(self))
            .serve_with_incoming(uds_stream)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("agent server error: {e}"),
            })
    }
}

#[tonic::async_trait]
impl proto::reconcile_service_server::ReconcileService for AgentServer {
    async fn apply_node_desired_state(
        &self,
        req: Request<proto::ApplyNodeDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "node", &inner.node_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        if let Some(frag) = inner.fragment {
            cache.observe_generation("node", &inner.node_id, &frag.generation);
            self.persist_cache(&cache).await;
        }
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "node desired state accepted".to_string(),
            }),
        }))
    }

    async fn apply_vm_desired_state(
        &self,
        req: Request<proto::ApplyVmDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        if let Some(frag) = inner.fragment {
            cache.observe_generation("vm", &inner.vm_id, &frag.generation);
            cache.store_fragment(
                "vm",
                &inner.vm_id,
                crate::cache::DesiredStateFragment {
                    id: frag.id,
                    kind: frag.kind,
                    generation: frag.generation,
                    spec_json: frag.spec_json,
                    policy_json: frag.policy_json,
                    updated_at: frag.updated_at,
                    updated_by: frag.updated_by,
                },
            );
            self.persist_cache(&cache).await;
        }
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "vm desired state accepted".to_string(),
            }),
        }))
    }

    async fn apply_volume_desired_state(
        &self,
        req: Request<proto::ApplyVolumeDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;

        let spec_json = {
            let mut cache = self.cache.lock().await;
            ControlPlaneClient::stale_generation_check(meta, &cache, "volume", &inner.volume_id)
                .map_err(|e| Status::failed_precondition(e.to_string()))?;
            let mut spec_json = None;
            if let Some(frag) = inner.fragment {
                cache.observe_generation("volume", &inner.volume_id, &frag.generation);
                spec_json = Some(frag.spec_json.clone());
                cache.store_fragment(
                    "volume",
                    &inner.volume_id,
                    crate::cache::DesiredStateFragment {
                        id: frag.id,
                        kind: frag.kind,
                        generation: frag.generation,
                        spec_json: frag.spec_json,
                        policy_json: frag.policy_json,
                        updated_at: frag.updated_at,
                        updated_by: frag.updated_by,
                    },
                );
                self.persist_cache(&cache).await;
            }
            spec_json
            // lock dropped here
        };

        if let Some(spec_json) = spec_json {
            let spec = serde_json::from_slice::<serde_json::Value>(&spec_json)
                .map_err(|e| Status::invalid_argument(format!("invalid spec_json: {}", e)))?;
            if let Some(vm_id) = spec
                .get("vm_id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                let handle = self
                    .open_and_attach_volume(&inner.volume_id, vm_id, &spec_json, &meta.operation_id)
                    .await?;

                let mut cache = self.cache.lock().await;
                cache.volume_handles.insert(inner.volume_id.clone(), handle);
                cache.observe_vm_attachment(vm_id, std::slice::from_ref(&inner.volume_id), &[]);
                self.persist_cache(&cache).await;
            }
        }

        let observed_generation = self.cache.lock().await.observed_generation.clone();
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "volume desired state accepted".to_string(),
            }),
        }))
    }

    async fn apply_network_desired_state(
        &self,
        req: Request<proto::ApplyNetworkDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "network", &inner.network_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        if let Some(frag) = inner.fragment {
            cache.observe_generation("network", &inner.network_id, &frag.generation);
            cache.store_fragment(
                "network",
                &inner.network_id,
                crate::cache::DesiredStateFragment {
                    id: frag.id,
                    kind: frag.kind,
                    generation: frag.generation,
                    spec_json: frag.spec_json.clone(),
                    policy_json: frag.policy_json,
                    updated_at: frag.updated_at,
                    updated_by: frag.updated_by,
                },
            );
            self.persist_cache(&cache).await;

            let spec =
                serde_json::from_slice::<serde_json::Value>(&frag.spec_json).unwrap_or_default();
            let bridge = spec
                .get("bridge_name")
                .and_then(|v| v.as_str())
                .unwrap_or("br0");
            let cidr = spec
                .get("cidr")
                .and_then(|v| v.as_str())
                .unwrap_or("10.0.0.0/24");
            let gateway = spec
                .get("gateway")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mut nwd = crate::daemon_clients::NwdClient::connect(&self.nwd_socket)
                .await
                .map_err(|e| Status::unavailable(format!("nwd unavailable: {}", e)))?;

            nwd.ensure_network_topology(&inner.network_id, bridge, cidr, gateway, Some(&meta.operation_id))
                .await
                .map_err(|e| Status::internal(format!("ensure_network_topology failed: {}", e)))?;

            if let Some(exposures) = spec.get("exposures").and_then(|v| v.as_array()) {
                for exp in exposures {
                    let eid = exp
                        .get("exposure_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let proto_str = exp
                        .get("protocol")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tcp");
                    let ext_port = exp
                        .get("external_port")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let tip = exp.get("target_ip").and_then(|v| v.as_str()).unwrap_or("");
                    let tport = exp.get("target_port").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let mode = exp.get("mode").and_then(|v| v.as_str()).unwrap_or("nat");
                    if !eid.is_empty() {
                        let _ = nwd
                            .expose_service(
                                &inner.network_id,
                                eid,
                                proto_str,
                                ext_port,
                                tip,
                                tport,
                                mode,
                                Some(&meta.operation_id),
                            )
                            .await;
                    }
                }
            }
        }
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "network desired state accepted".to_string(),
            }),
        }))
    }

    async fn acknowledge_desired_state_version(
        &self,
        req: Request<proto::AcknowledgeDesiredStateVersionRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        let observed = cache
            .get_generation(&inner.fragment_kind, &inner.fragment_id)
            .cloned()
            .unwrap_or_default();
        if observed != inner.observed_generation {
            return Err(Status::failed_precondition(format!(
                "generation mismatch: observed {}, got {}",
                observed, inner.observed_generation
            )));
        }
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "desired state version acknowledged".to_string(),
            }),
        }))
    }
}

#[tonic::async_trait]
impl proto::lifecycle_service_server::LifecycleService for AgentServer {
    async fn create_vm(
        &self,
        req: Request<proto::CreateVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let vm = inner
            .vm
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing vm"))?;
        {
            let cache = self.cache.lock().await;
            ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &vm.vm_id)
                .map_err(|e| Status::failed_precondition(e.to_string()))?;
            let node_state = cache
                .node_state
                .parse::<crate::state_machine::NodeState>()
                .unwrap_or(crate::state_machine::NodeState::Bootstrapping);
            if node_state != crate::state_machine::NodeState::TenantReady {
                return Err(Status::failed_precondition(format!(
                    "node not schedulable: {}",
                    cache.node_state
                )));
            }
        }
        let vm_spec =
            crate::spec::VmSpec::from_json(std::str::from_utf8(&vm.vm_spec_json).unwrap_or(""))
                .map_err(|e| Status::invalid_argument(format!("invalid vm_spec_json: {}", e)))?;
        vm_spec
            .validate()
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let op_id = meta.operation_id.as_str();
        let mut disks = Vec::new();
        if !vm_spec.disks.is_empty() {
            let mut stord = crate::daemon_clients::StordClient::connect(&self.stord_socket)
                .await
                .map_err(|e| Status::unavailable(format!("stord unavailable: {}", e)))?;
            for disk in &vm_spec.disks {
                let (_, handle, export_path) = stord
                    .open_volume(
                        &disk.volume_id,
                        "local",
                        &format!("{}.img", disk.volume_id),
                        Some(op_id),
                    )
                    .await
                    .map_err(|e| Status::internal(format!("open_volume failed: {}", e)))?;
                stord
                    .attach_volume_to_vm(&disk.volume_id, &vm.vm_id, &handle, Some(op_id))
                    .await
                    .map_err(|e| Status::internal(format!("attach_volume_to_vm failed: {}", e)))?;
                disks.push(chv_agent_runtime_ch::adapter::VmDiskConfig {
                    path: std::path::PathBuf::from(export_path),
                    read_only: disk.read_only,
                });
                {
                    let mut cache = self.cache.lock().await;
                    cache.volume_handles.insert(disk.volume_id.clone(), handle);
                    self.persist_cache(&cache).await;
                }
            }
        }

        let mut nics = Vec::new();
        if !vm_spec.nics.is_empty() {
            let mut nwd = crate::daemon_clients::NwdClient::connect(&self.nwd_socket)
                .await
                .map_err(|e| Status::unavailable(format!("nwd unavailable: {}", e)))?;
            for nic in &vm_spec.nics {
                let nic_cidr = if nic.cidr.is_empty() {
                    "10.0.0.0/24".to_string()
                } else {
                    nic.cidr.clone()
                };
                let nic_gateway = nic.gateway.clone();
                if let Err(e) = nwd
                    .ensure_network_topology(
                        &nic.network_id,
                        &format!("br-{}", nic.network_id),
                        &nic_cidr,
                        &nic_gateway,
                        Some(op_id),
                    )
                    .await
                {
                    warn!(network_id = %nic.network_id, error = %e, "failed to ensure network topology in create_vm");
                }
                let nic_id = format!("{}-{}", vm.vm_id, nic.network_id);
                let (_ns, tap_handle) = nwd
                    .attach_vm_nic(
                        &nic_id,
                        &vm.vm_id,
                        &nic.network_id,
                        &nic.mac_address,
                        &nic.ip_address,
                        Some(op_id),
                    )
                    .await
                    .map_err(|e| Status::internal(format!("attach_vm_nic failed: {}", e)))?;
                nics.push(chv_agent_runtime_ch::adapter::VmNicConfig {
                    network_id: nic.network_id.clone(),
                    mac_address: nic.mac_address.clone(),
                    ip_address: nic.ip_address.clone(),
                    tap_name: tap_handle,
                });
            }
        }

        {
            let mut cache = self.cache.lock().await;
            let volume_ids = vm_spec
                .disks
                .iter()
                .map(|disk| disk.volume_id.clone())
                .collect::<Vec<_>>();
            let nics = vm_spec
                .nics
                .iter()
                .map(|nic| VmNicAttachment {
                    nic_id: format!("{}-{}", vm.vm_id, nic.network_id),
                    network_id: nic.network_id.clone(),
                })
                .collect::<Vec<_>>();
            cache.observe_vm_attachment(&vm.vm_id, &volume_ids, &nics);
            self.persist_cache(&cache).await;
        }

        let vm_dir = vm_runtime_dir(&self.runtime_dir, &vm.vm_id);
        let config = VmConfig {
            vm_id: vm.vm_id.clone(),
            cpus: vm_spec.cpus,
            memory_bytes: vm_spec.memory_bytes,
            kernel_path: std::path::PathBuf::from(vm_spec.kernel_path),
            firmware_path: vm_spec.firmware_path.as_ref().map(std::path::PathBuf::from),
            disks,
            nics,
            api_socket_path: vm_dir.join("vm.sock"),
            cloud_init_userdata: vm_spec.cloud_init_userdata.clone(),
        };
        self.vm_runtime
            .create_vm(&vm.vm_id, &meta.desired_state_version, &config, Some(op_id))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let observed_generation = self.cache.lock().await.observed_generation.clone();
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "vm created".to_string(),
            }),
        }))
    }

    async fn start_vm(
        &self,
        req: Request<proto::StartVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        self.vm_runtime
            .start_vm(&inner.vm_id, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "vm started".to_string(),
            }),
        }))
    }

    async fn stop_vm(
        &self,
        req: Request<proto::StopVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        self.vm_runtime
            .stop_vm(&inner.vm_id, inner.force, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "vm stopped".to_string(),
            }),
        }))
    }

    async fn reboot_vm(
        &self,
        req: Request<proto::RebootVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        self.vm_runtime
            .reboot_vm(&inner.vm_id, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "vm rebooted".to_string(),
            }),
        }))
    }

    async fn delete_vm(
        &self,
        req: Request<proto::DeleteVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let observed_generation = {
            let cache = self.cache.lock().await;
            ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
                .map_err(|e| Status::failed_precondition(e.to_string()))?;
            cache.observed_generation.clone()
        };
        cleanup_vm_resources(
            &self.cache,
            &self.stord_socket,
            &self.nwd_socket,
            &inner.vm_id,
            Some(&meta.operation_id),
        )
        .await
        .map_err(|e| Status::internal(format!("vm cleanup failed: {}", e)))?;
        self.vm_runtime
            .delete_vm(&inner.vm_id, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;
        {
            let mut cache = self.cache.lock().await;
            cache.remove_vm_state(&inner.vm_id);
            self.persist_cache(&cache).await;
        }
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "vm deleted".to_string(),
            }),
        }))
    }

    async fn resize_vm(
        &self,
        req: Request<proto::ResizeVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        drop(cache);
        self.vm_runtime
            .resize_vm(
                &inner.vm_id,
                inner.desired_vcpus,
                inner.desired_memory_bytes,
                Some(&meta.operation_id),
            )
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let observed_generation = self.cache.lock().await.observed_generation.clone();
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "vm resized".to_string(),
            }),
        }))
    }

    async fn attach_volume(
        &self,
        req: Request<proto::AttachVolumeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let vol = inner
            .volume
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing volume"))?;
        {
            let cache = self.cache.lock().await;
            ControlPlaneClient::stale_generation_check(meta, &cache, "volume", &vol.volume_id)
                .map_err(|e| Status::failed_precondition(e.to_string()))?;
            // lock dropped here
        }

        let handle = self
            .open_and_attach_volume(
                &vol.volume_id,
                &vol.vm_id,
                &vol.volume_spec_json,
                &meta.operation_id,
            )
            .await?;

        let mut cache = self.cache.lock().await;
        cache.volume_handles.insert(vol.volume_id.clone(), handle);
        cache.observe_vm_attachment(&vol.vm_id, std::slice::from_ref(&vol.volume_id), &[]);
        self.persist_cache(&cache).await;

        let observed_generation = cache.observed_generation.clone();
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "volume attached".to_string(),
            }),
        }))
    }

    async fn detach_volume(
        &self,
        req: Request<proto::DetachVolumeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "volume", &inner.volume_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;

        let mut stord = crate::daemon_clients::StordClient::connect(&self.stord_socket)
            .await
            .map_err(|e| Status::unavailable(format!("stord unavailable: {}", e)))?;

        stord
            .detach_volume_from_vm(
                &inner.volume_id,
                &inner.vm_id,
                inner.force,
                Some(&meta.operation_id),
            )
            .await
            .map_err(|e| Status::internal(format!("detach_volume_from_vm failed: {}", e)))?;

        if let Some(handle) = cache.volume_handles.remove(&inner.volume_id) {
            let _ = stord
                .close_volume(&inner.volume_id, &handle, Some(&meta.operation_id))
                .await;
        }
        self.persist_cache(&cache).await;

        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "volume detached".to_string(),
            }),
        }))
    }

    async fn resize_volume(
        &self,
        _req: Request<proto::ResizeVolumeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("resize_volume in Phase 4"))
    }

    async fn pause_node_scheduling(
        &self,
        req: Request<proto::PauseNodeSchedulingRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        if let Err(e) = cache.transition_node_state(NodeState::Degraded) {
            return Err(Status::failed_precondition(e.to_string()));
        }
        self.persist_cache(&cache).await;
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "node scheduling paused".to_string(),
            }),
        }))
    }

    async fn resume_node_scheduling(
        &self,
        req: Request<proto::ResumeNodeSchedulingRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        if let Err(e) = cache.transition_node_state(NodeState::TenantReady) {
            return Err(Status::failed_precondition(e.to_string()));
        }
        self.persist_cache(&cache).await;
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "node scheduling resumed".to_string(),
            }),
        }))
    }

    async fn drain_node(
        &self,
        req: Request<proto::DrainNodeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        if let Err(e) = cache.transition_node_state(NodeState::Draining) {
            return Err(Status::failed_precondition(e.to_string()));
        }
        self.persist_cache(&cache).await;
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "node draining".to_string(),
            }),
        }))
    }

    async fn enter_maintenance(
        &self,
        req: Request<proto::EnterMaintenanceRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        if let Err(e) = cache.transition_node_state(NodeState::Maintenance) {
            return Err(Status::failed_precondition(e.to_string()));
        }
        self.persist_cache(&cache).await;
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "node entering maintenance".to_string(),
            }),
        }))
    }

    async fn exit_maintenance(
        &self,
        req: Request<proto::ExitMaintenanceRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        if let Err(e) = cache.transition_node_state(NodeState::Bootstrapping) {
            return Err(Status::failed_precondition(e.to_string()));
        }
        self.persist_cache(&cache).await;
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "node exiting maintenance".to_string(),
            }),
        }))
    }

    async fn pause_vm(
        &self,
        req: Request<proto::PauseVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        drop(cache);
        self.vm_runtime
            .pause_vm(&inner.vm_id, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let observed_generation = self.cache.lock().await.observed_generation.clone();
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "vm paused".to_string(),
            }),
        }))
    }

    async fn resume_vm(
        &self,
        req: Request<proto::ResumeVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        drop(cache);
        self.vm_runtime
            .resume_vm(&inner.vm_id, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let observed_generation = self.cache.lock().await.observed_generation.clone();
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "vm resumed".to_string(),
            }),
        }))
    }

    async fn power_button_vm(
        &self,
        req: Request<proto::PowerButtonVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        drop(cache);
        self.vm_runtime
            .power_button(&inner.vm_id, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let observed_generation = self.cache.lock().await.observed_generation.clone();
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "vm power button pressed".to_string(),
            }),
        }))
    }

    async fn add_disk(
        &self,
        req: Request<proto::AddDiskRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        drop(cache);
        let params = chv_agent_runtime_ch::adapter::AddDiskParams {
            path: std::path::PathBuf::from(&inner.disk_path),
            read_only: inner.read_only,
            id: if inner.disk_id.is_empty() { None } else { Some(inner.disk_id.clone()) },
        };
        self.vm_runtime
            .add_disk(&inner.vm_id, &params, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let observed_generation = self.cache.lock().await.observed_generation.clone();
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "disk added".to_string(),
            }),
        }))
    }

    async fn remove_device(
        &self,
        req: Request<proto::RemoveDeviceRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        drop(cache);
        self.vm_runtime
            .remove_device(&inner.vm_id, &inner.device_id, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let observed_generation = self.cache.lock().await.observed_generation.clone();
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "device removed".to_string(),
            }),
        }))
    }

    async fn add_net(
        &self,
        req: Request<proto::AddNetRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        drop(cache);
        let params = chv_agent_runtime_ch::adapter::AddNetParams {
            tap_name: inner.tap_name.clone(),
            mac_address: inner.mac_address.clone(),
            id: if inner.net_id.is_empty() { None } else { Some(inner.net_id.clone()) },
        };
        self.vm_runtime
            .add_net(&inner.vm_id, &params, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let observed_generation = self.cache.lock().await.observed_generation.clone();
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "network interface added".to_string(),
            }),
        }))
    }

    async fn resize_disk(
        &self,
        req: Request<proto::ResizeDiskRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        drop(cache);
        self.vm_runtime
            .resize_disk(&inner.vm_id, &inner.disk_id, inner.new_size_bytes, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let observed_generation = self.cache.lock().await.observed_generation.clone();
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "disk resized".to_string(),
            }),
        }))
    }

    async fn snapshot_vm(
        &self,
        req: Request<proto::SnapshotVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        drop(cache);
        self.vm_runtime
            .snapshot_vm(&inner.vm_id, &inner.destination, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let observed_generation = self.cache.lock().await.observed_generation.clone();
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "vm snapshot taken".to_string(),
            }),
        }))
    }

    async fn restore_snapshot(
        &self,
        req: Request<proto::RestoreSnapshotRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        drop(cache);
        self.vm_runtime
            .restore_snapshot(&inner.vm_id, &inner.source, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let observed_generation = self.cache.lock().await.observed_generation.clone();
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "snapshot restored".to_string(),
            }),
        }))
    }

    async fn coredump_vm(
        &self,
        req: Request<proto::CoredumpVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner
            .meta
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        drop(cache);
        self.vm_runtime
            .coredump(&inner.vm_id, &inner.destination, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let observed_generation = self.cache.lock().await.observed_generation.clone();
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: observed_generation,
                error_code: "".to_string(),
                human_summary: "vm coredump complete".to_string(),
            }),
        }))
    }

    async fn ping_vmm(
        &self,
        req: Request<proto::PingVmmRequest>,
    ) -> Result<Response<proto::PingVmmResponse>, Status> {
        let inner = req.into_inner();
        let result = self.vm_runtime
            .ping(&inner.vm_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(proto::PingVmmResponse { alive: result }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter;
    use std::sync::Arc;

    fn test_server() -> AgentServer {
        let mut cache = NodeCache::new("node-1");
        cache.node_state = crate::state_machine::NodeState::TenantReady
            .as_str()
            .to_string();
        AgentServer::new(
            Arc::new(tokio::sync::Mutex::new(cache)),
            VmRuntime::new(Arc::new(MockCloudHypervisorAdapter::default())),
            std::path::PathBuf::from("/run/chv/stord/api.sock"),
            std::path::PathBuf::from("/run/chv/nwd/api.sock"),
            None,
            std::path::PathBuf::from("/var/lib/chv/agent"),
        )
    }

    fn test_meta(desired_state_version: &str) -> proto::RequestMeta {
        proto::RequestMeta {
            operation_id: "op-1".to_string(),
            requested_by: "cp".to_string(),
            target_node_id: "node-1".to_string(),
            desired_state_version: desired_state_version.to_string(),
            request_unix_ms: 0,
        }
    }

    #[tokio::test]
    async fn apply_vm_desired_state_updates_generation_and_fragment() {
        let server = test_server();
        let req = proto::ApplyVmDesiredStateRequest {
            meta: Some(test_meta("5")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
            fragment: Some(proto::DesiredStateFragment {
                id: "vm-1".to_string(),
                kind: "vm".to_string(),
                generation: "5".to_string(),
                spec_json: vec![],
                policy_json: vec![],
                updated_at: "".to_string(),
                updated_by: "".to_string(),
            }),
        };
        let resp = proto::reconcile_service_server::ReconcileService::apply_vm_desired_state(
            &server,
            Request::new(req),
        )
        .await;
        assert!(resp.is_ok());
        let cache = server.cache.lock().await;
        assert_eq!(cache.get_generation("vm", "vm-1"), Some(&"5".to_string()));
        assert!(cache.get_fragment("vm", "vm-1").is_some());
    }

    #[tokio::test]
    async fn create_vm_lifecycle_flow() {
        let server = test_server();
        let vm_spec_json = r#"{"name":"vm-1","cpus":2,"memory_bytes":1024,"kernel_path":"/dev/null","disks":[],"nics":[]}"#;
        let create_req = proto::CreateVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm: Some(proto::VmMutationSpec {
                vm_id: "vm-1".to_string(),
                vm_spec_json: vm_spec_json.as_bytes().to_vec(),
            }),
        };
        let resp = proto::lifecycle_service_server::LifecycleService::create_vm(
            &server,
            Request::new(create_req),
        )
        .await;
        assert!(resp.is_ok());

        let start_req = proto::StartVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
        };
        let resp = proto::lifecycle_service_server::LifecycleService::start_vm(
            &server,
            Request::new(start_req),
        )
        .await;
        assert!(resp.is_ok());
        assert_eq!(
            server.vm_runtime.get("vm-1").unwrap().runtime_status,
            "Running"
        );

        let stop_req = proto::StopVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
            force: false,
        };
        let resp = proto::lifecycle_service_server::LifecycleService::stop_vm(
            &server,
            Request::new(stop_req),
        )
        .await;
        assert!(resp.is_ok());
        assert_eq!(
            server.vm_runtime.get("vm-1").unwrap().runtime_status,
            "Stopped"
        );

        let delete_req = proto::DeleteVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
            force: false,
        };
        let resp = proto::lifecycle_service_server::LifecycleService::delete_vm(
            &server,
            Request::new(delete_req),
        )
        .await;
        assert!(resp.is_ok());
        assert!(server.vm_runtime.get("vm-1").is_none());
    }

    #[tokio::test]
    async fn lifecycle_stale_generation_rejected() {
        let server = test_server();
        let mut cache = server.cache.lock().await;
        cache.observe_generation("vm", "vm-1", "10");
        drop(cache);

        let req = proto::StartVmRequest {
            meta: Some(test_meta("9")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
        };
        let resp =
            proto::lifecycle_service_server::LifecycleService::start_vm(&server, Request::new(req))
                .await;
        assert_eq!(resp.unwrap_err().code(), tonic::Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn reboot_vm_success() {
        let server = test_server();
        let vm_spec_json = r#"{"name":"vm-1","cpus":2,"memory_bytes":1024,"kernel_path":"/dev/null","disks":[],"nics":[]}"#;
        let create_req = proto::CreateVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm: Some(proto::VmMutationSpec {
                vm_id: "vm-1".to_string(),
                vm_spec_json: vm_spec_json.as_bytes().to_vec(),
            }),
        };
        let resp = proto::lifecycle_service_server::LifecycleService::create_vm(
            &server,
            Request::new(create_req),
        )
        .await;
        assert!(resp.is_ok());

        let reboot_req = proto::RebootVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
            force: false,
        };
        let resp = proto::lifecycle_service_server::LifecycleService::reboot_vm(
            &server,
            Request::new(reboot_req),
        )
        .await;
        assert!(resp.is_ok());
        assert_eq!(
            server.vm_runtime.get("vm-1").unwrap().runtime_status,
            "Running"
        );
    }

    #[tokio::test]
    async fn acknowledge_generation_matches() {
        let server = test_server();
        let mut cache = server.cache.lock().await;
        cache.observe_generation("vm", "vm-1", "5");
        drop(cache);

        let req = proto::AcknowledgeDesiredStateVersionRequest {
            meta: Some(test_meta("5")),
            node_id: "node-1".to_string(),
            fragment_kind: "vm".to_string(),
            fragment_id: "vm-1".to_string(),
            observed_generation: "5".to_string(),
            apply_status: "ok".to_string(),
        };
        let resp =
            proto::reconcile_service_server::ReconcileService::acknowledge_desired_state_version(
                &server,
                Request::new(req),
            )
            .await;
        assert!(resp.is_ok());
    }

    #[tokio::test]
    async fn acknowledge_generation_mismatch() {
        let server = test_server();
        let mut cache = server.cache.lock().await;
        cache.observe_generation("vm", "vm-1", "5");
        drop(cache);

        let req = proto::AcknowledgeDesiredStateVersionRequest {
            meta: Some(test_meta("4")),
            node_id: "node-1".to_string(),
            fragment_kind: "vm".to_string(),
            fragment_id: "vm-1".to_string(),
            observed_generation: "4".to_string(),
            apply_status: "ok".to_string(),
        };
        let resp =
            proto::reconcile_service_server::ReconcileService::acknowledge_desired_state_version(
                &server,
                Request::new(req),
            )
            .await;
        assert_eq!(resp.unwrap_err().code(), tonic::Code::FailedPrecondition);
    }

    struct MockStord;
    #[tonic::async_trait]
    impl chv_stord_api::chv_stord_api::storage_service_server::StorageService for MockStord {
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
            Ok(Response::new(
                chv_stord_api::chv_stord_api::OpenVolumeResponse {
                    result: Some(chv_stord_api::chv_stord_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                    volume_id: req.into_inner().volume_id,
                    attachment_handle: "handle-1".to_string(),
                    export_kind: "".to_string(),
                    export_path: "".to_string(),
                },
            ))
        }

        async fn attach_volume_to_vm(
            &self,
            req: Request<chv_stord_api::chv_stord_api::AttachVolumeToVmRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::AttachVolumeToVmResponse>, Status>
        {
            let inner = req.into_inner();
            Ok(Response::new(
                chv_stord_api::chv_stord_api::AttachVolumeToVmResponse {
                    result: Some(chv_stord_api::chv_stord_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                    volume_id: inner.volume_id,
                    vm_id: inner.vm_id,
                    export_kind: "".to_string(),
                    export_path: "".to_string(),
                },
            ))
        }

        async fn close_volume(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::CloseVolumeRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }

        async fn get_volume_health(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::VolumeHealthRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::VolumeHealthResponse>, Status> {
            Err(Status::unimplemented(""))
        }

        async fn detach_volume_from_vm(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::DetachVolumeFromVmRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Err(Status::unimplemented(""))
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

    #[derive(Clone, Default)]
    struct CleanupTracker {
        detached_volumes: Arc<std::sync::Mutex<Vec<String>>>,
        closed_volumes: Arc<std::sync::Mutex<Vec<String>>>,
        detached_nics: Arc<std::sync::Mutex<Vec<String>>>,
    }

    struct MockCleanupStord {
        tracker: CleanupTracker,
    }

    #[tonic::async_trait]
    impl chv_stord_api::chv_stord_api::storage_service_server::StorageService for MockCleanupStord {
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
            _req: Request<chv_stord_api::chv_stord_api::OpenVolumeRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::OpenVolumeResponse>, Status> {
            Err(Status::unimplemented(""))
        }

        async fn attach_volume_to_vm(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::AttachVolumeToVmRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::AttachVolumeToVmResponse>, Status>
        {
            Err(Status::unimplemented(""))
        }

        async fn close_volume(
            &self,
            req: Request<chv_stord_api::chv_stord_api::CloseVolumeRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            self.tracker
                .closed_volumes
                .lock()
                .unwrap()
                .push(req.into_inner().volume_id);
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

        async fn detach_volume_from_vm(
            &self,
            req: Request<chv_stord_api::chv_stord_api::DetachVolumeFromVmRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            self.tracker
                .detached_volumes
                .lock()
                .unwrap()
                .push(req.into_inner().volume_id);
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

    struct MockCleanupNwd {
        tracker: CleanupTracker,
    }

    #[tonic::async_trait]
    impl chv_nwd_api::chv_nwd_api::network_service_server::NetworkService for MockCleanupNwd {
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
            _req: Request<chv_nwd_api::chv_nwd_api::EnsureNetworkTopologyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }

        async fn delete_network_topology(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::DeleteNetworkTopologyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Err(Status::unimplemented(""))
        }

        async fn get_network_health(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::NetworkHealthRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::NetworkHealthResponse>, Status> {
            Err(Status::unimplemented(""))
        }

        async fn attach_vm_nic(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::AttachVmNicRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::AttachVmNicResponse>, Status> {
            Err(Status::unimplemented(""))
        }

        async fn detach_vm_nic(
            &self,
            req: Request<chv_nwd_api::chv_nwd_api::DetachVmNicRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            let nic_id = req.into_inner().nic_id;
            self.tracker.detached_nics.lock().unwrap().push(nic_id);
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

    #[tokio::test]
    async fn delete_vm_cleans_up_storage_and_network_resources() {
        let dir = tempfile::tempdir().unwrap();
        let stord_socket = dir.path().join("cleanup-stord.sock");
        let nwd_socket = dir.path().join("cleanup-nwd.sock");
        let tracker = CleanupTracker::default();

        {
            let tracker = tracker.clone();
            let uds = tokio::net::UnixListener::bind(&stord_socket).unwrap();
            tokio::spawn(async move {
                tonic::transport::Server::builder()
                    .add_service(
                        chv_stord_api::chv_stord_api::storage_service_server::StorageServiceServer::new(
                            MockCleanupStord { tracker },
                        ),
                    )
                    .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(uds))
                    .await
                    .ok();
            });
        }

        {
            let tracker = tracker.clone();
            let uds = tokio::net::UnixListener::bind(&nwd_socket).unwrap();
            tokio::spawn(async move {
                tonic::transport::Server::builder()
                    .add_service(
                        chv_nwd_api::chv_nwd_api::network_service_server::NetworkServiceServer::new(
                            MockCleanupNwd { tracker },
                        ),
                    )
                    .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(uds))
                    .await
                    .ok();
            });
        }

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let mut cache = NodeCache::new("node-1");
        cache.node_state = crate::state_machine::NodeState::TenantReady
            .as_str()
            .to_string();
        cache.observe_generation("vm", "vm-1", "1");
        cache.observe_vm_attachment(
            "vm-1",
            &["vol-1".to_string()],
            &[VmNicAttachment {
                nic_id: "vm-1-net-1".to_string(),
                network_id: "net-1".to_string(),
            }],
        );
        cache
            .volume_handles
            .insert("vol-1".to_string(), "handle-1".to_string());

        let runtime = VmRuntime::new(Arc::new(MockCloudHypervisorAdapter::default()));
        let config = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 1,
            memory_bytes: 1024,
            kernel_path: std::path::PathBuf::from("/dev/null"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: dir.path().join("vms/vm-1/vm.sock"),
            cloud_init_userdata: None,
        };
        runtime
            .create_vm("vm-1", "1", &config, Some("op-1"))
            .await
            .unwrap();

        let server = AgentServer::new(
            Arc::new(tokio::sync::Mutex::new(cache)),
            runtime,
            stord_socket,
            nwd_socket,
            None,
            dir.path().to_path_buf(),
        );

        let req = proto::DeleteVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
            force: false,
        };
        let resp = proto::lifecycle_service_server::LifecycleService::delete_vm(
            &server,
            Request::new(req),
        )
        .await;
        assert!(resp.is_ok());

        assert_eq!(
            tracker.detached_volumes.lock().unwrap().as_slice(),
            ["vol-1"]
        );
        assert_eq!(tracker.closed_volumes.lock().unwrap().as_slice(), ["vol-1"]);
        assert_eq!(
            tracker.detached_nics.lock().unwrap().as_slice(),
            ["vm-1-net-1"]
        );
        assert!(server.vm_runtime.get("vm-1").is_none());
        assert!(!server
            .cache
            .lock()
            .await
            .volume_handles
            .contains_key("vol-1"));
    }

    #[tokio::test]
    async fn delete_vm_removes_cached_desired_state() {
        let server = test_server();
        {
            let mut cache = server.cache.lock().await;
            cache.observe_generation("vm", "vm-1", "1");
            cache.store_fragment(
                "vm",
                "vm-1",
                crate::cache::DesiredStateFragment {
                    id: "vm-1".to_string(),
                    kind: "vm".to_string(),
                    generation: "1".to_string(),
                    spec_json: vec![],
                    policy_json: vec![],
                    updated_at: String::new(),
                    updated_by: String::new(),
                },
            );
        }

        let config = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 1,
            memory_bytes: 1024,
            kernel_path: std::path::PathBuf::from("/dev/null"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: std::path::PathBuf::from("/var/lib/chv/agent/vms/vm-1/vm.sock"),
            cloud_init_userdata: None,
        };
        server
            .vm_runtime
            .create_vm("vm-1", "1", &config, Some("op-1"))
            .await
            .unwrap();

        let req = proto::DeleteVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
            force: false,
        };
        let resp = proto::lifecycle_service_server::LifecycleService::delete_vm(
            &server,
            Request::new(req),
        )
        .await;
        assert!(resp.is_ok());

        let cache = server.cache.lock().await;
        assert!(cache.get_generation("vm", "vm-1").is_none());
        assert!(cache.get_fragment("vm", "vm-1").is_none());
        assert!(cache.vm_attachment_state("vm-1").is_none());
    }

    #[tokio::test]
    async fn apply_volume_desired_state_attaches_to_vm() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("stord.sock");

        let uds = tokio::net::UnixListener::bind(&socket).unwrap();
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(
                    chv_stord_api::chv_stord_api::storage_service_server::StorageServiceServer::new(
                        MockStord,
                    ),
                )
                .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(uds))
                .await
                .ok();
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let mut cache = NodeCache::new("node-1");
        cache.node_state = crate::state_machine::NodeState::TenantReady
            .as_str()
            .to_string();
        let server = AgentServer::new(
            Arc::new(tokio::sync::Mutex::new(cache)),
            VmRuntime::new(Arc::new(MockCloudHypervisorAdapter::default())),
            socket,
            std::path::PathBuf::from("/run/chv/nwd/api.sock"),
            None,
            dir.path().to_path_buf(),
        );

        let req = proto::ApplyVolumeDesiredStateRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            volume_id: "vol-1".to_string(),
            fragment: Some(proto::DesiredStateFragment {
                id: "vol-1".to_string(),
                kind: "volume".to_string(),
                generation: "1".to_string(),
                spec_json: r#"{"vm_id":"vm-1","backend_class":"local","locator":"vol-1.img"}"#
                    .as_bytes()
                    .to_vec(),
                policy_json: vec![],
                updated_at: "".to_string(),
                updated_by: "".to_string(),
            }),
        };
        let resp = proto::reconcile_service_server::ReconcileService::apply_volume_desired_state(
            &server,
            Request::new(req),
        )
        .await;
        assert!(resp.is_ok());
        let cache = server.cache.lock().await;
        assert_eq!(
            cache.volume_handles.get("vol-1"),
            Some(&"handle-1".to_string())
        );
    }

    #[tokio::test]
    async fn apply_volume_desired_state_rejects_stale() {
        let server = test_server();
        let mut cache = server.cache.lock().await;
        cache.observe_generation("volume", "vol-1", "10");
        drop(cache);

        let req = proto::ApplyVolumeDesiredStateRequest {
            meta: Some(test_meta("9")),
            node_id: "node-1".to_string(),
            volume_id: "vol-1".to_string(),
            fragment: Some(proto::DesiredStateFragment {
                id: "vol-1".to_string(),
                kind: "volume".to_string(),
                generation: "9".to_string(),
                spec_json: vec![],
                policy_json: vec![],
                updated_at: "".to_string(),
                updated_by: "".to_string(),
            }),
        };
        let resp = proto::reconcile_service_server::ReconcileService::apply_volume_desired_state(
            &server,
            Request::new(req),
        )
        .await;
        assert_eq!(resp.unwrap_err().code(), tonic::Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn pause_and_resume_node_scheduling() {
        let server = test_server();
        let pause_req = proto::PauseNodeSchedulingRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
        };
        let resp = proto::lifecycle_service_server::LifecycleService::pause_node_scheduling(
            &server,
            Request::new(pause_req),
        )
        .await;
        assert!(resp.is_ok());
        assert_eq!(server.cache.lock().await.node_state, "Degraded");

        let resume_req = proto::ResumeNodeSchedulingRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
        };
        let resp = proto::lifecycle_service_server::LifecycleService::resume_node_scheduling(
            &server,
            Request::new(resume_req),
        )
        .await;
        assert!(resp.is_ok());
        assert_eq!(server.cache.lock().await.node_state, "TenantReady");
    }

    #[tokio::test]
    async fn drain_node_transitions_state() {
        let server = test_server();
        let req = proto::DrainNodeRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            allow_workload_stop: false,
        };
        let resp = proto::lifecycle_service_server::LifecycleService::drain_node(
            &server,
            Request::new(req),
        )
        .await;
        assert!(resp.is_ok());
        assert_eq!(server.cache.lock().await.node_state, "Draining");
    }

    #[tokio::test]
    async fn enter_and_exit_maintenance() {
        let server = test_server();
        let enter_req = proto::EnterMaintenanceRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            reason: "".to_string(),
        };
        let resp = proto::lifecycle_service_server::LifecycleService::enter_maintenance(
            &server,
            Request::new(enter_req),
        )
        .await;
        assert!(resp.is_ok());
        assert_eq!(server.cache.lock().await.node_state, "Maintenance");

        let exit_req = proto::ExitMaintenanceRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
        };
        let resp = proto::lifecycle_service_server::LifecycleService::exit_maintenance(
            &server,
            Request::new(exit_req),
        )
        .await;
        assert!(resp.is_ok());
        assert_eq!(server.cache.lock().await.node_state, "Bootstrapping");
    }
}
