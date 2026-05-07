use std::time::Duration;

/// Manages the send window for flow control during block streaming.
///
/// The sender tracks how many blocks have been sent without acknowledgement.
/// When the window is full, the sender must wait for Ack messages before
/// sending more blocks.
pub struct SendWindow {
    /// Maximum number of unacknowledged blocks allowed in flight.
    max_unacked: u32,
    /// How often the receiver sends an Ack (in number of blocks).
    ack_interval: u32,
    /// Timeout waiting for an acknowledgement.
    timeout: Duration,
    /// Current number of unacknowledged blocks.
    unacked: u32,
    /// Sequence number of the last received Ack.
    last_ack_sequence: u32,
}

impl SendWindow {
    /// Create a new SendWindow with default parameters.
    ///
    /// Defaults: max_unacked=128, ack_interval=64, timeout=30s
    pub fn new() -> Self {
        Self {
            max_unacked: 128,
            ack_interval: 64,
            timeout: Duration::from_secs(30),
            unacked: 0,
            last_ack_sequence: 0,
        }
    }

    /// Create a SendWindow with custom parameters.
    pub fn with_params(max_unacked: u32, ack_interval: u32, timeout: Duration) -> Self {
        Self {
            max_unacked,
            ack_interval,
            timeout,
            unacked: 0,
            last_ack_sequence: 0,
        }
    }

    /// Returns true if the sender is allowed to send another block.
    pub fn can_send(&self) -> bool {
        self.unacked < self.max_unacked
    }

    /// Record that a block has been sent (increments unacked counter).
    pub fn sent(&mut self) {
        self.unacked = self.unacked.saturating_add(1);
    }

    /// Record that an Ack was received for a given sequence number.
    /// Decreases the unacked count by the number of blocks acknowledged.
    pub fn acked(&mut self, sequence_num: u32) {
        if sequence_num > self.last_ack_sequence {
            let acked_count = sequence_num - self.last_ack_sequence;
            self.unacked = self.unacked.saturating_sub(acked_count);
            self.last_ack_sequence = sequence_num;
        }
    }

    /// Returns true if the sender should request an Ack from the receiver.
    /// This is based on whether the number of unacked blocks has reached
    /// the ack_interval threshold.
    pub fn should_request_ack(&self) -> bool {
        self.unacked >= self.ack_interval
    }

    /// Returns the configured timeout duration.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Returns the ack interval.
    pub fn ack_interval(&self) -> u32 {
        self.ack_interval
    }
}

impl Default for SendWindow {
    fn default() -> Self {
        Self::new()
    }
}
