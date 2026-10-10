//! Persistent agent identity: the install id (stable across reboots,
//! rotated only by explicit reinstall) and the per-boot sequence
//! counter (the dedup key component that must never move backwards).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("failed to read state: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse state: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize)]
struct StateFile {
    install_id: String,
}

/// Identity + sequence bookkeeping under the agent's state dir.
///
/// The sequence counter is persisted *before* the batch it numbers is
/// sent: a crash between send and ack may lose a batch but can never
/// reuse a `(boot_id, sequence)` key with a different body, which the
/// manager treats as a replay conflict.
pub struct AgentState {
    dir: PathBuf,
    install_id: String,
}

impl AgentState {
    /// Load or create the state under `dir`.
    pub fn load(dir: &Path) -> Result<Self, StateError> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("state.json");
        let install_id = match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str::<StateFile>(&text)?.install_id,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let install_id = Uuid::new_v4().to_string();
                let state = StateFile {
                    install_id: install_id.clone(),
                };
                write_atomic(&path, &serde_json::to_vec_pretty(&state)?)?;
                install_id
            }
            Err(e) => return Err(e.into()),
        };
        Ok(Self {
            dir: dir.to_path_buf(),
            install_id,
        })
    }

    pub fn install_id(&self) -> &str {
        &self.install_id
    }

    /// The next sequence number for this boot, durably allocated.
    pub fn next_sequence(&self, boot_id: &str) -> Result<u64, StateError> {
        let seq_dir = self.dir.join("sequences");
        std::fs::create_dir_all(&seq_dir)?;
        // boot_id comes from /proc (a UUID); keep the filename honest
        // even if a future source is less tidy.
        let safe: String = boot_id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        let safe = if safe.is_empty() {
            "unknown-boot".to_string()
        } else {
            safe
        };
        let path = seq_dir.join(safe);
        let current: u64 = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| t.trim().parse().ok())
            .unwrap_or(0);
        let next = current + 1;
        write_atomic(&path, next.to_string().as_bytes())?;
        Ok(next)
    }

    /// Drop sequence counters older than the newest `keep` boots
    /// (housekeeping; a boot that will never run again has nothing
    /// left to dedup).
    pub fn prune_sequences(&self, keep: usize) -> Result<(), StateError> {
        let seq_dir = self.dir.join("sequences");
        let mut entries: Vec<(u64, PathBuf)> = std::fs::read_dir(&seq_dir)?
            .filter_map(|e| e.ok())
            .map(|e| {
                let modified = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                (modified, e.path())
            })
            .collect();
        entries.sort();
        let excess = entries.len().saturating_sub(keep);
        for (_, path) in entries.into_iter().take(excess) {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }
}

/// Write via tmp+rename so a crash never leaves a torn state file.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_id_is_stable_across_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let a = AgentState::load(dir.path()).unwrap();
        let b = AgentState::load(dir.path()).unwrap();
        assert_eq!(a.install_id(), b.install_id());
        assert_ne!(a.install_id(), "");
    }

    #[test]
    fn sequences_advance_and_never_reuse() {
        let dir = tempfile::tempdir().unwrap();
        let st = AgentState::load(dir.path()).unwrap();
        assert_eq!(st.next_sequence("boot-1").unwrap(), 1);
        assert_eq!(st.next_sequence("boot-1").unwrap(), 2);
        // Simulate a restart: reload and allocate again — no reuse.
        let st2 = AgentState::load(dir.path()).unwrap();
        assert_eq!(st2.next_sequence("boot-1").unwrap(), 3);
        // A different boot starts fresh.
        assert_eq!(st2.next_sequence("boot-2").unwrap(), 1);
    }

    #[test]
    fn boot_ids_are_sanitized_into_filenames() {
        let dir = tempfile::tempdir().unwrap();
        let st = AgentState::load(dir.path()).unwrap();
        assert_eq!(st.next_sequence("../../etc/passwd").unwrap(), 1);
        // Traversal characters are stripped: the counter lands inside
        // the sequences dir under a flat, safe name.
        assert_eq!(
            std::fs::read_dir(dir.path().join("sequences"))
                .unwrap()
                .count(),
            1
        );
        // A fully hostile id still gets a usable, distinct file.
        assert_eq!(st.next_sequence("///").unwrap(), 1);
        assert_eq!(
            std::fs::read_dir(dir.path().join("sequences"))
                .unwrap()
                .count(),
            2
        );
    }
}
