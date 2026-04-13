use chv_agent_core::{
    agent_server::AgentServer,
    cache::NodeCache,
    config::load_agent_config,
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
use chv_observability::init_logger;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = std::env::args().nth(1).map(PathBuf::from);
    let config = load_agent_config(config_path.as_deref())?;

    init_logger(&config.log_level)?;

    info!("chv-agent starting");

    let mut cache = match NodeCache::load(&config.cache_path).await {
        Ok(c) => {
            info!(node_id = %c.node_id, "loaded cache");
            c
        }
        Err(chv_errors::ChvError::NotFound { .. }) => {
            let node_id = if config.node_id.is_empty() {
                "unknown".to_string()
            } else {
                config.node_id.clone()
            };
            NodeCache::new(node_id)
        }
        Err(e) => {
            warn!(error = %e, "failed to load cache, starting fresh");
            let node_id = if config.node_id.is_empty() {
                "unknown".to_string()
            } else {
                config.node_id.clone()
            };
            NodeCache::new(node_id)
        }
    };

    cache.node_state = NodeState::Bootstrapping.as_str().to_string();

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
                    let reporter = InventoryReporter::new(&cache.node_id, &hostname);
                    let inventory = reporter.build_inventory();
                    let versions = reporter.build_versions();
                    match EnrollmentClient::connect(&config.control_plane_addr).await {
                        Ok(mut client) => {
                            match client.enroll_node(token, inventory, versions).await {
                                Ok(resp) => {
                                    let cert_path = config.runtime_dir.join("agent.crt");
                                    let key_path = config.runtime_dir.join("agent.key");
                                    let ca_path = config.runtime_dir.join("ca.crt");
                                    if let Err(e) = tokio::fs::create_dir_all(&config.runtime_dir).await {
                                        warn!(error = %e, "failed to create runtime dir");
                                    } else {
                                        let mut ok = true;
                                        if let Err(e) = tokio::fs::write(&cert_path, &resp.certificate_pem).await {
                                            warn!(error = %e, "failed to write certificate");
                                            ok = false;
                                        }
                                        if let Err(e) = tokio::fs::write(&key_path, &resp.private_key_pem).await {
                                            warn!(error = %e, "failed to write private key");
                                            ok = false;
                                        }
                                        if let Err(e) = tokio::fs::write(&ca_path, &resp.ca_pem).await {
                                            warn!(error = %e, "failed to write ca certificate");
                                            ok = false;
                                        }
                                        if ok {
                                            cache.node_id = resp.node_id.clone();
                                            cache.certificate_path = Some(cert_path.to_string_lossy().to_string());
                                            cache.private_key_path = Some(key_path.to_string_lossy().to_string());
                                            cache.ca_path = Some(ca_path.to_string_lossy().to_string());
                                            cache.enrollment_complete = true;
                                            if let Err(e) = cache.save(&config.cache_path).await {
                                                warn!(error = %e, "failed to save cache after enrollment");
                                            } else {
                                                info!(node_id = %resp.node_id, "enrollment complete");
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

    let agent_server = AgentServer::new(
        cache.clone(),
        vm_runtime.clone(),
        config.stord_socket.clone(),
        config.nwd_socket.clone(),
    );
    let server_socket = config.socket_path.clone();
    tokio::spawn(async move {
        if let Err(e) = agent_server.serve(&server_socket).await {
            warn!(error = %e, "agent server exited");
        }
    });

    let mut reconciler = Reconciler::new(
        cache.clone(),
        vm_runtime.clone(),
        config.stord_socket.clone(),
        config.nwd_socket.clone(),
    );

    let mut supervisor = DaemonSupervisor::new(
        config.stord_binary_path.clone(),
        config.nwd_binary_path.clone(),
        config.stord_socket.clone(),
        config.nwd_socket.clone(),
        config.runtime_dir.clone(),
    );

    let tls_cert = cache.certificate_path.as_ref().map(PathBuf::from).or_else(|| config.tls_cert_path.clone());
    let tls_key = cache.private_key_path.as_ref().map(PathBuf::from).or_else(|| config.tls_key_path.clone());
    let ca_cert = cache.ca_path.as_ref().map(PathBuf::from).or_else(|| config.ca_cert_path.clone());

    let mut telemetry = match ControlPlaneClient::new(
        &config.control_plane_addr,
        tls_cert.as_deref(),
        tls_key.as_deref(),
        ca_cert.as_deref(),
    )
    .await
    {
        Ok(client) => {
            info!("connected to control plane");
            Some((TelemetryReporter::new(&cache.node_id), client))
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
    let inventory_reporter = InventoryReporter::new(&cache.node_id, hostname);
    let mut tick_count = 0u64;
    let mut consecutive_health_failures = 0u32;
    const FAILED_THRESHOLD: u32 = 6; // 6 ticks * 5s = 30s

    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;

        if let Err(e) = supervisor.restart_if_needed().await {
            warn!(error = %e, "supervisor restart failed");
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

        let current_state = reconciler.state_machine.current();
        let derived = health.derive_node_state(current_state);
        if derived != current_state {
            let from_str = current_state.as_str().to_string();
            info!(
                from = %from_str,
                to = %derived.as_str(),
                "state transition"
            );
            if let Err(e) = reconciler.state_machine.transition(derived) {
                warn!(error = %e, "invalid state transition ignored");
            } else {
                cache.node_state = reconciler.state_machine.current().as_str().to_string();
                if let Err(e) = cache.save(&config.cache_path).await {
                    warn!(error = %e, "failed to save cache");
                }
                if derived != NodeState::Degraded {
                    consecutive_health_failures = 0;
                }
                if let Some((ref reporter, ref mut client)) = telemetry {
                    let severity = match derived {
                        NodeState::Failed => "critical",
                        NodeState::Degraded => "warning",
                        NodeState::TenantReady => "info",
                        _ => "info",
                    };
                    let event = reporter.event_report(
                        control_plane_node_api::control_plane_node_api::RequestMeta {
                            operation_id: format!("state-transition-{}", tick_count),
                            requested_by: "agent".to_string(),
                            target_node_id: cache.node_id.clone(),
                            desired_state_version: cache.observed_generation.clone(),
                            request_unix_ms: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as i64,
                        },
                        severity,
                        "NodeStateTransition",
                        &format!("node transitioned from {} to {}", from_str, derived.as_str()),
                    );
                    if let Err(e) = client.publish_event(event).await {
                        warn!(error = %e, "failed to publish state transition event");
                    }
                }
            }
        } else if current_state == NodeState::Degraded {
            consecutive_health_failures += 1;
            if consecutive_health_failures >= FAILED_THRESHOLD {
                info!("health failures exceeded threshold, transitioning to Failed");
                if let Err(e) = reconciler.state_machine.transition(NodeState::Failed) {
                    warn!(error = %e, "failed to transition to Failed");
                } else {
                    cache.node_state = NodeState::Failed.as_str().to_string();
                    if let Err(e) = cache.save(&config.cache_path).await {
                        warn!(error = %e, "failed to save cache");
                    }
                    if let Some((ref reporter, ref mut client)) = telemetry {
                        let event = reporter.event_report(
                            control_plane_node_api::control_plane_node_api::RequestMeta {
                                operation_id: format!("state-transition-failed-{}", tick_count),
                                requested_by: "agent".to_string(),
                                target_node_id: cache.node_id.clone(),
                                desired_state_version: cache.observed_generation.clone(),
                                request_unix_ms: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as i64,
                            },
                            "critical",
                            "NodeStateTransition",
                            "node transitioned to Failed after persistent health degradation",
                        );
                        if let Err(e) = client.publish_event(event).await {
                            warn!(error = %e, "failed to publish Failed event");
                        }
                    }
                }
                consecutive_health_failures = 0;
            }
        } else {
            consecutive_health_failures = 0;
        }

        if let Some((ref reporter, ref mut client)) = telemetry {
            let report = reporter.node_state_report(
                cache.node_state.as_str(),
                cache.observed_generation.as_str(),
                if reconciler.state_machine.current() == NodeState::TenantReady {
                    "Healthy"
                } else {
                    "Degraded"
                },
                cache.last_error.clone(),
            );
            if let Err(e) = client.report_node_state(report).await {
                warn!(error = %e, "failed to report node state");
            }

            for vm in reconciler.vm_runtime.list() {
                let vm_report = control_plane_node_api::control_plane_node_api::VmStateReport {
                    node_id: cache.node_id.clone(),
                    vm_id: vm.vm_id.clone(),
                    runtime_status: vm.runtime_status.clone(),
                    observed_generation: vm.observed_generation.clone(),
                    health_status: "Healthy".to_string(),
                    last_error: vm.last_error.unwrap_or_default(),
                    reported_unix_ms: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64,
                };
                if let Err(e) = client.report_vm_state(vm_report).await {
                    warn!(vm_id = %vm.vm_id, error = %e, "failed to report vm state");
                }
            }

            for volume_id in cache.volume_handles.keys() {
                let vol_report = reporter.volume_state_report(
                    volume_id,
                    "Attached",
                    cache.volume_generations.get(volume_id).map(|s| s.as_str()).unwrap_or(""),
                );
                if let Err(e) = client.report_volume_state(vol_report).await {
                    warn!(volume_id = %volume_id, error = %e, "failed to report volume state");
                }
            }

            for (network_id, fragment) in &cache.network_fragments {
                let net_report = reporter.network_state_report(
                    network_id,
                    "Ready",
                    &fragment.generation,
                );
                if let Err(e) = client.report_network_state(net_report).await {
                    warn!(network_id = %network_id, error = %e, "failed to report network state");
                }
            }

            tick_count += 1;
            if tick_count.is_multiple_of(6) {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                let op_id = format!("inventory-{}", tick_count);
                let inv = inventory_reporter.build_inventory();
                let ver = inventory_reporter.build_versions();
                let inv_req = control_plane_node_api::control_plane_node_api::ReportNodeInventoryRequest {
                    meta: Some(control_plane_node_api::control_plane_node_api::RequestMeta {
                        operation_id: op_id.clone(),
                        requested_by: "agent".to_string(),
                        target_node_id: cache.node_id.clone(),
                        desired_state_version: "".to_string(),
                        request_unix_ms: now,
                    }),
                    inventory: Some(inv),
                };
                if let Err(e) = client.report_node_inventory(inv_req).await {
                    warn!(error = %e, "failed to report node inventory");
                }
                let ver_req = control_plane_node_api::control_plane_node_api::ReportServiceVersionsRequest {
                    meta: Some(control_plane_node_api::control_plane_node_api::RequestMeta {
                        operation_id: op_id,
                        requested_by: "agent".to_string(),
                        target_node_id: cache.node_id.clone(),
                        desired_state_version: "".to_string(),
                        request_unix_ms: now,
                    }),
                    versions: Some(ver),
                };
                if let Err(e) = client.report_service_versions(ver_req).await {
                    warn!(error = %e, "failed to report service versions");
                }
            }
        }

        if let Err(e) = reconciler.run_once().await {
            warn!(error = %e, "reconcile tick failed");
        }

        // Periodically persist cache so gRPC mutations are durable even without state transitions.
        if let Err(e) = cache.save(&config.cache_path).await {
            warn!(error = %e, "failed to save cache");
        }
    }
}
