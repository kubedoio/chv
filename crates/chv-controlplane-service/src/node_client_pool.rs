use dashmap::DashMap;
use std::path::Path;
use std::time::{Duration, Instant};
use crate::node_client::NodeClient;
use chv_errors::ChvError;

#[derive(Clone)]
pub struct NodeClientPool {
    clients: DashMap<String, (NodeClient, Instant)>,
    ttl: Duration,
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
            ttl: Duration::from_secs(300),
        }
    }

    pub async fn get_or_connect(
        &self,
        node_id: &str,
        socket_path: &Path,
    ) -> Result<NodeClient, ChvError> {
        if let Some(entry) = self.clients.get(node_id) {
            if entry.1.elapsed() < self.ttl {
                return Ok(entry.0.clone());
            }
            // expired, fall through to reconnect
        }
        let client = NodeClient::connect(socket_path).await?;
        self.clients.insert(node_id.to_string(), (client.clone(), Instant::now()));
        Ok(client)
    }

    pub fn evict(&self, node_id: &str) {
        self.clients.remove(node_id);
    }
}
