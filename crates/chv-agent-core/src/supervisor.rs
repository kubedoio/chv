use chv_errors::ChvError;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tracing::{debug, info, warn};

const MIN_RESTART_INTERVAL: Duration = Duration::from_secs(5);

/// Quote a string for embedding in the generated daemon TOML config.
/// Rust's `{:?}` is NOT a TOML serializer: it emits `\u{1}`-style escapes
/// for control characters, which the `toml` crate rejects — a path
/// containing one would make every respawned daemon die on config parse.
/// `toml::Value`'s Display emits a valid quoted/escaped TOML string.
fn toml_quote(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

pub struct DaemonSupervisor {
    stord_bin: PathBuf,
    nwd_bin: PathBuf,
    pub(crate) stord_socket: PathBuf,
    pub(crate) nwd_socket: PathBuf,
    runtime_dir: PathBuf,
    /// Paths written into the generated chv-stord.toml's `path_allowlist`
    /// when the supervisor respawns stord (#376). Empty keeps the historical
    /// behavior (the key is omitted; stord then treats an absent allowlist
    /// as allow-all) — deployments relying on stord's path confinement must
    /// configure it in the agent config so the respawned daemon keeps the
    /// operator's posture.
    stord_path_allowlist: Vec<PathBuf>,
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
        stord_path_allowlist: Vec<PathBuf>,
    ) -> Self {
        Self {
            stord_bin,
            nwd_bin,
            stord_socket,
            nwd_socket,
            runtime_dir,
            stord_path_allowlist,
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
        // #376: the generated stord config must preserve the operator's
        // path confinement when configured. An empty allowlist omits the
        // key (stord then allows all paths — the documented pre-#376
        // behavior, kept as the default so respawns never break volume
        // locators or seed paths the operator has not allowlisted).
        let stord_extra_config = if self.stord_path_allowlist.is_empty() {
            String::new()
        } else {
            let entries: Vec<String> = self
                .stord_path_allowlist
                .iter()
                .map(|p| toml_quote(&p.to_string_lossy()))
                .collect();
            format!("path_allowlist = [{}]\n", entries.join(", "))
        };
        start_daemon(
            &self.stord_bin,
            &self.stord_socket,
            &self.runtime_dir,
            &mut self.stord_child,
            &mut self.stord_last_restart,
            "chv-stord",
            &stord_extra_config,
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
            "",
        )
        .await
    }

    pub async fn health_check(&mut self) -> (bool, bool) {
        let stord_ok = if let Some(ref mut child) = self.stord_child {
            matches!(child.try_wait(), Ok(None))
        } else {
            self.stord_socket.exists()
                && tokio::net::UnixStream::connect(&self.stord_socket)
                    .await
                    .is_ok()
        };
        let nwd_ok = if let Some(ref mut child) = self.nwd_child {
            matches!(child.try_wait(), Ok(None))
        } else {
            self.nwd_socket.exists()
                && tokio::net::UnixStream::connect(&self.nwd_socket)
                    .await
                    .is_ok()
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
    extra_config: &str,
) -> Result<(), ChvError> {
    if child.is_some() {
        return Ok(());
    }
    if socket.exists() && tokio::net::UnixStream::connect(socket).await.is_ok() {
        info!(socket = %socket.display(), "external {} daemon is already listening on socket; skipping sub-process spawn", name);
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
        "socket_path = {}\nruntime_dir = {}\nlog_level = \"info\"\n{}",
        toml_quote(&socket.to_string_lossy()),
        toml_quote(&runtime_dir.to_string_lossy()),
        extra_config
    );
    if let Err(e) = tokio::fs::write(&config_path, toml).await {
        return Err(ChvError::Io {
            path: config_path.to_string_lossy().to_string(),
            source: e,
        });
    }
    // INHERIT the agent's stdio for supervisor-spawned daemons (M4.4
    // re-qualification lesson): the daemon the supervisor respawns after a
    // crash used to be a black box — its output went to /dev/null, so the
    // restarted nwd's "ensuring topology"/error lines were invisible
    // exactly when they mattered most (a guest-impacting failure was
    // traced through the restarted daemon's missing logs). The agent's own
    // stderr (a file under a service manager, or the deploy's log capture)
    // carries the lines; crate-prefixed tracing lines disambiguate them.
    let mut cmd = Command::new(bin);
    cmd.arg(&config_path)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
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
    use std::os::unix::fs::PermissionsExt;

    async fn fake_daemon_script(path: &std::path::Path, behaviour: &str) {
        let script = format!("#!/bin/sh\n{}\n", behaviour);
        tokio::fs::write(path, script).await.unwrap();
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }

    /// Per-test directory for fake daemon scripts and the supervisor runtime
    /// dir: fixed /tmp paths are shared across concurrent CI jobs on one
    /// host and can make one job's script rewrite visible to another job's
    /// supervisor.
    struct FakeDaemonDir {
        _dir: tempfile::TempDir,
        stord_bin: PathBuf,
        nwd_bin: PathBuf,
        runtime_dir: PathBuf,
    }

    fn fake_daemon_dir() -> FakeDaemonDir {
        let dir = tempfile::tempdir().unwrap();
        FakeDaemonDir {
            stord_bin: dir.path().join("chv-test-stord"),
            nwd_bin: dir.path().join("chv-test-nwd"),
            runtime_dir: dir.path().join("runtime"),
            _dir: dir,
        }
    }

    /// Bounded wait for both supervised fake daemons to be reported dead by
    /// `health_check`. A fixed sleep is only a load heuristic: on a loaded
    /// CI host the fake `exit 0` script can still be alive after it and flip
    /// the asserts (an observed flake), while a deadline-bounded poll is
    /// deterministic.
    async fn wait_until_dead(supervisor: &mut DaemonSupervisor) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let (stord_ok, nwd_ok) = supervisor.health_check().await;
            if !stord_ok && !nwd_ok {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "fake daemons did not exit within 10s"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn supervisor_start_and_shutdown() {
        let dir = fake_daemon_dir();
        fake_daemon_script(&dir.stord_bin, "sleep 10").await;
        fake_daemon_script(&dir.nwd_bin, "sleep 10").await;
        let mut supervisor = DaemonSupervisor::new(
            dir.stord_bin.clone(),
            dir.nwd_bin.clone(),
            PathBuf::from("dummy"),
            PathBuf::from("dummy"),
            dir.runtime_dir.clone(),
            vec![],
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
        let dir = fake_daemon_dir();
        fake_daemon_script(&dir.stord_bin, "exit 0").await;
        fake_daemon_script(&dir.nwd_bin, "exit 0").await;
        let mut supervisor = DaemonSupervisor::new(
            dir.stord_bin.clone(),
            dir.nwd_bin.clone(),
            PathBuf::from("dummy"),
            PathBuf::from("dummy"),
            dir.runtime_dir.clone(),
            vec![],
        );
        supervisor.start_stord().await.unwrap();
        supervisor.start_nwd().await.unwrap();
        wait_until_dead(&mut supervisor).await;
        let (s, n) = supervisor.health_check().await;
        assert!(!s);
        assert!(!n);
        supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn supervisor_restart_if_needed_restarts_dead_process() {
        let dir = fake_daemon_dir();
        fake_daemon_script(&dir.stord_bin, "exit 0").await;
        fake_daemon_script(&dir.nwd_bin, "exit 0").await;
        let mut supervisor = DaemonSupervisor::new(
            dir.stord_bin.clone(),
            dir.nwd_bin.clone(),
            PathBuf::from("dummy"),
            PathBuf::from("dummy"),
            dir.runtime_dir.clone(),
            vec![],
        );
        supervisor.start_stord().await.unwrap();
        supervisor.start_nwd().await.unwrap();
        wait_until_dead(&mut supervisor).await;
        // Reset last restart timestamps so throttle allows restart
        supervisor.stord_last_restart = None;
        supervisor.nwd_last_restart = None;
        // Rewrite scripts so restarted processes stay alive
        fake_daemon_script(&dir.stord_bin, "sleep 10").await;
        fake_daemon_script(&dir.nwd_bin, "sleep 10").await;
        supervisor.restart_if_needed().await.unwrap();
        let (s2, n2) = supervisor.health_check().await;
        assert!(s2);
        assert!(n2);
        supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn supervisor_restart_throttle_prevents_spam() {
        let dir = fake_daemon_dir();
        fake_daemon_script(&dir.stord_bin, "exit 0").await;
        fake_daemon_script(&dir.nwd_bin, "exit 0").await;
        let mut supervisor = DaemonSupervisor::new(
            dir.stord_bin.clone(),
            dir.nwd_bin.clone(),
            PathBuf::from("dummy"),
            PathBuf::from("dummy"),
            dir.runtime_dir.clone(),
            vec![],
        );
        supervisor.start_stord().await.unwrap();
        supervisor.start_nwd().await.unwrap();
        wait_until_dead(&mut supervisor).await;
        // Reset last restart timestamps so first restart is allowed
        supervisor.stord_last_restart = None;
        supervisor.nwd_last_restart = None;
        // Rewrite scripts so restarted processes stay alive for the health check
        fake_daemon_script(&dir.stord_bin, "sleep 10").await;
        fake_daemon_script(&dir.nwd_bin, "sleep 10").await;
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

    // #376: the supervisor-generated chv-stord.toml must preserve the
    // operator's path confinement when configured — a respawned stord used
    // to silently drop path_allowlist (stord then allows all locator
    // paths), so the fresh daemon was confined and the respawned one was
    // not. Empty must keep the historical shape (key omitted).
    #[tokio::test]
    async fn supervisor_generated_stord_config_carries_path_allowlist() {
        let dir = fake_daemon_dir();
        fake_daemon_script(&dir.stord_bin, "sleep 10").await;
        fake_daemon_script(&dir.nwd_bin, "sleep 10").await;
        let allowlist = vec![
            PathBuf::from("/var/lib/chv/storage"),
            PathBuf::from("/var/lib/chv/images"),
        ];
        let mut supervisor = DaemonSupervisor::new(
            dir.stord_bin.clone(),
            dir.nwd_bin.clone(),
            PathBuf::from("dummy"),
            PathBuf::from("dummy"),
            dir.runtime_dir.clone(),
            allowlist.clone(),
        );
        supervisor.start_stord().await.unwrap();
        supervisor.start_nwd().await.unwrap();
        let config_path = dir.runtime_dir.join("chv-stord.toml");
        let config = std::fs::read_to_string(&config_path).unwrap();
        assert!(
            config.contains("path_allowlist = [\"/var/lib/chv/storage\", \"/var/lib/chv/images\"]"),
            "generated stord config must carry the configured allowlist, got:\n{config}"
        );

        // The nwd config must NOT carry the stord allowlist.
        let nwd_config = std::fs::read_to_string(dir.runtime_dir.join("chv-nwd.toml")).unwrap();
        assert!(!nwd_config.contains("path_allowlist"));

        // Empty allowlist keeps the historical shape: key omitted, not an
        // empty list (stord's absent-vs-empty distinction is none today,
        // but the generated file must stay byte-compatible with pre-#376).
        let mut unconfigured = DaemonSupervisor::new(
            dir.stord_bin.clone(),
            dir.nwd_bin.clone(),
            PathBuf::from("dummy"),
            PathBuf::from("dummy"),
            dir.runtime_dir.clone(),
            vec![],
        );
        // Drop the socket so start_daemon spawns (the fake is alive but
        // 'dummy' sockets never existed, so it always spawns).
        unconfigured.start_stord().await.unwrap();
        let config2 = std::fs::read_to_string(&config_path).unwrap();
        assert!(!config2.contains("path_allowlist"));
        supervisor.shutdown().await;
        unconfigured.shutdown().await;
    }
}
