use crate::cache::NodeCache;
use crate::control_plane::ControlPlaneClient;
use crate::vm_runtime::VmRuntime;
use chv_agent_runtime_ch::adapter::VmConfig;
use chv_errors::ChvError;
use control_plane_node_api::control_plane_node_api as proto;
use std::path::Path;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct AgentServer {
    pub cache: Arc<tokio::sync::Mutex<NodeCache>>,
    pub vm_runtime: VmRuntime,
    pub stord_socket: std::path::PathBuf,
    pub nwd_socket: std::path::PathBuf,
}

impl AgentServer {
    pub fn new(
        cache: NodeCache,
        vm_runtime: VmRuntime,
        stord_socket: std::path::PathBuf,
        nwd_socket: std::path::PathBuf,
    ) -> Self {
        Self {
            cache: Arc::new(tokio::sync::Mutex::new(cache)),
            vm_runtime,
            stord_socket,
            nwd_socket,
        }
    }

    pub async fn serve(self, socket_path: &Path) -> Result<(), ChvError> {
        if let Some(parent) = socket_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| ChvError::Io {
                path: parent.to_string_lossy().to_string(),
                source: e,
            })?;
        }
        if socket_path.exists() {
            tokio::fs::remove_file(socket_path).await.map_err(|e| ChvError::Io {
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
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "node", &inner.node_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        if let Some(frag) = inner.fragment {
            cache.observe_generation("node", &inner.node_id, &frag.generation);
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
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        if let Some(frag) = inner.fragment {
            cache.observe_generation("vm", &inner.vm_id, &frag.generation);
            cache.store_fragment("vm", &inner.vm_id, crate::cache::DesiredStateFragment {
                id: frag.id,
                kind: frag.kind,
                generation: frag.generation,
                spec_json: frag.spec_json,
                policy_json: frag.policy_json,
                updated_at: frag.updated_at,
                updated_by: frag.updated_by,
            });
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
        _req: Request<proto::ApplyVolumeDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("volume desired state in Phase 3"))
    }

    async fn apply_network_desired_state(
        &self,
        req: Request<proto::ApplyNetworkDesiredStateRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "network", &inner.network_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        if let Some(frag) = inner.fragment {
            cache.observe_generation("network", &inner.network_id, &frag.generation);
            cache.store_fragment("network", &inner.network_id, crate::cache::DesiredStateFragment {
                id: frag.id,
                kind: frag.kind,
                generation: frag.generation,
                spec_json: frag.spec_json.clone(),
                policy_json: frag.policy_json,
                updated_at: frag.updated_at,
                updated_by: frag.updated_by,
            });

            let spec = serde_json::from_slice::<serde_json::Value>(&frag.spec_json).unwrap_or_default();
            let bridge = spec.get("bridge_name").and_then(|v| v.as_str()).unwrap_or("br0");
            let cidr = spec.get("subnet_cidr").and_then(|v| v.as_str()).unwrap_or("10.0.0.0/24");

            let mut nwd = crate::daemon_clients::NwdClient::connect(&self.nwd_socket)
                .await
                .map_err(|e| Status::unavailable(format!("nwd unavailable: {}", e)))?;

            nwd.ensure_network_topology(&inner.network_id, bridge, cidr, Some(&meta.operation_id)
            ).await.map_err(|e| Status::internal(format!("ensure_network_topology failed: {}", e)))?;

            if let Some(exposures) = spec.get("exposures").and_then(|v| v.as_array()) {
                for exp in exposures {
                    let eid = exp.get("exposure_id").and_then(|v| v.as_str()).unwrap_or("");
                    let proto_str = exp.get("protocol").and_then(|v| v.as_str()).unwrap_or("tcp");
                    let ext_port = exp.get("external_port").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let tip = exp.get("target_ip").and_then(|v| v.as_str()).unwrap_or("");
                    let tport = exp.get("target_port").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let mode = exp.get("mode").and_then(|v| v.as_str()).unwrap_or("nat");
                    if !eid.is_empty() {
                        let _ = nwd.expose_service(
                            &inner.network_id, eid, proto_str, ext_port, tip, tport, mode, Some(&meta.operation_id)
                        ).await;
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
        _req: Request<proto::AcknowledgeDesiredStateVersionRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("acknowledge desired state in Phase 3"))
    }
}

#[tonic::async_trait]
impl proto::lifecycle_service_server::LifecycleService for AgentServer {
    async fn create_vm(
        &self,
        req: Request<proto::CreateVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let vm = inner.vm.as_ref().ok_or_else(|| Status::invalid_argument("missing vm"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &vm.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        let node_state = cache.node_state.parse::<crate::state_machine::NodeState>()
            .unwrap_or(crate::state_machine::NodeState::Bootstrapping);
        if node_state != crate::state_machine::NodeState::TenantReady {
            return Err(Status::failed_precondition(
                format!("node not schedulable: {}", cache.node_state)
            ));
        }
        let config = VmConfig {
            vm_id: vm.vm_id.clone(),
            cpus: 2,
            memory_bytes: 1024,
            kernel_path: std::path::PathBuf::from("/dev/null"),
            disk_paths: vec![],
            api_socket_path: std::path::PathBuf::from(format!("/run/chv/agent/vm-{}.sock", vm.vm_id)),
        };
        let op_id = meta.operation_id.as_str();
        self.vm_runtime.create_vm(&vm.vm_id, &meta.desired_state_version, &config, Some(op_id)).await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
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
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        self.vm_runtime.start_vm(&inner.vm_id, Some(&meta.operation_id)).await
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
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        self.vm_runtime.stop_vm(&inner.vm_id, inner.force, Some(&meta.operation_id)).await
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
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        Err(Status::unimplemented("reboot_vm in Phase 3"))
    }

    async fn delete_vm(
        &self,
        req: Request<proto::DeleteVmRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "vm", &inner.vm_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        self.vm_runtime.delete_vm(&inner.vm_id, Some(&meta.operation_id)).await
            .map_err(|e| Status::not_found(e.to_string()))?;
        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
                error_code: "".to_string(),
                human_summary: "vm deleted".to_string(),
            }),
        }))
    }

    async fn attach_volume(
        &self,
        req: Request<proto::AttachVolumeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        let inner = req.into_inner();
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let vol = inner.volume.as_ref().ok_or_else(|| Status::invalid_argument("missing volume"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "volume", &vol.volume_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;

        let (backend_class, locator) = if let Ok(spec) = serde_json::from_slice::<serde_json::Value>(&vol.volume_spec_json) {
            let bc = spec.get("backend_class").and_then(|v| v.as_str()).unwrap_or("local");
            let loc = spec.get("locator").and_then(|v| v.as_str()).unwrap_or(&vol.volume_id);
            (bc.to_string(), loc.to_string())
        } else {
            ("local".to_string(), vol.volume_id.clone())
        };

        let mut stord = crate::daemon_clients::StordClient::connect(&self.stord_socket)
            .await
            .map_err(|e| Status::unavailable(format!("stord unavailable: {}", e)))?;

        let (_, handle, _) = stord
            .open_volume(&vol.volume_id, &backend_class, &locator, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::internal(format!("open_volume failed: {}", e)))?;

        stord
            .attach_volume_to_vm(&vol.volume_id, &vol.vm_id, &handle, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::internal(format!("attach_volume_to_vm failed: {}", e)))?;

        cache.volume_handles.insert(vol.volume_id.clone(), handle);

        Ok(Response::new(proto::AckResponse {
            result: Some(proto::ResultMeta {
                operation_id: meta.operation_id.clone(),
                status: "ok".to_string(),
                node_observed_generation: cache.observed_generation.clone(),
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
        let meta = inner.meta.as_ref().ok_or_else(|| Status::invalid_argument("missing meta"))?;
        let mut cache = self.cache.lock().await;
        ControlPlaneClient::stale_generation_check(meta, &cache, "volume", &inner.volume_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;

        let mut stord = crate::daemon_clients::StordClient::connect(&self.stord_socket)
            .await
            .map_err(|e| Status::unavailable(format!("stord unavailable: {}", e)))?;

        stord
            .detach_volume_from_vm(&inner.volume_id, &inner.vm_id, inner.force, Some(&meta.operation_id))
            .await
            .map_err(|e| Status::internal(format!("detach_volume_from_vm failed: {}", e)))?;

        if let Some(handle) = cache.volume_handles.remove(&inner.volume_id) {
            let _ = stord
                .close_volume(&inner.volume_id, &handle, Some(&meta.operation_id))
                .await;
        }

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
        Err(Status::unimplemented("resize_volume in Phase 3"))
    }

    async fn pause_node_scheduling(
        &self,
        _req: Request<proto::PauseNodeSchedulingRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("pause_node_scheduling in Phase 3"))
    }

    async fn resume_node_scheduling(
        &self,
        _req: Request<proto::ResumeNodeSchedulingRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("resume_node_scheduling in Phase 3"))
    }

    async fn drain_node(
        &self,
        _req: Request<proto::DrainNodeRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("drain_node in Phase 3"))
    }

    async fn enter_maintenance(
        &self,
        _req: Request<proto::EnterMaintenanceRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("enter_maintenance in Phase 3"))
    }

    async fn exit_maintenance(
        &self,
        _req: Request<proto::ExitMaintenanceRequest>,
    ) -> Result<Response<proto::AckResponse>, Status> {
        Err(Status::unimplemented("exit_maintenance in Phase 3"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter;
    use std::sync::Arc;

    fn test_server() -> AgentServer {
        let mut cache = NodeCache::new("node-1");
        cache.node_state = crate::state_machine::NodeState::TenantReady.as_str().to_string();
        AgentServer::new(
            cache,
            VmRuntime::new(Arc::new(MockCloudHypervisorAdapter::default())),
            std::path::PathBuf::from("/run/chv/stord/api.sock"),
            std::path::PathBuf::from("/run/chv/nwd/api.sock"),
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
            &server, Request::new(req)
        ).await;
        assert!(resp.is_ok());
        let cache = server.cache.lock().await;
        assert_eq!(cache.get_generation("vm", "vm-1"), Some(&"5".to_string()));
        assert!(cache.get_fragment("vm", "vm-1").is_some());
    }

    #[tokio::test]
    async fn create_vm_lifecycle_flow() {
        let server = test_server();
        let create_req = proto::CreateVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm: Some(proto::VmMutationSpec {
                vm_id: "vm-1".to_string(),
                vm_spec_json: vec![],
            }),
        };
        let resp = proto::lifecycle_service_server::LifecycleService::create_vm(
            &server, Request::new(create_req)
        ).await;
        assert!(resp.is_ok());

        let start_req = proto::StartVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
        };
        let resp = proto::lifecycle_service_server::LifecycleService::start_vm(
            &server, Request::new(start_req)
        ).await;
        assert!(resp.is_ok());
        assert_eq!(server.vm_runtime.get("vm-1").unwrap().runtime_status, "Running");

        let stop_req = proto::StopVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
            force: false,
        };
        let resp = proto::lifecycle_service_server::LifecycleService::stop_vm(
            &server, Request::new(stop_req)
        ).await;
        assert!(resp.is_ok());
        assert_eq!(server.vm_runtime.get("vm-1").unwrap().runtime_status, "Stopped");

        let delete_req = proto::DeleteVmRequest {
            meta: Some(test_meta("1")),
            node_id: "node-1".to_string(),
            vm_id: "vm-1".to_string(),
            force: false,
        };
        let resp = proto::lifecycle_service_server::LifecycleService::delete_vm(
            &server, Request::new(delete_req)
        ).await;
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
        let resp = proto::lifecycle_service_server::LifecycleService::start_vm(
            &server, Request::new(req)
        ).await;
        assert_eq!(resp.unwrap_err().code(), tonic::Code::FailedPrecondition);
    }
}
