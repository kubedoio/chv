use chv_config::load_stord_config;
use chv_observability::init_logger;
use chv_stord_backends::LocalFileBackend;
use chv_stord_core::StorageServer;
use std::path::PathBuf;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = std::env::args().nth(1).map(PathBuf::from);
    let config = load_stord_config(config_path.as_deref())?;

    init_logger(&config.log_level)?;

    info!("chv-stord starting");

    let backend = LocalFileBackend::new(config.runtime_dir.clone());
    let server = StorageServer::new(
        backend,
        chv_observability::Metrics::new(),
        config.backend_allowlist,
    );

    server.serve(&config.socket_path).await?;

    Ok(())
}
