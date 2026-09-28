use chv_config::load_nwd_config;
use chv_nwd_core::NetworkServer;
use chv_observability::init_logger;
use std::path::PathBuf;
use tokio::signal::unix::{signal, SignalKind};
use tracing::info;

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

    let config_path = std::env::args().nth(1).map(PathBuf::from);
    let config = load_nwd_config(config_path.as_deref())?;

    init_logger(&config.log_level)?;

    info!(
        "{} starting (version {}, commit {}, channel {})",
        env!("CARGO_PKG_NAME"),
        env!("CHV_VERSION"),
        env!("CHV_GIT_SHA"),
        env!("CHV_RELEASE_CHANNEL"),
    );

    let mut executor = chv_nwd_core::executor::LinuxExecutor::new(config.runtime_dir.clone());
    if config.fabric.enabled {
        let fabric = chv_nwd_core::fabric::NwdFabricProvider::real(&config.fabric)?;
        info!(
            state_dir = %config.fabric.state_dir.display(),
            name_prefix = %config.fabric.name_prefix,
            wireguard_port = config.fabric.wireguard_port,
            vxlan_port = config.fabric.vxlan_port,
            default_tenant_mtu = config.fabric.default_tenant_mtu,
            "stretched-L2 fabric provider enabled (ADR-021)"
        );
        executor = executor.with_fabric(fabric);
    } else {
        info!("stretched-L2 fabric provider disabled; fabric RPCs fail closed");
    }

    let server = NetworkServer::new(executor, chv_observability::Metrics::new());

    let socket_path = config.socket_path.clone();
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    tokio::select! {
        result = server.serve(&config.socket_path) => {
            result?;
        }
        _ = sigterm.recv() => {
            info!("received SIGTERM, shutting down");
        }
        _ = sigint.recv() => {
            info!("received SIGINT, shutting down");
        }
    }

    let _ = std::fs::remove_file(&socket_path);
    Ok(())
}
