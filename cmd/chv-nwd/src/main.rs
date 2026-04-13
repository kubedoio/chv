use chv_config::load_nwd_config;
use chv_nwd_core::NetworkServer;
use chv_observability::init_logger;
use std::path::PathBuf;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = std::env::args().nth(1).map(PathBuf::from);
    let config = load_nwd_config(config_path.as_deref())?;

    init_logger(&config.log_level)?;

    info!("chv-nwd starting");

    // TODO: replace with LinuxExecutor once ip/nft are available in test env
    let server = NetworkServer::new(
        chv_nwd_core::executor::LinuxExecutor::new(config.runtime_dir.clone()),
        chv_observability::Metrics::new(),
        None,
    );

    server.serve(&config.socket_path, None).await?;

    Ok(())
}
