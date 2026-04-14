use chv_config::ControlPlaneConfig;
use chv_controlplane_service::{
    ControlPlaneComponents, ControlPlaneRuntime, ControlPlaneService, ControlPlaneServiceError,
    EnrollmentServiceImplementation, InventoryServiceImplementation,
    TelemetryServiceImplementation,
};
use chv_controlplane_store::{
    connect_pool, run_migrations, AlertRepository, BootstrapTokenRepository, ControlPlaneStoreConfig,
    EventRepository, NodeRepository, ObservedStateRepository,
};

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

    let node_repo = NodeRepository::new(pool.clone());
    let token_repo = BootstrapTokenRepository::new(pool.clone());
    let observed_state_repo = ObservedStateRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let alert_repo = AlertRepository::new(pool.clone());

    let ca_cert_path = config.tls.ca_cert_path.as_ref().ok_or_else(|| {
        ControlPlaneServiceError::Internal("missing ca_cert_path in config".into())
    })?;
    let ca_key_path = config.tls.ca_key_path.as_ref().ok_or_else(|| {
        ControlPlaneServiceError::Internal("missing ca_key_path in config".into())
    })?;

    let ca_cert_pem = tokio::fs::read_to_string(ca_cert_path).await.map_err(|e| {
        ControlPlaneServiceError::Internal(format!("failed to read CA certificate: {}", e))
    })?;
    let ca_key_pem = tokio::fs::read_to_string(ca_key_path)
        .await
        .map_err(|e| ControlPlaneServiceError::Internal(format!("failed to read CA key: {}", e)))?;

    let cert_issuer = std::sync::Arc::new(
        chv_controlplane_service::CaBackedCertificateIssuer::new(&ca_cert_pem, &ca_key_pem)?,
    );

    let enrollment_service =
        EnrollmentServiceImplementation::new(node_repo.clone(), token_repo.clone(), cert_issuer);
    let inventory_service = InventoryServiceImplementation::new(node_repo.clone());
    let telemetry_service = TelemetryServiceImplementation::new(
        observed_state_repo.clone(),
        event_repo.clone(),
        alert_repo.clone(),
    );

    let runtime = ControlPlaneRuntime::new(config.grpc_bind, config.runtime_dir.clone());

    Ok(ControlPlaneService::new(
        runtime,
        ControlPlaneComponents::new(
            pool,
            enrollment_service,
            inventory_service,
            telemetry_service,
        ),
    ))
}
