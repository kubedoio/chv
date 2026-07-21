//! Unwired process-wide authority facade for the legacy NodeCache.
//!
//! Production handlers still access [`crate::NodeCache`] directly. This module
//! defines the boundary they must all move behind during the Core cutover.

use crate::cache::{
    validated_cache_path, DesiredStateFragment, NodeCache, PendingControlPlaneMessage,
    VmAttachmentState, VmNicAttachment,
};
use crate::connectivity::ConnectivityState;
use crate::state_machine::NodeState;
use chv_errors::ChvError;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

pub struct AgentCoreActivation {
    pub store: cellhv_core_startup::ActivatedStore,
    pub node_cache: Option<NodeCacheAuthority>,
}

impl AgentCoreActivation {
    pub fn from_pending(
        pending: cellhv_core_startup::PendingActivatedStore,
    ) -> Result<Self, ChvError> {
        if let (Some(bytes), Some(expected)) = (
            pending.cache_bytes(),
            pending.provenance().source_checksum(),
        ) {
            let actual = format!("{:x}", Sha256::digest(bytes));
            if actual != expected {
                return Err(ChvError::Conflict {
                    resource: "node_cache_activation".to_owned(),
                    id: "source_checksum".to_owned(),
                });
            }
        }
        let node_cache = pending
            .cache_bytes()
            .map(|bytes| {
                serde_json::from_slice(bytes).map_err(|error| ChvError::InvalidArgument {
                    field: "node_cache".to_owned(),
                    reason: format!("activation snapshot parse error: {error}"),
                })
            })
            .transpose()?
            .map(|cache| NodeCacheAuthority::core(cache, pending.node_cache_path()))
            .transpose()?;
        let store = pending.finish();
        Ok(Self { store, node_cache })
    }
}

#[derive(Clone, PartialEq, Eq)]
struct VmAuthoritativeProjection {
    cache_version: u32,
    node_id: String,
    node_state: String,
    vm_generations: BTreeMap<String, String>,
    volume_generations: BTreeMap<String, String>,
    network_generations: BTreeMap<String, String>,
    vm_fragments: BTreeMap<String, DesiredStateFragment>,
    volume_fragments: BTreeMap<String, DesiredStateFragment>,
    network_fragments: BTreeMap<String, DesiredStateFragment>,
    vm_attachments: BTreeMap<String, VmAttachmentState>,
    volume_handles: BTreeMap<String, String>,
}

impl VmAuthoritativeProjection {
    fn capture(cache: &NodeCache) -> Self {
        Self {
            cache_version: cache.cache_version,
            node_id: cache.node_id.clone(),
            node_state: cache.node_state.clone(),
            vm_generations: cache.vm_generations.clone().into_iter().collect(),
            volume_generations: cache.volume_generations.clone().into_iter().collect(),
            network_generations: cache.network_generations.clone().into_iter().collect(),
            vm_fragments: cache.vm_fragments.clone().into_iter().collect(),
            volume_fragments: cache.volume_fragments.clone().into_iter().collect(),
            network_fragments: cache.network_fragments.clone().into_iter().collect(),
            vm_attachments: cache.vm_attachments.clone().into_iter().collect(),
            volume_handles: cache.volume_handles.clone().into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrollmentMetadata {
    pub complete: bool,
    pub certificate_path: Option<String>,
    pub private_key_path: Option<String>,
    pub ca_path: Option<String>,
    pub last_certificate_rotation_unix_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeCacheCompatibilitySnapshot {
    pub observed_generation: String,
    pub node_state: String,
    pub enrollment: EnrollmentMetadata,
    pub pending_control_plane: Vec<PendingControlPlaneMessage>,
    pub last_error: Option<String>,
    pub connectivity_state: ConnectivityState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LegacyVmAuthority;

#[derive(Clone, PartialEq, Eq)]
pub struct CoreVmAuthority {
    frozen: Box<VmAuthoritativeProjection>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Blocked {
    reason: String,
}

impl Blocked {
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum NodeCacheAuthorityMode {
    LegacyVmAuthority(LegacyVmAuthority),
    CoreVmAuthority(CoreVmAuthority),
    Blocked(Blocked),
}

/// Owns one NodeCache and enforces its process-wide VM authority mode.
///
/// There is deliberately no mutable cache accessor or `into_inner`: callers
/// cannot obtain a VM-writable cache after selecting Core or blocked mode.
pub struct NodeCacheAuthority {
    cache: NodeCache,
    cache_path: std::path::PathBuf,
    mode: NodeCacheAuthorityMode,
}

impl NodeCacheAuthority {
    #[allow(dead_code, reason = "unwired Phase B facade constructors")]
    pub(crate) fn legacy(cache: NodeCache, path: &Path) -> Result<Self, ChvError> {
        Ok(Self {
            cache,
            cache_path: validated_cache_path(path)?,
            mode: NodeCacheAuthorityMode::LegacyVmAuthority(LegacyVmAuthority),
        })
    }

    #[allow(dead_code, reason = "unwired Phase B facade constructors")]
    pub(crate) fn core(cache: NodeCache, path: &Path) -> Result<Self, ChvError> {
        let frozen = VmAuthoritativeProjection::capture(&cache);
        Ok(Self {
            cache,
            cache_path: validated_cache_path(path)?,
            mode: NodeCacheAuthorityMode::CoreVmAuthority(CoreVmAuthority {
                frozen: Box::new(frozen),
            }),
        })
    }

    #[allow(dead_code, reason = "unwired Phase B facade constructors")]
    pub(crate) fn blocked(
        cache: NodeCache,
        path: &Path,
        reason: impl Into<String>,
    ) -> Result<Self, ChvError> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(ChvError::InvalidArgument {
                field: "blocked_reason".to_owned(),
                reason: "must not be empty".to_owned(),
            });
        }
        Ok(Self {
            cache,
            cache_path: validated_cache_path(path)?,
            mode: NodeCacheAuthorityMode::Blocked(Blocked { reason }),
        })
    }

    pub fn mode(&self) -> &NodeCacheAuthorityMode {
        &self.mode
    }

    pub fn compatibility_snapshot(&self) -> NodeCacheCompatibilitySnapshot {
        NodeCacheCompatibilitySnapshot {
            observed_generation: self.cache.observed_generation.clone(),
            node_state: self.cache.node_state.clone(),
            enrollment: EnrollmentMetadata {
                complete: self.cache.enrollment_complete,
                certificate_path: self.cache.certificate_path.clone(),
                private_key_path: self.cache.private_key_path.clone(),
                ca_path: self.cache.ca_path.clone(),
                last_certificate_rotation_unix_ms: self.cache.last_certificate_rotation_unix_ms,
            },
            pending_control_plane: self.cache.pending_control_plane.clone(),
            last_error: self.cache.last_error.clone(),
            connectivity_state: self.cache.connectivity_state,
        }
    }

    pub fn host_id(&self) -> &str {
        &self.cache.node_id
    }

    pub fn observe_generation(
        &mut self,
        kind: &str,
        id: &str,
        generation: impl Into<String>,
    ) -> Result<(), ChvError> {
        if kind == "node" {
            return self.observe_node_generation(id, generation);
        }
        self.require_legacy_vm_mutation("observe_generation")?;
        self.cache.observe_generation(kind, id, generation);
        Ok(())
    }

    pub fn store_fragment(
        &mut self,
        kind: &str,
        id: &str,
        fragment: DesiredStateFragment,
    ) -> Result<(), ChvError> {
        self.require_legacy_vm_mutation("store_fragment")?;
        self.cache.store_fragment(kind, id, fragment);
        Ok(())
    }

    pub fn remove_fragment(&mut self, kind: &str, id: &str) -> Result<(), ChvError> {
        self.require_legacy_vm_mutation("remove_fragment")?;
        self.cache.remove_fragment(kind, id);
        Ok(())
    }

    pub fn observe_vm_attachment(
        &mut self,
        vm_id: &str,
        volume_ids: &[String],
        nics: &[VmNicAttachment],
    ) -> Result<(), ChvError> {
        self.require_legacy_vm_mutation("observe_vm_attachment")?;
        self.cache.observe_vm_attachment(vm_id, volume_ids, nics);
        Ok(())
    }

    pub fn remove_vm_state(&mut self, vm_id: &str) -> Result<(), ChvError> {
        self.require_legacy_vm_mutation("remove_vm_state")?;
        self.cache.remove_vm_state(vm_id);
        Ok(())
    }

    pub fn update_vm_desired_state(
        &mut self,
        vm_id: &str,
        desired_state: &str,
    ) -> Result<(), ChvError> {
        self.require_legacy_vm_mutation("update_vm_desired_state")?;
        self.cache.update_vm_desired_state(vm_id, desired_state);
        Ok(())
    }

    pub fn transition_node_state(&mut self, to: NodeState) -> Result<NodeState, ChvError> {
        self.require_legacy_vm_mutation("transition_node_state")?;
        self.cache.transition_node_state(to)
    }

    pub fn observe_node_generation(
        &mut self,
        id: &str,
        generation: impl Into<String>,
    ) -> Result<(), ChvError> {
        self.require_compatibility_mutation("observe_node_generation")?;
        if id.is_empty() || id != self.cache.node_id {
            return Err(ChvError::InvalidArgument {
                field: "node_id".to_owned(),
                reason: "must exactly match the cache host identity".to_owned(),
            });
        }
        self.cache.observe_generation("node", id, generation);
        Ok(())
    }

    pub fn set_enrollment_metadata(
        &mut self,
        metadata: EnrollmentMetadata,
    ) -> Result<(), ChvError> {
        self.require_compatibility_mutation("set_enrollment_metadata")?;
        self.cache.enrollment_complete = metadata.complete;
        self.cache.certificate_path = metadata.certificate_path;
        self.cache.private_key_path = metadata.private_key_path;
        self.cache.ca_path = metadata.ca_path;
        self.cache.last_certificate_rotation_unix_ms = metadata.last_certificate_rotation_unix_ms;
        Ok(())
    }

    pub fn apply_enrollment(
        &mut self,
        node_id: &str,
        metadata: EnrollmentMetadata,
    ) -> Result<(), ChvError> {
        if node_id.trim().is_empty() {
            return Err(ChvError::InvalidArgument {
                field: "node_id".to_owned(),
                reason: "must not be empty".to_owned(),
            });
        }
        match &self.mode {
            NodeCacheAuthorityMode::LegacyVmAuthority(_) => {
                self.cache.node_id = node_id.to_owned();
            }
            NodeCacheAuthorityMode::CoreVmAuthority(_) if node_id == self.cache.node_id => {}
            NodeCacheAuthorityMode::CoreVmAuthority(_) => {
                return Err(core_write_error("apply_enrollment changed host identity"));
            }
            NodeCacheAuthorityMode::Blocked(blocked) => {
                return Err(blocked_error(blocked, "apply_enrollment"));
            }
        }
        self.set_enrollment_metadata(metadata)
    }

    pub fn record_certificate_rotation(&mut self, unix_ms: i64) -> Result<(), ChvError> {
        self.require_compatibility_mutation("record_certificate_rotation")?;
        if unix_ms < 0 {
            return Err(ChvError::InvalidArgument {
                field: "last_certificate_rotation_unix_ms".to_owned(),
                reason: "must not be negative".to_owned(),
            });
        }
        self.cache.last_certificate_rotation_unix_ms = Some(unix_ms);
        Ok(())
    }

    pub fn enqueue_pending_message(
        &mut self,
        message: PendingControlPlaneMessage,
    ) -> Result<(), ChvError> {
        self.require_compatibility_mutation("enqueue_pending_message")?;
        self.cache.enqueue_pending_message(message);
        Ok(())
    }

    pub fn replace_pending_control_plane_messages(
        &mut self,
        messages: Vec<PendingControlPlaneMessage>,
    ) -> Result<(), ChvError> {
        self.require_compatibility_mutation("replace_pending_control_plane_messages")?;
        self.cache.replace_pending_control_plane_messages(messages);
        Ok(())
    }

    pub fn set_last_error(&mut self, error: Option<String>) -> Result<(), ChvError> {
        self.require_compatibility_mutation("set_last_error")?;
        self.cache.last_error = error;
        Ok(())
    }

    pub fn set_connectivity_state(&mut self, state: ConnectivityState) -> Result<(), ChvError> {
        self.require_compatibility_mutation("set_connectivity_state")?;
        self.cache.connectivity_state = state;
        Ok(())
    }

    pub fn set_volume_handle(&mut self, volume_id: &str, handle: String) -> Result<(), ChvError> {
        self.require_legacy_vm_mutation("set_volume_handle")?;
        self.cache
            .volume_handles
            .insert(volume_id.to_owned(), handle);
        Ok(())
    }

    pub fn remove_volume_handle(&mut self, volume_id: &str) -> Result<(), ChvError> {
        self.require_legacy_vm_mutation("remove_volume_handle")?;
        self.cache.volume_handles.remove(volume_id);
        Ok(())
    }

    pub fn remove_vm_attachment(&mut self, vm_id: &str) -> Result<(), ChvError> {
        self.require_legacy_vm_mutation("remove_vm_attachment")?;
        self.cache.vm_attachments.remove(vm_id);
        Ok(())
    }

    pub fn complete_volume_snapshot_operation(&mut self, volume_id: &str) -> Result<(), ChvError> {
        self.remove_volume_spec_fields(volume_id, &["snapshot_op", "snapshot_name"])
    }

    pub fn complete_volume_clone_operation(&mut self, volume_id: &str) -> Result<(), ChvError> {
        self.remove_volume_spec_fields(volume_id, &["clone_source_volume_id"])
    }

    pub async fn save(&self) -> Result<(), ChvError> {
        match &self.mode {
            NodeCacheAuthorityMode::LegacyVmAuthority(_) => self.cache.save(&self.cache_path).await,
            NodeCacheAuthorityMode::CoreVmAuthority(core) => {
                if VmAuthoritativeProjection::capture(&self.cache) != *core.frozen {
                    return Err(core_write_error("save observed changed frozen VM state"));
                }
                self.cache.save(&self.cache_path).await
            }
            NodeCacheAuthorityMode::Blocked(blocked) => Err(blocked_error(blocked, "save")),
        }
    }

    fn require_legacy_vm_mutation(&self, operation: &str) -> Result<(), ChvError> {
        match &self.mode {
            NodeCacheAuthorityMode::LegacyVmAuthority(_) => Ok(()),
            NodeCacheAuthorityMode::CoreVmAuthority(_) => Err(core_write_error(operation)),
            NodeCacheAuthorityMode::Blocked(blocked) => Err(blocked_error(blocked, operation)),
        }
    }

    fn require_compatibility_mutation(&self, operation: &str) -> Result<(), ChvError> {
        match &self.mode {
            NodeCacheAuthorityMode::LegacyVmAuthority(_)
            | NodeCacheAuthorityMode::CoreVmAuthority(_) => Ok(()),
            NodeCacheAuthorityMode::Blocked(blocked) => Err(blocked_error(blocked, operation)),
        }
    }

    fn remove_volume_spec_fields(
        &mut self,
        volume_id: &str,
        fields: &[&str],
    ) -> Result<(), ChvError> {
        self.require_legacy_vm_mutation("complete_volume_operation")?;
        let Some(fragment) = self.cache.volume_fragments.get_mut(volume_id) else {
            return Ok(());
        };
        let mut spec: serde_json::Value =
            serde_json::from_slice(&fragment.spec_json).map_err(|e| ChvError::InvalidArgument {
                field: "volume_fragment.spec_json".to_owned(),
                reason: e.to_string(),
            })?;
        let object = spec
            .as_object_mut()
            .ok_or_else(|| ChvError::InvalidArgument {
                field: "volume_fragment.spec_json".to_owned(),
                reason: "must be a JSON object".to_owned(),
            })?;
        for field in fields {
            object.remove(*field);
        }
        fragment.spec_json = serde_json::to_vec(&spec).map_err(|e| ChvError::Internal {
            reason: format!("serialize volume fragment: {e}"),
        })?;
        Ok(())
    }
}

fn core_write_error(operation: &str) -> ChvError {
    ChvError::AccessDenied {
        resource: "node_cache_vm_authority".to_owned(),
        reason: format!("{operation} is forbidden after Core authority cutover"),
    }
}

fn blocked_error(blocked: &Blocked, operation: &str) -> ChvError {
    ChvError::AccessDenied {
        resource: "node_cache_authority".to_owned(),
        reason: format!("{operation} is blocked: {}", blocked.reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{PendingControlPlaneMessageKind, VmNicAttachment};
    use std::os::unix::fs::PermissionsExt;

    fn fragment(kind: &str, id: &str) -> DesiredStateFragment {
        DesiredStateFragment {
            id: id.to_owned(),
            kind: kind.to_owned(),
            generation: "1".to_owned(),
            spec_json: br#"{"desired_state":"Running"}"#.to_vec(),
            policy_json: vec![],
            updated_at: "now".to_owned(),
            updated_by: "test".to_owned(),
        }
    }

    fn populated_cache() -> NodeCache {
        let mut cache = NodeCache::new("node-1");
        for (kind, id) in [("vm", "vm-1"), ("volume", "vol-1"), ("network", "net-1")] {
            cache.observe_generation(kind, id, "1");
            cache.store_fragment(kind, id, fragment(kind, id));
        }
        cache.observe_vm_attachment(
            "vm-1",
            &["vol-1".to_owned()],
            &[VmNicAttachment {
                nic_id: "nic-1".to_owned(),
                network_id: "net-1".to_owned(),
            }],
        );
        cache
            .volume_handles
            .insert("vol-1".to_owned(), "/dev/test".to_owned());
        cache
    }

    fn assert_denied(result: Result<(), ChvError>) {
        assert!(matches!(result, Err(ChvError::AccessDenied { .. })));
    }

    fn enrollment() -> EnrollmentMetadata {
        EnrollmentMetadata {
            complete: true,
            certificate_path: Some("/cert".to_owned()),
            private_key_path: Some("/key".to_owned()),
            ca_path: Some("/ca".to_owned()),
            last_certificate_rotation_unix_ms: Some(42),
        }
    }

    #[test]
    fn core_mode_denies_every_nodecache_vm_mutator() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.json");
        let mut authority = NodeCacheAuthority::core(populated_cache(), &path).unwrap();
        let frozen = VmAuthoritativeProjection::capture(&authority.cache);

        for kind in ["vm", "volume", "network"] {
            assert_denied(authority.observe_generation(kind, "new", "2"));
            assert_denied(authority.store_fragment(kind, "new", fragment(kind, "new")));
            assert_denied(authority.remove_fragment(kind, "new"));
        }
        assert_denied(authority.observe_vm_attachment("vm-1", &["vol-2".to_owned()], &[]));
        assert_denied(authority.remove_vm_state("vm-1"));
        assert_denied(authority.update_vm_desired_state("vm-1", "Stopped"));
        assert_denied(authority.set_volume_handle("vol-2", "handle".to_owned()));
        assert_denied(authority.remove_volume_handle("vol-1"));
        assert_denied(authority.remove_vm_attachment("vm-1"));
        assert_denied(authority.complete_volume_snapshot_operation("vol-1"));
        assert_denied(authority.complete_volume_clone_operation("vol-1"));
        for state in [NodeState::Draining, NodeState::Maintenance] {
            assert!(matches!(
                authority.transition_node_state(state),
                Err(ChvError::AccessDenied { .. })
            ));
        }
        assert!(VmAuthoritativeProjection::capture(&authority.cache) == frozen);
    }

    #[tokio::test]
    async fn core_mode_allows_only_compatibility_updates_and_unchanged_save() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().join("cache.json");
        let mut authority = NodeCacheAuthority::core(populated_cache(), &path).unwrap();
        authority.apply_enrollment("node-1", enrollment()).unwrap();
        assert_denied(authority.apply_enrollment("replacement", enrollment()));
        authority.record_certificate_rotation(84).unwrap();
        assert_eq!(authority.host_id(), "node-1");
        authority
            .set_last_error(Some("reported-only".to_owned()))
            .unwrap();
        for wrong_id in ["", "other-node"] {
            assert!(matches!(
                authority.observe_node_generation(wrong_id, "2"),
                Err(ChvError::InvalidArgument { .. })
            ));
        }
        authority.observe_node_generation("node-1", "2").unwrap();
        authority
            .set_connectivity_state(ConnectivityState::Connected)
            .unwrap();
        authority
            .enqueue_pending_message(PendingControlPlaneMessage {
                kind: PendingControlPlaneMessageKind::NodeStateReport,
                payload: vec![1, 2, 3],
            })
            .unwrap();
        let snapshot = authority.compatibility_snapshot();
        assert!(snapshot.enrollment.complete);
        assert_eq!(
            snapshot.enrollment.last_certificate_rotation_unix_ms,
            Some(84)
        );
        assert_eq!(
            snapshot.enrollment.certificate_path.as_deref(),
            Some("/cert")
        );
        assert_eq!(snapshot.connectivity_state, ConnectivityState::Connected);
        assert_eq!(snapshot.pending_control_plane.len(), 1);
        authority
            .replace_pending_control_plane_messages(Vec::new())
            .unwrap();
        authority.save().await.unwrap();
        let saved = NodeCache::load(&path).await.unwrap();
        assert!(saved.enrollment_complete);
        assert_eq!(saved.last_error.as_deref(), Some("reported-only"));
        assert_eq!(
            saved.get_generation("node", "node-1").map(String::as_str),
            Some("2")
        );
        assert!(
            VmAuthoritativeProjection::capture(&saved)
                == VmAuthoritativeProjection::capture(&authority.cache)
        );
    }

    #[tokio::test]
    async fn core_mode_save_rechecks_projection() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().join("cache.json");
        let mut authority = NodeCacheAuthority::core(populated_cache(), &path).unwrap();
        authority.cache.vm_fragments.clear();
        assert_denied(authority.save().await);
        assert!(!path.exists());
    }

    #[test]
    fn compatibility_snapshot_has_no_authoritative_vm_state() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.json");
        let mut authority = NodeCacheAuthority::core(populated_cache(), &path).unwrap();
        authority.set_last_error(Some("copy".to_owned())).unwrap();
        let mut snapshot = authority.compatibility_snapshot();
        snapshot.last_error = Some("detached".to_owned());
        assert_eq!(
            authority.compatibility_snapshot().last_error.as_deref(),
            Some("copy")
        );
    }

    #[test]
    fn construction_binds_a_validated_canonical_cache_path() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let real = directory.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let alias = directory.path().join("alias");
        symlink(&real, &alias).unwrap();
        assert!(matches!(
            NodeCacheAuthority::core(NodeCache::new("node-1"), &alias.join("cache.json")),
            Err(ChvError::Io { .. })
        ));

        let authority =
            NodeCacheAuthority::core(NodeCache::new("node-1"), &real.join("../real/cache.json"))
                .unwrap();
        assert_eq!(authority.cache_path, real.join("cache.json"));
    }

    #[tokio::test]
    async fn blocked_mode_denies_mutation_compatibility_update_and_save() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().join("cache.json");
        let mut authority =
            NodeCacheAuthority::blocked(populated_cache(), &path, "identity conflict").unwrap();
        assert_denied(authority.remove_vm_state("vm-1"));
        assert!(matches!(
            authority.set_last_error(Some("x".to_owned())),
            Err(ChvError::AccessDenied { .. })
        ));
        assert!(matches!(
            authority.transition_node_state(NodeState::Draining),
            Err(ChvError::AccessDenied { .. })
        ));
        assert_denied(authority.save().await);
        assert!(!path.exists());
        assert_eq!(
            match authority.mode() {
                NodeCacheAuthorityMode::Blocked(blocked) => blocked.reason(),
                _ => panic!("wrong mode"),
            },
            "identity conflict"
        );
    }

    #[tokio::test]
    async fn legacy_mode_preserves_existing_mutation_and_save_behavior() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().join("cache.json");
        let mut authority = NodeCacheAuthority::legacy(NodeCache::new("node-1"), &path).unwrap();
        authority
            .apply_enrollment("legacy-issued", enrollment())
            .unwrap();
        assert_eq!(authority.host_id(), "legacy-issued");
        authority.observe_generation("vm", "vm-1", "1").unwrap();
        authority
            .store_fragment("vm", "vm-1", fragment("vm", "vm-1"))
            .unwrap();
        authority
            .observe_vm_attachment("vm-1", &["vol-1".to_owned()], &[])
            .unwrap();
        authority
            .set_volume_handle("vol-1", "legacy-handle".to_owned())
            .unwrap();
        let mut volume = fragment("volume", "vol-1");
        volume.spec_json =
            br#"{"snapshot_op":"create","snapshot_name":"s1","clone_source_volume_id":"source"}"#
                .to_vec();
        authority.store_fragment("volume", "vol-1", volume).unwrap();
        authority
            .complete_volume_snapshot_operation("vol-1")
            .unwrap();
        authority.complete_volume_clone_operation("vol-1").unwrap();
        assert_eq!(
            authority.cache.volume_fragments["vol-1"].spec_json,
            br#"{}"#
        );
        authority
            .update_vm_desired_state("vm-1", "Stopped")
            .unwrap();
        authority.save().await.unwrap();
        assert!(NodeCache::load(&path)
            .await
            .unwrap()
            .get_fragment("vm", "vm-1")
            .is_some());
        authority.remove_fragment("vm", "vm-1").unwrap();
        authority.remove_volume_handle("vol-1").unwrap();
        authority.remove_vm_attachment("vm-1").unwrap();
        authority.remove_vm_state("vm-1").unwrap();
    }

    #[tokio::test]
    async fn pending_activation_builds_exact_facade_before_releasing_cache_lock() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let cache_path = directory.path().join("cache.json");
        NodeCache::new("node-a").save(&cache_path).await.unwrap();
        let paths = cellhv_core_startup::StartupPaths {
            node_cache: cache_path.clone(),
            core_database: directory.path().join("core.db"),
            node_cache_archive: directory.path().join("cache.archive"),
        };
        let pending = cellhv_core_startup::StartupTransaction::begin(&paths)
            .unwrap()
            .prepare_activation(Some("node-a".to_owned()), None)
            .unwrap();

        let (sent, received) = std::sync::mpsc::channel();
        let blocked_path = cache_path.clone();
        let waiter = std::thread::spawn(move || {
            let lock = cellhv_core_fs::AuthorityLock::acquire(&blocked_path).unwrap();
            sent.send(()).unwrap();
            lock
        });
        assert!(received
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err());

        let activation = AgentCoreActivation::from_pending(pending).unwrap();
        received
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        drop(waiter.join().unwrap());
        let facade = activation.node_cache.as_ref().unwrap();
        assert_eq!(facade.host_id(), "node-a");
        facade.save().await.unwrap();
    }

    #[test]
    fn fresh_activation_returns_no_synthetic_nodecache_facade() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let paths = cellhv_core_startup::StartupPaths {
            node_cache: directory.path().join("cache.json"),
            core_database: directory.path().join("core.db"),
            node_cache_archive: directory.path().join("cache.archive"),
        };
        let pending = cellhv_core_startup::StartupTransaction::begin(&paths)
            .unwrap()
            .prepare_activation(Some("node-fresh".to_owned()), None)
            .unwrap();
        let activation = AgentCoreActivation::from_pending(pending).unwrap();
        assert!(activation.node_cache.is_none());
        assert!(!paths.node_cache.exists());
    }
}
