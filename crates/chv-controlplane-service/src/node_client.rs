use chv_errors::ChvError;
use control_plane_node_api::control_plane_node_api as proto;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UnixStream;
use tokio::time::timeout;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;
use tracing::Instrument;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

struct MethodCircuit {
    state: CircuitState,
    failures: Vec<Instant>,
    opened_at: Option<Instant>,
}

pub struct CircuitBreaker {
    inner: Mutex<HashMap<String, MethodCircuit>>,
    failure_threshold: usize,
    failure_window: Duration,
    open_duration: Duration,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            failure_threshold: 5,
            failure_window: Duration::from_secs(30),
            open_duration: Duration::from_secs(30),
        }
    }

    pub fn check(&self, method: &str) -> Result<(), ChvError> {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();
        let entry = inner.entry(method.to_string()).or_insert_with(|| MethodCircuit {
            state: CircuitState::Closed,
            failures: Vec::new(),
            opened_at: None,
        });

        match entry.state {
            CircuitState::Closed => Ok(()),
            CircuitState::Open => {
                if let Some(opened_at) = entry.opened_at {
                    if now.duration_since(opened_at) >= self.open_duration {
                        entry.state = CircuitState::HalfOpen;
                        entry.opened_at = None;
                        Ok(())
                    } else {
                        Err(ChvError::BackendUnavailable {
                            backend: "agent".to_string(),
                            reason: format!("circuit breaker open for {method}"),
                        })
                    }
                } else {
                    entry.state = CircuitState::HalfOpen;
                    Ok(())
                }
            }
            CircuitState::HalfOpen => Ok(()),
        }
    }

    pub fn record_success(&self, method: &str) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(entry) = inner.get_mut(method) {
            entry.state = CircuitState::Closed;
            entry.failures.clear();
            entry.opened_at = None;
        }
    }

    pub fn record_failure(&self, method: &str) {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();
        let entry = inner.entry(method.to_string()).or_insert_with(|| MethodCircuit {
            state: CircuitState::Closed,
            failures: Vec::new(),
            opened_at: None,
        });

        match entry.state {
            CircuitState::HalfOpen => {
                entry.state = CircuitState::Open;
                entry.opened_at = Some(now);
                metrics::counter!(
                    "chv_node_client_circuit_breaker_trips_total",
                    "method" => method.to_string(),
                )
                .increment(1);
            }
            CircuitState::Closed => {
                entry.failures.retain(|&t| now.duration_since(t) < self.failure_window);
                entry.failures.push(now);
                if entry.failures.len() >= self.failure_threshold {
                    entry.state = CircuitState::Open;
                    entry.opened_at = Some(now);
                    entry.failures.clear();
                    metrics::counter!(
                        "chv_node_client_circuit_breaker_trips_total",
                        "method" => method.to_string(),
                    )
                    .increment(1);
                }
            }
            CircuitState::Open => {}
        }
    }
}

async fn with_timeout<F, T>(
    future: F,
    backend: &str,
    method: &str,
) -> Result<T, ChvError>
where
    F: std::future::Future<Output = Result<tonic::Response<T>, tonic::Status>>,
{
    timeout(Duration::from_secs(30), future)
        .await
        .map_err(|_| ChvError::BackendUnavailable {
            backend: backend.to_string(),
            reason: format!("{method} timed out after 30s"),
        })?
        .map_err(|e| ChvError::BackendUnavailable {
            backend: backend.to_string(),
            reason: format!("{method} failed: {e}"),
        })
        .map(|r| r.into_inner())
}

#[derive(Clone)]
pub struct NodeClient {
    reconcile: proto::reconcile_service_client::ReconcileServiceClient<Channel>,
    lifecycle: proto::lifecycle_service_client::LifecycleServiceClient<Channel>,
    circuit_breaker: Arc<CircuitBreaker>,
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
            circuit_breaker: Arc::new(CircuitBreaker::new()),
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
        let method = "apply_vm_desired_state";
        let span = tracing::info_span!("apply_vm_desired_state", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.reconcile
                .apply_vm_desired_state(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "apply_volume_desired_state";
        let span = tracing::info_span!("apply_volume_desired_state", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.reconcile
                .apply_volume_desired_state(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "apply_network_desired_state";
        let span = tracing::info_span!("apply_network_desired_state", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.reconcile
                .apply_network_desired_state(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "create_vm";
        let span = tracing::info_span!("create_vm", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .create_vm(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "start_vm";
        let span = tracing::info_span!("start_vm", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .start_vm(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "stop_vm";
        let span = tracing::info_span!("stop_vm", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .stop_vm(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "reboot_vm";
        let span = tracing::info_span!("reboot_vm", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .reboot_vm(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "delete_vm";
        let span = tracing::info_span!("delete_vm", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .delete_vm(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "snapshot_vm";
        let span = tracing::info_span!("snapshot_vm", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .snapshot_vm(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "restore_snapshot";
        let span = tracing::info_span!("restore_snapshot", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .restore_snapshot(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "attach_volume";
        let span = tracing::info_span!("attach_volume", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .attach_volume(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "detach_volume";
        let span = tracing::info_span!("detach_volume", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .detach_volume(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "resize_volume";
        let span = tracing::info_span!("resize_volume", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .resize_volume(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "snapshot_volume";
        let span = tracing::info_span!("snapshot_volume", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .snapshot_volume(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "restore_volume";
        let span = tracing::info_span!("restore_volume", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .restore_volume(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "delete_volume_snapshot";
        let span = tracing::info_span!("delete_volume_snapshot", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .delete_volume_snapshot(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "clone_volume";
        let span = tracing::info_span!("clone_volume", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .clone_volume(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "start_network";
        let span = tracing::info_span!("start_network", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .start_network(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "stop_network";
        let span = tracing::info_span!("stop_network", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .stop_network(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
        let method = "restart_network";
        let span = tracing::info_span!("restart_network", operation_id);
        self.circuit_breaker.check(method)?;
        let result = with_timeout(
            self.lifecycle
                .restart_network(with_operation_id_metadata(req, operation_id))
                .instrument(span),
            "agent",
            method,
        )
        .await;
        match &result {
            Ok(_) => self.circuit_breaker.record_success(method),
            Err(_) => self.circuit_breaker.record_failure(method),
        };
        result
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
