//! Link health monitoring for network interfaces.
//!
//! Provides basic link state monitoring by reading sysfs entries under
//! `/sys/class/net/{iface}/`. Tracks operational state, carrier presence,
//! and link flap events (transitions from up to down).
//!
//! This is NOT a full BFD (Bidirectional Forwarding Detection) implementation,
//! but provides the essential link health visibility needed for M1.

use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Snapshot of link health for a single interface.
#[derive(Debug, Clone)]
pub struct LinkHealthSnapshot {
    /// Interface name (e.g. "eth0", "br0", "tap-vm1").
    pub iface: String,
    /// Whether the interface is operationally up.
    pub is_up: bool,
    /// Whether the physical carrier is detected.
    pub carrier: bool,
    /// Number of up-to-down transitions observed since monitoring started.
    pub flap_count: u64,
    /// Time of the last flap event (None if no flaps observed).
    pub last_flap_time: Option<Instant>,
}

/// Internal per-interface tracking state.
#[derive(Debug, Clone)]
struct LinkState {
    was_up: bool,
    flap_count: u64,
    last_flap_time: Option<Instant>,
}

// ---------------------------------------------------------------------------
// Link monitor
// ---------------------------------------------------------------------------

/// Link health monitor that tracks interface state over time.
///
/// Reads sysfs entries to determine operational state and carrier presence.
/// Tracks transitions from up-to-down as "flaps".
pub struct LinkMonitor {
    /// Base path for sysfs net class (overridable for testing).
    sysfs_base: String,
    /// Per-interface tracked state.
    state: HashMap<String, LinkState>,
}

impl LinkMonitor {
    /// Create a new monitor using the real sysfs path.
    pub fn new() -> Self {
        Self {
            sysfs_base: "/sys/class/net".to_string(),
            state: HashMap::new(),
        }
    }

    /// Read the operstate file for an interface.
    /// Returns true if the content is "up".
    fn read_operstate(&self, iface: &str) -> bool {
        let path = format!("{}/{}/operstate", self.sysfs_base, iface);
        match std::fs::read_to_string(&path) {
            Ok(content) => content.trim() == "up",
            Err(e) => {
                debug!(iface = %iface, error = %e, "failed to read operstate");
                false
            }
        }
    }

    /// Read the carrier file for an interface.
    /// Returns true if the content is "1".
    fn read_carrier(&self, iface: &str) -> bool {
        let path = format!("{}/{}/carrier", self.sysfs_base, iface);
        match std::fs::read_to_string(&path) {
            Ok(content) => content.trim() == "1",
            Err(e) => {
                // carrier file may not exist or be unreadable if interface is down
                debug!(iface = %iface, error = %e, "failed to read carrier");
                false
            }
        }
    }

    /// Check a single interface and return its health snapshot.
    /// Updates internal flap tracking state.
    pub fn check_link(&mut self, iface: &str) -> LinkHealthSnapshot {
        let is_up = self.read_operstate(iface);
        let carrier = self.read_carrier(iface);

        let state = self.state.entry(iface.to_string()).or_insert(LinkState {
            was_up: is_up,
            flap_count: 0,
            last_flap_time: None,
        });

        // Detect flap: transition from up to down
        if state.was_up && !is_up {
            state.flap_count += 1;
            state.last_flap_time = Some(Instant::now());
            warn!(
                iface = %iface,
                flap_count = state.flap_count,
                "link flap detected (up -> down)"
            );
        }

        state.was_up = is_up;

        LinkHealthSnapshot {
            iface: iface.to_string(),
            is_up,
            carrier,
            flap_count: state.flap_count,
            last_flap_time: state.last_flap_time,
        }
    }

    /// Check all provided interfaces and return their health snapshots.
    pub fn check_all(&mut self, interfaces: &[String]) -> Vec<LinkHealthSnapshot> {
        interfaces
            .iter()
            .map(|iface| self.check_link(iface))
            .collect()
    }

    /// Get the current flap count for an interface without re-reading state.
    pub fn flap_count(&self, iface: &str) -> u64 {
        self.state.get(iface).map(|s| s.flap_count).unwrap_or(0)
    }
}

impl Default for LinkMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Run a periodic link health check loop, reporting results via the provided
/// callback. Shuts down when the watch channel fires.
pub async fn link_health_loop(
    interfaces: Vec<String>,
    interval_secs: u64,
    mut shutdown: tokio::sync::watch::Receiver<()>,
    on_snapshot: impl Fn(&[LinkHealthSnapshot]) + Send + 'static,
) {
    let mut monitor = LinkMonitor::new();
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let snapshots = monitor.check_all(&interfaces);
                on_snapshot(&snapshots);
            }
            _ = shutdown.changed() => {
                debug!("link health monitor shutting down");
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_fake_sysfs(base: &std::path::Path, iface: &str, operstate: &str, carrier: &str) {
        let iface_dir = base.join(iface);
        fs::create_dir_all(&iface_dir).unwrap();
        fs::write(iface_dir.join("operstate"), operstate).unwrap();
        fs::write(iface_dir.join("carrier"), carrier).unwrap();
    }

    #[test]
    fn detects_link_up() {
        let tmp = tempfile::tempdir().unwrap();
        setup_fake_sysfs(tmp.path(), "eth0", "up\n", "1\n");

        let mut monitor = LinkMonitor {
            sysfs_base: tmp.path().to_string_lossy().to_string(),
            state: HashMap::new(),
        };
        let snap = monitor.check_link("eth0");

        assert!(snap.is_up);
        assert!(snap.carrier);
        assert_eq!(snap.flap_count, 0);
        assert!(snap.last_flap_time.is_none());
    }

    #[test]
    fn detects_link_down() {
        let tmp = tempfile::tempdir().unwrap();
        setup_fake_sysfs(tmp.path(), "eth0", "down\n", "0\n");

        let mut monitor = LinkMonitor {
            sysfs_base: tmp.path().to_string_lossy().to_string(),
            state: HashMap::new(),
        };
        let snap = monitor.check_link("eth0");

        assert!(!snap.is_up);
        assert!(!snap.carrier);
        assert_eq!(snap.flap_count, 0);
    }

    #[test]
    fn detects_flap() {
        let tmp = tempfile::tempdir().unwrap();
        let iface_dir = tmp.path().join("eth0");
        fs::create_dir_all(&iface_dir).unwrap();

        // Start with link up
        fs::write(iface_dir.join("operstate"), "up\n").unwrap();
        fs::write(iface_dir.join("carrier"), "1\n").unwrap();

        let mut monitor = LinkMonitor {
            sysfs_base: tmp.path().to_string_lossy().to_string(),
            state: HashMap::new(),
        };
        let snap = monitor.check_link("eth0");
        assert!(snap.is_up);
        assert_eq!(snap.flap_count, 0);

        // Transition to down
        fs::write(iface_dir.join("operstate"), "down\n").unwrap();
        fs::write(iface_dir.join("carrier"), "0\n").unwrap();

        let snap = monitor.check_link("eth0");
        assert!(!snap.is_up);
        assert_eq!(snap.flap_count, 1);
        assert!(snap.last_flap_time.is_some());
    }

    #[test]
    fn multiple_flaps_counted() {
        let tmp = tempfile::tempdir().unwrap();
        let iface_dir = tmp.path().join("eth0");
        fs::create_dir_all(&iface_dir).unwrap();

        let mut monitor = LinkMonitor {
            sysfs_base: tmp.path().to_string_lossy().to_string(),
            state: HashMap::new(),
        };

        // up -> down -> up -> down = 2 flaps
        fs::write(iface_dir.join("operstate"), "up\n").unwrap();
        fs::write(iface_dir.join("carrier"), "1\n").unwrap();
        monitor.check_link("eth0");

        fs::write(iface_dir.join("operstate"), "down\n").unwrap();
        fs::write(iface_dir.join("carrier"), "0\n").unwrap();
        monitor.check_link("eth0");

        fs::write(iface_dir.join("operstate"), "up\n").unwrap();
        fs::write(iface_dir.join("carrier"), "1\n").unwrap();
        monitor.check_link("eth0");

        fs::write(iface_dir.join("operstate"), "down\n").unwrap();
        fs::write(iface_dir.join("carrier"), "0\n").unwrap();
        let snap = monitor.check_link("eth0");

        assert_eq!(snap.flap_count, 2);
    }

    #[test]
    fn check_all_returns_multiple() {
        let tmp = tempfile::tempdir().unwrap();
        setup_fake_sysfs(tmp.path(), "eth0", "up\n", "1\n");
        setup_fake_sysfs(tmp.path(), "br0", "down\n", "0\n");

        let mut monitor = LinkMonitor {
            sysfs_base: tmp.path().to_string_lossy().to_string(),
            state: HashMap::new(),
        };
        let ifaces = vec!["eth0".to_string(), "br0".to_string()];
        let snaps = monitor.check_all(&ifaces);

        assert_eq!(snaps.len(), 2);
        assert!(snaps[0].is_up);
        assert!(!snaps[1].is_up);
    }

    #[test]
    fn missing_interface_returns_down() {
        let tmp = tempfile::tempdir().unwrap();
        let mut monitor = LinkMonitor {
            sysfs_base: tmp.path().to_string_lossy().to_string(),
            state: HashMap::new(),
        };
        let snap = monitor.check_link("nonexistent0");

        assert!(!snap.is_up);
        assert!(!snap.carrier);
        assert_eq!(snap.flap_count, 0);
    }
}
