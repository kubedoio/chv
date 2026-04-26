use crate::session::{Session, SessionTable};
use chv_common::types::{BackendLocator, DevicePolicy};
use chv_errors::{ChvError, ErrorCode};
use chv_observability::{operation_span, Metrics};
use chv_stord_api::chv_stord_api as proto;
use chv_stord_backends::StorageBackend;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct StorageServiceImpl<B: StorageBackend> {
    backend: Arc<B>,
    sessions: Arc<SessionTable>,
    metrics: Arc<Metrics>,
    backend_allowlist: Vec<String>,
    store: Option<Arc<tokio::sync::Mutex<crate::store::SessionStore>>>,
}

impl<B: StorageBackend> StorageServiceImpl<B> {
    pub fn new(
        backend: Arc<B>,
        sessions: Arc<SessionTable>,
        metrics: Arc<Metrics>,
        backend_allowlist: Vec<String>,
    ) -> Self {
        Self {
            backend,
            sessions,
            metrics,
            backend_allowlist,
            store: None,
        }
    }

    pub fn sessions(&self) -> Arc<SessionTable> {
        self.sessions.clone()
    }

    pub fn set_store(&mut self, store: crate::store::SessionStore) {
        self.store = Some(Arc::new(tokio::sync::Mutex::new(store)));
    }

    async fn persist_upsert(&self, session: &crate::session::Session) {
        if let Some(store) = &self.store {
            let store = store.lock().await;
            if let Err(e) = store.upsert(session) {
                tracing::error!(error = %e, "failed to persist session to SQLite");
            }
        }
    }

    async fn persist_remove(&self, volume_id: &str, handle: &str) {
        if let Some(store) = &self.store {
            let store = store.lock().await;
            if let Err(e) = store.remove(volume_id, handle) {
                tracing::error!(error = %e, "failed to remove session from SQLite");
            }
        }
    }

    fn map_backend_locator(b: Option<proto::BackendLocator>) -> Result<BackendLocator, ChvError> {
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
            read_only: p.read_only,
            no_exec: p.no_exec,
            io_scheduler: p.io_scheduler,
            cache_mode: p.cache_mode,
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

    fn check_allowlist(&self, backend_class: &str) -> Result<(), ChvError> {
        if self.backend_allowlist.is_empty() {
            return Ok(());
        }
        if self.backend_allowlist.iter().any(|b| b == backend_class) {
            return Ok(());
        }
        Err(ChvError::BackendUnavailable {
            backend: backend_class.to_string(),
            reason: format!("backend class '{}' not in allowlist", backend_class),
        })
    }
}

#[tonic::async_trait]
impl<B: StorageBackend> proto::storage_service_server::StorageService for StorageServiceImpl<B> {
    async fn open_volume(
        &self,
        request: Request<proto::OpenVolumeRequest>,
    ) -> Result<Response<proto::OpenVolumeResponse>, Status> {
        self.metrics.increment_counter("stord_open_volume_total");
        let req = request.into_inner();
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));
        let _enter = _span.enter();

        let locator = match Self::map_backend_locator(req.backend) {
            Ok(l) => l,
            Err(e) => {
                return Ok(Response::new(proto::OpenVolumeResponse {
                    result: Some(e.to_proto_result()),
                    volume_id: req.volume_id,
                    attachment_handle: String::new(),
                    export_kind: String::new(),
                    export_path: String::new(),
                }));
            }
        };

        if let Err(e) = self.check_allowlist(&locator.backend_class) {
            return Ok(Response::new(proto::OpenVolumeResponse {
                result: Some(e.to_proto_result()),
                volume_id: req.volume_id,
                attachment_handle: String::new(),
                export_kind: String::new(),
                export_path: String::new(),
            }));
        }

        let policy = Self::map_device_policy(req.policy);

        // Idempotency: if already open with same volume+path, return existing
        let precompute_path = if std::path::Path::new(&locator.locator).is_absolute() {
            locator.locator.clone()
        } else {
            // We don't know runtime_dir here; backend handles resolution.
            // For local backend idempotency we rely on the backend trait eventually.
            // As a best-effort shortcut, skip pre-check for relative locators.
            String::new()
        };
        if !precompute_path.is_empty() {
            if let Some(s) = self
                .sessions
                .find_by_volume_and_path(&req.volume_id, &precompute_path)
            {
                return Ok(Response::new(proto::OpenVolumeResponse {
                    result: Some(Self::ok_result()),
                    volume_id: s.volume_id,
                    attachment_handle: s.attachment_handle,
                    export_kind: s.export_kind,
                    export_path: s.export_path,
                }));
            }
        }

        let export = match self.backend.open(&req.volume_id, &locator, &policy).await {
            Ok(e) => e,
            Err(e) => {
                return Ok(Response::new(proto::OpenVolumeResponse {
                    result: Some(e.to_proto_result()),
                    volume_id: req.volume_id,
                    attachment_handle: String::new(),
                    export_kind: String::new(),
                    export_path: String::new(),
                }));
            }
        };

        // Post-open idempotency: same handle may already exist
        if let Some(s) = self.sessions.get(&req.volume_id, &export.attachment_handle) {
            return Ok(Response::new(proto::OpenVolumeResponse {
                result: Some(Self::ok_result()),
                volume_id: s.volume_id,
                attachment_handle: s.attachment_handle,
                export_kind: s.export_kind,
                export_path: s.export_path,
            }));
        }

        let session = Session {
            volume_id: req.volume_id.clone(),
            vm_id: None,
            attachment_handle: export.attachment_handle.clone(),
            export_kind: export.export_kind.clone(),
            export_path: export.export_path.clone(),
            runtime_status: "open".to_string(),
        };
        self.sessions.upsert(session.clone());
        self.persist_upsert(&session).await;

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
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));
        let _enter = _span.enter();

        if let Some(s) = self.sessions.get(&req.volume_id, &req.attachment_handle) {
            if let Err(e) = self.backend.close(&s.volume_id, &s.attachment_handle).await {
                return Ok(Response::new(e.to_proto_result()));
            }
            self.sessions.remove(&req.volume_id, &req.attachment_handle);
            self.persist_remove(&req.volume_id, &req.attachment_handle)
                .await;
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

        let (status, backend_state, last_error) = if let Some(s) = session {
            match self
                .backend
                .health(&s.volume_id, &s.attachment_handle)
                .await
            {
                Ok(h) => (h.status, h.backend_state, h.last_error),
                Err(e) => ("unhealthy".to_string(), "error".to_string(), e.to_string()),
            }
        } else {
            ("unknown".to_string(), "closed".to_string(), String::new())
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
        request: Request<proto::AttachVolumeToVmRequest>,
    ) -> Result<Response<proto::AttachVolumeToVmResponse>, Status> {
        self.metrics.increment_counter("stord_attach_volume_total");
        let req = request.into_inner();
        let span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));
        let _enter = span.enter();

        if self
            .sessions
            .get(&req.volume_id, &req.attachment_handle)
            .is_none()
        {
            let e = ChvError::NotFound {
                resource: "session".to_string(),
                id: format!("{}/{}", req.volume_id, req.attachment_handle),
            };
            return Ok(Response::new(proto::AttachVolumeToVmResponse {
                result: Some(e.to_proto_result()),
                volume_id: req.volume_id,
                vm_id: req.vm_id,
                export_kind: String::new(),
                export_path: String::new(),
            }));
        }

        let export = match self
            .backend
            .attach(&req.volume_id, &req.attachment_handle, &req.vm_id)
            .await
        {
            Ok(e) => e,
            Err(e) => {
                return Ok(Response::new(proto::AttachVolumeToVmResponse {
                    result: Some(e.to_proto_result()),
                    volume_id: req.volume_id,
                    vm_id: req.vm_id,
                    export_kind: String::new(),
                    export_path: String::new(),
                }));
            }
        };

        let updated = self.sessions.update_vm_id(
            &req.volume_id,
            &req.attachment_handle,
            Some(req.vm_id.clone()),
            "attached".to_string(),
        );

        if !updated {
            if let Err(e) = self
                .backend
                .detach(&req.volume_id, &req.attachment_handle, &req.vm_id, false)
                .await
            {
                tracing::warn!(
                    volume_id = %req.volume_id,
                    attachment_handle = %req.attachment_handle,
                    vm_id = %req.vm_id,
                    error = %e,
                    "rollback detach failed after concurrent session removal"
                );
            }
            let e = ChvError::NotFound {
                resource: "session".to_string(),
                id: format!("{}/{}", req.volume_id, req.attachment_handle),
            };
            return Ok(Response::new(proto::AttachVolumeToVmResponse {
                result: Some(e.to_proto_result()),
                volume_id: req.volume_id,
                vm_id: req.vm_id,
                export_kind: String::new(),
                export_path: String::new(),
            }));
        }

        if let Some(session) = self.sessions.get(&req.volume_id, &req.attachment_handle) {
            self.persist_upsert(&session).await;
        }

        Ok(Response::new(proto::AttachVolumeToVmResponse {
            result: Some(Self::ok_result()),
            volume_id: req.volume_id,
            vm_id: req.vm_id,
            export_kind: export.export_kind,
            export_path: export.export_path,
        }))
    }

    async fn detach_volume_from_vm(
        &self,
        request: Request<proto::DetachVolumeFromVmRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics.increment_counter("stord_detach_volume_total");
        let req = request.into_inner();
        let span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));
        let _enter = span.enter();

        // TODO: add a secondary index or find_by_volume_and_vm method if this becomes a hot path.
        let sessions = self.sessions.list();
        let session = sessions
            .into_iter()
            .find(|s| s.volume_id == req.volume_id && s.vm_id.as_deref() == Some(&req.vm_id));

        if let Some(s) = session {
            if let Err(e) = self
                .backend
                .detach(&req.volume_id, &s.attachment_handle, &req.vm_id, req.force)
                .await
            {
                if !req.force {
                    return Ok(Response::new(e.to_proto_result()));
                }
                tracing::warn!(
                    volume_id = %req.volume_id,
                    vm_id = %req.vm_id,
                    error = %e,
                    "force detach swallowed backend error"
                );
            }

            let updated = self.sessions.update_vm_id(
                &req.volume_id,
                &s.attachment_handle,
                None,
                "open".to_string(),
            );

            if !updated {
                tracing::warn!(
                    volume_id = %req.volume_id,
                    vm_id = %req.vm_id,
                    "concurrent session removal detected during detach"
                );
            }

            if let Some(session) = self.sessions.get(&req.volume_id, &s.attachment_handle) {
                self.persist_upsert(&session).await;
            }
        }

        Ok(Response::new(Self::ok_result()))
    }

    async fn resize_volume(
        &self,
        request: Request<proto::ResizeVolumeRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics.increment_counter("stord_resize_volume_total");
        let req = request.into_inner();
        let span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));
        let _enter = span.enter();

        let sessions = self.sessions.list();
        let session = sessions.into_iter().find(|s| s.volume_id == req.volume_id);

        let Some(s) = session else {
            let e = ChvError::NotFound {
                resource: "session".to_string(),
                id: req.volume_id.clone(),
            };
            return Ok(Response::new(e.to_proto_result()));
        };

        if let Err(e) = self
            .backend
            .resize(&s.volume_id, &s.attachment_handle, req.new_size_bytes)
            .await
        {
            return Ok(Response::new(e.to_proto_result()));
        }

        Ok(Response::new(Self::ok_result()))
    }

    async fn prepare_snapshot(
        &self,
        request: Request<proto::PrepareSnapshotRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics
            .increment_counter("stord_prepare_snapshot_total");
        let req = request.into_inner();
        let span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));
        let _enter = span.enter();

        let sessions = self.sessions.list();
        let session = sessions.into_iter().find(|s| s.volume_id == req.volume_id);

        let Some(s) = session else {
            let e = ChvError::NotFound {
                resource: "session".to_string(),
                id: req.volume_id.clone(),
            };
            return Ok(Response::new(e.to_proto_result()));
        };

        if let Err(e) = self
            .backend
            .prepare_snapshot(&s.volume_id, &s.attachment_handle, &req.snapshot_name)
            .await
        {
            return Ok(Response::new(e.to_proto_result()));
        }

        Ok(Response::new(Self::ok_result()))
    }

    async fn prepare_clone(
        &self,
        request: Request<proto::PrepareCloneRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics.increment_counter("stord_prepare_clone_total");
        let req = request.into_inner();
        let span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));
        let _enter = span.enter();

        let sessions = self.sessions.list();
        let session = sessions.into_iter().find(|s| s.volume_id == req.volume_id);

        let Some(s) = session else {
            let e = ChvError::NotFound {
                resource: "session".to_string(),
                id: req.volume_id.clone(),
            };
            return Ok(Response::new(e.to_proto_result()));
        };

        if let Err(e) = self
            .backend
            .prepare_clone(&s.volume_id, &s.attachment_handle, &req.clone_name)
            .await
        {
            return Ok(Response::new(e.to_proto_result()));
        }

        Ok(Response::new(Self::ok_result()))
    }

    async fn restore_snapshot(
        &self,
        request: Request<proto::RestoreSnapshotRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics
            .increment_counter("stord_restore_snapshot_total");
        let req = request.into_inner();
        let span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));
        let _enter = span.enter();

        let sessions = self.sessions.list();
        let session = sessions.into_iter().find(|s| s.volume_id == req.volume_id);

        let Some(s) = session else {
            let e = ChvError::NotFound {
                resource: "session".to_string(),
                id: req.volume_id.clone(),
            };
            return Ok(Response::new(e.to_proto_result()));
        };

        if let Err(e) = self
            .backend
            .restore_snapshot(&s.volume_id, &s.attachment_handle, &req.snapshot_name)
            .await
        {
            return Ok(Response::new(e.to_proto_result()));
        }

        Ok(Response::new(Self::ok_result()))
    }

    async fn delete_snapshot(
        &self,
        request: Request<proto::DeleteSnapshotRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics
            .increment_counter("stord_delete_snapshot_total");
        let req = request.into_inner();
        let span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));
        let _enter = span.enter();

        let sessions = self.sessions.list();
        let session = sessions.into_iter().find(|s| s.volume_id == req.volume_id);

        let Some(s) = session else {
            let e = ChvError::NotFound {
                resource: "session".to_string(),
                id: req.volume_id.clone(),
            };
            return Ok(Response::new(e.to_proto_result()));
        };

        if let Err(e) = self
            .backend
            .delete_snapshot(&s.volume_id, &s.attachment_handle, &req.snapshot_name)
            .await
        {
            return Ok(Response::new(e.to_proto_result()));
        }

        Ok(Response::new(Self::ok_result()))
    }

    async fn set_device_policy(
        &self,
        request: Request<proto::SetDevicePolicyRequest>,
    ) -> Result<Response<proto::Result>, Status> {
        self.metrics
            .increment_counter("stord_set_device_policy_total");
        let req = request.into_inner();
        let span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));
        let _enter = span.enter();

        let sessions = self.sessions.list();
        let session = sessions.into_iter().find(|s| s.volume_id == req.volume_id);

        let Some(s) = session else {
            let e = ChvError::NotFound {
                resource: "session".to_string(),
                id: req.volume_id.clone(),
            };
            return Ok(Response::new(e.to_proto_result()));
        };

        let policy = Self::map_device_policy(req.policy);

        if let Err(e) = self
            .backend
            .set_device_policy(&s.volume_id, &s.attachment_handle, &policy)
            .await
        {
            return Ok(Response::new(e.to_proto_result()));
        }

        Ok(Response::new(Self::ok_result()))
    }
}
