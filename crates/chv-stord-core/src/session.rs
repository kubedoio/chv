use dashmap::DashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Session {
    pub volume_id: String,
    pub vm_id: Option<String>,
    pub attachment_handle: String,
    pub export_kind: String,
    pub export_path: String,
    pub runtime_status: String,
}

#[derive(Debug, Clone, Default)]
pub struct SessionTable {
    inner: Arc<DashMap<(String, String), Session>>,
}

impl SessionTable {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    pub fn upsert(&self, session: Session) {
        self.inner.insert(
            (session.volume_id.clone(), session.attachment_handle.clone()),
            session,
        );
    }

    pub fn remove(&self, volume_id: &str, handle: &str) -> Option<Session> {
        self.inner
            .remove(&(volume_id.to_string(), handle.to_string()))
            .map(|(_, v)| v)
    }

    pub fn get(&self, volume_id: &str, handle: &str) -> Option<Session> {
        self.inner
            .get(&(volume_id.to_string(), handle.to_string()))
            .map(|r| r.clone())
    }

    pub fn find_by_volume_and_path(
        &self,
        volume_id: &str,
        path: &str,
    ) -> Option<Session> {
        self.inner.iter().find(|r| {
            r.value().volume_id == volume_id && r.value().export_path == path
        }).map(|r| r.clone())
    }

    pub fn list(&self) -> Vec<Session> {
        self.inner.iter().map(|r| r.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_session(volume_id: &str, handle: &str) -> Session {
        Session {
            volume_id: volume_id.to_string(),
            vm_id: None,
            attachment_handle: handle.to_string(),
            export_kind: "raw".to_string(),
            export_path: "/dev/null".to_string(),
            runtime_status: "open".to_string(),
        }
    }

    #[test]
    fn session_upsert_and_get() {
        let table = SessionTable::new();
        let s = dummy_session("vol-1", "h1");
        table.upsert(s.clone());
        let got = table.get("vol-1", "h1").unwrap();
        assert_eq!(got.volume_id, "vol-1");
    }

    #[test]
    fn session_remove_missing_is_none() {
        let table = SessionTable::new();
        assert!(table.remove("vol-1", "h1").is_none());
    }

    #[test]
    fn session_list_returns_all() {
        let table = SessionTable::new();
        table.upsert(dummy_session("vol-1", "h1"));
        table.upsert(dummy_session("vol-2", "h2"));
        assert_eq!(table.list().len(), 2);
    }

    #[test]
    fn session_idempotency_overwrite() {
        let table = SessionTable::new();
        table.upsert(dummy_session("vol-1", "h1"));
        table.upsert(Session {
            runtime_status: "attached".to_string(),
            ..dummy_session("vol-1", "h1")
        });
        assert_eq!(table.get("vol-1", "h1").unwrap().runtime_status, "attached");
    }

    #[test]
    fn session_find_by_volume_and_path() {
        let table = SessionTable::new();
        let mut s = dummy_session("vol-1", "h1");
        s.export_path = "/data/vol-1.img".to_string();
        table.upsert(s);
        assert!(table.find_by_volume_and_path("vol-1", "/data/vol-1.img").is_some());
        assert!(table.find_by_volume_and_path("vol-1", "/data/other.img").is_none());
    }
}
