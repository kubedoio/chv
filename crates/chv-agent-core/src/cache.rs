use crate::state_machine::{NodeState, StateMachine};
use chv_errors::ChvError;
use control_plane_node_api::control_plane_node_api as proto;
use prost::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

const CACHE_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
}

impl NodeCache {
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            cache_version: CACHE_VERSION,
            node_id: node_id.into(),
            observed_generation: String::new(),
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
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ChvError::Io {
                    path: parent.to_string_lossy().to_string(),
                    source: e,
                })?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| ChvError::Internal {
            reason: format!("serialize error: {}", e),
        })?;
        tokio::fs::write(path, text)
            .await
            .map_err(|e| ChvError::Io {
                path: path.to_string_lossy().to_string(),
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
        let mut state_machine = StateMachine::new(self.current_node_state());
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use control_plane_node_api::control_plane_node_api as proto;

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
