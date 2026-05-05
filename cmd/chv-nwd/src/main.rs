use chv_config::load_nwd_config;
use chv_nwd_core::NetworkServer;
use chv_observability::init_logger;
use std::path::PathBuf;
use tokio::signal::unix::{signal, SignalKind};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = std::env::args().nth(1).map(PathBuf::from);
    let config = load_nwd_config(config_path.as_deref())?;

    init_logger(&config.log_level)?;

    info!("chv-nwd starting");

    let server = NetworkServer::new(
        chv_nwd_core::executor::LinuxExecutor::new(config.runtime_dir.clone()),
        chv_observability::Metrics::new(),
        None,
    );

    let socket_path = config.socket_path.clone();
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    tokio::select! {
        result = server.serve(&config.socket_path, None) => {
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
