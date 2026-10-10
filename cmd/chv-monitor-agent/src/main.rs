//! `chv-monitor-agent` binary entry point. All logic lives in the
//! library crate so integration tests exercise the identical code.

use chv_monitor_agent::{Agent, AgentConfig, TickOutcome};
use std::path::PathBuf;
use tokio::signal::unix::{signal, SignalKind};
use tracing::{error, info, warn};

const DEFAULT_CONFIG_PATH: &str = "/etc/chv-monitor/agent.toml";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!(
            "{} {} (commit {}, build {}, channel {})",
            env!("CARGO_PKG_NAME"),
            env!("CHV_VERSION"),
            env!("CHV_GIT_SHA"),
            env!("CHV_BUILD_DATE"),
            env!("CHV_RELEASE_CHANNEL"),
        );
        return Ok(());
    }

    let config_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));
    let config = AgentConfig::load(&config_path)?;

    chv_observability::init_logger(&config.log_level)?;

    info!(
        "{} starting (version {}, commit {}, channel {})",
        env!("CARGO_PKG_NAME"),
        env!("CHV_VERSION"),
        env!("CHV_GIT_SHA"),
        env!("CHV_RELEASE_CHANNEL"),
    );
    info!(
        server_url = %config.server_url,
        interval_seconds = config.interval_seconds,
        spool_dir = %config.spool_dir.display(),
        "guest monitoring agent starting"
    );

    let claim_path = config.claim_path.clone();
    let interval_seconds = config.interval_seconds;
    let mut agent = match Agent::start(config) {
        Ok(a) => a,
        Err(e) => {
            error!(error = %e, "failed to initialize agent state; is the state directory writable?");
            return Err(e.into());
        }
    };

    if !agent.is_enrolled() {
        info!(
            "not enrolled; waiting for a claim at {}",
            claim_path.display()
        );
    }

    let mut interval =
        tokio::time::interval(std::time::Duration::from_secs(interval_seconds.max(1)));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    loop {
        tokio::select! {
            _ = interval.tick() => {}
            _ = sigterm.recv() => {
                info!("SIGTERM received; shutting down");
                break;
            }
            _ = sigint.recv() => {
                info!("SIGINT received; shutting down");
                break;
            }
        }

        match agent.tick().await {
            TickOutcome::Idle => {}
            TickOutcome::Enrolled => info!("enrolled; first collection on the next tick"),
            TickOutcome::Delivered { samples } => {
                info!(samples, spooled = agent.spool_len(), "batch delivered")
            }
            TickOutcome::Spooled => warn!(
                spooled = agent.spool_len(),
                "manager unreachable; batches spooled for replay"
            ),
            TickOutcome::Unauthorized => error!(
                "credential refused by the manager; operator must revoke the agent \
                 and place a fresh claim at {}",
                claim_path.display()
            ),
        }
    }

    info!("shutdown complete");
    Ok(())
}
