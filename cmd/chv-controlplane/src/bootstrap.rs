use chv_config::{ControlPlaneConfig, ControlPlaneTlsConfig};
use chv_controlplane_service::{
    compat::{CompatibilityMatrix, Component},
    ControlPlaneComponents, ControlPlaneMutationService, ControlPlaneRuntime, ControlPlaneService,
    ControlPlaneServiceError, EnrollmentServiceImplementation, InventoryServiceImplementation,
    LifecycleServiceImplementation, NodeClientPool, Orchestrator, ReconcileServiceImplementation,
    TelemetryServiceImplementation,
};
use chv_controlplane_store::{
    connect_pool, run_migrations, AlertRepository, BackupRepository, BootstrapTokenRepository,
    ControlPlaneStoreConfig, DesiredStateRepository, EventRepository, NodeRepository,
    ObservedStateRepository, OperationRepository, VtepRepository,
};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Validate the TLS configuration for the control plane.
///
/// Returns `Ok(())` when:
/// - Both `server_cert_path` and `server_key_path` are set, `client_ca_path` is set
///   (production mTLS), OR
/// - `allow_insecure` is `true` (dev / `CHV_ALLOW_INSECURE=1`).
///
/// Returns `Err(ControlPlaneServiceError::Internal)` with a stable, operator-greppable
/// message when the configuration is incomplete. The message strings here are part of
/// the operator contract — changing them will break runbooks/log searches.
///
/// This is extracted as a pure function (no I/O, no env reads) so the polarity of the
/// gate is exercised by unit tests. See ADR-008 (error handling) and ADR-009 (logging).
pub fn validate_tls(
    tls: &ControlPlaneTlsConfig,
    allow_insecure: bool,
) -> Result<(), ControlPlaneServiceError> {
    if tls.server_cert_path.is_some() != tls.server_key_path.is_some() {
        return Err(ControlPlaneServiceError::Internal(
            "both server_cert_path and server_key_path must be set to enable TLS".into(),
        ));
    }

    if !allow_insecure {
        if tls.server_cert_path.is_none() || tls.server_key_path.is_none() {
            return Err(ControlPlaneServiceError::Internal(
                "TLS required: set server_cert_path + server_key_path, or CHV_ALLOW_INSECURE=1 for dev".into(),
            ));
        }
        if tls.client_ca_path.is_none() {
            return Err(ControlPlaneServiceError::Internal(
                "mTLS required: set client_ca_path for client certificate verification, or CHV_ALLOW_INSECURE=1 for dev".into(),
            ));
        }
    }

    Ok(())
}

/// Run the compatibility-matrix gate against enrolled node versions.
///
/// The matrix is operator-opt-in via `CHV_COMPAT_MATRIX_PATH`. Once opted in, the gate
/// is **fail-closed**: a DB query failure at boot is treated as "cannot verify, refuse
/// to start" rather than silently bypassed. A `tracing::warn!` here would let an
/// incompatible-version node reconcile during a transient DB glitch — see H8 in the
/// security review.
///
/// Behavior:
/// - File missing at `matrix_path`: log warn and continue (configured-but-not-yet-deployed
///   is acceptable; the operator may be staging the file).
/// - File present, parse error: log warn and continue (parse errors should surface during
///   matrix authoring, not block prod boot for an isolated control plane).
/// - File present, parsed OK, query fails: **return Err** (fail-closed).
/// - Incompatible versions detected: return Err with the violation summary.
///
/// See ADR-008 (error handling) and ADR-009 (logging/observability).
pub async fn check_compat_matrix(
    pool: &SqlitePool,
    matrix_path: &Path,
) -> Result<(), ControlPlaneServiceError> {
    if !matrix_path.exists() {
        tracing::warn!(
            path = %matrix_path.display(),
            "CHV_COMPAT_MATRIX_PATH set but file does not exist, skipping compatibility check"
        );
        return Ok(());
    }

    let matrix = match CompatibilityMatrix::load_from_file(matrix_path) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(
                path = %matrix_path.display(),
                error = %e,
                "failed to load compatibility matrix file"
            );
            return Ok(());
        }
    };

    let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>)>(
        "SELECT node_id, chv_agent_version, chv_stord_version, chv_nwd_version \
         FROM node_inventory \
         WHERE chv_agent_version IS NOT NULL \
            OR chv_stord_version IS NOT NULL \
            OR chv_nwd_version IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        // FAIL-CLOSED: H8. When the operator has opted into the compat matrix
        // (CHV_COMPAT_MATRIX_PATH is set) and we cannot query node versions,
        // refuse to start. The previous behavior (warn + continue) silently
        // bypassed the gate during a DB glitch, letting an incompatible node
        // reconcile.
        tracing::error!(
            error = %e,
            path = %matrix_path.display(),
            "compatibility matrix gate: failed to query node_inventory; refusing to start (fail-closed)"
        );
        ControlPlaneServiceError::Internal(format!("compat matrix query failed: {e}"))
    })?;

    let mut all_reports = Vec::new();
    for (node_id, agent_ver, stord_ver, nwd_ver) in &rows {
        let mut versions: HashMap<Component, String> = HashMap::new();
        if let Some(v) = agent_ver {
            versions.insert(Component::Agent, v.clone());
        }
        if let Some(v) = stord_ver {
            versions.insert(Component::Stord, v.clone());
        }
        if let Some(v) = nwd_ver {
            versions.insert(Component::Nwd, v.clone());
        }

        let reports = matrix.check_all(&versions);
        for report in &reports {
            tracing::error!(
                node_id = %node_id,
                component = %report.component,
                current_version = %report.current_version,
                min_allowed = %report.min_allowed,
                max_allowed = %report.max_allowed,
                "version incompatibility detected: {}",
                report.message
            );
        }
        all_reports.extend(reports);
    }

    if !all_reports.is_empty() {
        let summary: Vec<String> = all_reports.iter().map(|r| r.message.clone()).collect();
        return Err(ControlPlaneServiceError::Internal(format!(
            "boot blocked: {} incompatible component version(s) detected: {}",
            all_reports.len(),
            summary.join("; ")
        )));
    }

    tracing::info!(
        nodes_checked = rows.len(),
        "compatibility matrix check passed — all versions compatible"
    );
    Ok(())
}

pub async fn build_service(
    config: &ControlPlaneConfig,
) -> Result<ControlPlaneService, ControlPlaneServiceError> {
    tokio::fs::create_dir_all(&config.runtime_dir).await?;

    let allow_insecure = std::env::var("CHV_ALLOW_INSECURE")
        .map(|v| v == "1")
        .unwrap_or(false);
    validate_tls(&config.tls, allow_insecure)?;

    let store_config = ControlPlaneStoreConfig {
        database_url: config.database.url.clone(),
        migrations_dir: config.database.migrations_dir.clone(),
        max_connections: config.database.max_connections,
        acquire_timeout_secs: config.database.acquire_timeout_secs,
    };

    let pool = connect_pool(&store_config).await?;
    run_migrations(&pool, Some(&store_config)).await?;

    // Seed the six canonical starter topologies on first deployment so the
    // operator lands on a populated /architectures dashboard. The seeder is
    // idempotent — once `system_settings.seed_starters_completed = '1'` it
    // is a cheap no-op on every subsequent boot. Per-fixture failures are
    // logged and skipped (fail-open) so a malformed starter cannot block
    // the control plane from coming up; only a failure to read or update
    // the sentinel row itself propagates as a boot error.
    //
    // See `docs/plans/2026-06-16-starter-topologies-and-auto-seed.md` §4
    // for the full design and `crates/chv-controlplane-seed` for the
    // implementation.
    let seed_topology_repo = chv_controlplane_store::TopologyRepository::new(pool.clone());
    match chv_controlplane_seed::seed_if_first_deployment(&seed_topology_repo).await {
        Ok(chv_controlplane_seed::SeedOutcome::Seeded { count }) => {
            tracing::info!(count, "starter topologies seeded on first deployment");
        }
        Ok(chv_controlplane_seed::SeedOutcome::Skipped) => {
            tracing::debug!("starter topology seeding already completed; skipping");
        }
        Err(err) => {
            // Preserve structured fields for ops dashboards via the dedicated
            // Seed(SeedError) variant — `?err` keeps thiserror's source chain
            // intact for downstream `error = ?` consumers.
            tracing::error!(?err, "starter topology seed fatal");
            return Err(ControlPlaneServiceError::Seed(err));
        }
    }

    // --- Compatibility matrix check (hard gate on incompatibilities) ---
    // Operator-opt-in: only runs when CHV_COMPAT_MATRIX_PATH is set. Once opted
    // in, the gate is fail-closed (see check_compat_matrix docs and H8 review).
    if let Ok(matrix_path) = std::env::var("CHV_COMPAT_MATRIX_PATH") {
        check_compat_matrix(&pool, Path::new(&matrix_path)).await?;
    }

    // Warn if the bootstrap admin password has not been changed
    let default_hash = "$2b$12$JbNLkka47ajSOyzKo8fKI.CBvQav06.Vrnh4pbZf4VSaLwS7yI71m";
    if let Ok(Some(hash)) =
        sqlx::query_scalar::<_, String>("SELECT password_hash FROM users WHERE username = 'admin'")
            .fetch_optional(&pool)
            .await
    {
        if hash == default_hash {
            tracing::warn!(
                "SECURITY: the default 'admin' user still has the bootstrap password. \
                 Change it immediately via the UI or API."
            );
        }
    }

    let node_repo = NodeRepository::new(pool.clone());
    let token_repo = BootstrapTokenRepository::new(pool.clone());
    let observed_state_repo = ObservedStateRepository::new(pool.clone());
    let event_repo = EventRepository::new(pool.clone());
    let alert_repo = AlertRepository::new(pool.clone());
    let desired_state_repo = DesiredStateRepository::new(pool.clone());
    let operation_repo = OperationRepository::new(pool.clone());
    let backup_repo = BackupRepository::new(pool.clone());
    let topology_repo = chv_controlplane_store::TopologyRepository::new(pool.clone());
    let vtep_repo = VtepRepository::new(pool.clone());

    let lifecycle_service = Arc::new(LifecycleServiceImplementation::new(
        node_repo.clone(),
        operation_repo.clone(),
        event_repo.clone(),
        desired_state_repo.clone(),
    ));

    let bff_state = chv_webui_bff::AppState {
        pool: pool.clone(),
        node_repo: node_repo.clone(),
        operation_repo: operation_repo.clone(),
        event_repo: event_repo.clone(),
        alert_repo: alert_repo.clone(),
        desired_state_repo: desired_state_repo.clone(),
        observed_state_repo: observed_state_repo.clone(),
        backup_repo: backup_repo.clone(),
        topology_repo: topology_repo.clone(),
        network_repo: chv_controlplane_store::NetworkRepository::new(pool.clone()),
        image_repo: chv_controlplane_store::ImageRepository::new(pool.clone()),
        apply_runs: Arc::new(chv_controlplane_store::ApplyRunRepository::new(
            pool.clone(),
        )),
        drift_reports: Arc::new(chv_controlplane_store::DriftReportRepository::new(
            pool.clone(),
        )),
        mutations: Arc::new(ControlPlaneMutationService::new(
            pool.clone(),
            lifecycle_service.clone(),
        )),
        jwt_secret: config.jwt_secret.clone(),
        agent_runtime_dir: config.agent_runtime_dir.clone(),
        cache: chv_webui_bff::BffCache::new(5),
        clock: Arc::new(chv_common::SystemClock),
    };

    let convergence_metrics = chv_controlplane_service::convergence_metrics::new_shared();

    let router =
        chv_controlplane_service::api::router::admin_router(bff_state, convergence_metrics.clone());
    let http_listener = tokio::net::TcpListener::bind(config.http_bind)
        .await
        .map_err(|e| {
            ControlPlaneServiceError::Internal(format!("failed to bind HTTP listener: {}", e))
        })?;
    let (http_shutdown_tx, mut http_shutdown_rx) = tokio::sync::watch::channel(());
    let http_join_handle = tokio::spawn(async move {
        axum::serve(
            http_listener,
            router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            let _ = http_shutdown_rx.changed().await;
        })
        .await
    });

    let cert_issuer = if let (Some(ca_cert_path), Some(ca_key_path)) =
        (&config.tls.ca_cert_path, &config.tls.ca_key_path)
    {
        let ca_cert_pem = tokio::fs::read_to_string(ca_cert_path).await.map_err(|e| {
            ControlPlaneServiceError::Internal(format!("failed to read CA certificate: {}", e))
        })?;
        let ca_key_pem = tokio::fs::read_to_string(ca_key_path).await.map_err(|e| {
            ControlPlaneServiceError::Internal(format!("failed to read CA key: {}", e))
        })?;

        Some(
            Arc::new(chv_controlplane_service::CaBackedCertificateIssuer::new(
                &ca_cert_pem,
                &ca_key_pem,
            )?) as Arc<dyn chv_controlplane_service::CertificateIssuer>,
        )
    } else {
        None
    };

    let enrollment_service = EnrollmentServiceImplementation::new(
        node_repo.clone(),
        token_repo.clone(),
        cert_issuer,
        vtep_repo.clone(),
    );
    let inventory_service = InventoryServiceImplementation::new(node_repo.clone());
    let telemetry_service = TelemetryServiceImplementation::new(
        node_repo.clone(),
        observed_state_repo.clone(),
        event_repo.clone(),
        alert_repo.clone(),
    );
    let reconcile_service = ReconcileServiceImplementation::new(
        node_repo.clone(),
        desired_state_repo.clone(),
        event_repo.clone(),
        observed_state_repo.clone(),
        operation_repo.clone(),
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
            server_tls =
                server_tls.client_ca_root(tonic::transport::Certificate::from_pem(client_ca_pem));
        }
        tls_config = Some(server_tls);
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());

    let runtime = ControlPlaneRuntime::new(
        config.grpc_bind,
        config.runtime_dir.clone(),
        tls_config,
        allow_insecure,
        http_shutdown_tx,
        http_join_handle,
        shutdown_rx.clone(),
    );

    let node_client_pool = NodeClientPool::new();

    let overlay_manager = chv_controlplane_service::OverlayManager::new(
        vtep_repo.clone(),
        node_client_pool.clone(),
        config.agent_socket_pattern.clone(),
    );

    let orchestrator = Orchestrator::new(
        pool.clone(),
        operation_repo.clone(),
        config.agent_socket_pattern.clone(),
        config.kernel_path.clone(),
        config.firmware_path.clone(),
        node_client_pool.clone(),
        convergence_metrics,
    )
    .with_overlay_manager(overlay_manager);
    let orchestrator_handle = tokio::spawn(orchestrator.run(shutdown_rx.clone()));

    let backup_staging_dir = config.runtime_dir.join("backups");
    if let Err(e) = std::fs::create_dir_all(&backup_staging_dir) {
        tracing::warn!(
            error = %e,
            path = %backup_staging_dir.display(),
            "failed to create backup staging directory"
        );
    }
    let backup_worker = chv_controlplane_service::BackupWorker::new(
        pool.clone(),
        backup_repo.clone(),
        config.agent_socket_pattern.clone(),
        node_client_pool.clone(),
        backup_staging_dir,
    );
    let backup_worker_handle = tokio::spawn(backup_worker.run(shutdown_rx.clone()));

    let migration_reaper = chv_controlplane_service::MigrationReaper::new(
        pool.clone(),
        node_client_pool.clone(),
        config.agent_socket_pattern.clone(),
    );
    let reaper_handle = tokio::spawn(migration_reaper.run(shutdown_rx));

    Ok(ControlPlaneService::new(
        runtime,
        ControlPlaneComponents::new(
            pool,
            enrollment_service,
            inventory_service,
            telemetry_service,
            reconcile_service,
            (*lifecycle_service).clone(),
        ),
        shutdown_tx,
        vec![orchestrator_handle, backup_worker_handle, reaper_handle],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ---------------------------------------------------------------------
    // H7: validate_tls — pure-function unit tests for the TLS/mTLS gate.
    //
    // These tests pin the polarity of the gate. A future refactor that
    // accidentally flips a condition (e.g., && → ||) will fail here long
    // before a production binary ships with mTLS disabled.
    // ---------------------------------------------------------------------

    fn p(s: &str) -> Option<PathBuf> {
        Some(PathBuf::from(s))
    }

    fn err_msg(r: Result<(), ControlPlaneServiceError>) -> String {
        match r {
            Err(ControlPlaneServiceError::Internal(m)) => m,
            other => panic!("expected Err(Internal), got {other:?}"),
        }
    }

    #[test]
    fn validate_tls_full_mtls_ok() {
        let tls = ControlPlaneTlsConfig {
            server_cert_path: p("/tls/server.crt"),
            server_key_path: p("/tls/server.key"),
            client_ca_path: p("/tls/ca.crt"),
            ca_cert_path: None,
            ca_key_path: None,
        };
        assert!(validate_tls(&tls, false).is_ok());
    }

    #[test]
    fn validate_tls_cert_only_rejects_with_pair_message() {
        let tls = ControlPlaneTlsConfig {
            server_cert_path: p("/tls/server.crt"),
            server_key_path: None,
            client_ca_path: None,
            ca_cert_path: None,
            ca_key_path: None,
        };
        let msg = err_msg(validate_tls(&tls, false));
        assert!(
            msg.contains("both server_cert_path and server_key_path"),
            "expected pair-required message, got: {msg}"
        );
    }

    #[test]
    fn validate_tls_no_certs_rejects_with_tls_required() {
        let tls = ControlPlaneTlsConfig::default();
        let msg = err_msg(validate_tls(&tls, false));
        assert!(
            msg.contains("TLS required"),
            "expected TLS-required message, got: {msg}"
        );
    }

    #[test]
    fn validate_tls_certs_without_client_ca_rejects_with_mtls_required() {
        let tls = ControlPlaneTlsConfig {
            server_cert_path: p("/tls/server.crt"),
            server_key_path: p("/tls/server.key"),
            client_ca_path: None,
            ca_cert_path: None,
            ca_key_path: None,
        };
        let msg = err_msg(validate_tls(&tls, false));
        assert!(
            msg.contains("mTLS required"),
            "expected mTLS-required message, got: {msg}"
        );
    }

    #[test]
    fn validate_tls_no_certs_with_allow_insecure_ok() {
        let tls = ControlPlaneTlsConfig::default();
        assert!(validate_tls(&tls, true).is_ok());
    }

    // ---------------------------------------------------------------------
    // H8: check_compat_matrix — fail-closed on DB query failure when the
    // operator has opted into the matrix gate.
    //
    // We exercise the query-failure path by pointing check_compat_matrix at
    // an in-memory SQLite pool that has no `node_inventory` table. Sqlx will
    // return a "no such table" error from fetch_all; we want that error to
    // propagate as Err, not be swallowed as warn-and-continue.
    // ---------------------------------------------------------------------

    async fn empty_pool() -> SqlitePool {
        SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite")
    }

    fn write_minimal_matrix() -> tempfile::NamedTempFile {
        // A minimal-but-valid matrix file. Format mirrors crates/chv-controlplane-service/src/compat.rs.
        let f = tempfile::Builder::new()
            .suffix(".toml")
            .tempfile()
            .expect("tempfile");
        std::fs::write(
            f.path(),
            r#"[compatibility]
[[compatibility.entry]]
component = "agent"
min_version = "0.0.0"
max_version = "999.0.0"
"#,
        )
        .expect("write matrix");
        f
    }

    #[tokio::test]
    async fn compat_matrix_query_failure_when_opted_in_returns_err() {
        // node_inventory does not exist in this pool → fetch_all errors.
        // Operator HAS opted in (we're calling check_compat_matrix with a
        // valid path), so the gate must fail-closed.
        let pool = empty_pool().await;
        let matrix = write_minimal_matrix();

        let result = check_compat_matrix(&pool, matrix.path()).await;

        match result {
            Err(ControlPlaneServiceError::Internal(msg)) => {
                assert!(
                    msg.contains("compat matrix query failed"),
                    "expected fail-closed message, got: {msg}"
                );
            }
            other => panic!("expected Err(Internal) from fail-closed gate, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn compat_matrix_query_failure_when_opt_out_continues() {
        // Operator has NOT opted in: in build_service that means the env var
        // CHV_COMPAT_MATRIX_PATH is unset, so check_compat_matrix is never
        // called. We model "opt-out" by simply not invoking the function and
        // asserting that build_service's surrounding flow is unaffected.
        //
        // The pure equivalent we can test directly is: when the path does
        // not exist (e.g., CHV_COMPAT_MATRIX_PATH points at a not-yet-deployed
        // file), we warn-and-continue rather than fail. This matches the
        // documented "configured-but-not-yet-deployed is OK" behavior and
        // preserves the contract that an unset env var means no gate.
        let pool = empty_pool().await;
        let missing_path =
            std::path::PathBuf::from("/nonexistent/compat-matrix-do-not-create.toml");

        let result = check_compat_matrix(&pool, &missing_path).await;
        assert!(
            result.is_ok(),
            "missing matrix file must NOT fail boot (opt-out / not-yet-deployed path), got: {result:?}"
        );
    }
}
