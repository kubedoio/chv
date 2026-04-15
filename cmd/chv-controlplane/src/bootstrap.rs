use chv_config::ControlPlaneConfig;
use chv_controlplane_service::{
    ControlPlaneComponents, ControlPlaneRuntime, ControlPlaneService, ControlPlaneServiceError,
    EnrollmentServiceImplementation, InventoryServiceImplementation,
    LifecycleServiceImplementation, ReconcileServiceImplementation, TelemetryServiceImplementation,
};
use chv_controlplane_store::{
    connect_pool, run_migrations, AlertRepository, BootstrapTokenRepository, ControlPlaneStoreConfig,
    DesiredStateRepository, EventRepository, NetworkExposureRepository, NodeRepository,
    ObservedStateRepository, OperationRepository,
};

pub async fn build_service(
    config: &ControlPlaneConfig,
) -> Result<ControlPlaneService, ControlPlaneServiceError> {
    tokio::fs::create_dir_all(&config.runtime_dir).await?;

    if config.tls.server_cert_path.is_some() != config.tls.server_key_path.is_some() {
        return Err(ControlPlaneServiceError::Internal(
            "both server_cert_path and server_key_path must be set to enable TLS".into(),
        ));
    }

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
    let desired_state_repo = DesiredStateRepository::new(pool.clone());
    let network_exposure_repo = NetworkExposureRepository::new(pool.clone());
    let operation_repo = OperationRepository::new(pool.clone());

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
    let reconcile_service = ReconcileServiceImplementation::new(
        node_repo.clone(),
        desired_state_repo.clone(),
        network_exposure_repo,
        event_repo.clone(),
    );
    let lifecycle_service = LifecycleServiceImplementation::new(
        node_repo.clone(),
        operation_repo,
        event_repo.clone(),
        desired_state_repo,
    );

    let mut tls_config = None;
    if let (Some(cert_path), Some(key_path)) =
        (&config.tls.server_cert_path, &config.tls.server_key_path)
    {
        let cert_pem = tokio::fs::read(cert_path).await.map_err(|e| {
            ControlPlaneServiceError::Internal(format!("failed to read TLS certificate: {}", e))
        })?;
        let key_pem = tokio::fs::read(key_path).await.map_err(|e| {
            ControlPlaneServiceError::Internal(format!("failed to read TLS key: {}", e))
        })?;
        let identity = tonic::transport::Identity::from_pem(cert_pem, key_pem);
        let mut server_tls = tonic::transport::ServerTlsConfig::new().identity(identity);
        if let Some(client_ca_path) = &config.tls.client_ca_path {
            let client_ca_pem = tokio::fs::read(client_ca_path).await.map_err(|e| {
                ControlPlaneServiceError::Internal(format!(
                    "failed to read client CA certificate: {}",
                    e
                ))
            })?;
            server_tls = server_tls.client_ca_root(tonic::transport::Certificate::from_pem(
                client_ca_pem,
            ));
        }
        tls_config = Some(server_tls);
    }

    let runtime = ControlPlaneRuntime::new(config.grpc_bind, config.runtime_dir.clone(), tls_config);

    Ok(ControlPlaneService::new(
        runtime,
        ControlPlaneComponents::new(
            pool,
            enrollment_service,
            inventory_service,
            telemetry_service,
            reconcile_service,
            lifecycle_service,
        ),
    ))
}
