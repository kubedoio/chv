use crate::connectivity::ConnectivityState;
use crate::state_machine::{guard_transition, NodeState, StateMachine};
use chv_errors::ChvError;
use control_plane_node_api::control_plane_node_api as proto;
use prost::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

const CACHE_VERSION: u32 = 1;
const CACHE_FILE_MODE: u32 = 0o600;
type SaveHook = Arc<dyn Fn() -> io::Result<()> + Send + Sync>;

#[derive(Default)]
struct SaveOrder {
    next: u64,
    last_committed: u64,
}

fn save_order(path: &Path) -> io::Result<Arc<Mutex<SaveOrder>>> {
    static ORDERS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<SaveOrder>>>>> = OnceLock::new();
    let key = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    {
        let mut orders = ORDERS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .map_err(|_| io::Error::other("cache save order registry poisoned"))?;
        Ok(Arc::clone(orders.entry(key).or_insert_with(|| {
            Arc::new(Mutex::new(SaveOrder::default()))
        })))
    }
}

fn reserve_save(path: &Path) -> io::Result<(Arc<Mutex<SaveOrder>>, u64)> {
    let order = save_order(path)?;
    let sequence = {
        let mut state = order
            .lock()
            .map_err(|_| io::Error::other("cache save order poisoned"))?;
        state.next = state
            .next
            .checked_add(1)
            .ok_or_else(|| io::Error::other("cache save sequence exhausted"))?;
        state.next
    };
    Ok((order, sequence))
}

fn validate_cache_parent(parent: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cache parent must be a real directory, not a symlink or special file",
        ));
    }
    if metadata.uid() != unsafe { libc::geteuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "cache parent must be owned by the service user",
        ));
    }
    if metadata.mode() & 0o022 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "cache parent must not be group- or world-writable",
        ));
    }
    Ok(())
}

fn validate_cache_destination(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.file_type().is_file()
                && metadata.uid() == unsafe { libc::geteuid() }
                && metadata.mode() & 0o777 == CACHE_FILE_MODE
                && metadata.nlink() == 1 =>
        {
            Ok(())
        }
        Ok(metadata) if metadata.file_type().is_file() => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "cache destination must be owner-owned 0600 with one link",
        )),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cache destination must be a regular file, not a symlink or special file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn atomic_replace_cache_inner<F, G>(
    path: &Path,
    contents: &[u8],
    before_rename: F,
    after_rename: G,
) -> io::Result<()>
where
    F: FnOnce() -> io::Result<bool>,
    G: FnOnce() -> io::Result<()>,
{
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    validate_cache_parent(parent)?;
    validate_cache_destination(path)?;

    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "cache path has no file name")
    })?;
    let mut temp_path = PathBuf::from(parent);
    temp_path.push(format!(
        ".{}.{}.tmp",
        file_name.to_string_lossy(),
        uuid::Uuid::new_v4()
    ));

    let mut renamed = false;
    let result = (|| {
        let mut temp = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(CACHE_FILE_MODE)
            .open(&temp_path)?;
        temp.write_all(contents)?;
        temp.flush()?;
        temp.sync_all()?;
        if !before_rename()? {
            return Ok(());
        }

        // Recheck after writing to narrow the replacement race and reject a
        // symlink or special file introduced while the temporary file was built.
        validate_cache_destination(path)?;
        fs::rename(&temp_path, path)?;
        renamed = true;
        after_rename()?;
        fs::File::open(parent)?.sync_all()
    })();

    if !renamed {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn atomic_replace_cache(
    path: &Path,
    contents: &[u8],
    order: &Mutex<SaveOrder>,
    sequence: u64,
    before_rename: Option<&SaveHook>,
) -> io::Result<()> {
    let _authority_lock = cellhv_core_fs::AuthorityLock::acquire(path)?;
    atomic_replace_cache_inner(
        path,
        contents,
        || {
            if let Some(hook) = before_rename {
                hook()?;
            }
            let state = order
                .lock()
                .map_err(|_| io::Error::other("cache save order poisoned"))?;
            Ok(sequence >= state.last_committed)
        },
        || {
            let mut state = order
                .lock()
                .map_err(|_| io::Error::other("cache save order poisoned"))?;
            state.last_committed = state.last_committed.max(sequence);
            Ok(())
        },
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesiredStateFragment {
    pub id: String,
    pub kind: String,
    pub generation: String,
    pub spec_json: Vec<u8>,
    pub policy_json: Vec<u8>,
    pub updated_at: String,
    pub updated_by: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmNicAttachment {
    pub nic_id: String,
    pub network_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmAttachmentState {
    #[serde(default)]
    pub volume_ids: Vec<String>,
    #[serde(default)]
    pub nics: Vec<VmNicAttachment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PendingControlPlaneMessageKind {
    NodeStateReport,
    VmStateReport,
    VolumeStateReport,
    NetworkStateReport,
    PublishEvent,
    PublishAlert,
    ReportNodeInventory,
    ReportServiceVersions,
    MigrationProgressReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingControlPlaneMessage {
    pub kind: PendingControlPlaneMessageKind,
    pub payload: Vec<u8>,
}

impl PendingControlPlaneMessage {
    fn encode<T: Message>(kind: PendingControlPlaneMessageKind, message: T) -> Self {
        Self {
            kind,
            payload: message.encode_to_vec(),
        }
    }

    fn decode<T: Message + Default>(&self) -> Result<T, ChvError> {
        T::decode(self.payload.as_slice()).map_err(|e| ChvError::InvalidArgument {
            field: "pending_control_plane_message".to_string(),
            reason: e.to_string(),
        })
    }

    pub fn node_state(message: proto::NodeStateReport) -> Self {
        Self::encode(PendingControlPlaneMessageKind::NodeStateReport, message)
    }

    pub fn vm_state(message: proto::VmStateReport) -> Self {
        Self::encode(PendingControlPlaneMessageKind::VmStateReport, message)
    }

    pub fn volume_state(message: proto::VolumeStateReport) -> Self {
        Self::encode(PendingControlPlaneMessageKind::VolumeStateReport, message)
    }

    pub fn network_state(message: proto::NetworkStateReport) -> Self {
        Self::encode(PendingControlPlaneMessageKind::NetworkStateReport, message)
    }

    pub fn event(message: proto::PublishEventRequest) -> Self {
        Self::encode(PendingControlPlaneMessageKind::PublishEvent, message)
    }

    pub fn alert(message: proto::PublishAlertRequest) -> Self {
        Self::encode(PendingControlPlaneMessageKind::PublishAlert, message)
    }

    pub fn node_inventory(message: proto::ReportNodeInventoryRequest) -> Self {
        Self::encode(PendingControlPlaneMessageKind::ReportNodeInventory, message)
    }

    pub fn service_versions(message: proto::ReportServiceVersionsRequest) -> Self {
        Self::encode(
            PendingControlPlaneMessageKind::ReportServiceVersions,
            message,
        )
    }

    pub fn migration_progress(message: proto::MigrationProgress) -> Self {
        Self::encode(
            PendingControlPlaneMessageKind::MigrationProgressReport,
            message,
        )
    }

    pub fn decode_node_state(&self) -> Result<proto::NodeStateReport, ChvError> {
        self.decode()
    }

    pub fn decode_vm_state(&self) -> Result<proto::VmStateReport, ChvError> {
        self.decode()
    }

    pub fn decode_volume_state(&self) -> Result<proto::VolumeStateReport, ChvError> {
        self.decode()
    }

    pub fn decode_network_state(&self) -> Result<proto::NetworkStateReport, ChvError> {
        self.decode()
    }

    pub fn decode_event(&self) -> Result<proto::PublishEventRequest, ChvError> {
        self.decode()
    }

    pub fn decode_alert(&self) -> Result<proto::PublishAlertRequest, ChvError> {
        self.decode()
    }

    pub fn decode_node_inventory(&self) -> Result<proto::ReportNodeInventoryRequest, ChvError> {
        self.decode()
    }

    pub fn decode_service_versions(&self) -> Result<proto::ReportServiceVersionsRequest, ChvError> {
        self.decode()
    }

    pub fn decode_migration_progress(&self) -> Result<proto::MigrationProgress, ChvError> {
        self.decode()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeCache {
    pub cache_version: u32,
    pub node_id: String,
    pub observed_generation: String,
    pub node_state: String,
    #[serde(default)]
    pub enrollment_complete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ca_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_certificate_rotation_unix_ms: Option<i64>,
    pub vm_generations: HashMap<String, String>,
    pub volume_generations: HashMap<String, String>,
    pub network_generations: HashMap<String, String>,
    pub vm_fragments: HashMap<String, DesiredStateFragment>,
    pub volume_fragments: HashMap<String, DesiredStateFragment>,
    pub network_fragments: HashMap<String, DesiredStateFragment>,
    #[serde(default)]
    pub vm_attachments: HashMap<String, VmAttachmentState>,
    #[serde(default)]
    pub volume_handles: HashMap<String, String>,
    #[serde(default)]
    pub pending_control_plane: Vec<PendingControlPlaneMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(skip)]
    pub connectivity_state: ConnectivityState,
}

impl NodeCache {
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            cache_version: CACHE_VERSION,
            node_id: node_id.into(),
            observed_generation: "0".to_string(),
            node_state: "Bootstrapping".to_string(),
            enrollment_complete: false,
            certificate_path: None,
            private_key_path: None,
            ca_path: None,
            last_certificate_rotation_unix_ms: None,
            vm_generations: HashMap::new(),
            volume_generations: HashMap::new(),
            network_generations: HashMap::new(),
            vm_fragments: HashMap::new(),
            volume_fragments: HashMap::new(),
            network_fragments: HashMap::new(),
            vm_attachments: HashMap::new(),
            volume_handles: HashMap::new(),
            pending_control_plane: Vec::new(),
            last_error: None,
            connectivity_state: ConnectivityState::Disconnected,
        }
    }

    pub async fn load(path: &Path) -> Result<Self, ChvError> {
        if !path.exists() {
            return Err(ChvError::NotFound {
                resource: "cache".to_string(),
                id: path.to_string_lossy().to_string(),
            });
        }
        let text = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ChvError::Io {
                path: path.to_string_lossy().to_string(),
                source: e,
            })?;
        let cache: NodeCache =
            serde_json::from_str(&text).map_err(|e| ChvError::InvalidArgument {
                field: "cache".to_string(),
                reason: format!("parse error: {}", e),
            })?;
        if cache.cache_version != CACHE_VERSION {
            return Err(ChvError::InvalidArgument {
                field: "cache".to_string(),
                reason: format!(
                    "cache version mismatch: expected {}, got {}",
                    CACHE_VERSION, cache.cache_version
                ),
            });
        }
        Ok(cache)
    }

    pub async fn save(&self, path: &Path) -> Result<(), ChvError> {
        self.save_ordered(path, None).await
    }

    async fn save_ordered(
        &self,
        path: &Path,
        before_rename: Option<SaveHook>,
    ) -> Result<(), ChvError> {
        let contents = serde_json::to_vec_pretty(self).map_err(|e| ChvError::Internal {
            reason: format!("serialize error: {}", e),
        })?;
        let (order, sequence) = reserve_save(path).map_err(|e| ChvError::Io {
            path: path.to_string_lossy().to_string(),
            source: e,
        })?;
        let owned_path = path.to_path_buf();
        let error_path = path.to_string_lossy().to_string();
        tokio::task::spawn_blocking(move || {
            atomic_replace_cache(
                &owned_path,
                &contents,
                &order,
                sequence,
                before_rename.as_ref(),
            )
        })
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("cache persistence task failed: {e}"),
        })?
        .map_err(|e| ChvError::Io {
            path: error_path,
            source: e,
        })
    }

    pub fn observe_generation(&mut self, kind: &str, id: &str, generation: impl Into<String>) {
        let gen = generation.into();
        match kind {
            "vm" => self.vm_generations.insert(id.to_string(), gen),
            "volume" => self.volume_generations.insert(id.to_string(), gen),
            "network" => self.network_generations.insert(id.to_string(), gen),
            "node" => {
                self.observed_generation = gen;
                None
            }
            _ => None,
        };
    }

    pub fn get_generation(&self, kind: &str, id: &str) -> Option<&String> {
        match kind {
            "vm" => self.vm_generations.get(id),
            "volume" => self.volume_generations.get(id),
            "network" => self.network_generations.get(id),
            "node" => Some(&self.observed_generation),
            _ => None,
        }
    }

    pub fn current_node_state(&self) -> NodeState {
        self.node_state.parse().unwrap_or(NodeState::Bootstrapping)
    }

    pub fn transition_node_state(&mut self, to: NodeState) -> Result<NodeState, ChvError> {
        let from = self.current_node_state();
        guard_transition(from, to);
        let mut state_machine = StateMachine::new(from);
        state_machine.transition(to)?;
        let current = state_machine.current();
        self.node_state = current.as_str().to_string();
        Ok(current)
    }

    pub fn is_stale(&self, kind: &str, id: &str, incoming: &str) -> Result<bool, ChvError> {
        let current = self
            .get_generation(kind, id)
            .map(|s| s.as_str())
            .unwrap_or("");
        if current.is_empty() {
            return Ok(false);
        }
        if incoming.is_empty() {
            return Ok(false);
        }
        match (incoming.parse::<u64>(), current.parse::<u64>()) {
            (Ok(a), Ok(b)) => Ok(a < b),
            _ => Err(ChvError::InvalidArgument {
                field: "desired_state_version".to_string(),
                reason: format!(
                    "generation must be numeric, current={}, incoming={}",
                    current, incoming
                ),
            }),
        }
    }

    pub fn store_fragment(&mut self, kind: &str, id: &str, fragment: DesiredStateFragment) {
        match kind {
            "vm" => self.vm_fragments.insert(id.to_string(), fragment),
            "volume" => self.volume_fragments.insert(id.to_string(), fragment),
            "network" => self.network_fragments.insert(id.to_string(), fragment),
            _ => None,
        };
    }

    pub fn get_fragment(&self, kind: &str, id: &str) -> Option<&DesiredStateFragment> {
        match kind {
            "vm" => self.vm_fragments.get(id),
            "volume" => self.volume_fragments.get(id),
            "network" => self.network_fragments.get(id),
            _ => None,
        }
    }

    pub fn remove_fragment(&mut self, kind: &str, id: &str) {
        match kind {
            "vm" => {
                self.vm_fragments.remove(id);
            }
            "volume" => {
                self.volume_fragments.remove(id);
            }
            "network" => {
                self.network_fragments.remove(id);
            }
            _ => {}
        };
    }

    pub fn observe_vm_attachment(
        &mut self,
        vm_id: &str,
        volume_ids: &[String],
        nics: &[VmNicAttachment],
    ) {
        let state = self.vm_attachments.entry(vm_id.to_string()).or_default();
        for volume_id in volume_ids {
            if !state.volume_ids.contains(volume_id) {
                state.volume_ids.push(volume_id.clone());
            }
        }
        for nic in nics {
            if !state
                .nics
                .iter()
                .any(|existing| existing.nic_id == nic.nic_id)
            {
                state.nics.push(nic.clone());
            }
        }
    }

    pub fn remove_vm_state(&mut self, vm_id: &str) {
        self.vm_generations.remove(vm_id);
        self.vm_fragments.remove(vm_id);
        self.vm_attachments.remove(vm_id);
    }

    pub fn vm_attachment_state(&self, vm_id: &str) -> Option<&VmAttachmentState> {
        self.vm_attachments.get(vm_id)
    }

    pub fn enqueue_pending_message(&mut self, message: PendingControlPlaneMessage) {
        self.pending_control_plane.push(message);
    }

    pub fn pending_control_plane_messages(&self) -> &[PendingControlPlaneMessage] {
        &self.pending_control_plane
    }

    pub fn replace_pending_control_plane_messages(
        &mut self,
        messages: Vec<PendingControlPlaneMessage>,
    ) {
        self.pending_control_plane = messages;
    }

    pub fn vm_network_ids(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        for frag in self.vm_fragments.values() {
            let raw = match std::str::from_utf8(&frag.spec_json) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(fragment_id = %frag.id, error = %e, "failed to decode vm_fragment spec_json as utf-8");
                    continue;
                }
            };
            match crate::spec::VmSpec::from_json(raw) {
                Ok(spec) => {
                    for nic in &spec.nics {
                        seen.insert(nic.network_id.clone());
                    }
                }
                Err(e) => {
                    tracing::warn!(fragment_id = %frag.id, error = %e, "failed to parse vm_fragment spec_json");
                }
            }
        }
        seen.into_iter().collect()
    }

    pub fn update_vm_desired_state(&mut self, vm_id: &str, desired_state: &str) {
        if let Some(frag) = self.vm_fragments.get_mut(vm_id) {
            if let Ok(mut spec) = serde_json::from_slice::<serde_json::Value>(&frag.spec_json) {
                spec["desired_state"] = serde_json::Value::String(desired_state.to_string());
                if let Ok(bytes) = serde_json::to_vec(&spec) {
                    frag.spec_json = bytes;
                }
            }
        }
    }

    pub fn vm_volume_handles(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (vm_id, frag) in &self.vm_fragments {
            let raw = match std::str::from_utf8(&frag.spec_json) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(fragment_id = %frag.id, error = %e, "failed to decode vm_fragment spec_json as utf-8");
                    continue;
                }
            };
            match crate::spec::VmSpec::from_json(raw) {
                Ok(spec) => {
                    for disk in &spec.disks {
                        out.push((vm_id.clone(), disk.volume_id.clone()));
                    }
                }
                Err(e) => {
                    tracing::warn!(fragment_id = %frag.id, error = %e, "failed to parse vm_fragment spec_json");
                }
            }
        }
        out
    }

    /// Project a single Core `VmDefinition` into the legacy compatibility VM
    /// axis of the cache (fragment + generation + attachments + desired state).
    ///
    /// M2.2b: this is a *projection* derived from one authoritative Core
    /// terminal outcome — it is never the authority for desired state. The
    /// legacy `VmSpec` shape carries exactly what the legacy readers (`VmSpec`,
    /// `vm_network_ids`, `vm_volume_handles`) can consume; fields Core M1 does
    /// not model (cloud-init userdata, hypervisor overrides, per-NIC addressing)
    /// are projected as the values the single effector used (None / shared
    /// default CIDR / empty addressing) — identical to the M2.2a residuals.
    pub fn project_vm(
        &mut self,
        def: &cellhv_core_types::VmDefinition,
        updated_at: String,
        updated_by: String,
    ) {
        let vm_id = def.id.as_str();
        let desired_state = match def.requested_power_state {
            cellhv_core_types::RequestedPowerState::Running => "Running",
            cellhv_core_types::RequestedPowerState::Stopped => "Stopped",
        };
        let spec = serde_json::json!({
            "name": def.name.clone(),
            "cpus": def.compute.vcpus,
            "memory_bytes": def.compute.memory_bytes,
            "kernel_path": def.boot.kernel.clone(),
            "firmware_path": def.boot.firmware.clone(),
            "disks": def.storage.iter().map(|storage| serde_json::json!({
                "volume_id": storage.storage_ref.clone(),
                "read_only": storage.read_only,
            })).collect::<Vec<_>>(),
            "nics": def.networks.iter().map(|network| serde_json::json!({
                "network_id": network.network_ref.clone(),
                "mac_address": network.mac_address.clone().unwrap_or_else(|| projected_mac(vm_id, &network.network_ref)),
                "ip_address": "",
                "tap_name": "",
                "cidr": chv_hypervisor_api::resources::DEFAULT_NIC_CIDR,
                "gateway": "",
            })).collect::<Vec<_>>(),
            "desired_state": desired_state,
            "cloud_init_userdata": None::<String>,
            "hypervisor_overrides": None::<String>,
        });
        let generation = def.resource_version.get().to_string();
        self.store_fragment(
            "vm",
            vm_id,
            DesiredStateFragment {
                id: vm_id.to_string(),
                kind: "vm".to_string(),
                generation: generation.clone(),
                spec_json: serde_json::to_vec(&spec).unwrap_or_default(),
                policy_json: Vec::new(),
                updated_at,
                updated_by,
            },
        );
        self.observe_generation("vm", vm_id, generation);
        let volume_ids = def
            .storage
            .iter()
            .map(|storage| storage.storage_ref.clone())
            .collect::<Vec<_>>();
        let nics = def
            .networks
            .iter()
            .map(|network| VmNicAttachment {
                nic_id: network.attachment_id.clone(),
                network_id: network.network_ref.clone(),
            })
            .collect::<Vec<_>>();
        self.observe_vm_attachment(vm_id, &volume_ids, &nics);
    }

    /// Rebuild the compatibility VM axis of the cache from the Core store's
    /// authoritative VM list (startup rebuild in core-managed mode).
    ///
    /// Only the VM axis is replaced — `vm_fragments`, `vm_generations`,
    /// `vm_attachments` — so a rebuild never discards volume/network axis state
    /// (Core M1's lifecycle does not yet model arbitrary volume/network desired
    /// state, and those axes belong to the legacy compatibility surface). Every
    /// entry is projected with `"core-rebuild"` attribution so a rebuilt entry
    /// is distinguishable from a live projection.
    pub fn rebuild_from_core(&mut self, vms: &[cellhv_core_types::VmDefinition]) {
        self.vm_fragments.clear();
        self.vm_generations.clear();
        self.vm_attachments.clear();
        for def in vms {
            self.project_vm(def, "core-rebuild".to_string(), "core".to_string());
        }
    }
}

/// Deterministic locally-administered unicast MAC derived from a VM + network
/// identity, used when a Core `NetworkAttachmentRef` carries no MAC.
///
/// Core M1 does not model a requested MAC (M2.2a's effector lets the hypervisor
/// assign one at runtime), but the legacy compatibility `VmSpec` requires a
/// non-empty `mac_address` (`VmSpec::validate`). Projecting a stable placeholder
/// keeps the compatibility surface valid; the actual runtime NIC MAC is
/// observable independently, not via this projected `VmSpec`.
///
/// Format: `02:00:00:HH:HH:HH` — unicast, locally administered — where HH are
/// the low 24 bits of an FNV-1a hash of `{vm_id}\0{network_ref}`. Deterministic
/// across restarts (no hasher-state dependency), collision-resistant enough for
/// a compatibility view.
fn projected_mac(vm_id: &str, network_ref: &str) -> String {
    let mut state: u64 = 0xcbf2_9ce4_8422_2325;
    let mut bytes = vm_id.as_bytes().to_vec();
    bytes.push(0);
    bytes.extend_from_slice(network_ref.as_bytes());
    for b in bytes {
        state ^= u64::from(b);
        state = state.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!(
        "02:00:00:{:02x}:{:02x}:{:02x}",
        ((state >> 16) & 0xff) as u8,
        ((state >> 8) & 0xff) as u8,
        (state & 0xff) as u8
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use control_plane_node_api::control_plane_node_api as proto;
    use std::os::unix::fs::{symlink, PermissionsExt};

    #[tokio::test]
    async fn cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        let mut cache = NodeCache::new("node-1");
        cache.observe_generation("vm", "vm-1", "5");
        cache.save(&path).await.unwrap();

        let loaded = NodeCache::load(&path).await.unwrap();
        assert_eq!(loaded.node_id, "node-1");
        assert_eq!(loaded.vm_generations.get("vm-1"), Some(&"5".to_string()));
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            CACHE_FILE_MODE
        );
    }

    fn projected_definition(vm_id: &str) -> cellhv_core_types::VmDefinition {
        use cellhv_core_types::{
            BootSpec, ComputeSpec, NetworkAttachmentRef, ObservedPowerState, RequestedPowerState,
            ResourceVersion, StorageAttachmentRef, VmDefinition, VmId,
        };
        VmDefinition {
            id: VmId::new(vm_id).unwrap(),
            name: format!("{vm_id}-guest"),
            boot: BootSpec::new("/kernel").unwrap(),
            compute: ComputeSpec::new(2, 1024).unwrap(),
            storage: vec![StorageAttachmentRef {
                attachment_id: "vol-0".to_string(),
                storage_ref: "vol-0".to_string(),
                read_only: false,
            }],
            networks: vec![NetworkAttachmentRef {
                attachment_id: "nic-0".to_string(),
                network_ref: "net-0".to_string(),
                mac_address: Some("02:00:00:00:00:01".to_string()),
            }],
            requested_power_state: RequestedPowerState::Running,
            observed_power_state: ObservedPowerState::Unknown,
            resource_version: ResourceVersion::new(3).unwrap(),
        }
    }

    #[test]
    fn project_vm_writes_the_legacy_vm_axis() {
        let mut cache = NodeCache::new("node-1");
        cache.project_vm(
            &projected_definition("vm-a"),
            "1700000000000".to_string(),
            "requester".to_string(),
        );

        let frag = cache.get_fragment("vm", "vm-a").unwrap();
        assert_eq!(frag.kind, "vm");
        assert_eq!(frag.generation, "3");
        assert_eq!(frag.updated_at, "1700000000000");
        assert_eq!(frag.updated_by, "requester");
        assert_eq!(cache.get_generation("vm", "vm-a"), Some(&"3".to_string()));

        let spec_json = std::str::from_utf8(&frag.spec_json).unwrap();
        let spec = crate::spec::VmSpec::from_json(spec_json).unwrap();
        assert_eq!(spec.name, "vm-a-guest");
        assert_eq!(spec.cpus, 2);
        assert_eq!(spec.memory_bytes, 1024);
        assert_eq!(spec.kernel_path, "/kernel");
        assert_eq!(spec.firmware_path, None);
        assert_eq!(spec.desired_state, "Running");
        assert_eq!(spec.cloud_init_userdata, None);
        assert_eq!(spec.hypervisor_overrides, None);
        assert_eq!(spec.disks.len(), 1);
        assert_eq!(spec.disks[0].volume_id, "vol-0");
        assert!(!spec.disks[0].read_only);
        assert_eq!(spec.nics.len(), 1);
        assert_eq!(spec.nics[0].network_id, "net-0");
        assert_eq!(spec.nics[0].mac_address, "02:00:00:00:00:01");
        assert_eq!(
            spec.nics[0].cidr,
            chv_hypervisor_api::resources::DEFAULT_NIC_CIDR
        );
        assert!(spec.validate().is_ok());

        let attachment = cache.vm_attachment_state("vm-a").unwrap();
        assert_eq!(attachment.volume_ids, vec!["vol-0".to_string()]);
        assert_eq!(attachment.nics.len(), 1);
        assert_eq!(attachment.nics[0].nic_id, "nic-0");
        assert_eq!(attachment.nics[0].network_id, "net-0");
    }

    #[test]
    fn project_vm_maps_power_state_and_defaults_unmodeled_fields() {
        use cellhv_core_types::{NetworkAttachmentRef, RequestedPowerState};
        let mut def = projected_definition("vm-stop");
        def.requested_power_state = RequestedPowerState::Stopped;
        def.networks = vec![NetworkAttachmentRef {
            attachment_id: "nic-1".to_string(),
            network_ref: "net-1".to_string(),
            mac_address: None,
        }];
        let mut cache = NodeCache::new("node-1");
        cache.project_vm(&def, "core-rebuild".to_string(), "core".to_string());

        let frag = cache.get_fragment("vm", "vm-stop").unwrap();
        let spec =
            crate::spec::VmSpec::from_json(std::str::from_utf8(&frag.spec_json).unwrap()).unwrap();
        assert_eq!(spec.desired_state, "Stopped");
        // Core M1 does not model per-NIC addressing or a MAC; the projection
        // mirrors the single effector's residual (empty addressing, default
        // CIDR) rather than inventing values.
        assert_eq!(spec.nics.len(), 1);
        assert_eq!(spec.nics[0].network_id, "net-1");
        // A Core def with no requested MAC must still produce a valid legacy
        // VmSpec: the projection emits a deterministic locally-administered MAC
        // instead of an empty string (which `VmSpec::validate` rejects).
        let mac = &spec.nics[0].mac_address;
        assert!(
            !mac.is_empty(),
            "projected MAC must be non-empty, got {mac:?}"
        );
        assert!(mac.starts_with("02:00:00:"), "projected MAC: {mac}");
        assert_eq!(mac.len(), "02:00:00:00:00:00".len());
        assert!(spec.validate().is_ok());
        assert_eq!(spec.nics[0].ip_address, "");
        assert_eq!(spec.nics[0].tap_name, "");
        assert_eq!(spec.nics[0].gateway, "");
    }

    #[test]
    fn rebuild_from_core_replaces_only_the_vm_axis() {
        let mut cache = NodeCache::new("node-1");
        // Pre-existing VM axis state the rebuild must discard...
        cache.project_vm(
            &projected_definition("vm-stale"),
            "old".to_string(),
            "legacy".to_string(),
        );
        // ...and volume/network axis state the rebuild must preserve.
        cache.store_fragment(
            "volume",
            "vol-keep",
            DesiredStateFragment {
                id: "vol-keep".to_string(),
                kind: "volume".to_string(),
                generation: "1".to_string(),
                spec_json: vec![],
                policy_json: vec![],
                updated_at: "keep".to_string(),
                updated_by: "cp".to_string(),
            },
        );
        cache.observe_generation("volume", "vol-keep", "1");
        cache.observe_generation("network", "net-keep", "2");

        let fresh = vec![projected_definition("vm-new")];
        cache.rebuild_from_core(&fresh);

        assert!(cache.get_fragment("vm", "vm-stale").is_none());
        assert!(cache.vm_attachment_state("vm-stale").is_none());
        assert!(cache.get_generation("vm", "vm-stale").is_none());
        let projected = cache.get_fragment("vm", "vm-new").unwrap();
        assert_eq!(projected.updated_by, "core");
        assert_eq!(projected.updated_at, "core-rebuild");
        assert_eq!(cache.get_generation("vm", "vm-new"), Some(&"3".to_string()));
        // Volume/network axes untouched.
        assert!(cache.get_fragment("volume", "vol-keep").is_some());
        assert_eq!(
            cache.get_generation("volume", "vol-keep"),
            Some(&"1".to_string())
        );
        assert_eq!(
            cache.get_generation("network", "net-keep"),
            Some(&"2".to_string())
        );
    }

    #[test]
    fn interrupted_atomic_replace_preserves_last_good_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        let original = serde_json::to_vec_pretty(&NodeCache::new("node-original")).unwrap();
        atomic_replace_cache_inner(&path, &original, || Ok(true), || Ok(())).unwrap();

        let replacement = serde_json::to_vec_pretty(&NodeCache::new("node-replacement")).unwrap();
        let error = atomic_replace_cache_inner(
            &path,
            &replacement,
            || Err(io::Error::other("injected failure before rename")),
            || Ok(()),
        )
        .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn cancelled_older_save_cannot_overwrite_newer_snapshot() {
        use std::sync::mpsc;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let entered_tx = Arc::new(Mutex::new(Some(entered_tx)));
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let hook: SaveHook = Arc::new(move || {
            entered_tx
                .lock()
                .map_err(|_| io::Error::other("test entered lock poisoned"))?
                .take()
                .ok_or_else(|| io::Error::other("test hook entered more than once"))?
                .send(())
                .map_err(|_| io::Error::other("test entered receiver dropped"))?;
            release_rx
                .lock()
                .map_err(|_| io::Error::other("test release lock poisoned"))?
                .recv()
                .map_err(|_| io::Error::other("test release sender dropped"))?;
            Ok(())
        });

        let old_path = path.clone();
        let old = tokio::spawn(async move {
            NodeCache::new("node-old")
                .save_ordered(&old_path, Some(hook))
                .await
        });
        entered_rx.await.unwrap();
        old.abort();

        let new_path = path.clone();
        let new = tokio::spawn(async move { NodeCache::new("node-new").save(&new_path).await });
        let order = save_order(&path).unwrap();
        for _ in 0..100 {
            if order.lock().unwrap().next >= 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(order.lock().unwrap().next, 2);
        release_tx.send(()).unwrap();
        new.await.unwrap().unwrap();

        assert_eq!(NodeCache::load(&path).await.unwrap().node_id, "node-new");
        assert!(fs::read_dir(dir.path()).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
    }

    #[tokio::test]
    async fn newer_reservation_without_a_job_does_not_suppress_older_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let entered_tx = Arc::new(Mutex::new(Some(entered_tx)));
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let hook: SaveHook = Arc::new(move || {
            entered_tx
                .lock()
                .map_err(|_| io::Error::other("test entered lock poisoned"))?
                .take()
                .ok_or_else(|| io::Error::other("test hook entered more than once"))?
                .send(())
                .map_err(|_| io::Error::other("test entered receiver dropped"))?;
            release_rx
                .lock()
                .map_err(|_| io::Error::other("test release lock poisoned"))?
                .recv()
                .map_err(|_| io::Error::other("test release sender dropped"))?;
            Ok(())
        });

        let old_path = path.clone();
        let old = tokio::spawn(async move {
            NodeCache::new("node-old")
                .save_ordered(&old_path, Some(hook))
                .await
        });
        entered_rx.await.unwrap();

        let (_, reserved_but_never_started) = reserve_save(&path).unwrap();
        assert_eq!(reserved_but_never_started, 2);
        release_tx.send(()).unwrap();
        old.await.unwrap().unwrap();

        assert_eq!(NodeCache::load(&path).await.unwrap().node_id, "node-old");
        assert_eq!(save_order(&path).unwrap().lock().unwrap().last_committed, 1);
    }

    #[tokio::test]
    async fn cache_save_rejects_symlink_destination_without_touching_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.json");
        let path = dir.path().join("cache.json");
        fs::write(&target, b"last-good").unwrap();
        symlink(&target, &path).unwrap();

        let result = NodeCache::new("node-1").save(&path).await;

        assert!(matches!(result, Err(ChvError::Io { .. })));
        assert_eq!(fs::read(&target).unwrap(), b"last-good");
        assert!(fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[tokio::test]
    async fn cache_save_rejects_unsafe_mode_and_hardlinked_destination() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        fs::write(&path, b"last-good").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();

        assert!(matches!(
            NodeCache::new("node-1").save(&path).await,
            Err(ChvError::Io { .. })
        ));
        assert_eq!(fs::read(&path).unwrap(), b"last-good");

        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let external = dir.path().join("external-link.json");
        fs::hard_link(&path, &external).unwrap();
        assert!(matches!(
            NodeCache::new("node-1").save(&path).await,
            Err(ChvError::Io { .. })
        ));
        assert_eq!(fs::read(&path).unwrap(), b"last-good");
        assert_eq!(fs::read(&external).unwrap(), b"last-good");
    }

    #[tokio::test]
    async fn cache_save_rejects_symlink_authority_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        let lock_path = cellhv_core_fs::lock_path(&path).unwrap();
        let target = dir.path().join("lock-target");
        fs::write(&target, b"do-not-touch").unwrap();
        symlink(&target, &lock_path).unwrap();

        let result = NodeCache::new("node-1").save(&path).await;

        assert!(matches!(result, Err(ChvError::Io { .. })));
        assert_eq!(fs::read(&target).unwrap(), b"do-not-touch");
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn cache_save_rejects_writable_service_parent_before_mutation() {
        let dir = tempfile::tempdir().unwrap();
        let unsafe_parent = dir.path().join("unsafe");
        fs::create_dir(&unsafe_parent).unwrap();
        fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o777)).unwrap();
        let path = unsafe_parent.join("cache.json");

        let result = NodeCache::new("node-1").save(&path).await;

        assert!(matches!(result, Err(ChvError::Io { .. })));
        assert_eq!(fs::read_dir(&unsafe_parent).unwrap().count(), 0);
        fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[tokio::test]
    async fn cache_save_requires_pre_existing_service_parent() {
        let dir = tempfile::tempdir().unwrap();
        let missing_parent = dir.path().join("not-created");
        let path = missing_parent.join("cache.json");

        let result = NodeCache::new("node-1").save(&path).await;

        assert!(matches!(result, Err(ChvError::Io { .. })));
        assert!(!missing_parent.exists());
    }

    #[tokio::test]
    async fn concurrent_cache_saves_leave_one_complete_document() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        let mut saves = Vec::new();
        for index in 0..32 {
            let path = path.clone();
            saves.push(tokio::spawn(async move {
                NodeCache::new(format!("node-{index}"))
                    .save(&path)
                    .await
                    .unwrap();
            }));
        }
        for save in saves {
            save.await.unwrap();
        }

        let loaded = NodeCache::load(&path).await.unwrap();
        assert!(loaded.node_id.starts_with("node-"));
        assert!(fs::read_dir(dir.path()).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
    }

    #[test]
    fn cache_stale_generation() {
        let mut cache = NodeCache::new("node-1");
        cache.observe_generation("vm", "vm-1", "10");
        assert!(cache.is_stale("vm", "vm-1", "9").unwrap());
        assert!(!cache.is_stale("vm", "vm-1", "10").unwrap());
        assert!(!cache.is_stale("vm", "vm-1", "11").unwrap());
        assert!(!cache.is_stale("vm", "vm-1", "").unwrap());
    }

    #[test]
    fn cache_empty_generation_not_stale() {
        let cache = NodeCache::new("node-1");
        assert!(!cache.is_stale("vm", "vm-1", "1").unwrap());
    }

    #[test]
    fn cache_non_numeric_generation_rejected() {
        let mut cache = NodeCache::new("node-1");
        cache.observe_generation("vm", "vm-1", "v2");
        let err = cache.is_stale("vm", "vm-1", "v1").unwrap_err();
        assert!(matches!(err, ChvError::InvalidArgument { .. }));
    }

    #[test]
    fn cache_transition_node_state_uses_persisted_state() {
        let mut cache = NodeCache::new("node-1");
        cache.node_state = NodeState::TenantReady.as_str().to_string();
        let current = cache.transition_node_state(NodeState::Draining).unwrap();
        assert_eq!(current, NodeState::Draining);
        assert_eq!(cache.node_state, "Draining");
    }

    #[test]
    fn cache_tracks_and_removes_vm_attachment_state() {
        let mut cache = NodeCache::new("node-1");
        cache.observe_vm_attachment(
            "vm-1",
            &["vol-1".to_string()],
            &[VmNicAttachment {
                nic_id: "vm-1-net-1".to_string(),
                network_id: "net-1".to_string(),
            }],
        );
        let attachments = cache.vm_attachment_state("vm-1").unwrap();
        assert_eq!(attachments.volume_ids, vec!["vol-1".to_string()]);
        assert_eq!(
            attachments.nics,
            vec![VmNicAttachment {
                nic_id: "vm-1-net-1".to_string(),
                network_id: "net-1".to_string(),
            }]
        );

        cache.observe_generation("vm", "vm-1", "2");
        cache.store_fragment(
            "vm",
            "vm-1",
            DesiredStateFragment {
                id: "vm-1".to_string(),
                kind: "vm".to_string(),
                generation: "2".to_string(),
                spec_json: vec![],
                policy_json: vec![],
                updated_at: String::new(),
                updated_by: String::new(),
            },
        );
        cache.remove_vm_state("vm-1");
        assert!(cache.vm_attachment_state("vm-1").is_none());
        assert!(cache.get_generation("vm", "vm-1").is_none());
        assert!(cache.get_fragment("vm", "vm-1").is_none());
    }

    #[tokio::test]
    async fn cache_version_mismatch_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        let mut cache = NodeCache::new("node-1");
        cache.cache_version = 999;
        cache.save(&path).await.unwrap();

        let result = NodeCache::load(&path).await;
        assert!(
            matches!(result, Err(ChvError::InvalidArgument { .. })),
            "expected version mismatch error, got {:?}",
            result
        );
    }

    #[test]
    fn cache_fragment_roundtrip() {
        let mut cache = NodeCache::new("node-1");
        let frag = DesiredStateFragment {
            id: "vm-1".to_string(),
            kind: "vm".to_string(),
            generation: "5".to_string(),
            spec_json: b"{}".to_vec(),
            policy_json: vec![],
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            updated_by: "cp".to_string(),
        };
        cache.store_fragment("vm", "vm-1", frag.clone());
        assert_eq!(cache.get_fragment("vm", "vm-1").unwrap().generation, "5");
        cache.remove_fragment("vm", "vm-1");
        assert!(cache.get_fragment("vm", "vm-1").is_none());
    }

    #[test]
    fn vm_network_ids_extracts_from_spec_json() {
        let mut cache = NodeCache::new("node-1");
        cache.store_fragment("vm", "vm-1", DesiredStateFragment {
            id: "vm-1".to_string(),
            kind: "vm".to_string(),
            generation: "1".to_string(),
            spec_json: br#"{"name":"vm-1","cpus":1,"memory_bytes":1024,"kernel_path":"/dev/null","disks":[],"nics":[{"network_id":"net-1","mac_address":"00:00:00:00:00:01","ip_address":"10.0.0.2"}]}"#.to_vec(),
            policy_json: vec![],
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            updated_by: "cp".to_string(),
        });
        let ids = cache.vm_network_ids();
        assert_eq!(ids, vec!["net-1"]);
    }

    #[test]
    fn vm_volume_handles_extracts_from_spec_json() {
        let mut cache = NodeCache::new("node-1");
        cache.store_fragment("vm", "vm-1", DesiredStateFragment {
            id: "vm-1".to_string(),
            kind: "vm".to_string(),
            generation: "1".to_string(),
            spec_json: br#"{"name":"vm-1","cpus":1,"memory_bytes":1024,"kernel_path":"/dev/null","disks":[{"volume_id":"vol-1","read_only":false},{"volume_id":"vol-2","read_only":false}],"nics":[]}"#.to_vec(),
            policy_json: vec![],
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            updated_by: "cp".to_string(),
        });
        let mut handles = cache.vm_volume_handles();
        handles.sort();
        assert_eq!(
            handles,
            vec![
                ("vm-1".to_string(), "vol-1".to_string()),
                ("vm-1".to_string(), "vol-2".to_string())
            ]
        );
    }

    #[test]
    fn vm_network_ids_deduplicates() {
        let mut cache = NodeCache::new("node-1");
        for id in ["vm-1", "vm-2"] {
            cache.store_fragment("vm", id, DesiredStateFragment {
                id: id.to_string(),
                kind: "vm".to_string(),
                generation: "1".to_string(),
                spec_json: br#"{"name":"vm-1","cpus":1,"memory_bytes":1024,"kernel_path":"/dev/null","disks":[],"nics":[{"network_id":"net-1","mac_address":"00:00:00:00:00:01","ip_address":"10.0.0.2"}]}"#.to_vec(),
                policy_json: vec![],
                updated_at: "2024-01-01T00:00:00Z".to_string(),
                updated_by: "cp".to_string(),
            });
        }
        let ids: std::collections::HashSet<String> = cache.vm_network_ids().into_iter().collect();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains("net-1"));
    }

    #[test]
    fn update_vm_desired_state_patches_spec_json() {
        let mut cache = NodeCache::new("node-1");
        cache.store_fragment("vm", "vm-1", DesiredStateFragment {
            id: "vm-1".to_string(),
            kind: "vm".to_string(),
            generation: "1".to_string(),
            spec_json: br#"{"name":"vm-1","cpus":1,"memory_bytes":1024,"kernel_path":"/dev/null","disks":[],"nics":[],"desired_state":"Running"}"#.to_vec(),
            policy_json: vec![],
            updated_at: String::new(),
            updated_by: String::new(),
        });
        cache.update_vm_desired_state("vm-1", "Stopped");
        let frag = cache.get_fragment("vm", "vm-1").unwrap();
        let spec: serde_json::Value = serde_json::from_slice(&frag.spec_json).unwrap();
        assert_eq!(spec["desired_state"], "Stopped");

        cache.update_vm_desired_state("vm-1", "Running");
        let frag = cache.get_fragment("vm", "vm-1").unwrap();
        let spec: serde_json::Value = serde_json::from_slice(&frag.spec_json).unwrap();
        assert_eq!(spec["desired_state"], "Running");
    }

    #[test]
    fn update_vm_desired_state_noop_for_missing_vm() {
        let mut cache = NodeCache::new("node-1");
        cache.update_vm_desired_state("nonexistent", "Stopped");
    }

    #[test]
    fn cache_tracks_pending_control_plane_messages() {
        let mut cache = NodeCache::new("node-1");
        let report = proto::NodeStateReport {
            node_id: "node-1".to_string(),
            state: "TenantReady".to_string(),
            observed_generation: "5".to_string(),
            health_status: "Healthy".to_string(),
            last_error: String::new(),
            reported_unix_ms: 0,
        };

        cache.enqueue_pending_message(PendingControlPlaneMessage::node_state(report.clone()));

        assert_eq!(cache.pending_control_plane_messages().len(), 1);
        let decoded = cache.pending_control_plane_messages()[0]
            .decode_node_state()
            .unwrap();
        assert_eq!(decoded.node_id, report.node_id);
        assert_eq!(decoded.state, report.state);
    }
}
