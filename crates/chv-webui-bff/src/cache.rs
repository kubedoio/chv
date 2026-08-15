use moka::future::Cache;
use std::time::Duration;

#[derive(Clone)]
pub struct BffCache {
    inner: Cache<String, String>,
}

impl BffCache {
    pub fn new(ttl_secs: u64) -> Self {
        let ttl = Duration::from_secs(ttl_secs);
        Self {
            inner: Cache::builder()
                .max_capacity(2000)
                .time_to_live(ttl)
                .build(),
        }
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        self.inner.get(key).await
    }

    pub async fn set(&self, key: &str, data: String) {
        self.inner.insert(key.to_string(), data).await;
    }

    pub async fn invalidate(&self, prefix: &str) {
        let prefix_owned = prefix.to_string();
        self.inner
            .invalidate_entries_if(move |k, _v| k.starts_with(&prefix_owned))
            .ok();
    }
}
