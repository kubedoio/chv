//! Inventory snapshot types and the [`InventoryProvider`] trait that owns
//! the I/O boundary. Implementations live in `chv-architecture-reconcile`;
//! tests in this crate construct snapshots directly.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::FleetError;

/// I/O boundary between the validator and live cluster state. Every method
/// returns a [`FleetError`] on failure rather than panicking; the validator
/// converts these into snapshot capture errors.
///
/// `?Sized` is allowed at call sites so callers may hold this behind a
/// `Box<dyn InventoryProvider>` or `&dyn InventoryProvider`.
#[async_trait::async_trait]
pub trait InventoryProvider: Send + Sync {
    async fn list_nodes(&self) -> Result<Vec<NodeInfo>, FleetError>;
    async fn list_networks(&self) -> Result<Vec<NetworkInfo>, FleetError>;
    async fn list_datastores(&self) -> Result<Vec<DatastoreInfo>, FleetError>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>, FleetError>;
    /// Returns `(targets, complete)` where `complete=false` indicates this
    /// snapshot is best-effort because no authoritative
    /// `BackupTargetRepository` exists yet (see
    /// [`InventorySnapshot::backup_targets_complete`]).
    async fn list_backup_targets(&self) -> Result<(Vec<BackupTargetInfo>, bool), FleetError>;
    /// Authorisation hook. `true` means the caller currently holds the
    /// `architecture:apply` permission for the project under inspection.
    async fn caller_can_deploy(&self) -> Result<bool, FleetError>;
}

/// Live-fleet snapshot captured at a single instant. Pure data — the entire
/// `fleet::checks` module operates on a borrowed reference of this and the
/// architecture model.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventorySnapshot {
    /// When the snapshot was captured (provider-side wall clock).
    pub captured_at: DateTime<Utc>,
    /// Free-form tag describing the source (e.g. `"sqlite"`, `"mock"`).
    pub source: String,
    pub nodes: Vec<NodeInfo>,
    pub networks: Vec<NetworkInfo>,
    pub datastores: Vec<DatastoreInfo>,
    pub images: Vec<ImageInfo>,
    pub backup_targets: Vec<BackupTargetInfo>,
    /// `false` while the BackupTargetRepository is a stub — fleet checks
    /// downgrade `BACKUP_TARGET_UNREACHABLE` to a warning when this is
    /// false. Flips to `true` once the inventory source is authoritative.
    pub backup_targets_complete: bool,
    /// Result of [`InventoryProvider::caller_can_deploy`]. Persisted on the
    /// snapshot so the deploy-permission check is deterministic given the
    /// snapshot alone.
    pub deploy_allowed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub name: String,
    pub schedulable: bool,
    pub cpu_cores: u32,
    pub memory_gb: u32,
    pub bridges: Vec<String>,
    pub vlans: Vec<u32>,
    pub used_ips: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub name: String,
    pub bridge: Option<String>,
    pub vlan_id: Option<u32>,
    pub cidr: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatastoreInfo {
    pub name: String,
    pub kind: String,
    pub capacity_gb: u64,
    pub free_gb: u64,
    pub host: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageInfo {
    pub name: String,
    pub format: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackupTargetInfo {
    pub name: String,
    pub reachable: bool,
}

/// Capture a full snapshot from a provider. Calls are sequential rather
/// than parallel — keeping this synchronous-feeling avoids a `tokio` dep
/// in this pure-data crate. Reconcile wraps it with whatever runtime it
/// already owns.
pub async fn capture<P: InventoryProvider + ?Sized>(
    provider: &P,
    source: impl Into<String>,
) -> Result<InventorySnapshot, FleetError> {
    let nodes = provider.list_nodes().await?;
    let networks = provider.list_networks().await?;
    let datastores = provider.list_datastores().await?;
    let images = provider.list_images().await?;
    let (backup_targets, backup_targets_complete) = provider.list_backup_targets().await?;
    let deploy_allowed = provider.caller_can_deploy().await?;
    Ok(InventorySnapshot {
        captured_at: Utc::now(),
        source: source.into(),
        nodes,
        networks,
        datastores,
        images,
        backup_targets,
        backup_targets_complete,
        deploy_allowed,
    })
}
