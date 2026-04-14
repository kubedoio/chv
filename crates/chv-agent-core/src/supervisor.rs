use chv_errors::ChvError;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tracing::{debug, info, warn};

const MIN_RESTART_INTERVAL: Duration = Duration::from_secs(5);

pub struct DaemonSupervisor {
    stord_bin: PathBuf,
    nwd_bin: PathBuf,
    pub(crate) stord_socket: PathBuf,
    pub(crate) nwd_socket: PathBuf,
    runtime_dir: PathBuf,
    stord_child: Option<Child>,
    nwd_child: Option<Child>,
    stord_last_restart: Option<Instant>,
    nwd_last_restart: Option<Instant>,
}

impl DaemonSupervisor {
    pub fn new(
        stord_bin: PathBuf,
        nwd_bin: PathBuf,
        stord_socket: PathBuf,
        nwd_socket: PathBuf,
        runtime_dir: PathBuf,
    ) -> Self {
        Self {
            stord_bin,
            nwd_bin,
            stord_socket,
            nwd_socket,
            runtime_dir,
            stord_child: None,
            nwd_child: None,
            stord_last_restart: None,
            nwd_last_restart: None,
        }
    }

    pub async fn start_all(&mut self) -> Result<(), ChvError> {
        self.start_stord().await?;
        self.start_nwd().await?;
        Ok(())
    }

    pub async fn start_stord(&mut self) -> Result<(), ChvError> {
        start_daemon(
            &self.stord_bin,
            &self.stord_socket,
            &self.runtime_dir,
            &mut self.stord_child,
            &mut self.stord_last_restart,
            "chv-stord",
        )
        .await
    }

    pub async fn start_nwd(&mut self) -> Result<(), ChvError> {
        start_daemon(
            &self.nwd_bin,
            &self.nwd_socket,
            &self.runtime_dir,
            &mut self.nwd_child,
            &mut self.nwd_last_restart,
            "chv-nwd",
        )
        .await
    }

    pub async fn health_check(&mut self) -> (bool, bool) {
        let stord_ok = if let Some(ref mut child) = self.stord_child {
            matches!(child.try_wait(), Ok(None))
        } else {
            false
        };
        let nwd_ok = if let Some(ref mut child) = self.nwd_child {
            matches!(child.try_wait(), Ok(None))
        } else {
            false
        };
        (stord_ok, nwd_ok)
    }

    pub async fn restart_if_needed(&mut self) -> Result<(), ChvError> {
        let (stord_ok, nwd_ok) = self.health_check().await;
        if !stord_ok {
            if let Some(mut child) = self.stord_child.take() {
                if let Err(e) = child.kill().await {
                    warn!(error = %e, "failed to kill chv-stord");
                }
                if let Err(e) = child.wait().await {
                    warn!(error = %e, "failed to wait for chv-stord");
                }
            }
            let can_restart = self
                .stord_last_restart
                .map(|t| t.elapsed() >= MIN_RESTART_INTERVAL)
                .unwrap_or(true);
            if can_restart {
                warn!("chv-stord not healthy, restarting");
                if let Err(e) = self.start_stord().await {
                    warn!(error = %e, "failed to restart chv-stord");
                    self.stord_last_restart = Some(Instant::now());
                }
            } else {
                warn!("chv-stord not healthy, restart throttled");
            }
        }
        if !nwd_ok {
            if let Some(mut child) = self.nwd_child.take() {
                if let Err(e) = child.kill().await {
                    warn!(error = %e, "failed to kill chv-nwd");
                }
                if let Err(e) = child.wait().await {
                    warn!(error = %e, "failed to wait for chv-nwd");
                }
            }
            let can_restart = self
                .nwd_last_restart
                .map(|t| t.elapsed() >= MIN_RESTART_INTERVAL)
                .unwrap_or(true);
            if can_restart {
                warn!("chv-nwd not healthy, restarting");
                if let Err(e) = self.start_nwd().await {
                    warn!(error = %e, "failed to restart chv-nwd");
                    self.nwd_last_restart = Some(Instant::now());
                }
            } else {
                warn!("chv-nwd not healthy, restart throttled");
            }
        }
        Ok(())
    }

    pub async fn shutdown(&mut self) {
        if let Some(mut child) = self.stord_child.take() {
            if let Err(e) = child.kill().await {
                debug!(error = %e, "failed to kill chv-stord (may already be dead)");
            }
            if let Err(e) = child.wait().await {
                warn!(error = %e, "failed to wait for chv-stord");
            }
        }
        if let Some(mut child) = self.nwd_child.take() {
            if let Err(e) = child.kill().await {
                debug!(error = %e, "failed to kill chv-nwd (may already be dead)");
            }
            if let Err(e) = child.wait().await {
                warn!(error = %e, "failed to wait for chv-nwd");
            }
        }
    }
}

async fn start_daemon(
    bin: &std::path::Path,
    socket: &std::path::Path,
    runtime_dir: &std::path::Path,
    child: &mut Option<Child>,
    last_restart: &mut Option<Instant>,
    name: &str,
) -> Result<(), ChvError> {
    if child.is_some() {
        return Ok(());
    }
    if let Err(e) = tokio::fs::create_dir_all(runtime_dir).await {
        return Err(ChvError::Io {
            path: runtime_dir.to_string_lossy().to_string(),
            source: e,
        });
    }
    let config_path = runtime_dir.join(format!("{}.toml", name));
    let toml = format!(
        r#"socket_path = {:?}
runtime_dir = {:?}
log_level = "info"
"#,
        socket.to_string_lossy(),
        runtime_dir.to_string_lossy()
    );
    if let Err(e) = tokio::fs::write(&config_path, toml).await {
        return Err(ChvError::Io {
            path: config_path.to_string_lossy().to_string(),
            source: e,
        });
    }
    let mut cmd = Command::new(bin);
    cmd.arg(&config_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    info!(bin = %bin.display(), config = %config_path.display(), "starting {}", name);
    let c = cmd.spawn().map_err(|e| ChvError::Io {
        path: bin.to_string_lossy().to_string(),
        source: e,
    })?;
    *child = Some(c);
    *last_restart = Some(Instant::now());
    Ok(())
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests {
    use super::*;

    #[tokio::test]
    async fn supervisor_start_and_shutdown() {
        // Use sleep as a fake daemon binary
        let mut supervisor = DaemonSupervisor::new(
            PathBuf::from("/bin/sleep"),
            PathBuf::from("/bin/sleep"),
            PathBuf::from("10"),
            PathBuf::from("10"),
            PathBuf::from("/tmp"),
        );
        supervisor.start_stord().await.unwrap();
        supervisor.start_nwd().await.unwrap();
        let (s, n) = supervisor.health_check().await;
        assert!(s);
        assert!(n);
        supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn supervisor_health_check_detects_dead_process() {
        let mut supervisor = DaemonSupervisor::new(
            PathBuf::from("/bin/sleep"),
            PathBuf::from("/bin/sleep"),
            PathBuf::from("0"),
            PathBuf::from("0"),
            PathBuf::from("/tmp"),
        );
        supervisor.start_stord().await.unwrap();
        supervisor.start_nwd().await.unwrap();
        // Give processes time to exit (sleep 0 exits immediately)
        tokio::time::sleep(Duration::from_millis(100)).await;
        let (s, n) = supervisor.health_check().await;
        assert!(!s);
        assert!(!n);
        supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn supervisor_restart_if_needed_restarts_dead_process() {
        let mut supervisor = DaemonSupervisor::new(
            PathBuf::from("/bin/sleep"),
            PathBuf::from("/bin/sleep"),
            PathBuf::from("0"),
            PathBuf::from("0"),
            PathBuf::from("/tmp"),
        );
        supervisor.start_stord().await.unwrap();
        supervisor.start_nwd().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let (s, n) = supervisor.health_check().await;
        assert!(!s);
        assert!(!n);
        // Reset last restart timestamps so throttle allows restart
        supervisor.stord_last_restart = None;
        supervisor.nwd_last_restart = None;
        // Change args so restarted processes stay alive
        supervisor.stord_socket = PathBuf::from("10");
        supervisor.nwd_socket = PathBuf::from("10");
        supervisor.restart_if_needed().await.unwrap();
        let (s2, n2) = supervisor.health_check().await;
        assert!(s2);
        assert!(n2);
        supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn supervisor_restart_throttle_prevents_spam() {
        let mut supervisor = DaemonSupervisor::new(
            PathBuf::from("/bin/sleep"),
            PathBuf::from("/bin/sleep"),
            PathBuf::from("0"),
            PathBuf::from("0"),
            PathBuf::from("/tmp"),
        );
        supervisor.start_stord().await.unwrap();
        supervisor.start_nwd().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        // Reset last restart timestamps so first restart is allowed
        supervisor.stord_last_restart = None;
        supervisor.nwd_last_restart = None;
        // Change args so restarted processes stay alive for the health check
        supervisor.stord_socket = PathBuf::from("10");
        supervisor.nwd_socket = PathBuf::from("10");
        // First restart should succeed
        supervisor.restart_if_needed().await.unwrap();
        assert!(supervisor.stord_last_restart.is_some());
        assert!(supervisor.nwd_last_restart.is_some());
        let (s1, n1) = supervisor.health_check().await;
        assert!(s1);
        assert!(n1);
        // Kill them again immediately
        supervisor.shutdown().await;
        // Ensure throttle is active for the next restart attempt
        supervisor.stord_last_restart = Some(Instant::now());
        supervisor.nwd_last_restart = Some(Instant::now());
        // Now restart_if_needed should be throttled
        let result = supervisor.restart_if_needed().await;
        assert!(result.is_ok());
        // stord should NOT have been restarted (throttled)
        assert!(supervisor.stord_child.is_none());
        // nwd should NOT have been restarted (throttled)
        assert!(supervisor.nwd_child.is_none());
    }
}
