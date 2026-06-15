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
    /// Returns `(secrets, complete)` where `complete=false` indicates the
    /// secret store is unmodelled in this snapshot (no authoritative
    /// `SecretRepository` yet). When `complete=false`, fleet checks
    /// downgrade `SECRET_REF_MISSING` to a warning rather than blocking.
    /// Default impl returns `(vec![], false)` so existing implementors
    /// stay compatible until they grow a real secret store.
    async fn list_secrets(&self) -> Result<(Vec<SecretInfo>, bool), FleetError> {
        Ok((Vec::new(), false))
    }
    /// Returns `true` when the provider can populate per-host network facts
    /// (`bridges`, `vlans`, `used_ips` on each [`NodeInfo`]). When `false`,
    /// fleet checks downgrade `BRIDGE_UNAVAILABLE`, `VLAN_UNAVAILABLE`, and
    /// `IP_ALREADY_USED` to warnings — the absence of facts cannot
    /// distinguish "bridge missing" from "bridge unreported". Default
    /// returns `false` so providers must opt-in once they wire authoritative
    /// network-fact reporting.
    async fn network_facts_complete(&self) -> Result<bool, FleetError> {
        Ok(false)
    }
    /// Authorisation hook. `true` means the caller currently holds the
    /// `architecture:apply` permission for the project under inspection.
    async fn caller_can_deploy(&self) -> Result<bool, FleetError>;
}

/// Live-fleet snapshot captured at a single instant. Pure data — the entire
/// `fleet::checks` module operates on a borrowed reference of this and the
/// architecture model.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventorySnapshot {
    /// When the snapshot was captured (validator-side wall clock at the end
    /// of [`capture`]; provider methods are not asked to supply timestamps).
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
    /// Authoritative platform secret store contents. Empty + `secrets_complete=false`
    /// while no real `SecretRepository` exists. **Reviewers note:** without
    /// this flag, a previously-used datastore-name placeholder caused
    /// `SECRET_REF_MISSING` to silently false-negative when a datastore
    /// happened to share a name with a referenced secret — see the Phase-3
    /// review trail (commit log) for context.
    pub secrets: Vec<SecretInfo>,
    /// `false` while no real `SecretRepository` exists — fleet checks
    /// downgrade `SECRET_REF_MISSING` to a warning when this is false.
    pub secrets_complete: bool,
    /// `false` when per-host `bridges`/`vlans`/`used_ips` are not authoritative
    /// (e.g. the `networks` SQL table has no `bridge` / `vlan_id` columns
    /// today, and the agent inventory does not report which IPs are in use).
    /// Fleet checks downgrade `BRIDGE_UNAVAILABLE`, `VLAN_UNAVAILABLE`, and
    /// `IP_ALREADY_USED` to warnings when this is false. Flips to `true`
    /// once provider implementors wire authoritative reporting.
    pub network_facts_complete: bool,
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

/// Authoritative secret-store record. `kind` is free-form (`"password"`,
/// `"ssh-key"`, `"opaque"`) — the validator does not inspect it today, only
/// the `name` is matched against the architecture's `secret_ref` fields.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecretInfo {
    pub name: String,
    pub kind: String,
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
    let (secrets, secrets_complete) = provider.list_secrets().await?;
    let network_facts_complete = provider.network_facts_complete().await?;
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
        secrets,
        secrets_complete,
        network_facts_complete,
        deploy_allowed,
    })
}
