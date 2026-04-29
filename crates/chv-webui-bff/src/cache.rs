use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct CacheEntry {
    pub data: String, // JSON response body
    pub cached_at: Instant,
    pub ttl: Option<Duration>,
}

#[derive(Clone)]
pub struct BffCache {
    inner: Arc<RwLock<HashMap<String, CacheEntry>>>,
    ttl: Duration,
}

impl BffCache {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        let guard = self.inner.read().await;
        guard.get(key).and_then(|entry| {
            let ttl = entry.ttl.unwrap_or(self.ttl);
            if entry.cached_at.elapsed() < ttl {
                Some(entry.data.clone())
            } else {
                None
            }
        })
    }

    pub async fn set(&self, key: &str, data: String) {
        self.set_with_ttl(key, data, None).await;
    }

    pub async fn set_with_ttl(&self, key: &str, data: String, ttl: Option<Duration>) {
        let mut guard = self.inner.write().await;
        guard.insert(
            key.to_string(),
            CacheEntry {
                data,
                cached_at: Instant::now(),
                ttl,
            },
        );
    }

    pub async fn invalidate(&self, prefix: &str) {
        let mut guard = self.inner.write().await;
        guard.retain(|key, _| !key.starts_with(prefix));
    }
}
