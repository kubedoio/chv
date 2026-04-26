use chv_errors::ChvError;
use control_plane_node_api::control_plane_node_api as proto;
use std::path::Path;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;
use tracing::Instrument;

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
        let span = tracing::info_span!("apply_vm_desired_state", operation_id);
        self.reconcile
            .apply_vm_desired_state(with_operation_id_metadata(req, operation_id))
            .instrument(span)
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
        let span = tracing::info_span!("apply_volume_desired_state", operation_id);
        self.reconcile
            .apply_volume_desired_state(with_operation_id_metadata(req, operation_id))
            .instrument(span)
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
        let span = tracing::info_span!("apply_network_desired_state", operation_id);
        self.reconcile
            .apply_network_desired_state(with_operation_id_metadata(req, operation_id))
            .instrument(span)
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
        let span = tracing::info_span!("create_vm", operation_id);
        self.lifecycle
            .create_vm(with_operation_id_metadata(req, operation_id))
            .instrument(span)
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
        let span = tracing::info_span!("start_vm", operation_id);
        self.lifecycle
            .start_vm(with_operation_id_metadata(req, operation_id))
            .instrument(span)
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
        let span = tracing::info_span!("stop_vm", operation_id);
        self.lifecycle
            .stop_vm(with_operation_id_metadata(req, operation_id))
            .instrument(span)
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
        let span = tracing::info_span!("reboot_vm", operation_id);
        self.lifecycle
            .reboot_vm(with_operation_id_metadata(req, operation_id))
            .instrument(span)
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
        let span = tracing::info_span!("delete_vm", operation_id);
        self.lifecycle
            .delete_vm(with_operation_id_metadata(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("delete_vm failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn snapshot_vm(
        &mut self,
        node_id: &str,
        vm_id: &str,
        generation: &str,
        destination: &str,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::SnapshotVmRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            vm_id: vm_id.to_string(),
            destination: destination.to_string(),
        };
        let span = tracing::info_span!("snapshot_vm", operation_id);
        self.lifecycle
            .snapshot_vm(with_operation_id_metadata(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("snapshot_vm failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn restore_snapshot(
        &mut self,
        node_id: &str,
        vm_id: &str,
        generation: &str,
        source: &str,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::RestoreSnapshotRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            vm_id: vm_id.to_string(),
            source: source.to_string(),
        };
        let span = tracing::info_span!("restore_snapshot", operation_id);
        self.lifecycle
            .restore_snapshot(with_operation_id_metadata(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("restore_snapshot failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn attach_volume(
        &mut self,
        node_id: &str,
        volume_id: &str,
        vm_id: &str,
        generation: &str,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::AttachVolumeRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            volume: Some(proto::VolumeMutationSpec {
                volume_id: volume_id.to_string(),
                vm_id: vm_id.to_string(),
                volume_spec_json: vec![],
            }),
        };
        let span = tracing::info_span!("attach_volume", operation_id);
        self.lifecycle
            .attach_volume(with_operation_id_metadata(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("attach_volume failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn detach_volume(
        &mut self,
        node_id: &str,
        volume_id: &str,
        vm_id: &str,
        generation: &str,
        force: bool,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::DetachVolumeRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            vm_id: vm_id.to_string(),
            volume_id: volume_id.to_string(),
            force,
        };
        let span = tracing::info_span!("detach_volume", operation_id);
        self.lifecycle
            .detach_volume(with_operation_id_metadata(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("detach_volume failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn resize_volume(
        &mut self,
        node_id: &str,
        volume_id: &str,
        generation: &str,
        new_size_bytes: u64,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::ResizeVolumeRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            volume_id: volume_id.to_string(),
            new_size_bytes,
        };
        let span = tracing::info_span!("resize_volume", operation_id);
        self.lifecycle
            .resize_volume(with_operation_id_metadata(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("resize_volume failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn snapshot_volume(
        &mut self,
        node_id: &str,
        volume_id: &str,
        generation: &str,
        snapshot_name: &str,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::SnapshotVolumeRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            volume_id: volume_id.to_string(),
            snapshot_name: snapshot_name.to_string(),
        };
        let span = tracing::info_span!("snapshot_volume", operation_id);
        self.lifecycle
            .snapshot_volume(with_operation_id_metadata(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("snapshot_volume failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn restore_volume(
        &mut self,
        node_id: &str,
        volume_id: &str,
        generation: &str,
        snapshot_name: &str,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::RestoreVolumeRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            volume_id: volume_id.to_string(),
            snapshot_name: snapshot_name.to_string(),
        };
        let span = tracing::info_span!("restore_volume", operation_id);
        self.lifecycle
            .restore_volume(with_operation_id_metadata(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("restore_volume failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn delete_volume_snapshot(
        &mut self,
        node_id: &str,
        volume_id: &str,
        generation: &str,
        snapshot_name: &str,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::DeleteVolumeSnapshotRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            volume_id: volume_id.to_string(),
            snapshot_name: snapshot_name.to_string(),
        };
        let span = tracing::info_span!("delete_volume_snapshot", operation_id);
        self.lifecycle
            .delete_volume_snapshot(with_operation_id_metadata(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("delete_volume_snapshot failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn clone_volume(
        &mut self,
        node_id: &str,
        source_volume_id: &str,
        target_volume_id: &str,
        generation: &str,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::CloneVolumeRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            source_volume_id: source_volume_id.to_string(),
            target_volume_id: target_volume_id.to_string(),
        };
        let span = tracing::info_span!("clone_volume", operation_id);
        self.lifecycle
            .clone_volume(with_operation_id_metadata(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("clone_volume failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn start_network(
        &mut self,
        node_id: &str,
        network_id: &str,
        generation: &str,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::StartNetworkRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            network_id: network_id.to_string(),
        };
        let span = tracing::info_span!("start_network", operation_id);
        self.lifecycle
            .start_network(with_operation_id_metadata(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("start_network failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn stop_network(
        &mut self,
        node_id: &str,
        network_id: &str,
        generation: &str,
        force: bool,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::StopNetworkRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            network_id: network_id.to_string(),
            force,
        };
        let span = tracing::info_span!("stop_network", operation_id);
        self.lifecycle
            .stop_network(with_operation_id_metadata(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("stop_network failed: {e}"),
            })
            .map(|r| r.into_inner())
    }

    pub async fn restart_network(
        &mut self,
        node_id: &str,
        network_id: &str,
        generation: &str,
        operation_id: &str,
        requested_by: Option<&str>,
    ) -> Result<proto::AckResponse, ChvError> {
        let req = proto::RestartNetworkRequest {
            meta: Some(proto::RequestMeta {
                operation_id: operation_id.to_string(),
                requested_by: requested_by.unwrap_or("control-plane").to_string(),
                target_node_id: node_id.to_string(),
                desired_state_version: generation.to_string(),
                request_unix_ms: now_unix_ms(),
            }),
            node_id: node_id.to_string(),
            network_id: network_id.to_string(),
        };
        let span = tracing::info_span!("restart_network", operation_id);
        self.lifecycle
            .restart_network(with_operation_id_metadata(req, operation_id))
            .instrument(span)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "agent".to_string(),
                reason: format!("restart_network failed: {e}"),
            })
            .map(|r| r.into_inner())
    }
}

fn with_operation_id_metadata<T>(req: T, operation_id: &str) -> tonic::Request<T> {
    let mut grpc_req = tonic::Request::new(req);
    if let Ok(val) = tonic::metadata::MetadataValue::try_from(operation_id) {
        grpc_req
            .metadata_mut()
            .insert(chv_common::OPERATION_ID_METADATA_KEY, val);
    }
    grpc_req
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
