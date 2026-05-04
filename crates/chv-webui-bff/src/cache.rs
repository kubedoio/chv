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
        // Evict expired entries without holding write lock during the scan
        {
            let guard = self.inner.read().await;
            if guard.len() > 1000 {
                let now = Instant::now();
                let default_ttl = self.ttl;
                let expired_keys: Vec<String> = guard
                    .iter()
                    .filter(|(_, entry)| {
                        let entry_ttl = entry.ttl.unwrap_or(default_ttl);
                        now.duration_since(entry.cached_at) >= entry_ttl
                    })
                    .map(|(k, _)| k.clone())
                    .collect();
                drop(guard);

                if !expired_keys.is_empty() {
                    let mut write_guard = self.inner.write().await;
                    let now = Instant::now();
                    for k in &expired_keys {
                        if let Some(entry) = write_guard.get(k) {
                            let entry_ttl = entry.ttl.unwrap_or(default_ttl);
                            if now.duration_since(entry.cached_at) >= entry_ttl {
                                write_guard.remove(k);
                            }
                        }
                    }
                    // Insert the new entry while we hold the write lock
                    write_guard.insert(
                        key.to_string(),
                        CacheEntry {
                            data,
                            cached_at: Instant::now(),
                            ttl,
                        },
                    );
                    return;
                }
            }
        }

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
