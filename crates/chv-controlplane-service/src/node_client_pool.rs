use dashmap::DashMap;
use std::path::Path;
use crate::node_client::NodeClient;
use chv_errors::ChvError;

#[derive(Clone)]
pub struct NodeClientPool {
    clients: DashMap<String, NodeClient>,
}

impl Default for NodeClientPool {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeClientPool {
    pub fn new() -> Self {
        Self {
            clients: DashMap::new(),
        }
    }

    pub async fn get_or_connect(
        &self,
        node_id: &str,
        socket_path: &Path,
    ) -> Result<NodeClient, ChvError> {
        // Fast path: check cache
        if let Some(entry) = self.clients.get(node_id) {
            return Ok(entry.clone());
        }

        // Slow path: connect and cache
        let client = NodeClient::connect(socket_path).await?;
        self.clients.insert(node_id.to_string(), client.clone());
        Ok(client)
    }

    pub fn evict(&self, node_id: &str) {
        self.clients.remove(node_id);
    }
}
