use dashmap::DashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::node_client::{CircuitBreaker, NodeClient};
use chv_errors::ChvError;

#[derive(Clone)]
pub struct NodeClientPool {
    clients: DashMap<String, (NodeClient, Instant)>,
    breakers: DashMap<String, Arc<CircuitBreaker>>,
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
            breakers: DashMap::new(),
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
            drop(entry);
            self.clients.remove(node_id);
        }
        let breaker = self
            .breakers
            .entry(node_id.to_string())
            .or_insert_with(|| Arc::new(CircuitBreaker::new()))
            .clone();
        let client = NodeClient::connect_with_breaker(socket_path, breaker).await?;
        let inserted = self
            .clients
            .entry(node_id.to_string())
            .or_insert_with(|| (client.clone(), Instant::now()));
        Ok(inserted.0.clone())
    }

    pub fn evict(&self, node_id: &str) {
        self.clients.remove(node_id);
    }
}
