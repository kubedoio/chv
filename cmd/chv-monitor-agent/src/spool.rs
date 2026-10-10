//! Durable batch spool for outages: batches that cannot be delivered
//! are written to disk (bounded count, age-pruned) and replayed
//! oldest-first when the manager is reachable again.

use crate::wire::EnvelopeJson;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum SpoolError {
    #[error("failed to read spool: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse spool entry: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpoolEntry {
    /// The durable key: this is what the manager dedups on.
    pub agent_id: String,
    pub boot_id: String,
    pub sequence: u64,
    pub envelope: EnvelopeJson,
}

/// Caller-side outcome of handing one entry to the transport.
pub enum DrainResult {
    /// Delivered (accepted or duplicate — both are "the manager has
    /// it durably"). Carries the manager's renewal hint.
    Delivered { renewal_due: bool },
    /// The manager rejects this specific batch forever (replay
    /// conflict / resync required): drop it and continue.
    Discard,
    /// The manager is unreachable, throttling, or degraded: keep the
    /// entry, stop draining, retry later.
    Retry,
    /// The credential was refused (revoked / identity conflict): stop
    /// draining; re-enrollment or operator action is required.
    Unauthorized,
}

/// What a drain pass accomplished.
#[derive(Debug, Default)]
pub struct DrainSummary {
    /// At least one entry was delivered or permanently discarded.
    pub delivered_any: bool,
    /// Any delivered response flagged `renewal_due`.
    pub renewal_due: bool,
    /// Draining stopped because the credential was refused.
    pub unauthorized: bool,
}

pub struct Spool {
    dir: PathBuf,
    max_batches: usize,
    max_age_seconds: u64,
    next_counter: u64,
}

impl Spool {
    pub fn open(dir: &Path, max_batches: usize, max_age_seconds: u64) -> Result<Self, SpoolError> {
        std::fs::create_dir_all(dir)?;
        let mut next_counter = 0;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let stem = name.to_string_lossy();
            if let Some(n) = stem
                .strip_suffix(".json")
                .and_then(|s| s.parse::<u64>().ok())
            {
                next_counter = next_counter.max(n + 1);
            }
        }
        let spool = Self {
            dir: dir.to_path_buf(),
            max_batches,
            max_age_seconds,
            next_counter,
        };
        spool.prune_age()?;
        spool.enforce_bound(0)?;
        Ok(spool)
    }

    /// Append one batch. When the spool is full, the OLDEST entries
    /// are dropped — losing the stalest data first is the only honest
    /// choice under a bounded outage budget.
    pub fn push(&mut self, entry: SpoolEntry) -> Result<(), SpoolError> {
        self.prune_age()?;
        self.enforce_bound(1)?;
        let path = self.entry_path(self.next_counter);
        self.next_counter += 1;
        let bytes = serde_json::to_vec(&entry)?;
        std::fs::write(path, bytes)?;
        self.enforce_bound(0)?;
        Ok(())
    }

    /// Drain entries oldest-first. The callback decides the fate of
    /// each entry (see `DrainResult`); draining stops at the first
    /// `Retry`/`Unauthorized` and the pass is summarized.
    pub async fn drain<F, Fut>(&self, mut deliver: F) -> Result<DrainSummary, SpoolError>
    where
        F: FnMut(SpoolEntry) -> Fut,
        Fut: std::future::Future<Output = DrainResult>,
    {
        let mut summary = DrainSummary::default();
        for path in self.sorted_paths()? {
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            let entry: SpoolEntry = serde_json::from_slice(&bytes)?;
            match deliver(entry).await {
                DrainResult::Delivered { renewal_due } => {
                    summary.delivered_any = true;
                    summary.renewal_due |= renewal_due;
                    let _ = std::fs::remove_file(&path);
                }
                DrainResult::Discard => {
                    summary.delivered_any = true;
                    let _ = std::fs::remove_file(&path);
                }
                DrainResult::Retry => break,
                DrainResult::Unauthorized => {
                    summary.unauthorized = true;
                    break;
                }
            }
        }
        Ok(summary)
    }

    pub fn len(&self) -> usize {
        self.sorted_paths().map(|p| p.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drop entries belonging to a different agent identity. Called
    /// after re-enrollment: data spooled under a revoked credential
    /// can never be attributed by the manager, and keeping it would
    /// poison the replay queue.
    pub fn purge_other_agents(&self, agent_id: &str) -> Result<usize, SpoolError> {
        let mut removed = 0;
        for path in self.sorted_paths()? {
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Ok(entry) = serde_json::from_slice::<SpoolEntry>(&bytes) else {
                // Unparseable entries are corrupt; drop them.
                let _ = std::fs::remove_file(&path);
                removed += 1;
                continue;
            };
            if entry.agent_id != agent_id {
                let _ = std::fs::remove_file(&path);
                removed += 1;
            }
        }
        Ok(removed)
    }

    fn entry_path(&self, counter: u64) -> PathBuf {
        self.dir.join(format!("{counter:016}.json"))
    }

    fn sorted_paths(&self) -> Result<Vec<PathBuf>, SpoolError> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&self.dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|e| e == "json").unwrap_or(false))
            .collect();
        paths.sort();
        Ok(paths)
    }

    fn prune_age(&self) -> Result<(), SpoolError> {
        if self.max_age_seconds == 0 {
            return Ok(());
        }
        let cutoff = std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(self.max_age_seconds))
            .unwrap_or(std::time::UNIX_EPOCH);
        for path in self.sorted_paths()? {
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            match meta.modified() {
                Ok(modified) if modified < cutoff => {
                    let _ = std::fs::remove_file(&path);
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Keep at most `max_batches - headroom` entries.
    fn enforce_bound(&self, headroom: usize) -> Result<(), SpoolError> {
        let limit = self.max_batches.saturating_sub(headroom);
        let paths = self.sorted_paths()?;
        let excess = paths.len().saturating_sub(limit);
        for path in paths.into_iter().take(excess) {
            let _ = std::fs::remove_file(&path);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn entry(seq: u64) -> SpoolEntry {
        SpoolEntry {
            agent_id: "a".into(),
            boot_id: "b".into(),
            sequence: seq,
            envelope: EnvelopeJson {
                schema_version: 1,
                agent_id: "a".into(),
                install_id: "i".into(),
                boot_id: "b".into(),
                sequence: seq,
                sent_at_ms: seq as i64,
                os: None,
                samples: vec![crate::wire::SampleJson {
                    schema_version: 1,
                    target_kind: "vm".into(),
                    target_id: "vm-1".into(),
                    metric_id: "vm.guest.load1".into(),
                    source: "guest_agent".into(),
                    kind: "gauge".into(),
                    unit: "count".into(),
                    observed_at_ms: seq as i64,
                    value: serde_json::json!(0.5),
                    quality: "valid".into(),
                    dimensions: BTreeMap::new(),
                    boot_id: "b".into(),
                    identity_epoch: "agent-credential-generation-1".into(),
                }],
            },
        }
    }

    fn open(max: usize) -> (tempfile::TempDir, Spool) {
        let dir = tempfile::tempdir().unwrap();
        let spool = Spool::open(dir.path(), max, 0).unwrap();
        (dir, spool)
    }

    #[tokio::test]
    async fn push_then_drain_delivers_oldest_first() {
        let (_dir, mut spool) = open(10);
        spool.push(entry(1)).unwrap();
        spool.push(entry(2)).unwrap();
        spool.push(entry(3)).unwrap();
        assert_eq!(spool.len(), 3);

        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        spool
            .drain(|e| {
                let seq = e.sequence;
                let seen = std::sync::Arc::clone(&seen);
                async move {
                    seen.lock().unwrap().push(seq);
                    DrainResult::Delivered { renewal_due: false }
                }
            })
            .await
            .unwrap();
        assert_eq!(*seen.lock().unwrap(), vec![1, 2, 3]);
        assert_eq!(spool.len(), 0);
    }

    #[tokio::test]
    async fn retry_keeps_entries_and_stops() {
        let (_dir, mut spool) = open(10);
        spool.push(entry(1)).unwrap();
        spool.push(entry(2)).unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        spool
            .drain(|e| {
                let seq = e.sequence;
                let seen = std::sync::Arc::clone(&seen);
                async move {
                    seen.lock().unwrap().push(seq);
                    DrainResult::Retry
                }
            })
            .await
            .unwrap();
        assert_eq!(
            *seen.lock().unwrap(),
            vec![1],
            "drain must stop at first retry"
        );
        assert_eq!(spool.len(), 2, "both entries must remain");
    }

    #[tokio::test]
    async fn discard_removes_entry_and_continues() {
        let (_dir, mut spool) = open(10);
        spool.push(entry(1)).unwrap();
        spool.push(entry(2)).unwrap();
        spool
            .drain(|e| {
                let seq = e.sequence;
                async move {
                    if seq == 1 {
                        DrainResult::Discard
                    } else {
                        DrainResult::Delivered { renewal_due: false }
                    }
                }
            })
            .await
            .unwrap();
        assert_eq!(spool.len(), 0);
    }

    #[tokio::test]
    async fn bound_drops_oldest() {
        let (_dir, mut spool) = open(2);
        spool.push(entry(1)).unwrap();
        spool.push(entry(2)).unwrap();
        spool.push(entry(3)).unwrap();
        assert_eq!(spool.len(), 2);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        spool
            .drain(|e| {
                let seq = e.sequence;
                let seen = std::sync::Arc::clone(&seen);
                async move {
                    seen.lock().unwrap().push(seq);
                    DrainResult::Delivered { renewal_due: false }
                }
            })
            .await
            .unwrap();
        assert_eq!(
            *seen.lock().unwrap(),
            vec![2, 3],
            "oldest entries are dropped first"
        );
    }
}
