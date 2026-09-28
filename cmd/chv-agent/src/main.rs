use chv_agent_core::{
    agent_server::AgentServer,
    cache::{NodeCache, PendingControlPlaneMessage},
    config::{load_agent_config, AgentAuthorityMode, AgentConfig},
    connectivity::{ConnectivityState, ConnectivityTracker},
    console_server::ConsoleServer,
    control_plane::ControlPlaneClient,
    daemon_clients::{NwdClient, StordClient},
    enrollment::EnrollmentClient,
    health::HealthAggregator,
    inventory::InventoryReporter,
    metrics_server::{metrics_router, MetricsState},
    projection::ProjectingCoreRuntime,
    reconcile::Reconciler,
    state_machine::NodeState,
    supervisor::DaemonSupervisor,
    telemetry::TelemetryReporter,
    vm_runtime::VmRuntime,
};
use chv_agent_runtime_ch::process::ProcessCloudHypervisorAdapter;
use chv_errors::ChvError;
use chv_observability::init_logger;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::unix::{signal, SignalKind};
use tracing::{info, warn};

const FAILED_THRESHOLD: u32 = 6; // 6 ticks * 5s = 30s
/// How often the composition-internal journal poller drives `scan_ready`.
const CORE_SCAN_INTERVAL: Duration = Duration::from_millis(250);
/// Upper bound on a single `scan_ready`. A wedged authority/store can hang the
/// scan RPC; the timeout turns a hang into a counted scan failure (health goes
/// unhealthy, backoff engages) instead of wedging the poller and shutdown.
const CORE_SCAN_TIMEOUT: Duration = Duration::from_secs(5);
/// Upper bound on the graceful journal drain at shutdown. The CH adapter bounds
/// individual ops at ~10s internally, so 60s covers a stalled batch; beyond
/// that the executor is explicitly cancelled (fail-closed -> InspectRequired).
const CORE_EXECUTOR_DRAIN_BUDGET: Duration = Duration::from_secs(60);
const CERT_ROTATION_INTERVAL_SECS: i64 = 12 * 60 * 60;

/// Write `contents` to `path` with mode 0600, normalizing the permissions of
/// an existing file as well. Used for TLS private key material.
/// Durable, atomic file publish for node-local security material: write to
/// a private temp sibling, fsync, rename into place, fsync the parent
/// directory. A crash or power loss mid-write never leaves a truncated
/// key/cert at the target path (the previous version, if any, stays intact
/// until the rename commits).
async fn write_file_durable(path: &Path, contents: &[u8], mode: u32) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path has no parent directory",
        )
    })?;
    let temp = std::path::PathBuf::from(format!("{}.tmp-{}", path.display(), std::process::id()));
    let write = async {
        // `.write(true)` is required: OpenOptions defaults to read-only
        // access, and create/truncate without write access fails at the
        // library level with InvalidInput before any file is created (a
        // defect the R1 review caught in the first version of this
        // function — every enrollment/rotation write silently failed).
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .mode(mode)
            .create(true)
            .truncate(true)
            .open(&temp)
            .await?;
        tokio::io::AsyncWriteExt::write_all(&mut file, contents).await?;
        file.sync_all().await?;
        Ok::<(), std::io::Error>(())
    }
    .await;
    match write {
        Ok(()) => {}
        Err(error) => {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(error);
        }
    }
    if let Err(error) = tokio::fs::rename(&temp, path).await {
        // The publish did not happen: clean up the temp sibling so a
        // transient failure does not leak *.tmp-{pid} files.
        let _ = tokio::fs::remove_file(&temp).await;
        return Err(error);
    }
    // Persist the rename itself: fsync the parent directory (O_RDONLY is
    // sufficient for a directory fsync on Linux). Best-effort by
    // necessity — the rename has already committed, so the target IS
    // updated; returning an error here would make callers report the
    // material as unwritten while it exists on disk (enrollment would
    // refuse to mark itself complete). A failed dir-fsync weakens the
    // crash-durability guarantee to that of a plain rename; log loudly.
    let dir_fsync = async {
        let directory = tokio::fs::File::open(parent).await?;
        directory.sync_all().await?;
        Ok::<(), std::io::Error>(())
    }
    .await;
    if let Err(error) = dir_fsync {
        tracing::warn!(
            path = %path.display(),
            error = %error,
            "published file but failed to fsync parent directory (crash durability weakened)"
        );
    }
    Ok(())
}

/// Private (0600) variant of [`write_file_durable`] for key material.
async fn write_private_file(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    write_file_durable(path, contents, 0o600).await
}

async fn start_core_managed(
    config: &AgentConfig,
    adapter: Arc<dyn chv_agent_runtime_ch::adapter::CloudHypervisorAdapter>,
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    cache_path: PathBuf,
) -> Result<cellhv_core_runtime_owner::CoreRuntimeOwner, Box<dyn std::error::Error>> {
    let paths = cellhv_core_startup::StartupPaths {
        node_cache: config.cache_path.clone(),
        core_database: config.core_store_path.clone(),
        node_cache_archive: config.core_archive_path.clone(),
    };
    let configured_seed = match config.node_id.trim() {
        "" => None,
        node_id => Some(node_id.to_owned()),
    };
    let activated =
        cellhv_core_startup::StartupTransaction::begin(&paths)?.activate(configured_seed, None)?;
    // M2.2b startup rebuild: seed NodeCache's VM axis from the Core store's
    // authoritative VM list BEFORE the executor poller starts, so a
    // crash-recovery operation can never race the rebuild (a projection that
    // landed after the snapshot would otherwise be wiped and never re-added
    // while the Reconciler is observe-only). On any open/list/rebuild/save
    // failure we warn and continue: the Reconciler and the legacy desired-state
    // RPCs have no mutation surface in core-managed mode (M2.3), so a stale
    // compatibility cache cannot silently launch a second authority.
    match activated.service().vms() {
        Ok(rebuild_vms) => {
            let mut cache = cache.lock().await;
            cache.rebuild_from_core(&rebuild_vms);
            if let Err(e) = cache.save(&cache_path).await {
                warn!(error = %e, "core startup rebuild: failed to persist rebuilt NodeCache");
            }
        }
        Err(e) => {
            warn!(error = %e, "core startup rebuild: skipping NodeCache rebuild from Core");
        }
    }
    let resources = Arc::new(chv_agent_core::resources::AgentResourceController::new(
        config.stord_socket.clone(),
        config.nwd_socket.clone(),
    ));
    let runtime = Arc::new(
        chv_agent_runtime_ch::core_runtime::CloudHypervisorCoreRuntime::new(
            adapter,
            resources,
            config.runtime_dir.clone(),
        ),
    );
    // M2.2b: wrap the single effector with the NodeCache compatibility
    // projection — Succeeded Core outcomes are projected into NodeCache and
    // persisted before the executor finishes the operation.
    let projecting = Arc::new(ProjectingCoreRuntime::new(
        runtime,
        cache.clone(),
        cache_path,
    ));
    Ok(cellhv_core_runtime_owner::CoreRuntimeOwner::start(
        projecting,
        activated,
        &config.core_api_socket_path,
        128,
        Duration::from_secs(2),
        cellhv_core_runtime_owner::JournalPollerConfig {
            scan_interval: CORE_SCAN_INTERVAL,
            scan_timeout: CORE_SCAN_TIMEOUT,
            drain_budget: CORE_EXECUTOR_DRAIN_BUDGET,
        },
    )
    .await?)
}

async fn start_core_native(
    config: &AgentConfig,
) -> Result<cellhv_core_runtime_owner::CoreRuntimeOwner, Box<dyn std::error::Error>> {
    let paths = cellhv_core_startup::StartupPaths {
        node_cache: config.cache_path.clone(),
        core_database: config.core_store_path.clone(),
        node_cache_archive: config.core_archive_path.clone(),
    };
    let configured_seed = match config.node_id.trim() {
        "" => None,
        node_id => Some(node_id.to_owned()),
    };
    let activated = cellhv_core_startup::StartupTransaction::begin(&paths)?
        .activate_native_only(configured_seed)?;
    let adapter: Arc<dyn chv_agent_runtime_ch::adapter::CloudHypervisorAdapter> = Arc::new(
        chv_agent_runtime_ch::process::ProcessCloudHypervisorAdapter::new(&config.chv_binary_path),
    );
    let resources = Arc::new(chv_agent_core::resources::AgentResourceController::new(
        config.stord_socket.clone(),
        config.nwd_socket.clone(),
    ));
    let runtime = Arc::new(
        chv_agent_runtime_ch::core_runtime::CloudHypervisorCoreRuntime::new(
            adapter,
            resources,
            config.runtime_dir.clone(),
        ),
    );
    Ok(cellhv_core_runtime_owner::CoreRuntimeOwner::start(
        runtime,
        activated,
        &config.core_api_socket_path,
        128,
        Duration::from_secs(2),
        cellhv_core_runtime_owner::JournalPollerConfig {
            scan_interval: CORE_SCAN_INTERVAL,
            scan_timeout: CORE_SCAN_TIMEOUT,
            drain_budget: CORE_EXECUTOR_DRAIN_BUDGET,
        },
    )
    .await?)
}

async fn run_core_native(config: &AgentConfig) -> Result<(), Box<dyn std::error::Error>> {
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let owner = start_core_native(config).await?;
    info!(socket = %owner.socket_path().display(), "core-native authority ready");
    let mut fatality_check = tokio::time::interval(Duration::from_millis(500));
    fatality_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                info!("received SIGTERM, shutting down core-native authority");
                break;
            }
            _ = sigint.recv() => {
                info!("received SIGINT, shutting down core-native authority");
                break;
            }
            _ = fatality_check.tick() => {
                // A fatally terminated executor must fail the process: the
                // authority would keep acknowledging operations that are
                // never executed. Exit non-zero so the supervisor restarts
                // the agent.
                if owner.executor_fatal() {
                    tracing::error!(
                        "core journal executor terminated fatally — exiting for supervisor restart"
                    );
                    owner.shutdown().await?;
                    return Err("core journal executor terminated fatally".into());
                }
            }
        }
    }
    owner.shutdown().await?;
    Ok(())
}

fn initial_node_id(config: &AgentConfig) -> String {
    if config.node_id.is_empty() {
        "unknown".to_string()
    } else {
        config.node_id.clone()
    }
}

async fn load_or_initialize_cache(config: &AgentConfig) -> NodeCache {
    // Fail fast and loud when the cache parent is unusable: a missing
    // directory would otherwise make every later save fail as a per-tick
    // warning while the node re-enrolls on each boot (state silently
    // non-persistent). Creating the parent here is best-effort; a
    // permission problem still surfaces through the first save's warn.
    if let Some(parent) = config.cache_path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            warn!(parent = %parent.display(), error = %e, "cannot create cache parent directory");
        }
    }
    match NodeCache::load(&config.cache_path).await {
        Ok(cache) => {
            info!(
                node_id = %cache.node_id,
                node_state = %cache.node_state,
                "loaded cache"
            );
            cache
        }
        Err(chv_errors::ChvError::NotFound { .. }) => NodeCache::new(initial_node_id(config)),
        Err(e) => {
            warn!(error = %e, "failed to load cache, starting fresh");
            NodeCache::new(initial_node_id(config))
        }
    }
}

fn now_unix_ms() -> i64 {
    chv_common::now_unix_ms()
}

async fn resolve_tls_paths(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    config: &AgentConfig,
) -> (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) {
    let cache = cache.lock().await;
    let tls_cert = cache
        .certificate_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| config.tls_cert_path.clone());
    let tls_key = cache
        .private_key_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| config.tls_key_path.clone());
    let ca_cert = cache
        .ca_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| config.ca_cert_path.clone());
    (tls_cert, tls_key, ca_cert)
}

async fn connect_control_plane(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    config: &AgentConfig,
) -> Result<ControlPlaneClient, ChvError> {
    let (tls_cert, tls_key, ca_cert) = resolve_tls_paths(cache, config).await;
    ControlPlaneClient::new(
        &config.control_plane_addr,
        tls_cert.as_deref(),
        tls_key.as_deref(),
        ca_cert.as_deref(),
    )
    .await
}

async fn connect_enrollment_client(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    config: &AgentConfig,
) -> Result<EnrollmentClient, ChvError> {
    let (tls_cert, tls_key, ca_cert) = resolve_tls_paths(cache, config).await;
    EnrollmentClient::connect_with_tls(
        &config.control_plane_addr,
        tls_cert.as_deref(),
        tls_key.as_deref(),
        ca_cert.as_deref(),
    )
    .await
}

async fn enqueue_pending_message(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    cache_path: &Path,
    message: PendingControlPlaneMessage,
    connectivity: &mut ConnectivityTracker,
) {
    connectivity.record_message_deferred();
    let mut cache = cache.lock().await;
    cache.enqueue_pending_message(message);
    if let Err(e) = cache.save(cache_path).await {
        warn!(error = %e, "failed to save cache after queueing deferred control-plane message");
    }
}

async fn flush_pending_messages(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    cache_path: &Path,
    client: &mut ControlPlaneClient,
) -> Result<(), ChvError> {
    let had_pending = {
        let cache = cache.lock().await;
        !cache.pending_control_plane_messages().is_empty()
    };
    if !had_pending {
        return Ok(());
    }

    let mut cache = cache.lock().await;
    client.flush_pending_messages(&mut cache).await?;
    cache.save(cache_path).await?;
    Ok(())
}

async fn send_or_defer_control_plane_message(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    cache_path: &Path,
    config: &AgentConfig,
    telemetry: &mut Option<ControlPlaneClient>,
    message: PendingControlPlaneMessage,
    connectivity: &mut ConnectivityTracker,
) {
    let now = now_unix_ms();
    if telemetry.is_none() {
        match connect_control_plane(cache, config).await {
            Ok(mut client) => {
                info!("connected to control plane");
                if let Err(e) = flush_pending_messages(cache, cache_path, &mut client).await {
                    warn!(error = %e, "failed to flush deferred control-plane messages");
                    *telemetry = None;
                    connectivity.record_failure(now);
                    enqueue_pending_message(cache, cache_path, message, connectivity).await;
                    return;
                }
                connectivity.record_success(now);
                *telemetry = Some(client);
            }
            Err(e) => {
                warn!(error = %e, "control plane unavailable; deferring report");
                connectivity.record_failure(now);
                enqueue_pending_message(cache, cache_path, message, connectivity).await;
                return;
            }
        }
    }

    let Some(client) = telemetry.as_mut() else {
        connectivity.record_failure(now);
        enqueue_pending_message(cache, cache_path, message, connectivity).await;
        return;
    };

    if let Err(e) = client.dispatch_pending_message(&message).await {
        warn!(error = %e, "failed to send control-plane message, deferring");
        *telemetry = None;
        connectivity.record_failure(now);
        enqueue_pending_message(cache, cache_path, message, connectivity).await;
    } else {
        connectivity.record_success(now);
    }
}

fn certificate_rotation_due(cache: &NodeCache, now_unix_ms: i64) -> bool {
    if !cache.enrollment_complete {
        return false;
    }
    let (cert_path, key_path) = match (
        cache.certificate_path.as_ref(),
        cache.private_key_path.as_ref(),
    ) {
        (Some(c), Some(k)) => (c, k),
        _ => return false,
    };
    if !std::path::Path::new(cert_path).exists() || !std::path::Path::new(key_path).exists() {
        return false;
    }
    cache
        .last_certificate_rotation_unix_ms
        .map(|last| now_unix_ms - last >= CERT_ROTATION_INTERVAL_SECS * 1000)
        .unwrap_or(true)
}

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

    let config_path = std::env::args().nth(1).map(PathBuf::from);
    let config = load_agent_config(config_path.as_deref())?;

    init_logger(&config.log_level)?;

    info!(
        "{} starting (version {}, commit {}, channel {})",
        env!("CARGO_PKG_NAME"),
        env!("CHV_VERSION"),
        env!("CHV_GIT_SHA"),
        env!("CHV_RELEASE_CHANNEL"),
    );

    if config.authority_mode == AgentAuthorityMode::CoreNative {
        return run_core_native(&config).await;
    }

    let mut cache = load_or_initialize_cache(&config).await;

    // Enrollment
    if !cache.enrollment_complete {
        if let Some(token_path) = &config.bootstrap_token_path {
            match tokio::fs::read_to_string(token_path).await {
                Ok(token) => {
                    let token = token.trim();
                    let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname")
                        .unwrap_or_else(|_| "unknown".to_string())
                        .trim()
                        .to_string();
                    let reporter = InventoryReporter::with_storage_base_dir(
                        &cache.node_id,
                        &hostname,
                        &config.storage_base_dir,
                    );
                    let inventory = reporter.build_inventory();
                    let versions = reporter.build_versions();
                    let tls_cert = cache
                        .certificate_path
                        .as_ref()
                        .map(PathBuf::from)
                        .or_else(|| config.tls_cert_path.clone());
                    let tls_key = cache
                        .private_key_path
                        .as_ref()
                        .map(PathBuf::from)
                        .or_else(|| config.tls_key_path.clone());
                    let ca_cert = cache
                        .ca_path
                        .as_ref()
                        .map(PathBuf::from)
                        .or_else(|| config.ca_cert_path.clone());
                    match EnrollmentClient::connect_with_tls(
                        &config.control_plane_addr,
                        tls_cert.as_deref(),
                        tls_key.as_deref(),
                        ca_cert.as_deref(),
                    )
                    .await
                    {
                        Ok(mut client) => {
                            match client.enroll_node(token, inventory, versions).await {
                                Ok(resp) => {
                                    let cert_path = config.runtime_dir.join("agent.crt");
                                    let key_path = config.runtime_dir.join("agent.key");
                                    let ca_path = config.runtime_dir.join("ca.crt");
                                    if let Err(e) =
                                        tokio::fs::create_dir_all(&config.runtime_dir).await
                                    {
                                        warn!(error = %e, "failed to create runtime dir");
                                    } else {
                                        let mut ok = true;
                                        if let Err(e) = write_file_durable(
                                            &cert_path,
                                            &resp.certificate_pem,
                                            0o644,
                                        )
                                        .await
                                        {
                                            warn!(error = %e, "failed to write certificate");
                                            ok = false;
                                        }
                                        if let Err(e) =
                                            write_private_file(&key_path, &resp.private_key_pem)
                                                .await
                                        {
                                            warn!(error = %e, "failed to write private key");
                                            ok = false;
                                        }
                                        if let Err(e) =
                                            write_file_durable(&ca_path, &resp.ca_pem, 0o644).await
                                        {
                                            warn!(error = %e, "failed to write ca certificate");
                                            ok = false;
                                        }
                                        if ok {
                                            cache.node_id = resp.node_id.clone();
                                            cache.certificate_path =
                                                Some(cert_path.to_string_lossy().to_string());
                                            cache.private_key_path =
                                                Some(key_path.to_string_lossy().to_string());
                                            cache.ca_path =
                                                Some(ca_path.to_string_lossy().to_string());
                                            cache.last_certificate_rotation_unix_ms =
                                                Some(now_unix_ms());
                                            cache.enrollment_complete = true;
                                            if let Err(e) = cache.save(&config.cache_path).await {
                                                warn!(error = %e, "failed to save cache after enrollment");
                                            } else {
                                                info!(node_id = %resp.node_id, "enrollment complete");
                                            }

                                            let meta =
                                                control_plane_node_api::control_plane_node_api::RequestMeta {
                                                    operation_id: format!(
                                                        "bootstrap-result-{}",
                                                        resp.node_id
                                                    ),
                                                    requested_by: "agent".to_string(),
                                                    target_node_id: resp.node_id.clone(),
                                                    desired_state_version: String::new(),
                                                    request_unix_ms: now_unix_ms(),
                                                };
                                            if let Err(e) = client
                                                .report_bootstrap_result(
                                                    meta,
                                                    &resp.node_id,
                                                    "ok",
                                                    "bootstrap complete",
                                                )
                                                .await
                                            {
                                                warn!(error = %e, "failed to report bootstrap result");
                                            }
                                        } else {
                                            let meta =
                                                control_plane_node_api::control_plane_node_api::RequestMeta {
                                                    operation_id: format!(
                                                        "bootstrap-result-{}",
                                                        resp.node_id
                                                    ),
                                                    requested_by: "agent".to_string(),
                                                    target_node_id: resp.node_id.clone(),
                                                    desired_state_version: String::new(),
                                                    request_unix_ms: now_unix_ms(),
                                                };
                                            if let Err(e) = client
                                                .report_bootstrap_result(
                                                    meta,
                                                    &resp.node_id,
                                                    "failed",
                                                    "failed to persist enrollment material",
                                                )
                                                .await
                                            {
                                                warn!(error = %e, "failed to report bootstrap result");
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(error = %e, "enrollment failed");
                                }
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "failed to connect to enrollment endpoint");
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, path = %token_path.display(), "failed to read bootstrap token");
                }
            }
        }
    }

    // Enforce mTLS post-enrollment (unless dev mode)
    let allow_insecure = std::env::var("CHV_ALLOW_INSECURE")
        .map(|v| v == "1")
        .unwrap_or(false);
    if !allow_insecure && cache.enrollment_complete {
        let has_certs = cache.certificate_path.is_some()
            && cache.private_key_path.is_some()
            && cache.ca_path.is_some();
        if !has_certs {
            return Err(
                "mTLS required: agent is enrolled but TLS credentials are missing. Set CHV_ALLOW_INSECURE=1 for dev"
                    .into(),
            );
        }
    }

    let adapter: Arc<dyn chv_agent_runtime_ch::adapter::CloudHypervisorAdapter> =
        Arc::new(ProcessCloudHypervisorAdapter::new(&config.chv_binary_path));
    let vm_runtime = VmRuntime::new(adapter.clone());

    let cache = Arc::new(tokio::sync::Mutex::new(cache));

    let mut core_owner = None;
    if config.authority_mode == AgentAuthorityMode::CoreManaged {
        let owner =
            start_core_managed(&config, adapter.clone(), &cache, config.cache_path.clone()).await?;
        core_owner = Some(owner);
    }

    let mut agent_server = AgentServer::new(
        cache.clone(),
        vm_runtime.clone(),
        config.stord_socket.clone(),
        config.nwd_socket.clone(),
        Some(config.cache_path.clone()),
        config.runtime_dir.clone(),
    );
    if let Some(owner) = &core_owner {
        agent_server = agent_server.with_core_authority(owner.authority());
    }
    // Share the migration registry with the reconciler so drain evacuation
    // can gate on in-flight migrations (see reconcile.rs Draining arm).
    let migration_registry = agent_server.migration_tasks.clone();
    let server_socket = config.socket_path.clone();
    let agent_server_clone = agent_server.clone();
    let mut agent_server_handle = tokio::spawn(async move {
        if let Err(e) = agent_server_clone.serve(&server_socket).await {
            tracing::error!(error = %e, "agent server exited with error — node is unreachable");
        }
    });

    let console_bind = config.console_bind.clone();
    let console_listener = ConsoleServer::try_bind(&console_bind).await.map_err(|e| {
        tracing::error!(
            bind = %console_bind,
            error = %e,
            "FATAL: console server cannot bind"
        );
        e
    })?;
    let console_server = ConsoleServer::new(vm_runtime.clone(), config.jwt_secret.clone());
    tokio::spawn(async move {
        if let Err(e) = console_server.run(console_listener).await {
            warn!(error = %e, bind = %console_bind, "console server exited");
        }
    });

    // Metrics HTTP server for Prometheus scraping
    let metrics_bind_addr = config
        .metrics_bind
        .clone()
        .unwrap_or_else(|| "0.0.0.0:9100".to_string());
    let metrics_state = Arc::new(tokio::sync::Mutex::new(MetricsState::new(
        cache.lock().await.node_id.clone(),
    )));
    let metrics_state_clone = metrics_state.clone();
    tokio::spawn(async move {
        let app = metrics_router(metrics_state_clone);
        let listener = match tokio::net::TcpListener::bind(&metrics_bind_addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(bind = %metrics_bind_addr, error = %e, "metrics server cannot bind");
                return;
            }
        };
        info!(bind = %metrics_bind_addr, "metrics server listening");
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!(error = %e, "metrics server exited with error");
        }
    });

    // The Reconciler drives the node state machine (including daemon health
    // probes) in every mode. Its provider-MUTATION surface is mode-selected at
    // construction (M2.3): core-managed builds an observe-only Reconciler that
    // structurally holds no mutation state and has no setter, so it can never
    // act as a second authority — the Core runtime is the sole provider
    // effector (NodeCache is rebuilt from the Core store and projected only
    // after Core execution). Legacy mode keeps the full mutation surface.
    let mut reconciler = match config.authority_mode {
        // Core-managed: observe-only Reconciler (no mutation surface at all).
        AgentAuthorityMode::CoreManaged => {
            Reconciler::new_observe_only(
                cache.clone(),
                vm_runtime.clone(),
                config.stord_socket.clone(),
                config.nwd_socket.clone(),
                migration_registry,
            )
            .await
        }
        // Legacy: the full legacy provider-mutation surface (explicit opt-in).
        AgentAuthorityMode::Legacy => {
            Reconciler::new_legacy(
                cache.clone(),
                vm_runtime.clone(),
                config.stord_socket.clone(),
                config.nwd_socket.clone(),
                config.runtime_dir.clone(),
                migration_registry,
            )
            .await
        }
        // CoreNative returns in run_core_native() well before this point; a
        // future variant here becomes a compile error instead of silently
        // defaulting to the mutation-capable Reconciler.
        AgentAuthorityMode::CoreNative => {
            unreachable!("CoreNative mode exited normally before Reconciler composition")
        }
    };

    let mut supervisor = DaemonSupervisor::new(
        config.stord_binary_path.clone(),
        config.nwd_binary_path.clone(),
        config.stord_socket.clone(),
        config.nwd_socket.clone(),
        config.runtime_dir.clone(),
    );

    if let Err(e) = supervisor.start_all().await {
        warn!(error = %e, "failed to start local daemons during bootstrap");
    }

    let mut telemetry = match connect_control_plane(&cache, &config).await {
        Ok(client) => {
            info!("connected to control plane");
            Some(client)
        }
        Err(e) => {
            warn!(error = %e, "control plane unavailable; will retry later");
            None
        }
    };

    // Connectivity tracker — makes partition-autonomous operation explicit and observable.
    let mut connectivity = ConnectivityTracker::new();
    if telemetry.is_some() {
        connectivity.record_success(now_unix_ms());
    }

    let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string();
    let node_id = cache.lock().await.node_id.clone();
    let inventory_reporter =
        InventoryReporter::with_storage_base_dir(&node_id, hostname, &config.storage_base_dir);
    let mut tick_count = 0u64;
    let mut consecutive_health_failures = 0u32;
    let mut consecutive_reconcile_failures: u32 = 0;
    let mut prev_connectivity = connectivity.state();

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    let mut interval = tokio::time::interval(Duration::from_secs(5));
    // Fatality watchdog: a fatally terminated Core executor must fail the
    // process promptly (≤500 ms, matching the core-native loop's cadence),
    // not on the next 5 s tick — the authority would otherwise keep
    // acknowledging operations that are never executed for up to 5 s.
    let mut fatality_check = tokio::time::interval(Duration::from_millis(500));
    loop {
        tokio::select! {
            _ = interval.tick() => {}
            _ = fatality_check.tick() => {
                // The health aggregation and telemetry paths have already
                // marked the node Degraded and reported it; exit non-zero so
                // the supervisor restarts the agent.
                if core_owner
                    .as_ref()
                    .is_some_and(|owner| owner.executor_fatal())
                {
                    tracing::error!(
                        "core journal executor terminated fatally — shutting down for supervisor restart"
                    );
                    supervisor.shutdown().await;
                    if let Some(owner) = core_owner.take() {
                        let _ = owner.shutdown().await;
                    }
                    return Err("core journal executor terminated fatally".into());
                }
                // Watchdog tick only: skip the 5 s body's heavy work.
                continue;
            }
            _ = sigterm.recv() => {
                info!("received SIGTERM, shutting down gracefully");
                supervisor.shutdown().await;
                if let Some(owner) = core_owner.take() { let _ = owner.shutdown().await; }
                break;
            }
            _ = sigint.recv() => {
                info!("received SIGINT, shutting down gracefully");
                supervisor.shutdown().await;
                if let Some(owner) = core_owner.take() { let _ = owner.shutdown().await; }
                break;
            }
            _ = &mut agent_server_handle => {
                tracing::error!("agent gRPC server exited unexpectedly — shutting down");
                supervisor.shutdown().await;
                if let Some(owner) = core_owner.take() { let _ = owner.shutdown().await; }
                // Exit non-zero: the systemd unit uses Restart=on-failure,
                // and in core-managed mode this process is the sole Core
                // authority — a clean exit here would strand the node with
                // no supervisor recovery (mirrors the executor-fatal path).
                return Err("agent gRPC server exited unexpectedly".into());
            }
        }

        if let Err(e) = supervisor.restart_if_needed().await {
            warn!(error = %e, "supervisor restart failed");
        }

        // Detect connectivity transition: Disconnected/Reconnecting -> Connected
        // and flush pending messages immediately on reconnect.
        let current_connectivity = connectivity.state();
        if current_connectivity == ConnectivityState::Connected
            && prev_connectivity != ConnectivityState::Connected
        {
            info!("connectivity restored, flushing pending messages");
            if let Some(client) = telemetry.as_mut() {
                if let Err(e) = flush_pending_messages(&cache, &config.cache_path, client).await {
                    warn!(error = %e, "failed to flush pending messages on reconnect");
                }
            }
        }
        prev_connectivity = current_connectivity;

        let now = now_unix_ms();
        let rotate_due = {
            let cache = cache.lock().await;
            certificate_rotation_due(&cache, now)
        };
        if rotate_due {
            match connect_enrollment_client(&cache, &config).await {
                Ok(mut client) => {
                    let node_id = cache.lock().await.node_id.clone();
                    let meta = control_plane_node_api::control_plane_node_api::RequestMeta {
                        operation_id: format!("cert-rotate-{}", tick_count),
                        requested_by: "agent".to_string(),
                        target_node_id: node_id.clone(),
                        desired_state_version: String::new(),
                        request_unix_ms: now,
                    };
                    match client.rotate_node_certificate(meta, &node_id).await {
                        Ok(resp) => {
                            let (cert_path, key_path, ca_path) = {
                                let cache = cache.lock().await;
                                (
                                    cache.certificate_path.clone(),
                                    cache.private_key_path.clone(),
                                    cache.ca_path.clone(),
                                )
                            };
                            if let (Some(cert_path), Some(key_path), Some(ca_path)) =
                                (cert_path, key_path, ca_path)
                            {
                                let write_result = async {
                                    write_file_durable(
                                        Path::new(&cert_path),
                                        &resp.certificate_pem,
                                        0o644,
                                    )
                                    .await?;
                                    write_private_file(Path::new(&key_path), &resp.private_key_pem)
                                        .await?;
                                    write_file_durable(Path::new(&ca_path), &resp.ca_pem, 0o644)
                                        .await?;
                                    Ok::<(), std::io::Error>(())
                                }
                                .await;
                                match write_result {
                                    Ok(()) => {
                                        let mut cache = cache.lock().await;
                                        cache.last_certificate_rotation_unix_ms = Some(now);
                                        if let Err(e) = cache.save(&config.cache_path).await {
                                            warn!(error = %e, "failed to save cache after certificate rotation");
                                        }
                                    }
                                    Err(e) => {
                                        warn!(error = %e, "failed to persist rotated certificate material");
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "certificate rotation failed");
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, "failed to connect for certificate rotation");
                }
            }
        }

        let stord_ok = match StordClient::connect(&config.stord_socket).await {
            Ok(mut c) => c.health_probe().await.unwrap_or(false),
            Err(_) => false,
        };

        let nwd_ok = match NwdClient::connect(&config.nwd_socket).await {
            Ok(mut c) => c.health_probe().await.unwrap_or(false),
            Err(_) => false,
        };

        let mut health = HealthAggregator::new();
        health.update_stord(stord_ok);
        health.update_nwd(nwd_ok);
        if let Some(owner) = &core_owner {
            // A wedged Core journal must not silently pass for a healthy node:
            // accepted operations would stop executing while the API keeps
            // acknowledging them. A fatal executor termination is included
            // here even though the poller stopped scanning at that point.
            health.update_core_journal(owner.journal_scan_healthy() && !owner.executor_fatal());
        }

        let current_state = reconciler.current_state().await;
        let derived = health.derive_node_state(current_state);
        if derived != current_state {
            let from_str = current_state.as_str().to_string();
            info!(
                from = %from_str,
                to = %derived.as_str(),
                "state transition"
            );
            if let Err(e) = reconciler.transition_state(derived).await {
                warn!(error = %e, "invalid state transition ignored");
            } else {
                {
                    let cache = cache.lock().await;
                    if let Err(e) = cache.save(&config.cache_path).await {
                        warn!(error = %e, "failed to save cache");
                    }
                }
                if derived != NodeState::Degraded {
                    consecutive_health_failures = 0;
                }
                let severity = match derived {
                    NodeState::Failed => "Critical",
                    NodeState::Degraded => "Warning",
                    NodeState::TenantReady => "Info",
                    _ => "Info",
                };
                let (target_node_id, desired_state_version) = {
                    let cache = cache.lock().await;
                    (cache.node_id.clone(), cache.observed_generation.clone())
                };
                let reporter = TelemetryReporter::new(&target_node_id);
                let event = reporter.event_report(
                    control_plane_node_api::control_plane_node_api::RequestMeta {
                        operation_id: format!("state-transition-{}", tick_count),
                        requested_by: "agent".to_string(),
                        target_node_id,
                        desired_state_version,
                        request_unix_ms: now_unix_ms(),
                    },
                    severity,
                    "StateTransition",
                    &format!(
                        "node transitioned from {} to {}",
                        from_str,
                        derived.as_str()
                    ),
                );
                send_or_defer_control_plane_message(
                    &cache,
                    &config.cache_path,
                    &config,
                    &mut telemetry,
                    PendingControlPlaneMessage::event(event),
                    &mut connectivity,
                )
                .await;
            }
        } else if current_state == NodeState::Degraded {
            consecutive_health_failures += 1;
            if consecutive_health_failures >= FAILED_THRESHOLD {
                info!("health failures exceeded threshold, transitioning to Failed");
                if let Err(e) = reconciler.transition_state(NodeState::Failed).await {
                    warn!(error = %e, "failed to transition to Failed");
                } else {
                    {
                        let cache = cache.lock().await;
                        if let Err(e) = cache.save(&config.cache_path).await {
                            warn!(error = %e, "failed to save cache");
                        }
                    }
                    let (target_node_id, desired_state_version) = {
                        let cache = cache.lock().await;
                        (cache.node_id.clone(), cache.observed_generation.clone())
                    };
                    let reporter = TelemetryReporter::new(&target_node_id);
                    let event = reporter.event_report(
                        control_plane_node_api::control_plane_node_api::RequestMeta {
                            operation_id: format!("state-transition-failed-{}", tick_count),
                            requested_by: "agent".to_string(),
                            target_node_id,
                            desired_state_version,
                            request_unix_ms: now_unix_ms(),
                        },
                        "Critical",
                        "StateTransition",
                        "node transitioned to Failed after persistent health degradation",
                    );
                    send_or_defer_control_plane_message(
                        &cache,
                        &config.cache_path,
                        &config,
                        &mut telemetry,
                        PendingControlPlaneMessage::event(event),
                        &mut connectivity,
                    )
                    .await;
                }
                consecutive_health_failures = 0;
            }
        } else {
            consecutive_health_failures = 0;
        }

        let (node_id, node_state, observed_generation, last_error) = {
            let cache = cache.lock().await;
            (
                cache.node_id.clone(),
                cache.node_state.clone(),
                cache.observed_generation.clone(),
                cache.last_error.clone(),
            )
        };
        let reporter = TelemetryReporter::new(&node_id);
        let report = reporter.node_state_report(
            node_state.as_str(),
            observed_generation.as_str(),
            if reconciler.current_state().await == NodeState::TenantReady {
                "Healthy"
            } else {
                "Degraded"
            },
            last_error,
        );
        send_or_defer_control_plane_message(
            &cache,
            &config.cache_path,
            &config,
            &mut telemetry,
            PendingControlPlaneMessage::node_state(report),
            &mut connectivity,
        )
        .await;

        for vm in reconciler.reported_vms().await {
            let mut counters = control_plane_node_api::control_plane_node_api::VmStateReport {
                node_id: node_id.clone(),
                vm_id: vm.vm_id.clone(),
                runtime_status: vm.runtime_status.clone(),
                observed_generation: vm.observed_generation.clone(),
                // Core-managed telemetry reports the Core projection
                // (desired state) as runtime_status — a documented residual
                // — so observed health is genuinely not known here, and a
                // stuck (inspect-required) VM must not be reported Healthy.
                // Legacy reports keep their historical value.
                health_status: if core_owner.is_some() {
                    "Unknown"
                } else {
                    "Healthy"
                }
                .to_string(),
                last_error: vm.last_error.unwrap_or_default(),
                reported_unix_ms: now_unix_ms(),
                cpu_percent: 0.0,
                memory_bytes_used: 0,
                memory_bytes_total: 0,
                disk_bytes_read: 0,
                disk_bytes_written: 0,
                net_bytes_rx: 0,
                net_bytes_tx: 0,
            };

            if vm.runtime_status == "Running" {
                if let Ok(c) = reconciler.vm_runtime().vm_counters(&vm.vm_id).await {
                    counters.cpu_percent = c.cpu_percent;
                    counters.memory_bytes_used = c.memory_bytes_used as i64;
                    counters.memory_bytes_total = c.memory_bytes_total as i64;
                    counters.disk_bytes_read = c.disk_bytes_read as i64;
                    counters.disk_bytes_written = c.disk_bytes_written as i64;
                    counters.net_bytes_rx = c.net_bytes_rx as i64;
                    counters.net_bytes_tx = c.net_bytes_tx as i64;
                }
            }

            send_or_defer_control_plane_message(
                &cache,
                &config.cache_path,
                &config,
                &mut telemetry,
                PendingControlPlaneMessage::vm_state(counters),
                &mut connectivity,
            )
            .await;
        }

        let volume_generations: std::collections::HashMap<String, String> = {
            let cache = cache.lock().await;
            cache
                .volume_handles
                .keys()
                .map(|k| {
                    (
                        k.clone(),
                        cache.volume_generations.get(k).cloned().unwrap_or_default(),
                    )
                })
                .collect()
        };
        for (volume_id, observed_generation) in volume_generations {
            let vol_report = reporter.volume_state_report(
                &volume_id,
                "Attached",
                observed_generation.as_str(),
                "Healthy",
            );
            send_or_defer_control_plane_message(
                &cache,
                &config.cache_path,
                &config,
                &mut telemetry,
                PendingControlPlaneMessage::volume_state(vol_report),
                &mut connectivity,
            )
            .await;
        }

        let network_fragments: Vec<(String, String)> = {
            let cache = cache.lock().await;
            cache
                .network_fragments
                .iter()
                .map(|(k, v)| (k.clone(), v.generation.clone()))
                .collect()
        };
        for (network_id, generation) in network_fragments {
            let net_report =
                reporter.network_state_report(&network_id, "Ready", &generation, "Healthy");
            send_or_defer_control_plane_message(
                &cache,
                &config.cache_path,
                &config,
                &mut telemetry,
                PendingControlPlaneMessage::network_state(net_report),
                &mut connectivity,
            )
            .await;
        }

        tick_count += 1;
        if tick_count.is_multiple_of(6) {
            let op_id = format!("inventory-{}", tick_count);
            let inv = inventory_reporter.build_inventory();
            let ver = inventory_reporter.build_versions();
            let inv_req =
                control_plane_node_api::control_plane_node_api::ReportNodeInventoryRequest {
                    meta: Some(
                        control_plane_node_api::control_plane_node_api::RequestMeta {
                            operation_id: op_id.clone(),
                            requested_by: "agent".to_string(),
                            target_node_id: node_id.clone(),
                            desired_state_version: String::new(),
                            request_unix_ms: now_unix_ms(),
                        },
                    ),
                    inventory: Some(inv),
                };
            send_or_defer_control_plane_message(
                &cache,
                &config.cache_path,
                &config,
                &mut telemetry,
                PendingControlPlaneMessage::node_inventory(inv_req),
                &mut connectivity,
            )
            .await;

            let ver_req =
                control_plane_node_api::control_plane_node_api::ReportServiceVersionsRequest {
                    meta: Some(
                        control_plane_node_api::control_plane_node_api::RequestMeta {
                            operation_id: op_id,
                            requested_by: "agent".to_string(),
                            target_node_id: node_id.clone(),
                            desired_state_version: String::new(),
                            request_unix_ms: now_unix_ms(),
                        },
                    ),
                    versions: Some(ver),
                };
            send_or_defer_control_plane_message(
                &cache,
                &config.cache_path,
                &config,
                &mut telemetry,
                PendingControlPlaneMessage::service_versions(ver_req),
                &mut connectivity,
            )
            .await;
        }

        if let Err(e) = reconciler.run_once().await {
            warn!(error = %e, "reconcile tick failed");
            consecutive_reconcile_failures += 1;
            let backoff_secs = std::cmp::min(1u64 << consecutive_reconcile_failures.min(6), 60);
            tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
        } else {
            consecutive_reconcile_failures = 0;
        }

        // Periodically persist cache so gRPC mutations are durable even without state transitions.
        {
            let cache = cache.lock().await;
            if let Err(e) = cache.save(&config.cache_path).await {
                warn!(error = %e, "failed to save cache");
            }
        }

        // Update metrics state for Prometheus scraping
        {
            let mut ms = metrics_state.lock().await;
            ms.node_id = cache.lock().await.node_id.clone();
            ms.node_state = reconciler.current_state().await.as_str().to_string();
            ms.vm_count = reconciler.reported_vms().await.len();
            ms.tick_count = tick_count;
            ms.reconcile_failures = consecutive_reconcile_failures;
            ms.health_failures = consecutive_health_failures;
            let cp_snapshot = connectivity.metrics_snapshot(now_unix_ms());
            ms.cp_connectivity_state = cp_snapshot.state;
            ms.cp_disconnected_duration_ms = cp_snapshot.disconnected_duration_ms;
            ms.cp_consecutive_failures = cp_snapshot.consecutive_failures;
            ms.cp_total_deferred_messages = cp_snapshot.total_deferred_messages;
            // Core journal health: emitted only when the core-managed owner
            // exists (legacy mode has no journal poller — the metrics are
            // absent, not zeroed).
            ms.core_journal = core_owner.as_ref().map(|owner| {
                chv_agent_core::metrics_server::CoreJournalMetrics {
                    scan_failures_total: owner.journal_scan_failures(),
                    healthy: owner.journal_scan_healthy() && !owner.executor_fatal(),
                    inspect_required: owner.journal_inspect_required_count(),
                }
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn core_config(directory: &tempfile::TempDir) -> AgentConfig {
        let cache = directory.path().join("cache");
        let core = directory.path().join("core");
        let run = directory.path().join("run");
        for path in [&cache, &core, &run] {
            std::fs::create_dir(path).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        AgentConfig {
            authority_mode: AgentAuthorityMode::CoreNative,
            cache_path: cache.join("agent-cache.json"),
            core_store_path: core.join("core.db"),
            core_archive_path: core.join("node-cache.archive"),
            core_api_socket_path: run.join("core.sock"),
            node_id: "native-test-host".to_owned(),
            ..AgentConfig::default()
        }
    }

    async fn unix_http(config: &AgentConfig, request: &str) -> String {
        let mut stream = tokio::net::UnixStream::connect(&config.core_api_socket_path)
            .await
            .unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        response
    }

    #[tokio::test]
    async fn write_file_durable_publishes_content_and_leaves_no_temp_files() {
        // Regression pin for the R1-review MAJOR: the first version of
        // write_file_durable omitted `.write(true)`, so OpenOptions failed
        // with InvalidInput before any file was created and EVERY
        // enrollment/rotation write silently failed (the node would have
        // re-enrolled with a fresh node_id on every boot). This test drives
        // the real function so a recurrence fails CI instead of production.
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("agent.crt");

        write_file_durable(&target, b"certificate-bytes", 0o644)
            .await
            .expect("durable write must succeed");
        let persisted = tokio::fs::read(&target).await.unwrap();
        assert_eq!(persisted, b"certificate-bytes");

        // The private-key variant must land owner-only.
        let key = directory.path().join("agent.key");
        write_private_file(&key, b"key-bytes").await.unwrap();
        let mode = std::fs::metadata(&key).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);

        // Overwrite (rotation) replaces the previous version atomically.
        write_file_durable(&target, b"certificate-bytes-v2", 0o644)
            .await
            .unwrap();
        let persisted = tokio::fs::read(&target).await.unwrap();
        assert_eq!(persisted, b"certificate-bytes-v2");

        // No temp siblings are leaked after success.
        let mut entries = tokio::fs::read_dir(directory.path()).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(!name.contains(".tmp-"), "leaked temp file: {name}");
        }
    }

    #[tokio::test]
    async fn core_native_http_create_survives_restart_and_excludes_second_instance() {
        let directory = tempfile::tempdir().unwrap();
        let config = core_config(&directory);
        let owner = start_core_native(&config).await.unwrap();
        assert!(start_core_native(&config).await.is_err());
        let body = serde_json::json!({"request_id":"create-1","definition":{
            "id":"vm-1","name":"vm-1","boot":{"kernel":"/kernel","firmware":null,"initial_disk":null},
            "compute":{"vcpus":1,"memory_bytes":1048576},"storage":[],"networks":[],
            "requested_power_state":"stopped","observed_power_state":"unknown","resource_version":1
        }}).to_string();
        let response = unix_http(
            &config,
            &format!("POST /v1/vms HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nIdempotency-Key: create-vm-1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body),
        ).await;
        assert!(response.starts_with("HTTP/1.1 202"), "{response}");
        owner.shutdown().await.unwrap();

        let owner = start_core_native(&config).await.unwrap();
        let response = unix_http(
            &config,
            "GET /v1/vms HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("vm-1"));
        owner.shutdown().await.unwrap();
        assert!(!config.cache_path.exists());
    }

    #[tokio::test]
    async fn core_native_concurrent_identical_creates_commit_once() {
        let directory = tempfile::tempdir().unwrap();
        let config = core_config(&directory);
        let owner = start_core_native(&config).await.unwrap();
        let body = serde_json::json!({"request_id":"create-1","definition":{
            "id":"vm-1","name":"vm-1","boot":{"kernel":"/kernel","firmware":null,"initial_disk":null},
            "compute":{"vcpus":1,"memory_bytes":1048576},"storage":[],"networks":[],
            "requested_power_state":"stopped","observed_power_state":"unknown","resource_version":1
        }}).to_string();
        let request = format!("POST /v1/vms HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nIdempotency-Key: create-vm-1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);

        async fn send(socket: &std::path::Path, request: &str) -> String {
            let mut stream = tokio::net::UnixStream::connect(socket).await.unwrap();
            stream.write_all(request.as_bytes()).await.unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).await.unwrap();
            response
        }

        // Two concurrent, byte-identical submissions race the acceptance
        // path. The store's transactional idempotency resolution must let
        // exactly one accept and turn the other into a replay of the same
        // operation: never two operations, never a double creation, never
        // a 5xx from a lost acceptance race.
        let (first, second) = tokio::join!(
            send(&config.core_api_socket_path, &request),
            send(&config.core_api_socket_path, &request)
        );
        let mut dispositions = Vec::new();
        for response in [&first, &second] {
            assert!(response.starts_with("HTTP/1.1 202"), "{response}");
            let body: serde_json::Value =
                serde_json::from_str(response.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            dispositions.push(
                body["disposition"]
                    .as_str()
                    .expect("disposition in acceptance body")
                    .to_owned(),
            );
        }
        dispositions.sort();
        assert_eq!(dispositions, vec!["accepted", "replay"]);

        // Exactly one creation committed: the listing contains the VM once.
        let listing = send(
            &config.core_api_socket_path,
            "GET /v1/vms HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await;
        assert!(listing.starts_with("HTTP/1.1 200"), "{listing}");
        let entries: serde_json::Value =
            serde_json::from_str(listing.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        let count = entries
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["id"] == "vm-1")
            .count();
        assert_eq!(count, 1, "{entries}");
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn core_native_refuses_legacy_cache_without_creating_core_state() {
        let directory = tempfile::tempdir().unwrap();
        let config = core_config(&directory);
        std::fs::write(&config.cache_path, b"{}").unwrap();
        std::fs::set_permissions(&config.cache_path, std::fs::Permissions::from_mode(0o600))
            .unwrap();
        assert!(start_core_native(&config).await.is_err());
        assert!(!config.core_store_path.exists());
        assert!(!config.core_api_socket_path.exists());
    }

    #[tokio::test]
    async fn core_native_treats_whitespace_node_id_as_absent() {
        let directory = tempfile::tempdir().unwrap();
        let mut config = core_config(&directory);
        config.node_id = "  \t ".to_owned();
        let owner = start_core_native(&config).await.unwrap();
        let response = unix_http(
            &config,
            "GET /v1/host HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await;
        assert!(response.starts_with("HTTP/1.1 200"));
        let body = response.split("\r\n\r\n").nth(1).unwrap();
        let body: serde_json::Value = serde_json::from_str(body).unwrap();
        let id = body["identity"]["id"].as_str().unwrap();
        assert!(!id.trim().is_empty());
        assert_ne!(id, config.node_id);
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn core_native_listener_start_failure_releases_runtime_lease() {
        let directory = tempfile::tempdir().unwrap();
        let config = core_config(&directory);
        std::fs::write(&config.core_api_socket_path, b"occupied").unwrap();
        assert!(start_core_native(&config).await.is_err());
        std::fs::remove_file(&config.core_api_socket_path).unwrap();
        let owner = start_core_native(&config).await.unwrap();
        owner.shutdown().await.unwrap();
    }

    #[test]
    fn omitted_authority_mode_is_legacy_and_does_not_select_core_paths() {
        let config = AgentConfig::default();
        assert_eq!(config.authority_mode, AgentAuthorityMode::Legacy);
    }

    #[tokio::test]
    async fn load_or_initialize_cache_preserves_persisted_state() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("agent-cache.json");
        let mut cache = NodeCache::new("node-1");
        cache.node_state = NodeState::Maintenance.as_str().to_string();
        cache.save(&cache_path).await.unwrap();

        let config = AgentConfig {
            cache_path,
            ..AgentConfig::default()
        };

        let loaded = load_or_initialize_cache(&config).await;
        assert_eq!(loaded.node_state, "Maintenance");
    }

    #[tokio::test]
    async fn load_or_initialize_cache_bootstraps_new_node() {
        let dir = tempfile::tempdir().unwrap();
        let config = AgentConfig {
            cache_path: dir.path().join("missing-cache.json"),
            node_id: "node-123".to_string(),
            ..AgentConfig::default()
        };

        let cache = load_or_initialize_cache(&config).await;
        assert_eq!(cache.node_id, "node-123");
        assert_eq!(cache.node_state, "Bootstrapping");
    }

    #[test]
    fn certificate_rotation_due_respects_interval() {
        let cert_file = tempfile::NamedTempFile::new().unwrap();
        let key_file = tempfile::NamedTempFile::new().unwrap();

        let mut cache = NodeCache::new("node-1");
        cache.enrollment_complete = true;
        cache.certificate_path = Some(cert_file.path().to_str().unwrap().to_string());
        cache.private_key_path = Some(key_file.path().to_str().unwrap().to_string());
        assert!(certificate_rotation_due(
            &cache,
            CERT_ROTATION_INTERVAL_SECS * 1000
        ));

        cache.last_certificate_rotation_unix_ms = Some(1_000);
        assert!(!certificate_rotation_due(
            &cache,
            1_000 + (CERT_ROTATION_INTERVAL_SECS * 1000) - 1
        ));
        assert!(certificate_rotation_due(
            &cache,
            1_000 + (CERT_ROTATION_INTERVAL_SECS * 1000)
        ));
    }
}
