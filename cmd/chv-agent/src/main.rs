use chv_agent_core::{
    agent_server::AgentServer,
    cache::NodeCache,
    config::load_agent_config,
    control_plane::ControlPlaneClient,
    daemon_clients::{NwdClient, StordClient},
    health::HealthAggregator,
    reconcile::Reconciler,
    state_machine::NodeState,
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

    let mut telemetry = match ControlPlaneClient::new(
        &config.control_plane_addr,
        config.tls_cert_path.as_deref(),
        config.tls_key_path.as_deref(),
        config.ca_cert_path.as_deref(),
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

    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;

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

        let derived = health.derive_node_state(reconciler.state_machine.current());
        if derived != reconciler.state_machine.current() {
            info!(
                from = %reconciler.state_machine.current().as_str(),
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
            }
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
