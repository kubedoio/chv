//! Certificate file watcher for automatic TLS cert rotation.
//!
//! Polls certificate, key, and CA files for mtime changes and triggers
//! a reload callback when any file has been updated on disk.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::{debug, info, warn};

/// Watches certificate files for changes and triggers a reload callback.
///
/// This uses filesystem polling (mtime comparison) rather than inotify/kqueue
/// for maximum portability across Linux and macOS. The polling interval should
/// be set appropriately (e.g., 30-60 seconds) since cert rotation is infrequent.
pub struct CertWatcher {
    cert_path: PathBuf,
    key_path: PathBuf,
    ca_path: PathBuf,
    last_modified: HashMap<PathBuf, SystemTime>,
    on_reload: Arc<dyn Fn() + Send + Sync>,
}

impl CertWatcher {
    /// Create a new certificate watcher.
    ///
    /// The `on_reload` callback is invoked whenever any of the watched files
    /// has a newer modification time than previously observed.
    pub fn new(
        cert_path: PathBuf,
        key_path: PathBuf,
        ca_path: PathBuf,
        on_reload: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let mut last_modified = HashMap::new();

        // Initialize with current mtimes (if files exist)
        for path in [&cert_path, &key_path, &ca_path] {
            if let Ok(metadata) = std::fs::metadata(path) {
                if let Ok(mtime) = metadata.modified() {
                    last_modified.insert(path.clone(), mtime);
                }
            }
        }

        info!(
            cert = %cert_path.display(),
            key = %key_path.display(),
            ca = %ca_path.display(),
            "certificate watcher initialized"
        );

        Self {
            cert_path,
            key_path,
            ca_path,
            last_modified,
            on_reload,
        }
    }

    /// Check all watched files for mtime changes.
    ///
    /// Returns `true` if any file has changed and the reload callback was invoked.
    /// Returns `false` if no changes were detected.
    pub fn check_for_changes(&mut self) -> bool {
        let paths = [
            self.cert_path.clone(),
            self.key_path.clone(),
            self.ca_path.clone(),
        ];

        let mut changed = false;

        for path in &paths {
            match std::fs::metadata(path) {
                Ok(metadata) => match metadata.modified() {
                    Ok(mtime) => {
                        let previously_seen = self.last_modified.get(path).copied();
                        match previously_seen {
                            Some(prev_mtime) if mtime > prev_mtime => {
                                info!(
                                    path = %path.display(),
                                    "certificate file changed, triggering reload"
                                );
                                self.last_modified.insert(path.clone(), mtime);
                                changed = true;
                            }
                            None => {
                                // File appeared for the first time (or first check)
                                debug!(
                                    path = %path.display(),
                                    "certificate file appeared"
                                );
                                self.last_modified.insert(path.clone(), mtime);
                                changed = true;
                            }
                            _ => {
                                // No change
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            path = %path.display(),
                            error = %e,
                            "failed to read modification time"
                        );
                    }
                },
                Err(e) => {
                    // File might have been temporarily removed during rotation
                    debug!(
                        path = %path.display(),
                        error = %e,
                        "certificate file not accessible (may be rotating)"
                    );
                }
            }
        }

        if changed {
            (self.on_reload)();
        }

        changed
    }

    /// Spawn a background tokio task that polls for certificate changes.
    ///
    /// The task runs indefinitely, checking files at the specified interval.
    /// Returns a `JoinHandle` that can be used to abort the watcher if needed.
    pub fn spawn_watcher(mut self, interval: Duration) -> tokio::task::JoinHandle<()> {
        info!(
            interval_secs = interval.as_secs(),
            "spawning certificate watcher background task"
        );

        tokio::spawn(async move {
            let mut tick = tokio::time::interval(interval);
            loop {
                tick.tick().await;
                self.check_for_changes();
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_new_watcher_initializes() {
        let dir = tempfile::tempdir().unwrap();
        let cert = dir.path().join("cert.pem");
        let key = dir.path().join("key.pem");
        let ca = dir.path().join("ca.pem");

        std::fs::write(&cert, "cert").unwrap();
        std::fs::write(&key, "key").unwrap();
        std::fs::write(&ca, "ca").unwrap();

        let reload_count = Arc::new(AtomicU32::new(0));
        let count_clone = reload_count.clone();

        let watcher = CertWatcher::new(
            cert,
            key,
            ca,
            Arc::new(move || {
                count_clone.fetch_add(1, Ordering::SeqCst);
            }),
        );

        assert_eq!(watcher.last_modified.len(), 3);
        assert_eq!(reload_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_no_change_detected() {
        let dir = tempfile::tempdir().unwrap();
        let cert = dir.path().join("cert.pem");
        let key = dir.path().join("key.pem");
        let ca = dir.path().join("ca.pem");

        std::fs::write(&cert, "cert").unwrap();
        std::fs::write(&key, "key").unwrap();
        std::fs::write(&ca, "ca").unwrap();

        let reload_count = Arc::new(AtomicU32::new(0));
        let count_clone = reload_count.clone();

        let mut watcher = CertWatcher::new(
            cert,
            key,
            ca,
            Arc::new(move || {
                count_clone.fetch_add(1, Ordering::SeqCst);
            }),
        );

        let changed = watcher.check_for_changes();
        assert!(!changed);
        assert_eq!(reload_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_change_detected_on_file_update() {
        let dir = tempfile::tempdir().unwrap();
        let cert = dir.path().join("cert.pem");
        let key = dir.path().join("key.pem");
        let ca = dir.path().join("ca.pem");

        std::fs::write(&cert, "cert-v1").unwrap();
        std::fs::write(&key, "key-v1").unwrap();
        std::fs::write(&ca, "ca-v1").unwrap();

        let reload_count = Arc::new(AtomicU32::new(0));
        let count_clone = reload_count.clone();

        let mut watcher = CertWatcher::new(
            cert.clone(),
            key,
            ca,
            Arc::new(move || {
                count_clone.fetch_add(1, Ordering::SeqCst);
            }),
        );

        // Ensure mtime advances (some filesystems have 1s resolution)
        std::thread::sleep(Duration::from_millis(1100));
        std::fs::write(&cert, "cert-v2").unwrap();

        let changed = watcher.check_for_changes();
        assert!(changed);
        assert_eq!(reload_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_missing_file_does_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        let cert = dir.path().join("nonexistent-cert.pem");
        let key = dir.path().join("nonexistent-key.pem");
        let ca = dir.path().join("nonexistent-ca.pem");

        let reload_count = Arc::new(AtomicU32::new(0));
        let count_clone = reload_count.clone();

        let mut watcher = CertWatcher::new(
            cert,
            key,
            ca,
            Arc::new(move || {
                count_clone.fetch_add(1, Ordering::SeqCst);
            }),
        );

        // Should not panic even though files don't exist
        let changed = watcher.check_for_changes();
        assert!(!changed);
    }

    #[test]
    fn test_file_appearing_triggers_reload() {
        let dir = tempfile::tempdir().unwrap();
        let cert = dir.path().join("cert.pem");
        let key = dir.path().join("key.pem");
        let ca = dir.path().join("ca.pem");

        // Start with no files
        let reload_count = Arc::new(AtomicU32::new(0));
        let count_clone = reload_count.clone();

        let mut watcher = CertWatcher::new(
            cert.clone(),
            key.clone(),
            ca.clone(),
            Arc::new(move || {
                count_clone.fetch_add(1, Ordering::SeqCst);
            }),
        );

        // Create files
        std::fs::write(&cert, "cert").unwrap();
        std::fs::write(&key, "key").unwrap();
        std::fs::write(&ca, "ca").unwrap();

        let changed = watcher.check_for_changes();
        assert!(changed);
        assert_eq!(reload_count.load(Ordering::SeqCst), 1);
    }
}
