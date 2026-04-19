use chv_agent_core::{
    agent_server::AgentServer,
    cache::{NodeCache, PendingControlPlaneMessage},
    config::{load_agent_config, AgentConfig},
    console_server::ConsoleServer,
    control_plane::ControlPlaneClient,
    daemon_clients::{NwdClient, StordClient},
    enrollment::EnrollmentClient,
    health::HealthAggregator,
    inventory::InventoryReporter,
    reconcile::Reconciler,
    state_machine::NodeState,
    supervisor::DaemonSupervisor,
    telemetry::TelemetryReporter,
    vm_runtime::VmRuntime,
};
use chv_agent_runtime_ch::process::ProcessCloudHypervisorAdapter;
use chv_errors::ChvError;
use chv_observability::init_logger;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

const FAILED_THRESHOLD: u32 = 6; // 6 ticks * 5s = 30s
const CERT_ROTATION_INTERVAL_SECS: i64 = 12 * 60 * 60;

fn initial_node_id(config: &AgentConfig) -> String {
    if config.node_id.is_empty() {
        "unknown".to_string()
    } else {
        config.node_id.clone()
    }
}

async fn load_or_initialize_cache(config: &AgentConfig) -> NodeCache {
    match NodeCache::load(&config.cache_path).await {
        Ok(cache) => {
            info!(
                node_id = %cache.node_id,
                node_state = %cache.node_state,
                "loaded cache"
            );
            cache
        }
        Err(chv_errors::ChvError::NotFound { .. }) => NodeCache::new(initial_node_id(config)),
        Err(e) => {
            warn!(error = %e, "failed to load cache, starting fresh");
            NodeCache::new(initial_node_id(config))
        }
    }
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

async fn resolve_tls_paths(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    config: &AgentConfig,
) -> (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) {
    let cache = cache.lock().await;
    let tls_cert = cache
        .certificate_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| config.tls_cert_path.clone());
    let tls_key = cache
        .private_key_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| config.tls_key_path.clone());
    let ca_cert = cache
        .ca_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| config.ca_cert_path.clone());
    (tls_cert, tls_key, ca_cert)
}

async fn connect_control_plane(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    config: &AgentConfig,
) -> Result<ControlPlaneClient, ChvError> {
    let (tls_cert, tls_key, ca_cert) = resolve_tls_paths(cache, config).await;
    ControlPlaneClient::new(
        &config.control_plane_addr,
        tls_cert.as_deref(),
        tls_key.as_deref(),
        ca_cert.as_deref(),
    )
    .await
}

async fn connect_enrollment_client(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    config: &AgentConfig,
) -> Result<EnrollmentClient, ChvError> {
    let (tls_cert, tls_key, ca_cert) = resolve_tls_paths(cache, config).await;
    EnrollmentClient::connect_with_tls(
        &config.control_plane_addr,
        tls_cert.as_deref(),
        tls_key.as_deref(),
        ca_cert.as_deref(),
    )
    .await
}

async fn enqueue_pending_message(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    cache_path: &Path,
    message: PendingControlPlaneMessage,
) {
    let mut cache = cache.lock().await;
    cache.enqueue_pending_message(message);
    if let Err(e) = cache.save(cache_path).await {
        warn!(error = %e, "failed to save cache after queueing deferred control-plane message");
    }
}

async fn flush_pending_messages(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    cache_path: &Path,
    client: &mut ControlPlaneClient,
) -> Result<(), ChvError> {
    let had_pending = {
        let cache = cache.lock().await;
        !cache.pending_control_plane_messages().is_empty()
    };
    if !had_pending {
        return Ok(());
    }

    let mut cache = cache.lock().await;
    client.flush_pending_messages(&mut cache).await?;
    cache.save(cache_path).await?;
    Ok(())
}

async fn send_or_defer_control_plane_message(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    cache_path: &Path,
    config: &AgentConfig,
    telemetry: &mut Option<ControlPlaneClient>,
    message: PendingControlPlaneMessage,
) {
    if telemetry.is_none() {
        match connect_control_plane(cache, config).await {
            Ok(mut client) => {
                info!("connected to control plane");
                if let Err(e) = flush_pending_messages(cache, cache_path, &mut client).await {
                    warn!(error = %e, "failed to flush deferred control-plane messages");
                    *telemetry = None;
                    enqueue_pending_message(cache, cache_path, message).await;
                    return;
                }
                *telemetry = Some(client);
            }
            Err(e) => {
                warn!(error = %e, "control plane unavailable; deferring report");
                enqueue_pending_message(cache, cache_path, message).await;
                return;
            }
        }
    }

    let Some(client) = telemetry.as_mut() else {
        enqueue_pending_message(cache, cache_path, message).await;
        return;
    };

    if let Err(e) = client.dispatch_pending_message(&message).await {
        warn!(error = %e, "failed to send control-plane message, deferring");
        *telemetry = None;
        enqueue_pending_message(cache, cache_path, message).await;
    }
}

fn certificate_rotation_due(cache: &NodeCache, now_unix_ms: i64) -> bool {
    if !cache.enrollment_complete {
        return false;
    }
    let (cert_path, key_path) = match (
        cache.certificate_path.as_ref(),
        cache.private_key_path.as_ref(),
    ) {
        (Some(c), Some(k)) => (c, k),
        _ => return false,
    };
    if !std::path::Path::new(cert_path).exists() || !std::path::Path::new(key_path).exists() {
        return false;
    }
    cache
        .last_certificate_rotation_unix_ms
        .map(|last| now_unix_ms - last >= CERT_ROTATION_INTERVAL_SECS * 1000)
        .unwrap_or(true)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls ring crypto provider");

    let config_path = std::env::args().nth(1).map(PathBuf::from);
    let config = load_agent_config(config_path.as_deref())?;

    init_logger(&config.log_level)?;

    info!("chv-agent starting");

    let mut cache = load_or_initialize_cache(&config).await;

    // Enrollment
    if !cache.enrollment_complete {
        if let Some(token_path) = &config.bootstrap_token_path {
            match tokio::fs::read_to_string(token_path).await {
                Ok(token) => {
                    let token = token.trim();
                    let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname")
                        .unwrap_or_else(|_| "unknown".to_string())
                        .trim()
                        .to_string();
                    let reporter = InventoryReporter::with_storage_base_dir(
                        &cache.node_id,
                        &hostname,
                        &config.storage_base_dir,
                    );
                    let inventory = reporter.build_inventory();
                    let versions = reporter.build_versions();
                    match EnrollmentClient::connect(&config.control_plane_addr).await {
                        Ok(mut client) => {
                            match client.enroll_node(token, inventory, versions).await {
                                Ok(resp) => {
                                    let cert_path = config.runtime_dir.join("agent.crt");
                                    let key_path = config.runtime_dir.join("agent.key");
                                    let ca_path = config.runtime_dir.join("ca.crt");
                                    if let Err(e) =
                                        tokio::fs::create_dir_all(&config.runtime_dir).await
                                    {
                                        warn!(error = %e, "failed to create runtime dir");
                                    } else {
                                        let mut ok = true;
                                        if let Err(e) =
                                            tokio::fs::write(&cert_path, &resp.certificate_pem)
                                                .await
                                        {
                                            warn!(error = %e, "failed to write certificate");
                                            ok = false;
                                        }
                                        if let Err(e) =
                                            tokio::fs::write(&key_path, &resp.private_key_pem).await
                                        {
                                            warn!(error = %e, "failed to write private key");
                                            ok = false;
                                        }
                                        if let Err(e) =
                                            tokio::fs::write(&ca_path, &resp.ca_pem).await
                                        {
                                            warn!(error = %e, "failed to write ca certificate");
                                            ok = false;
                                        }
                                        if ok {
                                            cache.node_id = resp.node_id.clone();
                                            cache.certificate_path =
                                                Some(cert_path.to_string_lossy().to_string());
                                            cache.private_key_path =
                                                Some(key_path.to_string_lossy().to_string());
                                            cache.ca_path =
                                                Some(ca_path.to_string_lossy().to_string());
                                            cache.last_certificate_rotation_unix_ms =
                                                Some(now_unix_ms());
                                            cache.enrollment_complete = true;
                                            if let Err(e) = cache.save(&config.cache_path).await {
                                                warn!(error = %e, "failed to save cache after enrollment");
                                            } else {
                                                info!(node_id = %resp.node_id, "enrollment complete");
                                            }

                                            let meta =
                                                control_plane_node_api::control_plane_node_api::RequestMeta {
                                                    operation_id: format!(
                                                        "bootstrap-result-{}",
                                                        resp.node_id
                                                    ),
                                                    requested_by: "agent".to_string(),
                                                    target_node_id: resp.node_id.clone(),
                                                    desired_state_version: String::new(),
                                                    request_unix_ms: now_unix_ms(),
                                                };
                                            if let Err(e) = client
                                                .report_bootstrap_result(
                                                    meta,
                                                    &resp.node_id,
                                                    "ok",
                                                    "bootstrap complete",
                                                )
                                                .await
                                            {
                                                warn!(error = %e, "failed to report bootstrap result");
                                            }
                                        } else {
                                            let meta =
                                                control_plane_node_api::control_plane_node_api::RequestMeta {
                                                    operation_id: format!(
                                                        "bootstrap-result-{}",
                                                        resp.node_id
                                                    ),
                                                    requested_by: "agent".to_string(),
                                                    target_node_id: resp.node_id.clone(),
                                                    desired_state_version: String::new(),
                                                    request_unix_ms: now_unix_ms(),
                                                };
                                            if let Err(e) = client
                                                .report_bootstrap_result(
                                                    meta,
                                                    &resp.node_id,
                                                    "failed",
                                                    "failed to persist enrollment material",
                                                )
                                                .await
                                            {
                                                warn!(error = %e, "failed to report bootstrap result");
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(error = %e, "enrollment failed");
                                }
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "failed to connect to enrollment endpoint");
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, path = %token_path.display(), "failed to read bootstrap token");
                }
            }
        }
    }

    let adapter: Arc<dyn chv_agent_runtime_ch::adapter::CloudHypervisorAdapter> =
        Arc::new(ProcessCloudHypervisorAdapter::new(&config.chv_binary_path));
    let vm_runtime = VmRuntime::new(adapter);

    let cache = Arc::new(tokio::sync::Mutex::new(cache));

    let agent_server = AgentServer::new(
        cache.clone(),
        vm_runtime.clone(),
        config.stord_socket.clone(),
        config.nwd_socket.clone(),
        Some(config.cache_path.clone()),
        config.runtime_dir.clone(),
    );
    let server_socket = config.socket_path.clone();
    tokio::spawn(async move {
        if let Err(e) = agent_server.serve(&server_socket).await {
            warn!(error = %e, "agent server exited");
        }
    });

    let console_bind = config.console_bind.clone();
    let console_server = ConsoleServer::new(vm_runtime.clone(), config.jwt_secret.clone());
    tokio::spawn(async move {
        if let Err(e) = console_server.run(&console_bind).await {
            warn!(error = %e, bind = %console_bind, "console server exited");
        }
    });

    let mut reconciler = Reconciler::new(
        cache.clone(),
        vm_runtime.clone(),
        config.stord_socket.clone(),
        config.nwd_socket.clone(),
        config.runtime_dir.clone(),
    )
    .await;

    let mut supervisor = DaemonSupervisor::new(
        config.stord_binary_path.clone(),
        config.nwd_binary_path.clone(),
        config.stord_socket.clone(),
        config.nwd_socket.clone(),
        config.runtime_dir.clone(),
    );

    if let Err(e) = supervisor.start_all().await {
        warn!(error = %e, "failed to start local daemons during bootstrap");
    }

    let mut telemetry = match connect_control_plane(&cache, &config).await {
        Ok(client) => {
            info!("connected to control plane");
            Some(client)
        }
        Err(e) => {
            warn!(error = %e, "control plane unavailable; will retry later");
            None
        }
    };

    let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string();
    let node_id = cache.lock().await.node_id.clone();
    let inventory_reporter =
        InventoryReporter::with_storage_base_dir(&node_id, hostname, &config.storage_base_dir);
    let mut tick_count = 0u64;
    let mut consecutive_health_failures = 0u32;

    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;

        if let Err(e) = supervisor.restart_if_needed().await {
            warn!(error = %e, "supervisor restart failed");
        }

        let now = now_unix_ms();
        let rotate_due = {
            let cache = cache.lock().await;
            certificate_rotation_due(&cache, now)
        };
        if rotate_due {
            match connect_enrollment_client(&cache, &config).await {
                Ok(mut client) => {
                    let node_id = cache.lock().await.node_id.clone();
                    let meta = control_plane_node_api::control_plane_node_api::RequestMeta {
                        operation_id: format!("cert-rotate-{}", tick_count),
                        requested_by: "agent".to_string(),
                        target_node_id: node_id.clone(),
                        desired_state_version: String::new(),
                        request_unix_ms: now,
                    };
                    match client.rotate_node_certificate(meta, &node_id).await {
                        Ok(resp) => {
                            let (cert_path, key_path, ca_path) = {
                                let cache = cache.lock().await;
                                (
                                    cache.certificate_path.clone(),
                                    cache.private_key_path.clone(),
                                    cache.ca_path.clone(),
                                )
                            };
                            if let (Some(cert_path), Some(key_path), Some(ca_path)) =
                                (cert_path, key_path, ca_path)
                            {
                                let write_result = async {
                                    tokio::fs::write(&cert_path, &resp.certificate_pem).await?;
                                    tokio::fs::write(&key_path, &resp.private_key_pem).await?;
                                    tokio::fs::write(&ca_path, &resp.ca_pem).await?;
                                    Ok::<(), std::io::Error>(())
                                }
                                .await;
                                match write_result {
                                    Ok(()) => {
                                        let mut cache = cache.lock().await;
                                        cache.last_certificate_rotation_unix_ms = Some(now);
                                        if let Err(e) = cache.save(&config.cache_path).await {
                                            warn!(error = %e, "failed to save cache after certificate rotation");
                                        }
                                    }
                                    Err(e) => {
                                        warn!(error = %e, "failed to persist rotated certificate material");
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "certificate rotation failed");
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, "failed to connect for certificate rotation");
                }
            }
        }

        let stord_ok = match StordClient::connect(&config.stord_socket).await {
            Ok(mut c) => c.health_probe().await.unwrap_or(false),
            Err(_) => false,
        };

        let nwd_ok = match NwdClient::connect(&config.nwd_socket).await {
            Ok(mut c) => c.health_probe().await.unwrap_or(false),
            Err(_) => false,
        };

        let mut health = HealthAggregator::new();
        health.update_stord(stord_ok);
        health.update_nwd(nwd_ok);

        let current_state = reconciler.current_state().await;
        let derived = health.derive_node_state(current_state);
        if derived != current_state {
            let from_str = current_state.as_str().to_string();
            info!(
                from = %from_str,
                to = %derived.as_str(),
                "state transition"
            );
            if let Err(e) = reconciler.transition_state(derived).await {
                warn!(error = %e, "invalid state transition ignored");
            } else {
                {
                    let cache = cache.lock().await;
                    if let Err(e) = cache.save(&config.cache_path).await {
                        warn!(error = %e, "failed to save cache");
                    }
                }
                if derived != NodeState::Degraded {
                    consecutive_health_failures = 0;
                }
                let severity = match derived {
                    NodeState::Failed => "Critical",
                    NodeState::Degraded => "Warning",
                    NodeState::TenantReady => "Info",
                    _ => "Info",
                };
                let (target_node_id, desired_state_version) = {
                    let cache = cache.lock().await;
                    (cache.node_id.clone(), cache.observed_generation.clone())
                };
                let reporter = TelemetryReporter::new(&target_node_id);
                let event = reporter.event_report(
                    control_plane_node_api::control_plane_node_api::RequestMeta {
                        operation_id: format!("state-transition-{}", tick_count),
                        requested_by: "agent".to_string(),
                        target_node_id,
                        desired_state_version,
                        request_unix_ms: now_unix_ms(),
                    },
                    severity,
                    "StateTransition",
                    &format!(
                        "node transitioned from {} to {}",
                        from_str,
                        derived.as_str()
                    ),
                );
                send_or_defer_control_plane_message(
                    &cache,
                    &config.cache_path,
                    &config,
                    &mut telemetry,
                    PendingControlPlaneMessage::event(event),
                )
                .await;
            }
        } else if current_state == NodeState::Degraded {
            consecutive_health_failures += 1;
            if consecutive_health_failures >= FAILED_THRESHOLD {
                info!("health failures exceeded threshold, transitioning to Failed");
                if let Err(e) = reconciler.transition_state(NodeState::Failed).await {
                    warn!(error = %e, "failed to transition to Failed");
                } else {
                    {
                        let cache = cache.lock().await;
                        if let Err(e) = cache.save(&config.cache_path).await {
                            warn!(error = %e, "failed to save cache");
                        }
                    }
                    let (target_node_id, desired_state_version) = {
                        let cache = cache.lock().await;
                        (cache.node_id.clone(), cache.observed_generation.clone())
                    };
                    let reporter = TelemetryReporter::new(&target_node_id);
                    let event = reporter.event_report(
                        control_plane_node_api::control_plane_node_api::RequestMeta {
                            operation_id: format!("state-transition-failed-{}", tick_count),
                            requested_by: "agent".to_string(),
                            target_node_id,
                            desired_state_version,
                            request_unix_ms: now_unix_ms(),
                        },
                        "Critical",
                        "StateTransition",
                        "node transitioned to Failed after persistent health degradation",
                    );
                    send_or_defer_control_plane_message(
                        &cache,
                        &config.cache_path,
                        &config,
                        &mut telemetry,
                        PendingControlPlaneMessage::event(event),
                    )
                    .await;
                }
                consecutive_health_failures = 0;
            }
        } else {
            consecutive_health_failures = 0;
        }

        let (node_id, node_state, observed_generation, last_error) = {
            let cache = cache.lock().await;
            (
                cache.node_id.clone(),
                cache.node_state.clone(),
                cache.observed_generation.clone(),
                cache.last_error.clone(),
            )
        };
        let reporter = TelemetryReporter::new(&node_id);
        let report = reporter.node_state_report(
            node_state.as_str(),
            observed_generation.as_str(),
            if reconciler.current_state().await == NodeState::TenantReady {
                "Healthy"
            } else {
                "Degraded"
            },
            last_error,
        );
        send_or_defer_control_plane_message(
            &cache,
            &config.cache_path,
            &config,
            &mut telemetry,
            PendingControlPlaneMessage::node_state(report),
        )
        .await;

        for vm in reconciler.vm_runtime.list() {
            let vm_report = control_plane_node_api::control_plane_node_api::VmStateReport {
                node_id: node_id.clone(),
                vm_id: vm.vm_id.clone(),
                runtime_status: vm.runtime_status.clone(),
                observed_generation: vm.observed_generation.clone(),
                health_status: "Healthy".to_string(),
                last_error: vm.last_error.unwrap_or_default(),
                reported_unix_ms: now_unix_ms(),
            };
            send_or_defer_control_plane_message(
                &cache,
                &config.cache_path,
                &config,
                &mut telemetry,
                PendingControlPlaneMessage::vm_state(vm_report),
            )
            .await;
        }

        let volume_generations: std::collections::HashMap<String, String> = {
            let cache = cache.lock().await;
            cache
                .volume_handles
                .keys()
                .map(|k| {
                    (
                        k.clone(),
                        cache.volume_generations.get(k).cloned().unwrap_or_default(),
                    )
                })
                .collect()
        };
        for (volume_id, observed_generation) in volume_generations {
            let vol_report =
                reporter.volume_state_report(&volume_id, "Attached", observed_generation.as_str());
            send_or_defer_control_plane_message(
                &cache,
                &config.cache_path,
                &config,
                &mut telemetry,
                PendingControlPlaneMessage::volume_state(vol_report),
            )
            .await;
        }

        let network_fragments: Vec<(String, String)> = {
            let cache = cache.lock().await;
            cache
                .network_fragments
                .iter()
                .map(|(k, v)| (k.clone(), v.generation.clone()))
                .collect()
        };
        for (network_id, generation) in network_fragments {
            let net_report = reporter.network_state_report(&network_id, "Ready", &generation);
            send_or_defer_control_plane_message(
                &cache,
                &config.cache_path,
                &config,
                &mut telemetry,
                PendingControlPlaneMessage::network_state(net_report),
            )
            .await;
        }

        tick_count += 1;
        if tick_count.is_multiple_of(6) {
            let op_id = format!("inventory-{}", tick_count);
            let inv = inventory_reporter.build_inventory();
            let ver = inventory_reporter.build_versions();
            let inv_req =
                control_plane_node_api::control_plane_node_api::ReportNodeInventoryRequest {
                    meta: Some(
                        control_plane_node_api::control_plane_node_api::RequestMeta {
                            operation_id: op_id.clone(),
                            requested_by: "agent".to_string(),
                            target_node_id: node_id.clone(),
                            desired_state_version: String::new(),
                            request_unix_ms: now_unix_ms(),
                        },
                    ),
                    inventory: Some(inv),
                };
            send_or_defer_control_plane_message(
                &cache,
                &config.cache_path,
                &config,
                &mut telemetry,
                PendingControlPlaneMessage::node_inventory(inv_req),
            )
            .await;

            let ver_req =
                control_plane_node_api::control_plane_node_api::ReportServiceVersionsRequest {
                    meta: Some(
                        control_plane_node_api::control_plane_node_api::RequestMeta {
                            operation_id: op_id,
                            requested_by: "agent".to_string(),
                            target_node_id: node_id.clone(),
                            desired_state_version: String::new(),
                            request_unix_ms: now_unix_ms(),
                        },
                    ),
                    versions: Some(ver),
                };
            send_or_defer_control_plane_message(
                &cache,
                &config.cache_path,
                &config,
                &mut telemetry,
                PendingControlPlaneMessage::service_versions(ver_req),
            )
            .await;
        }

        if let Err(e) = reconciler.run_once().await {
            warn!(error = %e, "reconcile tick failed");
        }

        // Periodically persist cache so gRPC mutations are durable even without state transitions.
        {
            let cache = cache.lock().await;
            if let Err(e) = cache.save(&config.cache_path).await {
                warn!(error = %e, "failed to save cache");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn load_or_initialize_cache_preserves_persisted_state() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("agent-cache.json");
        let mut cache = NodeCache::new("node-1");
        cache.node_state = NodeState::Maintenance.as_str().to_string();
        cache.save(&cache_path).await.unwrap();

        let config = AgentConfig {
            cache_path,
            ..AgentConfig::default()
        };

        let loaded = load_or_initialize_cache(&config).await;
        assert_eq!(loaded.node_state, "Maintenance");
    }

    #[tokio::test]
    async fn load_or_initialize_cache_bootstraps_new_node() {
        let dir = tempfile::tempdir().unwrap();
        let config = AgentConfig {
            cache_path: dir.path().join("missing-cache.json"),
            node_id: "node-123".to_string(),
            ..AgentConfig::default()
        };

        let cache = load_or_initialize_cache(&config).await;
        assert_eq!(cache.node_id, "node-123");
        assert_eq!(cache.node_state, "Bootstrapping");
    }

    #[test]
    fn certificate_rotation_due_respects_interval() {
        let cert_file = tempfile::NamedTempFile::new().unwrap();
        let key_file = tempfile::NamedTempFile::new().unwrap();

        let mut cache = NodeCache::new("node-1");
        cache.enrollment_complete = true;
        cache.certificate_path = Some(cert_file.path().to_str().unwrap().to_string());
        cache.private_key_path = Some(key_file.path().to_str().unwrap().to_string());
        assert!(certificate_rotation_due(
            &cache,
            CERT_ROTATION_INTERVAL_SECS * 1000
        ));

        cache.last_certificate_rotation_unix_ms = Some(1_000);
        assert!(!certificate_rotation_due(
            &cache,
            1_000 + (CERT_ROTATION_INTERVAL_SECS * 1000) - 1
        ));
        assert!(certificate_rotation_due(
            &cache,
            1_000 + (CERT_ROTATION_INTERVAL_SECS * 1000)
        ));
    }
}
