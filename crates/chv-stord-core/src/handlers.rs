use crate::session::{Session, SessionTable};
use chv_common::types::{BackendLocator, DevicePolicy};
use chv_errors::{ChvError, ErrorCode};
use chv_observability::Metrics;
use chv_stord_api::chv_stord_api as proto;
use chv_stord_backends::StorageBackend;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct StorageServiceImpl<B: StorageBackend> {
    backend: Arc<B>,
    sessions: Arc<SessionTable>,
    metrics: Arc<Metrics>,
}

impl<B: StorageBackend> StorageServiceImpl<B> {
    pub fn new(
        backend: Arc<B>,
        sessions: Arc<SessionTable>,
        metrics: Arc<Metrics>,
    ) -> Self {
        Self {
            backend,
            sessions,
            metrics,
        }
    }

    fn map_backend_locator(
        b: Option<proto::BackendLocator>,
    ) -> Result<BackendLocator, ChvError> {
        let b = b.ok_or_else(|| ChvError::InvalidArgument {
            field: "backend".to_string(),
            reason: "missing".to_string(),
        })?;
        Ok(BackendLocator {
            backend_class: b.backend_class,
            locator: b.locator,
            options: b.options.into_iter().collect(),
        })
    }

    fn map_device_policy(p: Option<proto::DevicePolicy>) -> DevicePolicy {
        p.map(|p| DevicePolicy {
            read_bps: p.read_bps,
            write_bps: p.write_bps,
            read_iops: p.read_iops,
            write_iops: p.write_iops,
            burst_allowed: p.burst_allowed,
        })
        .unwrap_or_default()
    }

    fn ok_result() -> proto::Result {
        proto::Result {
            status: ErrorCode::OK.to_string(),
            error_code: ErrorCode::OK.to_string(),
            human_summary: String::new(),
        }
    }
}

#[tonic::async_trait]
impl<B: StorageBackend> proto::storage_service_server::StorageService
    for StorageServiceImpl<B>
{
    async fn open_volume(
        &self,
        request: Request<proto::OpenVolumeRequest>,
    ) -> Result<Response<proto::OpenVolumeResponse>, Status> {
        self.metrics.increment_counter("stord_open_volume_total");
        let req = request.into_inner();
        let locator = Self::map_backend_locator(req.backend)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let policy = Self::map_device_policy(req.policy);

        let export = self
            .backend
            .open(&req.volume_id, &locator, &policy)
            .await
            .map_err(|e| match &e {
                ChvError::InvalidArgument { .. } => {
                    Status::invalid_argument(e.to_string())
                }
                ChvError::BackendUnavailable { .. } => {
                    Status::unavailable(e.to_string())
                }
                _ => Status::internal(e.to_string()),
            })?;

        let session = Session {
            volume_id: req.volume_id.clone(),
            vm_id: None,
            attachment_handle: export.attachment_handle.clone(),
            export_kind: export.export_kind.clone(),
            export_path: export.export_path.clone(),
            runtime_status: "open".to_string(),
        };
        self.sessions.upsert(session);

        Ok(Response::new(proto::OpenVolumeResponse {
            result: Some(Self::ok_result()),
            volume_id: req.volume_id,
            attachment_handle: export.attachment_handle,
            export_kind: export.export_kind,
            export_path: export.export_path,
        }))
    }

    async fn close_volume(
        &self,
        request: Request<proto::CloseVolumeRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics.increment_counter("stord_close_volume_total");
        let req = request.into_inner();

        if let Some(s) = self.sessions.get(&req.volume_id, &req.attachment_handle) {
            self.backend
                .close(&s.volume_id, &s.attachment_handle)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            self.sessions.remove(&req.volume_id, &req.attachment_handle);
        }

        Ok(Response::new(Self::ok_result()))
    }

    async fn get_volume_health(
        &self,
        request: Request<proto::VolumeHealthRequest>,
    ) -> Result<Response<proto::VolumeHealthResponse>, Status> {
        let req = request.into_inner();

        let sessions = self.sessions.list();
        let session = sessions.into_iter().find(|s| s.volume_id == req.volume_id);

        let (status, backend_state, last_error) =
            if let Some(s) = session {
                match self.backend.health(&s.volume_id, &s.attachment_handle).await
                {
                    Ok(h) => (h.status, h.backend_state, h.last_error),
                    Err(e) => (
                        "unhealthy".to_string(),
                        "error".to_string(),
                        e.to_string(),
                    ),
                }
            } else {
                (
                    "unknown".to_string(),
                    "closed".to_string(),
                    String::new(),
                )
            };

        Ok(Response::new(proto::VolumeHealthResponse {
            result: Some(Self::ok_result()),
            volume_id: req.volume_id,
            health_status: status,
            backend_state,
            last_error,
        }))
    }

    async fn list_volume_sessions(
        &self,
        _request: Request<proto::ListVolumeSessionsRequest>,
    ) -> Result<Response<proto::ListVolumeSessionsResponse>, Status> {
        let sessions: Vec<proto::VolumeSession> = self
            .sessions
            .list()
            .into_iter()
            .map(|s| proto::VolumeSession {
                volume_id: s.volume_id,
                vm_id: s.vm_id.unwrap_or_default(),
                attachment_handle: s.attachment_handle,
                export_kind: s.export_kind,
                export_path: s.export_path,
                runtime_status: s.runtime_status,
            })
            .collect();

        Ok(Response::new(proto::ListVolumeSessionsResponse {
            sessions,
        }))
    }

    async fn attach_volume_to_vm(
        &self,
        _request: Request<proto::AttachVolumeToVmRequest>,
    ) -> Result<Response<proto::AttachVolumeToVmResponse>, Status> {
        Err(Status::unimplemented(
            "attach_volume_to_vm not yet implemented",
        ))
    }

    async fn detach_volume_from_vm(
        &self,
        _request: Request<proto::DetachVolumeFromVmRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        Err(Status::unimplemented(
            "detach_volume_from_vm not yet implemented",
        ))
    }

    async fn resize_volume(
        &self,
        _request: Request<proto::ResizeVolumeRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        Err(Status::unimplemented(
            "resize_volume not yet implemented",
        ))
    }

    async fn prepare_snapshot(
        &self,
        _request: Request<proto::PrepareSnapshotRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        Err(Status::unimplemented(
            "prepare_snapshot not yet implemented",
        ))
    }

    async fn prepare_clone(
        &self,
        _request: Request<proto::PrepareCloneRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        Err(Status::unimplemented(
            "prepare_clone not yet implemented",
        ))
    }

    async fn set_device_policy(
        &self,
        _request: Request<proto::SetDevicePolicyRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        Err(Status::unimplemented(
            "set_device_policy not yet implemented",
        ))
    }
}
