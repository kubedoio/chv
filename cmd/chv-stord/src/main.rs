use chv_config::load_stord_config;
use chv_observability::init_logger;
use chv_stord_backends::{
    CephRbdBackend, IscsiBackend, LVMBackend, LocalFileBackend, StorageBackend,
};
use chv_stord_core::migration::tls_config::{
    ensure_migration_half_configured, load_migration_server_tls, load_migration_tls,
};
use chv_stord_core::store::SessionStore;
use chv_stord_core::StorageServer;
use std::path::PathBuf;
use tokio::signal::unix::{signal, SignalKind};
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

    // Install the rustls ring crypto provider. Required before any tonic/rustls
    // TLS connection can be established — in particular the mTLS storage
    // migration sender (issue #232).
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls ring crypto provider");

    let config_path = std::env::args().nth(1).map(PathBuf::from);
    let config = load_stord_config(config_path.as_deref())?;

    init_logger(&config.log_level)?;

    info!(
        "{} starting (version {}, commit {}, channel {})",
        env!("CARGO_PKG_NAME"),
        env!("CHV_VERSION"),
        env!("CHV_GIT_SHA"),
        env!("CHV_RELEASE_CHANNEL"),
    );

    let db_path = config.runtime_dir.join("stord.db");
    let store = SessionStore::new(&db_path)?;

    // Select backend based on configuration
    let backend: Box<dyn StorageBackend> = match config.backend_type.as_deref().unwrap_or("local") {
        "iscsi" => {
            let iscsi_cfg = config
                .iscsi
                .as_ref()
                .ok_or("backend_type is 'iscsi' but [iscsi] config section is missing")?;
            let backend_cfg = chv_stord_backends::iscsi::IscsiConfig {
                portal: iscsi_cfg.portal.clone(),
                target_iqn: iscsi_cfg.target_iqn.clone(),
                initiator_name: iscsi_cfg.initiator_name.clone(),
                chap_username: iscsi_cfg.chap_username.clone(),
                chap_secret: iscsi_cfg.chap_secret.clone(),
            };
            Box::new(IscsiBackend::new(backend_cfg)?)
        }
        "ceph" => {
            let ceph_cfg = config
                .ceph
                .as_ref()
                .ok_or("backend_type is 'ceph' but [ceph] config section is missing")?;
            let backend_cfg = chv_stord_backends::ceph::CephRbdConfig {
                cluster_name: ceph_cfg.cluster_name.clone(),
                pool_name: ceph_cfg.pool_name.clone(),
                user: ceph_cfg.user.clone(),
                keyring_path: ceph_cfg.keyring_path.clone(),
                monitors: ceph_cfg.monitors.clone(),
            };
            Box::new(CephRbdBackend::new(backend_cfg)?)
        }
        "lvm" => {
            let vg_name = config.lvm_volume_group.as_deref().unwrap_or("chv-vg");
            Box::new(LVMBackend::new(vg_name.to_string())?)
        }
        _ => Box::new(LocalFileBackend::new(config.runtime_dir.clone())),
    };

    info!(
        backend_type = config.backend_type.as_deref().unwrap_or("local"),
        "storage backend initialized"
    );

    // Load + validate the storage-migration mTLS identity at startup (issue #232).
    // Fail-closed: a partially configured client half, unreadable material or
    // a mismatched keypair is a startup error (never a runtime downgrade).
    // `migration.enabled = false` (default) starts without credentials and
    // migration actions fail as unavailable in the sender. Under
    // `enabled = true` the client half is independently optional (issue #401):
    // no client fields = destination-only stord that never initiates
    // migrations (outbound migration actions fail as unavailable).
    let migration_tls = load_migration_tls(
        config.migration.enabled,
        config.migration.client_cert_path.as_deref(),
        config.migration.client_key_path.as_deref(),
        config.migration.ca_cert_path.as_deref(),
        config.migration.dest_server_name.as_deref(),
    )?;
    if migration_tls.is_some() {
        info!("storage migration mTLS enabled (credentials validated at startup)");
    }

    // Server half (issue #390): load + validate the migration receiver's mTLS
    // material. `migration.enabled` is the master switch here too: receiver
    // fields with enabled = false are a startup error (an operator who
    // believes migration is off must not get an inbound TCP listener), and
    // the four receiver fields are all-or-nothing — a partially configured
    // receiver, unreadable files, a mismatched keypair, an invalid/empty
    // client CA bundle, or a bad listen address is a startup error
    // (fail-closed). No receiver fields = source-only stord: no TCP listener
    // is opened. Client-certificate authentication is mandatory on the
    // listener; there is no plaintext fallback. The "listening" log line is
    // emitted only after the TCP socket is actually bound (see server.rs).
    let migration_server_tls = load_migration_server_tls(
        config.migration.enabled,
        config.migration.listen_addr.as_deref(),
        config.migration.server_cert_path.as_deref(),
        config.migration.server_key_path.as_deref(),
        config.migration.client_ca_path.as_deref(),
    )?;
    if migration_server_tls.is_some() {
        info!(
            "storage migration receiver mTLS material validated (listener binds in server startup)"
        );
    }

    // Cross-half check (issue #401): the two halves are independently
    // optional under `enabled = true` (source-only, destination-only, or
    // both), but an enabled migration section that configures NEITHER half
    // is a misconfiguration — the daemon would run with migrations
    // unavailable in both directions while the operator believes migration
    // is on. Fail-closed at startup.
    ensure_migration_half_configured(
        config.migration.enabled,
        migration_tls.as_ref(),
        migration_server_tls.as_ref(),
    )?;

    let server = StorageServer::new(
        backend,
        config.runtime_dir.clone(),
        chv_observability::Metrics::new(),
        config.backend_allowlist,
        config.path_allowlist,
        config.device_allowlist,
        config.migration_dest_allowlist,
        migration_tls,
        migration_server_tls,
        Some(store),
    );

    let socket_path = config.socket_path.clone();
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    tokio::select! {
        result = server.serve(&config.socket_path, Some(&db_path)) => {
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
