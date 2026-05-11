mod bootstrap;
mod config;

use chv_observability::init_logger;
use config::{config_path_from_args, load_config};
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

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls ring crypto provider");

    let config_path = config_path_from_args();
    let config = load_config(config_path.as_deref())?;
    init_logger(&config.log_level)?;

    info!(
        "{} starting (version {}, commit {}, channel {})",
        env!("CARGO_PKG_NAME"),
        env!("CHV_VERSION"),
        env!("CHV_GIT_SHA"),
        env!("CHV_RELEASE_CHANNEL"),
    );

    let service = bootstrap::build_service(&config).await?;
    let shutdown_tx = service.shutdown_tx();

    tokio::spawn(async move {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .expect("failed to install SIGINT handler");

        tokio::select! {
            _ = sigterm.recv() => info!("received SIGTERM, shutting down gracefully"),
            _ = sigint.recv() => info!("received SIGINT, shutting down gracefully"),
        }

        let _ = shutdown_tx.send(());
    });

    service.run().await?;
    service.shutdown().await;

    info!("chv-controlplane stopped");
    Ok(())
}
