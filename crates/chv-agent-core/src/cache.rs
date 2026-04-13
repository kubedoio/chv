use chv_errors::ChvError;
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeCache {
    pub cache_version: u32,
    pub node_id: String,
    pub observed_generation: String,
    pub node_state: String,
    pub vm_generations: HashMap<String, String>,
    pub volume_generations: HashMap<String, String>,
    pub network_generations: HashMap<String, String>,
    pub vm_fragments: HashMap<String, DesiredStateFragment>,
    pub volume_fragments: HashMap<String, DesiredStateFragment>,
    pub network_fragments: HashMap<String, DesiredStateFragment>,
    #[serde(default)]
    pub volume_handles: HashMap<String, String>,
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
            vm_generations: HashMap::new(),
            volume_generations: HashMap::new(),
            network_generations: HashMap::new(),
            vm_fragments: HashMap::new(),
            volume_fragments: HashMap::new(),
            network_fragments: HashMap::new(),
            volume_handles: HashMap::new(),
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

    pub fn is_stale(&self, kind: &str, id: &str, incoming: &str) -> bool {
        let current = self
            .get_generation(kind, id)
            .map(|s| s.as_str())
            .unwrap_or("");
        // Treat empty current as "new" (not stale)
        if current.is_empty() {
            return false;
        }
        // Empty incoming means missing generation header: don't treat as stale.
        if incoming.is_empty() {
            return false;
        }
        // Try numeric comparison first. For non-numeric generations we cannot
        // determine ordering, so we conservatively accept the request.
        match (incoming.parse::<u64>(), current.parse::<u64>()) {
            (Ok(a), Ok(b)) => a < b,
            _ => false,
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

    pub fn vm_network_ids(&self) -> Vec<String> {
        self.vm_fragments.values().filter_map(|frag| {
            match serde_json::from_slice::<serde_json::Value>(&frag.spec_json) {
                Ok(v) => v.get("network_id")?.as_str().map(String::from),
                Err(e) => {
                    tracing::warn!(fragment_id = %frag.id, error = %e, "failed to parse vm_fragment spec_json");
                    None
                }
            }
        }).collect()
    }

    pub fn vm_volume_handles(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (vm_id, frag) in &self.vm_fragments {
            match serde_json::from_slice::<serde_json::Value>(&frag.spec_json) {
                Ok(v) => {
                    if let Some(arr) = v.get("volumes").and_then(|a| a.as_array()) {
                        for vol in arr {
                            if let Some(vid) = vol.get("volume_id").and_then(|s| s.as_str()) {
                                out.push((vm_id.clone(), vid.to_string()));
                            }
                        }
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
        assert!(cache.is_stale("vm", "vm-1", "9"));
        assert!(!cache.is_stale("vm", "vm-1", "10"));
        assert!(!cache.is_stale("vm", "vm-1", "11"));
        assert!(!cache.is_stale("vm", "vm-1", ""));
    }

    #[test]
    fn cache_empty_generation_not_stale() {
        let cache = NodeCache::new("node-1");
        assert!(!cache.is_stale("vm", "vm-1", "1"));
    }

    #[test]
    fn cache_non_numeric_generation_not_stale() {
        let mut cache = NodeCache::new("node-1");
        cache.observe_generation("vm", "vm-1", "v2");
        // We cannot order arbitrary strings, so newer-looking values are accepted.
        assert!(!cache.is_stale("vm", "vm-1", "v3"));
        assert!(!cache.is_stale("vm", "vm-1", "v2"));
        assert!(!cache.is_stale("vm", "vm-1", "v1"));
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
            spec_json: br#"{"network_id":"net-1"}"#.to_vec(),
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
            spec_json: br#"{"volumes":[{"volume_id":"vol-1"},{"volume_id":"vol-2"}]}"#.to_vec(),
            policy_json: vec![],
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            updated_by: "cp".to_string(),
        });
        let mut handles = cache.vm_volume_handles();
        handles.sort();
        assert_eq!(handles, vec![("vm-1".to_string(), "vol-1".to_string()), ("vm-1".to_string(), "vol-2".to_string())]);
    }

    #[test]
    fn vm_network_ids_deduplicates() {
        let mut cache = NodeCache::new("node-1");
        for id in ["vm-1", "vm-2"] {
            cache.store_fragment("vm", id, DesiredStateFragment {
                id: id.to_string(),
                kind: "vm".to_string(),
                generation: "1".to_string(),
                spec_json: br#"{"network_id":"net-1"}"#.to_vec(),
                policy_json: vec![],
                updated_at: "2024-01-01T00:00:00Z".to_string(),
                updated_by: "cp".to_string(),
            });
        }
        let ids: std::collections::HashSet<String> = cache.vm_network_ids().into_iter().collect();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains("net-1"));
    }
}
