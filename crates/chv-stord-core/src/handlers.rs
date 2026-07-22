use crate::migration::sender::{start_migration_to_peer, MigrationTlsConfig};
use crate::migration::task::{MigrationPhase, MigrationTask, MigrationTaskTable};
use crate::session::{Session, SessionTable};
use chv_common::types::{BackendLocator, DevicePolicy};
use chv_errors::{ChvError, ErrorCode};
use chv_observability::{operation_span, Metrics};
use chv_stord_api::chv_stord_api as proto;
use chv_stord_backends::StorageBackend;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub(crate) trait ChvErrorProtoExt {
    fn to_proto_result(&self) -> proto::Result;
}

impl ChvErrorProtoExt for ChvError {
    fn to_proto_result(&self) -> proto::Result {
        let (status, error_code, human_summary) = self.to_result_fields();
        proto::Result {
            status: status.to_string(),
            error_code: error_code.to_string(),
            human_summary,
        }
    }
}

pub struct StorageServiceImpl<B: StorageBackend> {
    backend: Arc<B>,
    sessions: Arc<SessionTable>,
    metrics: Arc<Metrics>,
    backend_allowlist: Vec<String>,
    path_allowlist: Vec<std::path::PathBuf>,
    device_allowlist: Vec<String>,
    store: Option<Arc<crate::store::SessionStore>>,
    migration_tasks: Arc<MigrationTaskTable>,
    migration_dest_allowlist: Vec<String>,
    migration_tls: Option<MigrationTlsConfig>,
}

impl<B: StorageBackend> StorageServiceImpl<B> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        backend: Arc<B>,
        sessions: Arc<SessionTable>,
        metrics: Arc<Metrics>,
        backend_allowlist: Vec<String>,
        path_allowlist: Vec<std::path::PathBuf>,
        device_allowlist: Vec<String>,
        migration_dest_allowlist: Vec<String>,
        migration_tls: Option<MigrationTlsConfig>,
    ) -> Self {
        Self {
            backend,
            sessions,
            metrics,
            backend_allowlist,
            path_allowlist,
            device_allowlist,
            store: None,
            migration_tasks: Arc::new(MigrationTaskTable::new()),
            migration_dest_allowlist,
            migration_tls,
        }
    }

    pub fn sessions(&self) -> Arc<SessionTable> {
        self.sessions.clone()
    }

    pub fn set_store(&mut self, store: crate::store::SessionStore) {
        self.store = Some(Arc::new(store));
    }

    async fn persist_upsert(&self, session: &crate::session::Session) {
        if let Some(store) = &self.store {
            if let Err(e) = store.upsert(session).await {
                tracing::error!(error = %e, "failed to persist session to SQLite");
            }
        }
    }

    async fn persist_remove(&self, volume_id: &str, handle: &str) {
        if let Some(store) = &self.store {
            if let Err(e) = store.remove(volume_id, handle).await {
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

    fn check_migration_dest_allowlist(&self, dest_endpoint: &str) -> Result<(), ChvError> {
        if self.migration_dest_allowlist.is_empty() {
            return Ok(());
        }
        // Extract host from endpoint, e.g. "https://host:50052" -> "host"
        let host = dest_endpoint
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .split(':')
            .next()
            .unwrap_or(dest_endpoint);
        if self.migration_dest_allowlist.iter().any(|h| h == host) {
            return Ok(());
        }
        Err(ChvError::BackendUnavailable {
            backend: "migration".to_string(),
            reason: format!("migration destination '{}' not in allowlist", dest_endpoint),
        })
    }

    fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
        let mut result = std::path::PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                    result.push(component);
                }
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    result.pop();
                }
                std::path::Component::Normal(c) => {
                    result.push(c);
                }
            }
        }
        result
    }

    fn check_path_allowlist(&self, path: &str) -> Result<(), ChvError> {
        if self.path_allowlist.is_empty() {
            return Ok(());
        }
        let path = std::path::Path::new(path);
        if path.is_relative() {
            // Relative paths are resolved by the backend against runtime_dir;
            // skip path allowlist for relative locators.
            return Ok(());
        }
        let normalized = Self::normalize_path(path);
        for allowed in &self.path_allowlist {
            if normalized.starts_with(allowed) {
                return Ok(());
            }
        }
        Err(ChvError::AccessDenied {
            resource: path.to_string_lossy().to_string(),
            reason: format!(
                "path '{}' not within allowed prefixes: {:?}",
                path.display(),
                self.path_allowlist
            ),
        })
    }

    fn matches_device_pattern(path: &str, pattern: &str) -> bool {
        // Simple glob matching: only '*' wildcard is supported.
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.is_empty() {
            return true;
        }
        if !path.starts_with(parts[0]) {
            return false;
        }
        let mut rest = &path[parts[0].len()..];
        for &part in &parts[1..] {
            if part.is_empty() {
                continue;
            }
            match rest.find(part) {
                Some(i) => rest = &rest[i + part.len()..],
                None => return false,
            }
        }
        if !pattern.ends_with('*') && !rest.is_empty() {
            return false;
        }
        true
    }

    fn check_device_allowlist(&self, path: &str) -> Result<(), ChvError> {
        if self.device_allowlist.is_empty() {
            return Ok(());
        }
        for pattern in &self.device_allowlist {
            if Self::matches_device_pattern(path, pattern) {
                return Ok(());
            }
        }
        Err(ChvError::AccessDenied {
            resource: path.to_string(),
            reason: format!(
                "device path '{}' not in device_allowlist: {:?}",
                path, self.device_allowlist
            ),
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

        if req.volume_id.is_empty() {
            return Ok(Response::new(proto::OpenVolumeResponse {
                result: Some(
                    ChvError::InvalidArgument {
                        field: "volume_id".to_string(),
                        reason: "volume_id must not be empty".to_string(),
                    }
                    .to_proto_result(),
                ),
                volume_id: req.volume_id,
                attachment_handle: String::new(),
                export_kind: String::new(),
                export_path: String::new(),
            }));
        }

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

        if let Err(e) = self.check_path_allowlist(&locator.locator) {
            return Ok(Response::new(proto::OpenVolumeResponse {
                result: Some(e.to_proto_result()),
                volume_id: req.volume_id,
                attachment_handle: String::new(),
                export_kind: String::new(),
                export_path: String::new(),
            }));
        }

        if locator.backend_class == "lvm" || locator.backend_class == "block" {
            if let Err(e) = self.check_device_allowlist(&locator.locator) {
                return Ok(Response::new(proto::OpenVolumeResponse {
                    result: Some(e.to_proto_result()),
                    volume_id: req.volume_id,
                    attachment_handle: String::new(),
                    export_kind: String::new(),
                    export_path: String::new(),
                }));
            }
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
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

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
                .detach(
                    &req.volume_id,
                    &req.attachment_handle,
                    chv_common::AttachmentOwnership {
                        vm_id: req.vm_id.clone(),
                        operation_id: req.meta.as_ref().map(|m| m.operation_id.clone()),
                        requester: None,
                    },
                    false,
                )
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
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

        // TODO: add a secondary index or find_by_volume_and_vm method if this becomes a hot path.
        let sessions = self.sessions.list();
        let session = sessions
            .into_iter()
            .find(|s| s.volume_id == req.volume_id && s.vm_id.as_deref() == Some(&req.vm_id));

        if let Some(s) = session {
            if let Err(e) = self
                .backend
                .detach(
                    &req.volume_id,
                    &s.attachment_handle,
                    chv_common::AttachmentOwnership {
                        vm_id: req.vm_id.clone(),
                        operation_id: req.meta.as_ref().map(|m| m.operation_id.clone()),
                        requester: None,
                    },
                    req.force,
                )
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
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

        if req.new_size_bytes == 0 {
            return Ok(Response::new(
                ChvError::InvalidArgument {
                    field: "new_size_bytes".to_string(),
                    reason: "new_size_bytes must be > 0".to_string(),
                }
                .to_proto_result(),
            ));
        }

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
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

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
            .prepare_snapshot(
                &s.volume_id,
                &s.attachment_handle,
                chv_common::AttachmentOwnership {
                    vm_id: s.vm_id.clone().unwrap_or_default(),
                    operation_id: req.meta.as_ref().map(|m| m.operation_id.clone()),
                    requester: None,
                },
                &req.snapshot_name,
            )
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
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

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
            .prepare_clone(
                &s.volume_id,
                &s.attachment_handle,
                chv_common::AttachmentOwnership {
                    vm_id: s.vm_id.clone().unwrap_or_default(),
                    operation_id: req.meta.as_ref().map(|m| m.operation_id.clone()),
                    requester: None,
                },
                &req.clone_name,
            )
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
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

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
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

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
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

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

    async fn trigger_disk_migration(
        &self,
        request: Request<proto::TriggerDiskMigrationRequest>,
    ) -> Result<Response<proto::TriggerDiskMigrationResponse>, Status> {
        self.metrics
            .increment_counter("stord_trigger_disk_migration_total");
        let req = request.into_inner();
        let _span = req
            .meta
            .as_ref()
            .map(|m| operation_span(&m.operation_id))
            .unwrap_or_else(|| operation_span(""));

        if req.volume_id.is_empty() {
            return Ok(Response::new(proto::TriggerDiskMigrationResponse {
                result: Some(
                    ChvError::InvalidArgument {
                        field: "volume_id".to_string(),
                        reason: "volume_id must not be empty".to_string(),
                    }
                    .to_proto_result(),
                ),
                migration_id: String::new(),
            }));
        }

        if let Err(e) = self.check_migration_dest_allowlist(&req.dest_endpoint) {
            return Ok(Response::new(proto::TriggerDiskMigrationResponse {
                result: Some(e.to_proto_result()),
                migration_id: String::new(),
            }));
        }

        // Find the session for this volume+handle
        let session = self.sessions.get(&req.volume_id, &req.attachment_handle);
        let Some(session) = session else {
            return Ok(Response::new(proto::TriggerDiskMigrationResponse {
                result: Some(
                    ChvError::NotFound {
                        resource: "session".to_string(),
                        id: format!("{}/{}", req.volume_id, req.attachment_handle),
                    }
                    .to_proto_result(),
                ),
                migration_id: String::new(),
            }));
        };

        // Verify volume health before starting migration
        match self
            .backend
            .health(&session.volume_id, &session.attachment_handle)
            .await
        {
            Ok(h) if h.status == "error" => {
                return Ok(Response::new(proto::TriggerDiskMigrationResponse {
                    result: Some(
                        ChvError::BackendUnavailable {
                            backend: "stord".to_string(),
                            reason: format!(
                                "volume {} is unhealthy: {}",
                                req.volume_id, h.last_error
                            ),
                        }
                        .to_proto_result(),
                    ),
                    migration_id: String::new(),
                }));
            }
            Err(e) => {
                return Ok(Response::new(proto::TriggerDiskMigrationResponse {
                    result: Some(e.to_proto_result()),
                    migration_id: String::new(),
                }));
            }
            _ => {}
        }

        let migration_id = format!(
            "dm-{}-{}",
            req.volume_id,
            uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("x")
        );

        let (task, _pause_rx) = MigrationTask::new(
            req.volume_id.clone(),
            req.attachment_handle.clone(),
            req.dest_endpoint.clone(),
        );

        // Update initial total_bytes from volume size
        let volume_size = match self
            .backend
            .volume_size(&req.volume_id, &req.attachment_handle)
            .await
        {
            Ok(size) => size,
            Err(e) => {
                return Ok(Response::new(proto::TriggerDiskMigrationResponse {
                    result: Some(e.to_proto_result()),
                    migration_id: String::new(),
                }));
            }
        };
        {
            let mut state = task.state.write().await;
            state.total_bytes = volume_size;
        }

        self.migration_tasks
            .insert(migration_id.clone(), task.clone());

        let backend = self.backend.clone();
        let tls_config = self.migration_tls.clone();
        let endpoint = req.dest_endpoint.clone();
        let volume_id = req.volume_id.clone();
        let handle = req.attachment_handle.clone();
        let tasks = self.migration_tasks.clone();
        let mig_id = migration_id.clone();

        tokio::spawn(async move {
            tracing::info!(
                migration_id = %mig_id,
                volume_id = %volume_id,
                dest_endpoint = %endpoint,
                "starting disk migration background task"
            );

            let task_clone = task.clone();
            let result = start_migration_to_peer(
                endpoint,
                volume_id.clone(),
                handle,
                backend,
                tls_config,
                Some(task_clone),
            )
            .await;

            if let Err(ref e) = result {
                tracing::error!(
                    migration_id = %mig_id,
                    volume_id = %volume_id,
                    error = %e,
                    "disk migration background task failed"
                );
                task.mark_failed(e.to_string());
            } else {
                tracing::info!(
                    migration_id = %mig_id,
                    volume_id = %volume_id,
                    "disk migration background task completed"
                );
            }

            // Keep the task record for a short while so status queries can read it.
            // A future enhancement could expire old entries.
            let _ = tasks;
        });

        Ok(Response::new(proto::TriggerDiskMigrationResponse {
            result: Some(Self::ok_result()),
            migration_id,
        }))
    }

    async fn get_disk_migration_status(
        &self,
        request: Request<proto::GetDiskMigrationStatusRequest>,
    ) -> Result<Response<proto::GetDiskMigrationStatusResponse>, Status> {
        let req = request.into_inner();
        let task = self.migration_tasks.get(&req.migration_id);

        let Some(task) = task else {
            return Ok(Response::new(proto::GetDiskMigrationStatusResponse {
                result: Some(
                    ChvError::NotFound {
                        resource: "migration".to_string(),
                        id: req.migration_id.clone(),
                    }
                    .to_proto_result(),
                ),
                phase: proto::get_disk_migration_status_response::Phase::Pending as i32,
                convergence_round: 0,
                dirty_blocks_remaining: 0,
                bytes_transferred: 0,
                total_bytes: 0,
                needs_vm_pause: false,
                error_message: "migration not found".to_string(),
            }));
        };

        let state = task.state.read().await;
        let phase = match state.phase {
            MigrationPhase::Pending => proto::get_disk_migration_status_response::Phase::Pending,
            MigrationPhase::BulkCopy => proto::get_disk_migration_status_response::Phase::BulkCopy,
            MigrationPhase::DirtySync => {
                proto::get_disk_migration_status_response::Phase::DirtySync
            }
            MigrationPhase::PausedFinalSync => {
                proto::get_disk_migration_status_response::Phase::PausedFinalSync
            }
            MigrationPhase::Completed => {
                proto::get_disk_migration_status_response::Phase::Completed
            }
            MigrationPhase::Failed => proto::get_disk_migration_status_response::Phase::Failed,
        };

        Ok(Response::new(proto::GetDiskMigrationStatusResponse {
            result: Some(Self::ok_result()),
            phase: phase as i32,
            convergence_round: state.convergence_round,
            dirty_blocks_remaining: state.dirty_blocks_remaining,
            bytes_transferred: state.bytes_transferred,
            total_bytes: state.total_bytes,
            needs_vm_pause: state.needs_vm_pause,
            error_message: state.error_message.clone(),
        }))
    }

    async fn resume_disk_migration(
        &self,
        request: Request<proto::ResumeDiskMigrationRequest>,
    ) -> Result<Response<proto::ResumeDiskMigrationResponse>, Status> {
        let req = request.into_inner();
        let task = self.migration_tasks.get(&req.migration_id);

        let Some(task) = task else {
            return Ok(Response::new(proto::ResumeDiskMigrationResponse {
                result: Some(
                    ChvError::NotFound {
                        resource: "migration".to_string(),
                        id: req.migration_id.clone(),
                    }
                    .to_proto_result(),
                ),
            }));
        };

        if req.vm_paused {
            if let Err(e) = task.pause_tx.send(true) {
                tracing::warn!(
                    migration_id = %req.migration_id,
                    error = %e,
                    "failed to send vm_paused signal to migration task"
                );
                return Ok(Response::new(proto::ResumeDiskMigrationResponse {
                    result: Some(
                        ChvError::Internal {
                            reason: format!("failed to resume migration: {e}"),
                        }
                        .to_proto_result(),
                    ),
                }));
            }
            tracing::info!(
                migration_id = %req.migration_id,
                "sent vm_paused signal to migration task"
            );
        }

        Ok(Response::new(proto::ResumeDiskMigrationResponse {
            result: Some(Self::ok_result()),
        }))
    }
}
