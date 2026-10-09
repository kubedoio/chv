use thiserror::Error;

/// Failures of the monitoring store. The monitoring store is disposable
/// telemetry: none of these errors may ever propagate into VM or node
/// lifecycle paths — callers degrade monitoring and continue.
#[derive(Debug, Error)]
pub enum MonitoringStoreError {
    #[error("monitoring database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("monitoring migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("invalid monitoring store configuration: {reason}")]
    InvalidConfiguration { reason: String },
    #[error("monitoring store is degraded: {reason}")]
    Degraded { reason: String },
    #[error("query rejected: {reason}")]
    QueryRejected { reason: String },
}

/// The durable outcome of an ingestion attempt, mirroring the ingestion
/// contract's response codes for the node batch transport.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IngestOutcome {
    /// Batch durably committed; the count of samples stored.
    Accepted { samples: u32 },
    /// An identical batch (same key, same digest) was already committed.
    Duplicate { samples: u32 },
    /// Same `(sender, boot_id, sequence)` key with a different body.
    ReplayConflict,
    /// The target already exceeds its series cap; the whole batch is
    /// rejected (v1 never ACKs an ambiguous subset).
    SeriesCapExceeded {
        target_id: String,
        series: i64,
        cap: i64,
    },
    /// The sequence is at or below the sender's high-water mark but its
    /// dedup proof is no longer retained — fails closed, the agent must
    /// resynchronize (start a new boot epoch).
    StaleSequence,
}

impl IngestOutcome {
    /// Whether the batch is durably committed (the only outcomes an
    /// agent may treat as delivered).
    pub fn is_committed(&self) -> bool {
        matches!(
            self,
            IngestOutcome::Accepted { .. } | IngestOutcome::Duplicate { .. }
        )
    }
}
