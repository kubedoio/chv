use chv_config::load_stord_config;
use chv_observability::init_logger;
use chv_stord_backends::{
    CephRbdBackend, IscsiBackend, LVMBackend, LocalFileBackend, StorageBackend,
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

    let server = StorageServer::new(
        backend,
        chv_observability::Metrics::new(),
        config.backend_allowlist,
        config.path_allowlist,
        config.device_allowlist,
        config.migration_dest_allowlist,
        None, // TODO: wire migration TLS config from stord config
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
