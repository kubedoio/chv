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
    max_entries: usize,
}

impl BffCache {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            ttl: Duration::from_secs(ttl_secs),
            max_entries: 2000,
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

        // Evict expired entries
        let now = Instant::now();
        let default_ttl = self.ttl;
        guard.retain(|_, entry| {
            let entry_ttl = entry.ttl.unwrap_or(default_ttl);
            now.duration_since(entry.cached_at) < entry_ttl
        });

        // Hard cap: if still at limit, evict oldest entries
        if guard.len() >= self.max_entries {
            let mut entries: Vec<(String, Instant)> = guard
                .iter()
                .map(|(k, v)| (k.clone(), v.cached_at))
                .collect();
            entries.sort_by_key(|(_, t)| *t);
            let evict_count = guard.len() - self.max_entries + 1;
            for (k, _) in entries.into_iter().take(evict_count) {
                guard.remove(&k);
            }
        }

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
