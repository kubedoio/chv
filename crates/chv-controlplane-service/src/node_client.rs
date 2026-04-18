use chv_errors::ChvError;
use control_plane_node_api::control_plane_node_api as proto;
use std::path::Path;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;

pub struct NodeClient {
    reconcile: proto::reconcile_service_client::ReconcileServiceClient<Channel>,
    lifecycle: proto::lifecycle_service_client::LifecycleServiceClient<Channel>,
}

impl NodeClient {
    pub async fn connect(socket_path: &Path) -> Result<Self, ChvError> {
        let path = socket_path.to_path_buf();
        let channel = Endpoint::try_from("http://[::]:50051")
            .map_err(|e| ChvError::InvalidArgument {
                field: "node_socket".to_string(),
                reason: e.to_string(),
            })?
            .connect_with_connector(service_fn(move |_: Uri| {
                let p = path.clone();
                async move {
                    let stream = UnixStream::connect(p).await?;
                    Ok::<_, std::io::Error>(hyper_util::rt::tokio::TokioIo::new(stream))
                }
            }))
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: e.to_string(),
            })?;
        Ok(Self {
            reconcile: proto::reconcile_service_client::ReconcileServiceClient::new(
                channel.clone(),
            ),
            lifecycle: proto::lifecycle_service_client::LifecycleServiceClient::new(channel),
        })
    }

    pub async fn apply_vm_desired_state(
        &mut self,
        node_id: &str,
        vm_id: &str,
        generation: &str,
        spec_json: Vec<u8>,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::ApplyVmDesiredStateRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            vm_id: vm_id.to_string(),
            fragment: Some(proto::DesiredStateFragment {
                id: vm_id.to_string(),
                kind: "vm".to_string(),
                generation: generation.to_string(),
                spec_json,
                policy_json: vec![],
                updated_at: now_iso(),
                updated_by: requested_by.unwrap_or("control-plane").to_string(),
            }),
        };
        self.reconcile
            .apply_vm_desired_state(req)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("apply_vm_desired_state failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn apply_volume_desired_state(
        &mut self,
        node_id: &str,
        volume_id: &str,
        generation: &str,
        spec_json: Vec<u8>,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::ApplyVolumeDesiredStateRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            volume_id: volume_id.to_string(),
            fragment: Some(proto::DesiredStateFragment {
                id: volume_id.to_string(),
                kind: "volume".to_string(),
                generation: generation.to_string(),
                spec_json,
                policy_json: vec![],
                updated_at: now_iso(),
                updated_by: requested_by.unwrap_or("control-plane").to_string(),
            }),
        };
        self.reconcile
            .apply_volume_desired_state(req)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("apply_volume_desired_state failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn apply_network_desired_state(
        &mut self,
        node_id: &str,
        network_id: &str,
        generation: &str,
        spec_json: Vec<u8>,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::ApplyNetworkDesiredStateRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            network_id: network_id.to_string(),
            fragment: Some(proto::DesiredStateFragment {
                id: network_id.to_string(),
                kind: "network".to_string(),
                generation: generation.to_string(),
                spec_json,
                policy_json: vec![],
                updated_at: now_iso(),
                updated_by: requested_by.unwrap_or("control-plane").to_string(),
            }),
        };
        self.reconcile
            .apply_network_desired_state(req)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("apply_network_desired_state failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn create_vm(
        &mut self,
        node_id: &str,
        vm_id: &str,
        generation: &str,
        vm_spec_json: Vec<u8>,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::CreateVmRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            vm: Some(proto::VmMutationSpec {
                vm_id: vm_id.to_string(),
                vm_spec_json,
            }),
        };
        self.lifecycle
            .create_vm(req)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("create_vm failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn start_vm(
        &mut self,
        node_id: &str,
        vm_id: &str,
        generation: &str,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::StartVmRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            vm_id: vm_id.to_string(),
        };
        self.lifecycle
            .start_vm(req)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("start_vm failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn stop_vm(
        &mut self,
        node_id: &str,
        vm_id: &str,
        generation: &str,
        force: bool,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::StopVmRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            vm_id: vm_id.to_string(),
            force,
        };
        self.lifecycle
            .stop_vm(req)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("stop_vm failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn reboot_vm(
        &mut self,
        node_id: &str,
        vm_id: &str,
        generation: &str,
        force: bool,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::RebootVmRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            vm_id: vm_id.to_string(),
            force,
        };
        self.lifecycle
            .reboot_vm(req)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("reboot_vm failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn delete_vm(
        &mut self,
        node_id: &str,
        vm_id: &str,
        generation: &str,
        force: bool,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::DeleteVmRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            vm_id: vm_id.to_string(),
            force,
        };
        self.lifecycle
            .delete_vm(req)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("delete_vm failed: {e}"),
            })
            .map(|r| r.into_inner())
    }
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn now_iso() -> String {
    // RFC 3339-ish format using current unix millis as a simple timestamp string.
    // Sufficient for fragment updated_at; agent does not parse this field.
    let ms = now_unix_ms();
    format!("{ms}")
}
