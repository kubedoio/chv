use crate::session::Session;
use chv_errors::ChvError;
use rusqlite::{params, Connection};
use std::path::Path;

pub struct SessionStore {
    conn: Connection,
}

impl SessionStore {
    pub fn new(db_path: &Path) -> Result<Self, ChvError> {
        let conn = Connection::open(db_path).map_err(|e| ChvError::Io {
            path: db_path.to_string_lossy().to_string(),
            source: std::io::Error::other(e),
        })?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                volume_id TEXT NOT NULL,
                attachment_handle TEXT NOT NULL,
                vm_id TEXT,
                export_kind TEXT NOT NULL,
                export_path TEXT NOT NULL,
                runtime_status TEXT NOT NULL,
                PRIMARY KEY (volume_id, attachment_handle)
            )",
            [],
        ).map_err(|e| ChvError::Internal { reason: format!("sqlite init failed: {}", e) })?;
        Ok(Self { conn })
    }

    pub fn upsert(&self, session: &Session) -> Result<(), ChvError> {
        self.conn.execute(
            "INSERT INTO sessions (volume_id, attachment_handle, vm_id, export_kind, export_path, runtime_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(volume_id, attachment_handle) DO UPDATE SET
               vm_id = excluded.vm_id,
               export_kind = excluded.export_kind,
               export_path = excluded.export_path,
               runtime_status = excluded.runtime_status",
            params![
                session.volume_id,
                session.attachment_handle,
                session.vm_id.as_deref().unwrap_or(""),
                session.export_kind,
                session.export_path,
                session.runtime_status,
            ],
        ).map_err(|e| ChvError::Internal { reason: format!("sqlite upsert failed: {}", e) })?;
        Ok(())
    }

    pub fn remove(&self, volume_id: &str, handle: &str) -> Result<(), ChvError> {
        self.conn.execute(
            "DELETE FROM sessions WHERE volume_id = ?1 AND attachment_handle = ?2",
            params![volume_id, handle],
        ).map_err(|e| ChvError::Internal { reason: format!("sqlite remove failed: {}", e) })?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<Session>, ChvError> {
        let mut stmt = self.conn.prepare(
            "SELECT volume_id, attachment_handle, vm_id, export_kind, export_path, runtime_status FROM sessions"
        ).map_err(|e| ChvError::Internal { reason: format!("sqlite prepare failed: {}", e) })?;
        let rows = stmt.query_map([], |row| {
            let vm_id: String = row.get(2)?;
            Ok(Session {
                volume_id: row.get(0)?,
                attachment_handle: row.get(1)?,
                vm_id: if vm_id.is_empty() { None } else { Some(vm_id) },
                export_kind: row.get(3)?,
                export_path: row.get(4)?,
                runtime_status: row.get(5)?,
            })
        }).map_err(|e| ChvError::Internal { reason: format!("sqlite query failed: {}", e) })?;
        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row.map_err(|e| ChvError::Internal { reason: format!("sqlite row failed: {}", e) })?);
        }
        Ok(sessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_session(volume_id: &str, handle: &str) -> Session {
        Session {
            volume_id: volume_id.to_string(),
            attachment_handle: handle.to_string(),
            vm_id: Some("vm-1".to_string()),
            export_kind: "raw".to_string(),
            export_path: "/dev/null".to_string(),
            runtime_status: "open".to_string(),
        }
    }

    #[test]
    fn store_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(&dir.path().join("stord.db")).unwrap();
        let s = dummy_session("vol-1", "h1");
        store.upsert(&s).unwrap();
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].volume_id, "vol-1");
        store.remove("vol-1", "h1").unwrap();
        assert!(store.list().unwrap().is_empty());
    }
}
