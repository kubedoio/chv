mod bootstrap;
mod config;

use chv_observability::init_logger;
use config::{config_path_from_args, load_config};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls ring crypto provider");

    let config_path = config_path_from_args();
    let config = load_config(config_path.as_deref())?;
    init_logger(&config.log_level)?;

    info!("chv-controlplane starting");

    let service = bootstrap::build_service(&config).await?;
    service.run().await?;

    Ok(())
}
