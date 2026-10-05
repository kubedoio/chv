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
    /// Operator's stord.toml, passed through verbatim on respawn (#385).
    /// When `Some`, the supervisor validates the file (readable, parses as
    /// a StordConfig, `socket_path` matches the supervisor's expected
    /// socket) and execs `chv-stord <path>` with it instead of generating
    /// a config — every operator key survives respawn. On any validation
    /// failure: loud warn + today's generated-config path (never worse
    /// than status quo). `None` keeps the generated-config respawn
    /// byte-exactly.
    stord_config_path: Option<PathBuf>,
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
        stord_config_path: Option<PathBuf>,
    ) -> Self {
        Self {
            stord_bin,
            nwd_bin,
            stord_socket,
            nwd_socket,
            runtime_dir,
            stord_path_allowlist,
            stord_config_path,
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
        // #385 pass-through: when the operator pointed the agent at their
        // stord.toml, respawn execs the daemon with that file directly
        // (every operator key — runtime_dir, backend_type,
        // device_allowlist, [migration], and future keys — survives by
        // construction; no config is generated). Only the socket is
        // validated: execing a stord that listens elsewhere would wedge
        // the agent's health check forever, so a socket mismatch — or an
        // unreadable/malformed file — degrades to today's generated
        // config below with a loud warn (never worse than status quo).
        let passthrough_config = self.stord_config_path.as_ref().and_then(|path| {
            match validate_passthrough_stord_config(path, &self.stord_socket) {
                Ok(()) => Some(path.clone()),
                Err(reason) => {
                    warn!(
                        config = %path.display(),
                        reason = %reason,
                        "stord_config_path unusable for respawn; falling back to the supervisor-generated config (operator stord.toml keys will NOT survive this respawn)"
                    );
                    None
                }
            }
        });
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
            passthrough_config.as_deref(),
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
            None,
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

#[allow(clippy::too_many_arguments)]
async fn start_daemon(
    bin: &std::path::Path,
    socket: &std::path::Path,
    runtime_dir: &std::path::Path,
    child: &mut Option<Child>,
    last_restart: &mut Option<Instant>,
    name: &str,
    extra_config: &str,
    config_path: Option<&std::path::Path>,
) -> Result<(), ChvError> {
    if child.is_some() {
        return Ok(());
    }
    if socket.exists() && tokio::net::UnixStream::connect(socket).await.is_ok() {
        info!(socket = %socket.display(), "external {} daemon is already listening on socket; skipping sub-process spawn", name);
        return Ok(());
    }
    // #385 pass-through: an operator-supplied config replaces the
    // generated file wholesale — nothing is written, and the daemon is
    // responsible for its own runtime dir (exactly the contract systemd
    // already relies on with `ExecStart=chv-stord /etc/chv/stord.toml`).
    let config_path = match config_path {
        Some(path) => path.to_path_buf(),
        None => {
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
            config_path
        }
    };
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

/// #385 pass-through validation for the operator's stord.toml: the file
/// must be readable, parse as a `StordConfig`, and listen on exactly the
/// socket the supervisor expects (its health check connects to
/// `AgentConfig.stord_socket` — a respawned daemon bound elsewhere would
/// wedge that check forever). Anything else is a reason string for the
/// fallback warn; the caller then takes today's generated-config path.
fn validate_passthrough_stord_config(
    path: &std::path::Path,
    expected_socket: &std::path::Path,
) -> Result<(), String> {
    match chv_config::load_stord_config(Some(path)) {
        Ok(cfg) => {
            // The comparison is exact, unnormalized `Path` equality:
            // lexically-equivalent spellings (redundant slashes, `.`
            // / `..` components, a relative path) spuriously fall back
            // + warn — safe (the fallback is the generated config,
            // never a wedge), and the mismatch warn below names both
            // paths so the operator can see and fix the spelling.
            if cfg.socket_path != expected_socket {
                Err(format!(
                    "socket_path mismatch: config listens at {}, agent expects {}",
                    cfg.socket_path.display(),
                    expected_socket.display()
                ))
            } else {
                Ok(())
            }
        }
        Err(e) => Err(format!("unreadable or malformed stord config: {e}")),
    }
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
            None,
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
            None,
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
            None,
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
            None,
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
            None,
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
            None,
        );
        // Drop the socket so start_daemon spawns (the fake is alive but
        // 'dummy' sockets never existed, so it always spawns).
        unconfigured.start_stord().await.unwrap();
        let config2 = std::fs::read_to_string(&config_path).unwrap();
        assert!(!config2.contains("path_allowlist"));
        supervisor.shutdown().await;
        unconfigured.shutdown().await;
    }

    // ------------------------------------------------------------------
    // #385: stord respawn config fidelity via pass-through. When the
    // operator points the agent at their stord.toml
    // (`AgentConfig.stord_config_path`), the supervisor execs
    // `chv-stord <operator-path>` directly — no config generated, every
    // operator key (and every future key) survives respawn by
    // construction. Any validation failure degrades to today's
    // generated config with a loud warn.
    // ------------------------------------------------------------------

    /// Minimal WARN-capture subscriber, per the house convention
    /// (chv-nwd-core's fabric tests, chv-controlplane-store's
    /// log_capture, console_server's warn_capture): installed
    /// per-thread with `set_default`, visible to everything the
    /// current-thread `#[tokio::test]` runtime runs on this thread —
    /// including the `warn!` inside `start_stord`.
    mod warn_capture {
        use std::sync::{Arc, Mutex as StdMutex};
        use tracing::field::Visit;
        use tracing::span::{Attributes, Id};
        use tracing::{Event, Metadata, Subscriber};

        #[derive(Clone, Debug)]
        pub struct CapturedWarn {
            pub fields: Vec<(String, String)>,
        }

        impl CapturedWarn {
            pub fn field(&self, name: &str) -> Option<&str> {
                self.fields
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, v)| v.as_str())
            }

            pub fn message(&self) -> &str {
                self.field("message").unwrap_or("")
            }
        }

        #[derive(Clone, Default)]
        pub struct WarnCollector {
            warnings: Arc<StdMutex<Vec<CapturedWarn>>>,
        }

        impl WarnCollector {
            pub fn warnings(&self) -> Vec<CapturedWarn> {
                self.warnings.lock().unwrap().clone()
            }
        }

        struct FieldVisitor(Vec<(String, String)>);

        impl Visit for FieldVisitor {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.0
                    .push((field.name().to_string(), format!("{:?}", value)));
            }

            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                self.0.push((field.name().to_string(), value.to_string()));
            }
        }

        impl Subscriber for WarnCollector {
            fn enabled(&self, metadata: &Metadata<'_>) -> bool {
                *metadata.level() == tracing::Level::WARN
            }

            fn new_span(&self, _span: &Attributes<'_>) -> Id {
                Id::from_u64(1)
            }

            fn record(&self, _span: &Id, _record: &tracing::span::Record<'_>) {}

            fn record_follows_from(&self, _span: &Id, _follows_from: &Id) {}

            fn event(&self, event: &Event<'_>) {
                let mut visitor = FieldVisitor(Vec::new());
                event.record(&mut visitor);
                self.warnings
                    .lock()
                    .unwrap()
                    .push(CapturedWarn { fields: visitor.0 });
            }

            fn enter(&self, _span: &Id) {}

            fn exit(&self, _span: &Id) {}
        }
    }

    /// Operator-shaped stord.toml fixture with non-default keys on every
    /// surface the generated respawn config historically dropped or
    /// mangled: a distinct `runtime_dir` (the recorded relocation trap),
    /// `backend_type` + its LVM section, `device_allowlist`, and a
    /// `[migration]` receiver block.
    fn operator_stord_fixture(root: &std::path::Path, socket: &std::path::Path) -> PathBuf {
        let path = root.join("operator-stord.toml");
        std::fs::write(
            &path,
            format!(
                "socket_path = {}\nruntime_dir = {}\nlog_level = \"debug\"\npath_allowlist = [\"/var/lib/chv/storage\", \"/var/lib/chv/agent\"]\ndevice_allowlist = [\"/dev/dm-*\", \"/dev/mapper/*\"]\nbackend_type = \"lvm\"\nlvm_volume_group = \"chv-vg\"\n\n[migration]\nenabled = true\nlisten_addr = \"127.0.0.1:50052\"\nserver_cert_path = \"/etc/chv/tls/stord-server.crt\"\nserver_key_path = \"/etc/chv/tls/stord-server.key\"\nclient_ca_path = \"/etc/chv/tls/chv-ca.crt\"\n",
                toml_quote(&socket.to_string_lossy()),
                toml_quote(&root.join("operator-stord-runtime").to_string_lossy()),
            ),
        )
        .unwrap();
        path
    }

    /// Bounded wait for the fake daemon to record its argv[1] (the config
    /// path it was exec'd with) — same deadline-poll discipline as
    /// `wait_until_dead`.
    async fn wait_for_recorded_argv(marker: &std::path::Path) -> String {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(contents) = std::fs::read_to_string(marker) {
                return contents.trim().to_string();
            }
            assert!(
                Instant::now() < deadline,
                "fake daemon did not record its argv within 10s"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Read the fake daemon's recorded argv attempts (one per line,
    /// one per exec) — for the crash-loop pin below, where the fake
    /// daemon appends instead of overwriting so retries are countable.
    fn argv_attempt_count(marker: &std::path::Path) -> usize {
        std::fs::read_to_string(marker)
            .map(|c| c.lines().filter(|l| !l.is_empty()).count())
            .unwrap_or(0)
    }

    /// Bounded wait for the fake daemon to have recorded at least `n`
    /// argv attempts; returns the recorded lines — same deadline-poll
    /// discipline as `wait_for_recorded_argv`.
    async fn wait_for_argv_attempts(marker: &std::path::Path, n: usize) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if argv_attempt_count(marker) >= n {
                return std::fs::read_to_string(marker)
                    .unwrap()
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| l.to_string())
                    .collect();
            }
            assert!(
                Instant::now() < deadline,
                "fake daemon did not record {n} argv attempts within 10s"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    // Happy path: a valid operator config is exec'd verbatim — argv[1]
    // is the operator's path, no generated config is written, and no
    // fallback warn fires.
    #[tokio::test]
    async fn supervisor_respawn_passes_operator_stord_config_through() {
        let dir = fake_daemon_dir();
        let root = dir._dir.path().to_path_buf();
        let stord_socket = root.join("stord-api.sock");
        let argv_marker = root.join("stord-argv.txt");
        fake_daemon_script(
            &dir.stord_bin,
            &format!("echo \"$1\" > \"{}\"\nsleep 10", argv_marker.display()),
        )
        .await;
        fake_daemon_script(&dir.nwd_bin, "sleep 10").await;
        let operator_config = operator_stord_fixture(&root, &stord_socket);
        let logs = warn_capture::WarnCollector::default();
        let _subscriber = tracing::subscriber::set_default(logs.clone());

        let mut supervisor = DaemonSupervisor::new(
            dir.stord_bin.clone(),
            dir.nwd_bin.clone(),
            stord_socket,
            PathBuf::from("dummy"),
            dir.runtime_dir.clone(),
            vec![],
            Some(operator_config.clone()),
        );
        supervisor.start_stord().await.unwrap();
        supervisor.start_nwd().await.unwrap();

        // The daemon was exec'd with the operator's config, not a
        // generated one.
        let recorded = wait_for_recorded_argv(&argv_marker).await;
        assert_eq!(
            recorded,
            operator_config.to_string_lossy().to_string(),
            "respawned stord must be exec'd with the operator config path"
        );
        // No config was generated for stord (the nwd config is still
        // generated — nwd is out of scope, #504).
        let generated = dir.runtime_dir.join("chv-stord.toml");
        assert!(
            !generated.exists(),
            "pass-through respawn must not write a generated stord config"
        );
        assert!(supervisor.stord_child.is_some());
        // No fallback warn fired.
        assert!(
            logs.warnings()
                .iter()
                .all(|w| !w.message().contains("stord_config_path unusable")),
            "valid operator config must not trigger the fallback warn"
        );
        supervisor.shutdown().await;
    }

    // Fallback legs: missing file / malformed TOML / socket-path
    // mismatch each produce the loud warn AND today's generated-config
    // behavior — the respawned daemon runs the generated config, never
    // worse than the pre-#385 status quo.
    #[tokio::test]
    async fn supervisor_falls_back_to_generated_config_when_operator_config_unusable() {
        struct Leg {
            name: &'static str,
            reason_needle: &'static str,
            config: Option<String>,
        }
        let legs = [
            // Unset path: the read fails before parsing.
            Leg {
                name: "missing file",
                reason_needle: "unreadable or malformed",
                config: None,
            },
            Leg {
                name: "malformed TOML",
                reason_needle: "unreadable or malformed",
                config: Some("this is not toml {{{".to_string()),
            },
            // A stord listening elsewhere would wedge the agent's health
            // check forever — the mandatory fallback leg.
            Leg {
                name: "socket-path mismatch",
                reason_needle: "socket_path mismatch",
                config: Some(
                    "socket_path = \"/run/elsewhere/stord.sock\"\nruntime_dir = \"/var/lib/chv/storage\"\nlog_level = \"info\"\n"
                        .to_string(),
                ),
            },
        ];

        for leg in legs {
            let dir = fake_daemon_dir();
            let root = dir._dir.path().to_path_buf();
            let stord_socket = root.join("stord-api.sock");
            let argv_marker = root.join("stord-argv.txt");
            fake_daemon_script(
                &dir.stord_bin,
                &format!("echo \"$1\" > \"{}\"\nsleep 10", argv_marker.display()),
            )
            .await;
            fake_daemon_script(&dir.nwd_bin, "sleep 10").await;
            let operator_config = root.join("operator-stord.toml");
            if let Some(contents) = &leg.config {
                std::fs::write(&operator_config, contents).unwrap();
            } // the "missing file" leg never writes it

            let logs = warn_capture::WarnCollector::default();
            let _subscriber = tracing::subscriber::set_default(logs.clone());
            let mut supervisor = DaemonSupervisor::new(
                dir.stord_bin.clone(),
                dir.nwd_bin.clone(),
                stord_socket.clone(),
                PathBuf::from("dummy"),
                dir.runtime_dir.clone(),
                vec![],
                Some(operator_config.clone()),
            );
            supervisor.start_stord().await.unwrap();

            // The loud warn fired with the specific reason.
            let fallback_warns = logs
                .warnings()
                .iter()
                .filter(|w| w.message().contains("stord_config_path unusable"))
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(
                fallback_warns.len(),
                1,
                "leg {:?}: exactly one fallback warn expected",
                leg.name
            );
            let reason = fallback_warns[0].field("reason").unwrap_or("");
            assert!(
                reason.contains(leg.reason_needle),
                "leg {:?}: warn reason {reason:?} must contain {:?}",
                leg.name,
                leg.reason_needle
            );

            // Today's generated-config behavior: the daemon was exec'd
            // with the supervisor-generated config carrying the expected
            // socket, and the file has the historical shape.
            let recorded = wait_for_recorded_argv(&argv_marker).await;
            let generated = dir.runtime_dir.join("chv-stord.toml");
            assert_eq!(
                recorded,
                generated.to_string_lossy().to_string(),
                "leg {:?}: fallback must exec the generated config",
                leg.name
            );
            let config = std::fs::read_to_string(&generated).unwrap();
            assert!(
                config.contains(&format!(
                    "socket_path = {}",
                    toml_quote(&stord_socket.to_string_lossy())
                )),
                "leg {:?}: generated fallback config must carry the expected socket",
                leg.name
            );
            assert!(
                config.contains("log_level = \"info\""),
                "leg {:?}: generated fallback config keeps the historical shape",
                leg.name
            );
            assert!(
                !config.contains("path_allowlist"),
                "leg {:?}: empty allowlist keeps the key omitted (byte-compat pin)",
                leg.name
            );
            supervisor.shutdown().await;
        }
    }

    // #385 residual, pinned: a pass-through config that VALIDATES
    // (parses, socket matches) but fails at daemon startup — e.g. a
    // missing runtime_dir parent (SessionStore::new aborts), a bad
    // backend constructor, or migration TLS material stord checks at
    // startup rather than parse time — crash-loops on the OPERATOR
    // path, never on the generated fallback: the supervisor
    // re-validates the file (still passes) and re-execs it under the
    // restart throttle, while the health check keeps observing the
    // exit (never wedged). Same posture as systemd Restart=on-failure
    // restarting the same broken file; the remedy is fixing the
    // operator config, not waiting for a fallback that never comes.
    #[tokio::test]
    async fn supervisor_passthrough_startup_failure_retries_operator_config_under_throttle() {
        let dir = fake_daemon_dir();
        let root = dir._dir.path().to_path_buf();
        let stord_socket = root.join("stord-api.sock");
        let argv_marker = root.join("stord-argv.txt");
        // A fake stord modeling "validates but unstartable": it appends
        // its argv[1] to the marker (so each retry attempt is
        // countable) and exits immediately — a daemon aborting in its
        // constructor before binding the socket.
        fake_daemon_script(
            &dir.stord_bin,
            &format!("echo \"$1\" >> \"{}\"\nexit 1", argv_marker.display()),
        )
        .await;
        fake_daemon_script(&dir.nwd_bin, "sleep 10").await;
        let operator_config = operator_stord_fixture(&root, &stord_socket);
        let logs = warn_capture::WarnCollector::default();
        let _subscriber = tracing::subscriber::set_default(logs.clone());

        let mut supervisor = DaemonSupervisor::new(
            dir.stord_bin.clone(),
            dir.nwd_bin.clone(),
            stord_socket,
            PathBuf::from("dummy"),
            dir.runtime_dir.clone(),
            vec![],
            Some(operator_config.clone()),
        );
        supervisor.start_stord().await.unwrap();
        supervisor.start_nwd().await.unwrap();

        // First attempt: validation passed, so the operator path.
        let operator_path = operator_config.to_string_lossy().to_string();
        assert_eq!(
            wait_for_argv_attempts(&argv_marker, 1).await,
            vec![operator_path.clone()],
            "the first spawn must exec the operator config"
        );

        // The supervisor is NOT wedged: try_wait keeps observing the
        // exit — the health check reports stord dead while nwd stays
        // alive, so restart_if_needed keeps making progress.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let (stord_ok, nwd_ok) = supervisor.health_check().await;
            if !stord_ok && nwd_ok {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "health check must keep observing the crashed pass-through stord within 10s"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Within the throttle window the retry is suppressed — no new
        // argv attempt (same discipline as
        // supervisor_restart_throttle_prevents_spam).
        supervisor.stord_last_restart = Some(Instant::now());
        supervisor.restart_if_needed().await.unwrap();
        assert_eq!(
            argv_attempt_count(&argv_marker),
            1,
            "a restart attempt inside the throttle window must not re-exec stord"
        );

        // Once the throttle window elapses (reset, as the existing
        // restart tests do), the retry goes to the OPERATOR path again
        // — never to a generated config.
        supervisor.stord_last_restart = None;
        supervisor.restart_if_needed().await.unwrap();
        assert_eq!(
            wait_for_argv_attempts(&argv_marker, 2).await,
            vec![operator_path.clone(), operator_path.clone()],
            "every retry must exec the operator config (no generated fallback)"
        );

        // No generated stord config was ever written...
        let generated = dir.runtime_dir.join("chv-stord.toml");
        assert!(
            !generated.exists(),
            "a startup-failure crash-loop must never write a generated stord config"
        );
        // ...and the health check still functions across the retries.
        let (stord_ok, nwd_ok) = supervisor.health_check().await;
        assert!(
            !stord_ok,
            "the crashed pass-through stord must stay unhealthy"
        );
        assert!(nwd_ok, "nwd must be unaffected by the stord crash-loop");
        // No fallback warn fired: the config validates on every retry —
        // the fallback is validation-scoped, and a startup failure
        // must not be misreported as a validation failure.
        assert!(
            logs.warnings()
                .iter()
                .all(|w| !w.message().contains("stord_config_path unusable")),
            "a validating config must never trigger the fallback warn, even when the daemon crash-loops"
        );
        supervisor.shutdown().await;
    }
}
