use chv_agent_core::{
    cache::NodeCache, config::load_agent_config, daemon_clients::{NwdClient, StordClient},
    health::HealthAggregator, reconcile::Reconciler, state_machine::NodeState,
};
use chv_observability::init_logger;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = std::env::args().nth(1).map(PathBuf::from);
    let config = load_agent_config(config_path.as_deref())?;

    init_logger(&config.log_level)?;

    info!("chv-agent starting");

    // Load or initialize cache
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
            let c = NodeCache::new(node_id);
            info!("initialized new cache");
            c
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

    // Transition from whatever cached state to Bootstrapping on startup
    if cache.node_state.parse::<NodeState>().unwrap_or(NodeState::Bootstrapping) == NodeState::Bootstrapping {
        cache.node_state = NodeState::Bootstrapping.as_str().to_string();
    }

    let mut reconciler = Reconciler::new(
        cache.clone(),
        config.stord_socket.clone(),
        config.nwd_socket.clone(),
    );

    // Simple health probe loop (Phase 1 skeleton)
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
            reconciler.state_machine.transition(derived).ok();
            cache.node_state = reconciler.state_machine.current().as_str().to_string();
            if let Err(e) = cache.save(&config.cache_path).await {
                warn!(error = %e, "failed to save cache");
            }
        }

        if let Err(e) = reconciler.run_once().await {
            warn!(error = %e, "reconcile tick failed");
        }
    }
}
