use chv_config::ControlPlaneConfig;
use chv_controlplane_service::{
    ControlPlaneComponents, ControlPlaneRuntime, ControlPlaneService, ControlPlaneServiceError,
};
use chv_controlplane_store::{connect_pool, run_migrations, ControlPlaneStoreConfig};

pub async fn build_service(
    config: &ControlPlaneConfig,
) -> Result<ControlPlaneService, ControlPlaneServiceError> {
    tokio::fs::create_dir_all(&config.runtime_dir).await?;

    let store_config = ControlPlaneStoreConfig {
        database_url: config.database.url.clone(),
        migrations_dir: config.database.migrations_dir.clone(),
        max_connections: config.database.max_connections,
        min_connections: config.database.min_connections,
        acquire_timeout_secs: config.database.acquire_timeout_secs,
        idle_timeout_secs: config.database.idle_timeout_secs,
        max_lifetime_secs: config.database.max_lifetime_secs,
    };

    let pool = connect_pool(&store_config).await?;
    run_migrations(&pool, Some(&store_config)).await?;

    let runtime = ControlPlaneRuntime::new(config.grpc_bind, config.runtime_dir.clone());

    Ok(ControlPlaneService::new(
        runtime,
        ControlPlaneComponents::new(pool),
    ))
}
