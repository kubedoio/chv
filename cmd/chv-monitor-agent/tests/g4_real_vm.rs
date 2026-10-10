//! G4 gate evidence (part 1): guest collectors, checks and the plugin
//! sandbox on a REAL VM.
//!
//! Env-gated (CI has no KVM; the real-host record lives in
//! `docs/evidence/native-monitoring/g4/README.md`):
//!
//! ```sh
//! CHV_G1_VMM_BINARY=/tmp/opencode/g0b/cloud-hypervisor \
//! CHV_G1_FIRMWARE=/var/lib/chv/qual/hypervisor-fw \
//! CHV_G1_IMAGE=/var/lib/chv/qual/images/noble-qual-patched.img \
//! CHV_G4_AGENT_DEB=dist/packages/chv-monitor-agent_<ver>_amd64.deb \
//! cargo test -p chv-monitor-agent --test g4_real_vm -- --nocapture
//! ```
//!
//! What this proves, end to end on the production paths (real VMM,
//! real firmware boot, real Ubuntu noble guest with cloud-init, real
//! package install, real systemd, real bridge networking, real rustls
//! on both ends):
//!
//! 1. **Real guest telemetry** — filesystem (size/available/inodes/
//!    read-only per mount), network (interface counters, TCP
//!    established), process selectors and their samples land in the
//!    manager's store, bound to the correct VM, with dimensions.
//! 2. **Check inventory reflects real OS behavior** — systemd service
//!    checks follow an actual stop/start of a real unit (ok → critical
//!    → ok); a configured-but-absent unit reports `unknown` / "not
//!    installed" with NO `service.up` sample; a declarative local
//!    HTTP check follows the same real outage; the `check.status` /
//!    `check.duration_seconds` trend series exist and accumulate
//!    points across the transitions.
//! 3. **Filesystem fill and recovery** — a real 200 MB write drops the
//!    reported available bytes for the root mount; deleting it
//!    recovers them; the history series shows the excursion.
//! 4. **Plugins disabled by default** — the plugin files are installed
//!    root-owned in the guest from the start, but with
//!    `[plugins] enabled = false` (the default) NO `plugin:` check
//!    ever appears. Only after an explicit local config change and
//!    service restart does the pinned plugin's check appear and
//!    report ok.
//! 5. **Plugins constrained when enabled** — replacing the plugin
//!    executable after manifest pinning degrades the check (unknown /
//!    stale, never ok) and the rogue executable is NEVER run (its
//!    side-effect marker never appears); the agent keeps reporting
//!    everything else; the allowlist directory stays root-owned and
//!    the manifest file byte-identical.
//! 6. **No code from the manager** — the manager actively ingests
//!    throughout; there is no plugin channel in the ingest protocol
//!    (samples and check records only), and the guest's plugin
//!    allowlist directory is untouched by anything the manager sent.
//!
//! What this deliberately does NOT claim: hardware attestation or
//! VM-image identity (ADR-026's recorded trust limitation), and
//! nothing about alerting (PR-6) or vsock transport (PR-7).

use chv_controlplane_service::api::tls::{build_https_config, serve_tls};
use chv_controlplane_service::monitoring_agent::{
    AgentCertificateIssuer, GuestIngestionLimits, MonitoringAgentService,
};
use chv_controlplane_store::test_util::TestDb;
use chv_controlplane_store::{EventRepository, MonitoringAgentRepository, MonitoringAgentRow};
use chv_monitoring_core::model::{CheckStatus, SampleQuality, TargetKind};
use chv_monitoring_store::{
    CurrentSample, MonitoringHealth, MonitoringStore, MonitoringStoreConfig, Resolution,
    StoredCheck,
};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Short interface names (IFNAMSIZ limit); unique to this rig so a
/// crashed run is visible and cleanable (`ip link del`). Distinct
/// subnet from the g3 rig so both can run on the same host.
const BRIDGE: &str = "br-g4ev";
const TAP: &str = "tap-g4ev";
const BRIDGE_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 63, 1);
const GUEST_IP: &str = "192.168.63.50";
const GUEST_MAC: &str = "52:54:00:63:00:50";
const VM_ID: &str = "g4-vm";
/// The in-guest loopback HTTP endpoint the local checks and the rig's
/// plugin probe (python3 http.server, bound to 127.0.0.1 only).
const GUEST_HTTP_PORT: u16 = 8080;
/// The rig plugin's check id (the `plugin:` namespace is part of the
/// check-id contract).
const PLUGIN_CHECK_ID: &str = "plugin:g4-http-health";
/// Where the guest's plugin allowlist lives (the documented install
/// location).
const GUEST_PLUGIN_DIR: &str = "/etc/chv-monitor/plugins.d";

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn checkpoint(msg: &str) {
    let _ = std::io::Write::write_fmt(
        &mut std::io::stderr(),
        format_args!("g4 checkpoint: {msg}\n"),
    );
}

fn run_ip(args: &[&str], fatal: bool) -> bool {
    let out = std::process::Command::new("ip").args(args).output();
    match out {
        Ok(o) if o.status.success() => true,
        other => {
            if fatal {
                panic!("`ip {}` failed: {:?}", args.join(" "), other);
            }
            false
        }
    }
}

/// Bridge + tap on the host (test-side topology; production nwd owns
/// this in a real deployment). The manager listener binds the bridge
/// IP; the guest reaches it via cloud-init's static IP.
fn setup_network() {
    run_ip(&["link", "del", TAP], false);
    run_ip(&["link", "del", BRIDGE], false);
    run_ip(&["link", "add", BRIDGE, "type", "bridge"], true);
    run_ip(&["addr", "add", "192.168.63.1/24", "dev", BRIDGE], true);
    run_ip(&["link", "set", BRIDGE, "up"], true);
    run_ip(&["tuntap", "add", "dev", TAP, "mode", "tap"], true);
    run_ip(&["link", "set", TAP, "master", BRIDGE], true);
    run_ip(&["link", "set", TAP, "up"], true);
}

fn teardown_network() {
    run_ip(&["link", "del", TAP], false);
    run_ip(&["link", "del", BRIDGE], false);
}

/// Synchronous best-effort cleanup for host resources a failed run
/// must not leak (the VMM via SIGKILL, the network topology). Owns
/// the VM's tempdir so diagnostics survive unwinding.
struct HostCleanup {
    _dir: Option<tempfile::TempDir>,
    vm_dir: Option<PathBuf>,
}

impl HostCleanup {
    fn new() -> Self {
        Self {
            _dir: None,
            vm_dir: None,
        }
    }

    fn keep_dir(&mut self, dir: tempfile::TempDir) {
        self._dir = Some(dir);
    }
}

impl Drop for HostCleanup {
    fn drop(&mut self) {
        if let Some(vm_dir) = self.vm_dir.take() {
            // SIGKILL the VMM if it is still up (a graceful stop is
            // done inline on the success path; this is the failure
            // path). Stale ch.pid content is handled by kill itself.
            if let Ok(pid) = std::fs::read_to_string(vm_dir.join("ch.pid")) {
                let pid = pid.trim();
                if !pid.is_empty() {
                    let _ = std::process::Command::new("kill")
                        .arg("-9")
                        .arg(pid)
                        .output();
                }
            }
            dump_diagnostics(&vm_dir);
        }
        teardown_network();
    }
}

/// Last ~40 lines of the guest console (progress visibility during
/// the enrollment wait).
fn dump_console_tail(vm_dir: &Path) {
    match std::fs::read_to_string(vm_dir.join("console.log")) {
        Ok(text) => {
            let lines: Vec<&str> = text.lines().collect();
            let start = lines.len().saturating_sub(40);
            let _ = std::io::Write::write_fmt(
                &mut std::io::stderr(),
                format_args!("{}\n", lines[start..].join("\n")),
            );
        }
        Err(e) => {
            let _ = std::io::Write::write_fmt(
                &mut std::io::stderr(),
                format_args!("g4 console.log: unreadable ({e})\n"),
            );
        }
    }
}

/// Last ~120 lines of the guest console and the VMM stderr log — the
/// difference between "cloud-init never ran" and "dpkg failed" when a
/// checkpoint assertion fires.
fn dump_diagnostics(vm_dir: &Path) {
    for (label, name) in [
        ("console.log", "console.log"),
        ("vmm stderr", "cloud-hypervisor.stderr.log"),
    ] {
        match std::fs::read_to_string(vm_dir.join(name)) {
            Ok(text) => {
                let lines: Vec<&str> = text.lines().collect();
                let start = lines.len().saturating_sub(120);
                let _ = std::io::Write::write_fmt(
                    &mut std::io::stderr(),
                    format_args!("g4 {label} (tail):\n{}\n", lines[start..].join("\n")),
                );
            }
            Err(e) => {
                let _ = std::io::Write::write_fmt(
                    &mut std::io::stderr(),
                    format_args!("g4 {label}: unreadable ({e})\n"),
                );
            }
        }
    }
}

/// A self-signed TLS server certificate for the bridge IP (the agent
/// trusts exactly this PEM as its manager CA).
fn bridge_server_cert() -> (String, String) {
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::default();
    params.subject_alt_names = vec![rcgen::SanType::IpAddress(std::net::IpAddr::V4(BRIDGE_IP))];
    let cert = params.self_signed(&key).unwrap();
    (cert.pem(), key.serialize_pem())
}

/// A throwaway self-signed agent CA (signs the enrolled client cert).
fn test_ca() -> (String, String) {
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::default();
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "g4-evidence-agent-ca");
    let cert = params.self_signed(&key).unwrap();
    (cert.pem(), key.serialize_pem())
}

/// The in-process manager: operational DB, monitoring store, agent
/// CA, service and a live TLS listener bound to the BRIDGE IP (the
/// guest's only route to a manager).
struct Manager {
    _ops: TestDb,
    _monitoring_dir: tempfile::TempDir,
    store: Arc<MonitoringStore>,
    repo: MonitoringAgentRepository,
    service: Arc<MonitoringAgentService>,
    server_cert_pem: String,
    port: u16,
    server: Option<tokio::task::JoinHandle<()>>,
    /// Held for the manager's lifetime: dropping it would signal the
    /// listener's graceful shutdown.
    _shutdown_tx: tokio::sync::watch::Sender<()>,
}

impl Manager {
    async fn new() -> Self {
        let ops = TestDb::new().await;
        let monitoring_dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            MonitoringStore::connect(MonitoringStoreConfig {
                database_url: format!("sqlite://{}/monitoring.db", monitoring_dir.path().display()),
                migrations_dir: PathBuf::from(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../cmd/chv-controlplane/monitoring-migrations"
                )),
                ..MonitoringStoreConfig::default()
            })
            .await
            .expect("monitoring store connect"),
        );

        sqlx::query("INSERT INTO vms (vm_id, display_name) VALUES ($1, $1)")
            .bind(VM_ID)
            .execute(&ops.pool)
            .await
            .unwrap();

        let (agent_ca_pem, agent_ca_key) = test_ca();
        let issuer = Arc::new(AgentCertificateIssuer::new(&agent_ca_pem, &agent_ca_key).unwrap());
        let repo = MonitoringAgentRepository::new(ops.pool.clone());
        // The claim is embedded in the seed at VM-creation time; the
        // TTL clock starts at issuance. A full firmware boot +
        // cloud-init + dpkg takes well under the default 10 minutes,
        // but this evidence run must not flake on a loaded host.
        let limits = GuestIngestionLimits {
            claim_ttl_ms: 30 * 60_000,
            ..GuestIngestionLimits::default()
        };
        let service = Arc::new(MonitoringAgentService::new(
            repo.clone(),
            issuer,
            Some(store.clone()),
            MonitoringHealth::new(),
            EventRepository::new(ops.pool.clone()),
            limits,
            None,
        ));

        let (server_cert_pem, server_key_pem) = bridge_server_cert();
        let https =
            build_https_config(&server_cert_pem, &server_key_pem, Some(&agent_ca_pem)).unwrap();

        let router = chv_controlplane_service::api::agent_routes::agent_routes(axum::Router::new())
            .layer(axum::Extension(service.clone()));
        let listener = tokio::net::TcpListener::bind((BRIDGE_IP, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());
        let server = tokio::spawn(async move {
            let _ = serve_tls(listener, https, router, shutdown_rx).await;
        });

        Self {
            _ops: ops,
            _monitoring_dir: monitoring_dir,
            store,
            repo,
            service,
            server_cert_pem,
            port,
            server: Some(server),
            _shutdown_tx: shutdown_tx,
        }
    }

    fn base_url(&self) -> String {
        format!("https://{BRIDGE_IP}:{}", self.port)
    }

    async fn active_agent(&self) -> Option<MonitoringAgentRow> {
        self.repo.find_active_agent_by_vm(VM_ID).await.unwrap()
    }

    /// The check inventory for this VM (the BFF endpoint's backing
    /// query).
    async fn checks(&self) -> Vec<StoredCheck> {
        self.store
            .query_checks(&TargetKind::Vm, VM_ID, now_ms().unsigned_abs())
            .await
            .unwrap()
    }

    async fn find_check(&self, check_id: &str) -> Option<StoredCheck> {
        self.checks()
            .await
            .into_iter()
            .find(|c| c.check_id == check_id)
    }

    async fn guest_points(&self, metric_id: &str) -> usize {
        let now = now_ms() as u64;
        self.store
            .query_history(
                &TargetKind::Vm,
                VM_ID,
                &[metric_id.to_string()],
                None,
                now.saturating_sub(3_600_000),
                now + 3_600_000,
                1000,
                Resolution::Raw,
            )
            .await
            .unwrap()
            .iter()
            .map(|s| {
                s.points
                    .iter()
                    .filter(|p| p.quality == SampleQuality::Valid)
                    .count()
            })
            .sum()
    }

    /// All current samples for one metric (dimensioned series come
    /// back one sample per dimension set).
    async fn current(&self, metric_id: &str) -> Vec<CurrentSample> {
        self.store
            .query_current(
                &TargetKind::Vm,
                VM_ID,
                &[metric_id.to_string()],
                None,
                now_ms() as u64,
            )
            .await
            .unwrap()
    }

    /// The current sample of one dimensioned series — `None` when the
    /// series has no current sample (honest absence, never 0).
    async fn current_dimensioned(
        &self,
        metric_id: &str,
        dim_key: &str,
        dim_value: &str,
    ) -> Option<CurrentSample> {
        self.current(metric_id)
            .await
            .into_iter()
            .find(|s| s.dimensions.get(dim_key).map(|v| v.as_str()) == Some(dim_value))
    }

    /// The current sample of the ROOT mount's series (mount_id is
    /// `<fstype>:<mountpoint>`, so the root mount ends in `:/`).
    async fn current_root_mount(&self, metric_id: &str) -> Option<CurrentSample> {
        self.current(metric_id).await.into_iter().find(|s| {
            s.dimensions
                .get("mount_id")
                .map(|v| v.ends_with(":/"))
                .unwrap_or(false)
        })
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        if let Some(server) = self.server.take() {
            server.abort();
        }
    }
}

/// The guest agent's G4 config: every family on, processes with
/// selectors, three configured services (one deliberately absent on
/// the guest — the not-installed path), discovery on, one local HTTP
/// check and one local TCP check, and plugins DISABLED — the
/// default-off phase needs the plugin files present in the guest but
/// never executed. 5 s interval so the phases fit in evidence time;
/// the fs/services/checks families still ride their 60 s cadence.
fn guest_agent_toml(manager: &Manager) -> String {
    format!(
        r#"server_url = "{url}"
manager_ca_path = "/etc/chv-monitor/manager-ca.pem"
claim_path = "/var/lib/chv-monitor/claim"
credential_path = "/var/lib/chv-monitor/credential.json"
state_dir = "/var/lib/chv-monitor"
spool_dir = "/var/lib/chv-monitor/spool"
interval_seconds = 5
max_spool_batches = 500
log_level = "info"

[collectors]
filesystems = true
network = true
services = true
processes = true
process_selectors = ["python3", "systemd"]

[services]
configured = ["ssh.service", "g4-http.service", "g4-absent.service"]
discover = true

[[checks.http]]
label = "app"
url = "http://127.0.0.1:{port}/"

[[checks.tcp]]
label = "ssh"
host = "127.0.0.1"
port = 22

[[checks.tcp]]
label = "closed"
host = "127.0.0.1"
port = 59999

[plugins]
enabled = false
directory = "{plugin_dir}"
"#,
        url = manager.base_url(),
        port = GUEST_HTTP_PORT,
        plugin_dir = GUEST_PLUGIN_DIR
    )
}

/// The rig's in-guest plugin: a variant of
/// `docs/examples/plugins/http-health.py` probing this rig's loopback
/// endpoint. Installed root-owned into the allowlist directory, its
/// digest pinned by the manifest written alongside it — exactly the
/// install discipline `docs/examples/plugins/README.md` documents.
const GUEST_PLUGIN: &str = r#"#!/usr/bin/env python3
import json, sys, time
from urllib.error import HTTPError
from urllib.request import urlopen

URL = "http://127.0.0.1:8080/"

started = time.monotonic()
status, summary = "ok", "Endpoint responded"
try:
    with urlopen(URL, timeout=3) as response:
        if response.status != 200:
            status, summary = "critical", "HTTP status {}".format(response.status)
except HTTPError as exc:
    status, summary = "critical", "HTTP status {}".format(exc.code)
except Exception as exc:
    status, summary = "critical", type(exc).__name__
print(json.dumps({
    "schema_version": 1,
    "check_id": "plugin:g4-http-health",
    "status": status,
    "summary": summary[:200],
    "metrics": [],
}))
sys.exit(0)
"#;

/// The rogue replacement for the tamper phase: if the sandbox ever
/// executed it, it would leave `/tmp/g4-rogue-ran` behind AND report
/// ok. The digest pin must prevent both — the assertion is that
/// neither happens.
const ROGUE_PLUGIN: &str = "#!/bin/sh\ntouch /tmp/g4-rogue-ran\nprintf '%s' '{\"schema_version\":1,\"check_id\":\"plugin:g4-http-health\",\"status\":\"ok\",\"summary\":\"rogue says ok\",\"metrics\":[]}'\n";

/// The manifest for the rig plugin, digest pinned to `executable`
/// bytes. The sha256 is computed host-side (sha256sum) from the exact
/// bytes written into the seed — the same file the guest installs.
fn plugin_manifest(plugin_path: &Path) -> String {
    let sha = sha256_of(plugin_path);
    format!(
        r#"{{"schema_version":1,"plugin_id":"g4-http-health","plugin_version":"1.0.0","executable":"{dir}/g4-http-health.py","sha256":"{sha}","checks":["{check}"],"interval_seconds":60,"timeout_seconds":5,"max_output_bytes":32768,"privilege_profile":"unprivileged"}}"#,
        dir = GUEST_PLUGIN_DIR,
        sha = sha,
        check = PLUGIN_CHECK_ID
    )
}

/// Host-side sha256 (the rig deliberately adds no crypto dependency
/// to the agent's dev-deps; the evidence host has coreutils).
fn sha256_of(path: &Path) -> String {
    let out = std::process::Command::new("sha256sum")
        .arg(path)
        .output()
        .expect("run sha256sum");
    assert!(
        out.status.success(),
        "sha256sum failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .expect("sha256sum output")
        .to_string()
}

/// Host-side base64 (shell-safe transfer of the rogue script into the
/// guest through the ssh command line).
fn base64_of(data: &str) -> String {
    use std::io::Write;
    let mut child = std::process::Command::new("base64")
        .arg("-w0")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn base64");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(data.as_bytes())
        .expect("pipe the rogue script to base64");
    let out = child.wait_with_output().expect("read base64 output");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// cloud-config executed by cloud-init inside the guest. Everything
/// comes from the NoCloud seed (no network fetch needed for the
/// install): the deb, the config, the manager CA, the one-time claim
/// — plus this rig's fixtures: a real systemd unit serving the
/// loopback HTTP endpoint the checks and the plugin probe, the plugin
/// files installed ROOT-OWNED into the allowlist directory exactly
/// the way the plugin README documents, and the rig's ssh key so the
/// scenario phases can act inside the guest. Order matters — dpkg
/// first (creates the chv-monitor user and the conffile), then
/// overwrite the conffile with the real config, then the fixtures,
/// then the services.
fn guest_userdata(ssh_pubkey: &str) -> String {
    format!(
        r#"#cloud-config
ssh_authorized_keys:
  - {ssh_pubkey}
write_files:
  - path: /etc/systemd/system/g4-http.service
    content: |
      [Unit]
      Description=G4 evidence loopback HTTP endpoint
      After=network.target
      [Service]
      ExecStart=/usr/bin/python3 -m http.server {port} --bind 127.0.0.1
      Restart=always
      [Install]
      WantedBy=multi-user.target
runcmd:
  - mkdir -p /media/cidata
  - mount -o ro "$(findfs LABEL=cidata)" /media/cidata
  - dpkg -i /media/cidata/chv-monitor-agent.deb
  - install -m 0644 /media/cidata/agent.toml /etc/chv-monitor/agent.toml
  - install -m 0644 /media/cidata/manager-ca.pem /etc/chv-monitor/manager-ca.pem
  - install -o chv-monitor -g chv-monitor -m 0600 /media/cidata/claim /var/lib/chv-monitor/claim
  - systemctl enable --now g4-http.service
  - mkdir -p {plugin_dir}
  - install -o root -g root -m 0755 /media/cidata/g4-plugin.py {plugin_dir}/g4-http-health.py
  - install -o root -g root -m 0644 /media/cidata/g4-plugin.json {plugin_dir}/g4-http-health.json
  - systemctl enable --now chv-monitor-agent
"#,
        port = GUEST_HTTP_PORT,
        plugin_dir = GUEST_PLUGIN_DIR
    )
}

/// Enrich the production-built NoCloud seed with the agent package,
/// its inputs and the rig fixtures (the production adapter writes
/// user-data from `cloud_init_userdata` — which already carries the
/// ssh key and the g4-http unit — so this only adds seed files).
async fn enrich_seed(vm_dir: &Path, manager: &Manager, deb: &Path, claim_token: &str) {
    let seed_dir = vm_dir.join("seed");
    std::fs::copy(deb, seed_dir.join("chv-monitor-agent.deb"))
        .expect("copy the agent deb into the seed");
    std::fs::write(seed_dir.join("agent.toml"), guest_agent_toml(manager))
        .expect("write the guest agent config into the seed");
    std::fs::write(seed_dir.join("manager-ca.pem"), &manager.server_cert_pem)
        .expect("write the manager CA into the seed");
    std::fs::write(seed_dir.join("claim"), claim_token).expect("write the claim into the seed");
    std::fs::write(
        seed_dir.join("g4-plugin.py"),
        GUEST_PLUGIN.trim_start().to_string() + "\n",
    )
    .expect("write the rig plugin into the seed");
    std::fs::write(
        seed_dir.join("g4-plugin.json"),
        plugin_manifest(&seed_dir.join("g4-plugin.py")),
    )
    .expect("write the rig plugin manifest into the seed");

    let iso = vm_dir.join("seed.iso");
    let iso_new = vm_dir.join("seed.iso.new");
    // PATH first, then the canonical install location — the same
    // resolution the production seed builder performs.
    let command = |binary: &str| {
        let mut command = tokio::process::Command::new(binary);
        command
            .arg("-output")
            .arg(&iso_new)
            .arg("-volid")
            .arg("cidata")
            .arg("-joliet")
            .arg("-rock")
            .arg(seed_dir.join("user-data"))
            .arg(seed_dir.join("meta-data"))
            .arg(seed_dir.join("network-config"))
            .arg(seed_dir.join("chv-monitor-agent.deb"))
            .arg(seed_dir.join("agent.toml"))
            .arg(seed_dir.join("manager-ca.pem"))
            .arg(seed_dir.join("claim"))
            .arg(seed_dir.join("g4-plugin.py"))
            .arg(seed_dir.join("g4-plugin.json"));
        command
    };
    let output = match command("genisoimage").output().await {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => command("/usr/bin/genisoimage")
            .output()
            .await
            .expect("run genisoimage for the enriched seed"),
        Err(e) => panic!("failed to run genisoimage: {e}"),
    };
    assert!(
        output.status.success(),
        "genisoimage failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::rename(&iso_new, &iso).expect("atomically replace the seed ISO");
    checkpoint("seed enriched with the agent package, fixtures and ssh key");
}

/// Run a command in the guest over ssh (the rig's scenario trigger —
/// real systemctl/dd/tamper actions inside the real guest). The noble
/// cloud image's default user has passwordless sudo.
fn ssh(key_path: &Path, command: &str) -> String {
    let out = std::process::Command::new("ssh")
        .arg("-i")
        .arg(key_path)
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-o")
        .arg("UserKnownHostsFile=/dev/null")
        .arg("-o")
        .arg("ConnectTimeout=10")
        .arg(format!("ubuntu@{GUEST_IP}"))
        .arg("--")
        .arg(command)
        .output()
        .unwrap_or_else(|e| panic!("ssh to the guest failed: {e}"));
    assert!(
        out.status.success(),
        "guest command {command:?} failed: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A fresh ed25519 keypair for the rig's guest access.
fn rig_ssh_key(dir: &Path) -> (PathBuf, String) {
    let key = dir.join("rig-ed25519");
    let st = std::process::Command::new("ssh-keygen")
        .arg("-t")
        .arg("ed25519")
        .arg("-N")
        .arg("")
        .arg("-f")
        .arg(&key)
        .output()
        .expect("run ssh-keygen for the rig key");
    assert!(
        st.status.success(),
        "ssh-keygen failed: {}",
        String::from_utf8_lossy(&st.stderr)
    );
    let pubkey = std::fs::read_to_string(key.with_extension("pub")).expect("read the rig pubkey");
    (key, pubkey.trim().to_string())
}

/// Poll `probe` until it returns Some, or fail with `what` after
/// `budget`. 2 s poll interval — the guest phases take tens of
/// seconds, not sub-second. The closure returns a fresh future each
/// call (async blocks borrowing the shared fixtures, never moving
/// `&mut` captures — the DrainSummary lesson).
async fn wait_until<T, F>(what: &str, budget: Duration, mut probe: impl FnMut() -> F) -> T
where
    F: std::future::Future<Output = Option<T>>,
{
    let deadline = Instant::now() + budget;
    loop {
        if let Some(value) = probe().await {
            return value;
        }
        if Instant::now() >= deadline {
            panic!("g4: timed out waiting for {what} (budget {budget:?})");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// The VMM process's accumulated CPU time (utime+stime, clock
/// ticks). A guest that is genuinely executing firmware/kernel code
/// grows this immediately; a zombie "Running" VM does not.
fn vmm_cpu_ticks(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = stat.rsplit_once(')')?.1;
    let fields: Vec<&str> = after.split_whitespace().collect();
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    Some(utime + stime)
}

fn is_root_with_ip() -> bool {
    let has_ip = std::process::Command::new("ip")
        .arg("link")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let is_root = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .map(|uid| uid == "0")
        })
        .unwrap_or(false);
    has_ip && is_root
}

#[tokio::test]
async fn g4_real_vm_collectors_checks_and_plugin_constraints() {
    let Ok(vmm_binary) = std::env::var("CHV_G1_VMM_BINARY") else {
        eprintln!("skipping: CHV_G1_VMM_BINARY not set (real-KVM evidence test)");
        return;
    };
    let firmware =
        std::env::var("CHV_G1_FIRMWARE").expect("CHV_G1_FIRMWARE with CHV_G1_VMM_BINARY");
    let image = std::env::var("CHV_G1_IMAGE").expect("CHV_G1_IMAGE with CHV_G1_VMM_BINARY");
    let Ok(agent_deb) = std::env::var("CHV_G4_AGENT_DEB") else {
        eprintln!("skipping: CHV_G4_AGENT_DEB not set (build the guest package first)");
        return;
    };
    let agent_deb = PathBuf::from(agent_deb);
    assert!(
        agent_deb.is_file(),
        "CHV_G4_AGENT_DEB={:?} is not a file",
        agent_deb
    );
    if !is_root_with_ip() {
        eprintln!("skipping: bridge setup needs root + iproute2 (test-side topology)");
        return;
    }

    let mut cleanup = HostCleanup::new();
    setup_network();
    checkpoint("network up (bridge + tap)");

    let manager = Manager::new().await;
    checkpoint(format!("manager listening on {}", manager.base_url()).as_str());

    // The claim is issued BEFORE the VM exists: it is embedded in the
    // seed, so its TTL covers boot + cloud-init + install.
    let claim = manager
        .service
        .issue_claim(VM_ID, "g4-evidence", now_ms())
        .await
        .expect("issue the one-time claim");

    // The guest agent install WRITES to the guest rootfs (dpkg,
    // systemd enable, the rig's fixtures) — but the pinned
    // qualification image is never written (G0b/G1/G2/G3 discipline).
    // A full sparse copy gives the VMM a writable boot disk.
    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let (ssh_key, ssh_pubkey) = rig_ssh_key(dir.path());
    let root_disk = dir.path().join("g4-root.qcow2");
    let cp = std::process::Command::new("cp")
        .arg("--sparse=always")
        .arg(&image)
        .arg(&root_disk)
        .status()
        .expect("run cp for the writable root copy");
    assert!(cp.success(), "cp of the qualified image failed");
    let adapter =
        chv_agent_runtime_ch::ProcessCloudHypervisorAdapter::new(PathBuf::from(&vmm_binary));
    let runtime = chv_agent_core::vm_runtime::VmRuntime::new(Arc::new(adapter));

    let vm_dir = dir.path().join("vms").join(VM_ID);
    cleanup.vm_dir = Some(vm_dir.clone());
    // The cleanup guard owns the tempdir: on a failed assert it dumps
    // the console/VMM logs BEFORE the directory is deleted (a local
    // TempDir would unwind first and take the evidence with it).
    cleanup.keep_dir(dir);

    // The qualification shape (G0b/G1/G2/G3): 2 vCPU, 512 MiB,
    // firmware boot — plus this rig's writable root copy, the NIC
    // (the production seed builder turns it into cloud-init static
    // network-config) and the rig's cloud-config userdata (ssh key +
    // the loopback HTTP unit).
    let config = chv_hypervisor_api::VmConfig {
        vm_id: VM_ID.to_string(),
        cpus: 2,
        memory_bytes: 512 * 1024 * 1024,
        kernel_path: PathBuf::from("/dev/null"),
        firmware_path: Some(PathBuf::from(&firmware)),
        disks: vec![chv_hypervisor_api::VmDiskConfig {
            path: root_disk,
            read_only: false,
            id: None,
        }],
        nics: vec![chv_hypervisor_api::VmNicConfig {
            network_id: "g4-evidence".to_string(),
            mac_address: GUEST_MAC.to_string(),
            ip_address: GUEST_IP.to_string(),
            tap_name: TAP.to_string(),
            cidr: "192.168.63.0/24".to_string(),
            gateway: BRIDGE_IP.to_string(),
        }],
        api_socket_path: vm_dir.join("vm.sock"),
        cloud_init_userdata: Some(guest_userdata(&ssh_pubkey)),
        hypervisor_overrides: None,
    };

    checkpoint("creating vm (production adapter, seed built)");
    runtime
        .create_vm(VM_ID, "g4-1", &config, None)
        .await
        .expect("production create_vm builds the cloud-init seed");
    enrich_seed(&vm_dir, &manager, &agent_deb, &claim.token).await;
    runtime
        .start_vm(VM_ID, None)
        .await
        .expect("production start_vm boots the enriched seed");

    {
        let pid: u32 = std::fs::read_to_string(vm_dir.join("ch.pid"))
            .expect("the adapter persists the VMM pid")
            .trim()
            .parse()
            .expect("ch.pid content");
        let boot_ticks = wait_until(
            "the guest to actually execute (VMM CPU time advancing)",
            Duration::from_secs(60),
            || async {
                let a = vmm_cpu_ticks(pid).unwrap_or(0);
                tokio::time::sleep(Duration::from_millis(500)).await;
                let b = vmm_cpu_ticks(pid).unwrap_or(0);
                (b > a + 2).then_some(b - a)
            },
        )
        .await;
        checkpoint(format!("guest executing (vmm cpu ticks +{boot_ticks})").as_str());
    }

    // Phase 1 — enrollment (the identical production path to G3).
    let mut poll_count: u32 = 0;
    let agent = wait_until(
        "the guest agent to enroll (cloud-init + dpkg + systemd + mTLS)",
        Duration::from_secs(420),
        || {
            poll_count += 1;
            if poll_count.is_multiple_of(30) {
                checkpoint(&format!(
                    "still waiting for enrollment (poll {poll_count}); guest console tail:"
                ));
                dump_console_tail(&vm_dir);
            }
            async { manager.active_agent().await }
        },
    )
    .await;
    checkpoint("agent enrolled");
    assert_eq!(agent.vm_id, VM_ID);

    // The rig's in-guest access: cloud-init has placed the key by the
    // time enrollment completed.
    wait_until(
        "ssh access to the guest",
        Duration::from_secs(120),
        || async {
            std::process::Command::new("ssh")
                .arg("-i")
                .arg(&ssh_key)
                .arg("-o")
                .arg("StrictHostKeyChecking=no")
                .arg("-o")
                .arg("UserKnownHostsFile=/dev/null")
                .arg("-o")
                .arg("ConnectTimeout=5")
                .arg(format!("ubuntu@{GUEST_IP}"))
                .arg("--")
                .arg("true")
                .output()
                .map(|o| o.status.success().then_some(()))
                .unwrap_or(None)
        },
    )
    .await;
    checkpoint("ssh reachable");
    let plugin_ls = ssh(&ssh_key, &format!("ls -l {GUEST_PLUGIN_DIR}/"));
    assert!(
        plugin_ls.contains("root root"),
        "plugin files are installed root-owned in the guest: {plugin_ls}"
    );
    let manifest_digest_in_guest = ssh(
        &ssh_key,
        &format!("sha256sum {GUEST_PLUGIN_DIR}/g4-http-health.json"),
    );

    // Phase 2 — real guest telemetry lands with dimensions.
    for metric in [
        "vm.guest.fs.available_bytes",
        "vm.guest.fs.total_bytes",
        "vm.guest.fs.inodes_utilization_ratio",
        "vm.guest.fs.read_only",
        "vm.guest.net.rx_bytes_total",
        "vm.guest.net.tx_bytes_total",
        "vm.guest.net.rx_errors_total",
        "vm.guest.net.tcp_established",
        "vm.guest.process.count",
        "vm.guest.process.rss_bytes",
        "vm.guest.process.cpu_utilization_ratio",
    ] {
        let points = wait_until(
            &format!("valid {metric} points from inside the guest"),
            Duration::from_secs(180),
            || async {
                let n = manager.guest_points(metric).await;
                (n > 0).then_some(n)
            },
        )
        .await;
        checkpoint(&format!("{metric}: {points} valid points"));
    }

    // The root mount: dimensioned, sane (available < total, both > 0),
    // writable (read_only = 0).
    let root_available = wait_until(
        "a valid available_bytes sample for the root mount",
        Duration::from_secs(60),
        || async {
            manager
                .current_root_mount("vm.guest.fs.available_bytes")
                .await
                .and_then(|s| s.integer_value.or(s.value.map(|v| v as i64)))
                .filter(|v| *v > 0)
        },
    )
    .await;
    let root_total = manager
        .current_root_mount("vm.guest.fs.total_bytes")
        .await
        .and_then(|s| s.integer_value.or(s.value.map(|v| v as i64)))
        .expect("total_bytes sample for the root mount");
    assert!(
        root_available < root_total,
        "root mount: available {root_available} should be below total {root_total}"
    );
    let root_read_only = manager
        .current_root_mount("vm.guest.fs.read_only")
        .await
        .and_then(|s| s.integer_value)
        .expect("read_only sample for the root mount");
    assert_eq!(root_read_only, 0, "the root mount is writable");
    checkpoint(&format!(
        "root mount: {root_available}/{root_total} bytes available"
    ));

    // Network counters: the guest NIC's byte counters advance between
    // two observations (the agent's own reporting traffic guarantees
    // movement — real counters, not constants).
    let nic_rx = |dims: &std::collections::BTreeMap<String, String>| {
        dims.get("interface_id")
            .map(|v| !v.starts_with("loopback:"))
            .unwrap_or(false)
    };
    let rx_before = wait_until(
        "a non-loopback interface rx counter",
        Duration::from_secs(60),
        || async {
            manager
                .current("vm.guest.net.rx_bytes_total")
                .await
                .into_iter()
                .find(|s| nic_rx(&s.dimensions) && s.integer_value.unwrap_or(0) > 0)
        },
    )
    .await;
    tokio::time::sleep(Duration::from_secs(12)).await;
    let rx_after = manager
        .current("vm.guest.net.rx_bytes_total")
        .await
        .into_iter()
        .find(|s| s.dimensions == rx_before.dimensions)
        .expect("the same interface's rx counter after the wait");
    assert!(
        rx_after.integer_value.unwrap_or(0) > rx_before.integer_value.unwrap_or(0),
        "rx counter advanced: {} -> {}",
        rx_before.integer_value.unwrap_or(0),
        rx_after.integer_value.unwrap_or(0)
    );
    checkpoint("guest NIC rx counters advance");

    // Process selectors: the python3 http.server is a real measured
    // process (count >= 1, rss > 0), systemd (pid 1) likewise.
    let python_count = manager
        .current_dimensioned("vm.guest.process.count", "process_selector", "python3")
        .await
        .and_then(|s| s.integer_value.or(s.value.map(|v| v as i64)))
        .expect("process.count for the python3 selector");
    assert!(
        python_count >= 1,
        "python3 selector measured {python_count}"
    );
    let python_rss = manager
        .current_dimensioned("vm.guest.process.rss_bytes", "process_selector", "python3")
        .await
        .and_then(|s| s.integer_value.or(s.value.map(|v| v as i64)))
        .expect("process.rss_bytes for the python3 selector");
    assert!(python_rss > 0, "python3 selector rss {python_rss}");
    let systemd_count = manager
        .current_dimensioned("vm.guest.process.count", "process_selector", "systemd")
        .await
        .and_then(|s| s.integer_value.or(s.value.map(|v| v as i64)))
        .expect("process.count for the systemd selector");
    assert!(
        systemd_count >= 1,
        "systemd selector measured {systemd_count}"
    );
    checkpoint("process selectors measure real processes");

    // Phase 3 — the check inventory reflects real OS state. The
    // services/checks families ride the 60 s cadence, so the first
    // records land within a couple of minutes of enrollment.
    let ssh_check = wait_until(
        "service:ssh.service to report ok",
        Duration::from_secs(240),
        || async {
            manager
                .find_check("service:ssh.service")
                .await
                .filter(|c| c.status == CheckStatus::Ok && !c.stale)
        },
    )
    .await;
    assert_eq!(ssh_check.service_key.as_deref(), Some("ssh.service"));
    let g4_http_check = wait_until(
        "service:g4-http.service to report ok",
        Duration::from_secs(60),
        || async {
            manager
                .find_check("service:g4-http.service")
                .await
                .filter(|c| c.status == CheckStatus::Ok && !c.stale)
        },
    )
    .await;
    assert_eq!(
        g4_http_check.service_key.as_deref(),
        Some("g4-http.service")
    );
    // The configured-but-absent unit: unknown, "not installed", and
    // NO service.up sample — three distinct honest states.
    let absent = wait_until(
        "service:g4-absent.service to report unknown (not installed)",
        Duration::from_secs(60),
        || async {
            manager
                .find_check("service:g4-absent.service")
                .await
                .filter(|c| {
                    c.status == CheckStatus::Unknown
                        && c.summary
                            .as_deref()
                            .map(|s| s.contains("not installed"))
                            .unwrap_or(false)
                })
        },
    )
    .await;
    assert!(
        manager
            .current_dimensioned("vm.guest.service.up", "service_key", "g4-absent.service")
            .await
            .is_none(),
        "no service.up sample for a not-installed unit (honest absence)"
    );
    let _ = absent;
    // service.up tracks the real unit state.
    let up = manager
        .current_dimensioned("vm.guest.service.up", "service_key", "g4-http.service")
        .await
        .and_then(|s| s.integer_value)
        .expect("service.up for g4-http.service");
    assert_eq!(up, 1, "g4-http.service is really running");
    // The local declarative checks: http ok (the loopback server is
    // up), tcp ok (sshd listens), both on the first cadence tick.
    let http_check = wait_until("http:app to report ok", Duration::from_secs(60), || async {
        manager
            .find_check("http:app")
            .await
            .filter(|c| c.status == CheckStatus::Ok && !c.stale)
    })
    .await;
    assert!(
        http_check.service_key.is_none(),
        "http checks carry no service_key"
    );
    let tcp_check = wait_until("tcp:ssh to report ok", Duration::from_secs(60), || async {
        manager
            .find_check("tcp:ssh")
            .await
            .filter(|c| c.status == CheckStatus::Ok && !c.stale)
    })
    .await;
    let _ = tcp_check;
    // The failing TCP check: nothing listens on the discard-range
    // port, so the check must report critical — a real refused
    // connection on the real guest, not a simulated status.
    let tcp_closed = wait_until(
        "tcp:closed to report critical (connection refused)",
        Duration::from_secs(60),
        || async {
            manager
                .find_check("tcp:closed")
                .await
                .filter(|c| c.status == CheckStatus::Critical && !c.stale)
        },
    )
    .await;
    let _ = tcp_closed;
    // Bounded discovery is on: beyond the three configured units, at
    // least one discovered running service appears (systemd-journald
    // and friends).
    let discovered = wait_until(
        "at least one discovered service check",
        Duration::from_secs(120),
        || async {
            let n = manager
                .checks()
                .await
                .into_iter()
                .filter(|c| c.check_id.starts_with("service:"))
                .count();
            (n > 3).then_some(n)
        },
    )
    .await;
    checkpoint(&format!(
        "service checks in inventory: {discovered} (3 configured + discovered)"
    ));
    // The check trend series exist and accumulate points.
    for metric in ["check.status", "check.duration_seconds"] {
        let points = wait_until(
            &format!("valid {metric} points"),
            Duration::from_secs(120),
            || async {
                let n = manager.guest_points(metric).await;
                (n > 0).then_some(n)
            },
        )
        .await;
        checkpoint(&format!("{metric}: {points} valid points"));
    }

    // Phase 4 — plugins are DISABLED by default: the plugin files sit
    // root-owned in the allowlist directory, yet no plugin check has
    // ever appeared (checks have been flowing for minutes now, so the
    // absence is meaningful).
    let plugin_checks = manager
        .checks()
        .await
        .into_iter()
        .filter(|c| c.check_id.starts_with("plugin:"))
        .count();
    assert_eq!(
        plugin_checks, 0,
        "no plugin check may appear while [plugins] enabled = false"
    );
    checkpoint("plugins disabled by default: files present, zero plugin checks");

    // Phase 5 — filesystem fill and recovery: a real 200 MB write on
    // the root filesystem drops the reported available bytes; deleting
    // it recovers them; the history shows the excursion.
    ssh(
        &ssh_key,
        "sudo dd if=/dev/zero of=/g4fill bs=1M count=200 status=none",
    );
    let filled_available = wait_until(
        "the root mount's available bytes to drop by >= 100 MB",
        Duration::from_secs(240),
        || async {
            manager
                .current_root_mount("vm.guest.fs.available_bytes")
                .await
                .and_then(|s| s.integer_value.or(s.value.map(|v| v as i64)))
                .filter(|v| *v <= root_available - 100 * 1024 * 1024)
        },
    )
    .await;
    checkpoint(&format!(
        "root available: {root_available} -> {filled_available} after the 200 MB fill"
    ));
    ssh(&ssh_key, "sudo rm -f /g4fill");
    let recovered_available = wait_until(
        "the root mount's available bytes to recover",
        Duration::from_secs(240),
        || async {
            manager
                .current_root_mount("vm.guest.fs.available_bytes")
                .await
                .and_then(|s| s.integer_value.or(s.value.map(|v| v as i64)))
                .filter(|v| *v >= root_available - 50 * 1024 * 1024)
        },
    )
    .await;
    checkpoint(&format!(
        "root available recovered: {recovered_available} (baseline {root_available})"
    ));
    let fs_points = manager.guest_points("vm.guest.fs.available_bytes").await;
    assert!(
        fs_points >= 3,
        "the available_bytes history shows the excursion ({fs_points} points)"
    );

    // Phase 5b — inode exhaustion on a scratch mount: a tmpfs with a
    // bounded inode count appears in the discovered mounts; filling
    // 36 of its 40 inodes drives the reported inode utilization for
    // exactly that mount_id up; unmounting removes it again. Real
    // inode accounting, safely fenced off from the root filesystem.
    ssh(
        &ssh_key,
        "sudo mkdir -p /mnt/g4inodes \
         && sudo mount -t tmpfs -o size=4M,nr_inodes=40 tmpfs /mnt/g4inodes",
    );
    async fn inode_util(manager: &Manager) -> Option<CurrentSample> {
        manager
            .current_dimensioned(
                "vm.guest.fs.inodes_utilization_ratio",
                "mount_id",
                "tmpfs:/mnt/g4inodes",
            )
            .await
    }
    let inode_baseline = wait_until(
        "the scratch tmpfs to appear in the fs inventory",
        Duration::from_secs(240),
        || async {
            inode_util(&manager)
                .await
                .and_then(|s| s.value)
                .filter(|v| *v < 0.5)
        },
    )
    .await;
    ssh(
        &ssh_key,
        "sudo bash -c 'for i in $(seq 1 36); do touch /mnt/g4inodes/f$i; done'",
    );
    let inode_filled = wait_until(
        "the scratch mount's inode utilization to climb past 80%",
        Duration::from_secs(240),
        || async {
            inode_util(&manager)
                .await
                .and_then(|s| s.value)
                .filter(|v| *v >= 0.8)
        },
    )
    .await;
    checkpoint(&format!(
        "scratch tmpfs inode utilization: {inode_baseline:.3} -> {inode_filled:.3} (36/40 inodes)"
    ));
    ssh(
        &ssh_key,
        "sudo umount /mnt/g4inodes && sudo rmdir /mnt/g4inodes",
    );
    // The unmounted fs stops being collected; its series stays
    // queryable but goes STALE once the 180 s fs-family freshness
    // window passes — the same signal the dashboard's stale badges
    // render. (query_current keeps returning the last sample, marked
    // stale; it never fabricates a disappearance.)
    wait_until(
        "the unmounted scratch fs to go stale",
        Duration::from_secs(300),
        || async {
            inode_util(&manager)
                .await
                .map(|s| s.stale)
                .filter(|stale| *stale)
        },
    )
    .await;
    checkpoint("inode exhaustion observed on a scratch mount; series went stale after unmount");

    // Phase 6 — a real service outage flips the checks: stopping the
    // g4-http unit makes BOTH the systemd check and the local http
    // check critical (one root cause, two honest signals); the tcp
    // check on sshd is the control that stays ok.
    ssh(&ssh_key, "sudo systemctl stop g4-http.service");
    let stopped = wait_until(
        "service:g4-http.service to report critical after the real stop",
        Duration::from_secs(240),
        || async {
            manager
                .find_check("service:g4-http.service")
                .await
                .filter(|c| c.status == CheckStatus::Critical && !c.stale)
        },
    )
    .await;
    assert!(
        stopped
            .summary
            .as_deref()
            .map(|s| s.contains("stopped"))
            .unwrap_or(false),
        "the critical summary names the real state: {:?}",
        stopped.summary
    );
    let http_down = wait_until(
        "http:app to report critical while the endpoint is down",
        Duration::from_secs(120),
        || async {
            manager
                .find_check("http:app")
                .await
                .filter(|c| c.status == CheckStatus::Critical && !c.stale)
        },
    )
    .await;
    let _ = http_down;
    let up_after_stop = manager
        .current_dimensioned("vm.guest.service.up", "service_key", "g4-http.service")
        .await
        .and_then(|s| s.integer_value)
        .expect("service.up for g4-http.service after the stop");
    assert_eq!(up_after_stop, 0, "service.up tracks the real stop");
    // Process exit: the unit's python3 process is really gone — the
    // process selector measures 0 (its own 30 s cadence).
    let python_after_stop = wait_until(
        "the python3 selector to measure 0 after the service exit",
        Duration::from_secs(150),
        || async {
            manager
                .current_dimensioned("vm.guest.process.count", "process_selector", "python3")
                .await
                .and_then(|s| s.integer_value.or(s.value.map(|v| v as i64)))
                .filter(|v| *v == 0)
        },
    )
    .await;
    let _ = python_after_stop;
    checkpoint("process exit observed: python3 selector measured 0");
    let control = manager
        .find_check("tcp:ssh")
        .await
        .expect("tcp:ssh still recorded");
    assert!(
        control.status == CheckStatus::Ok && !control.stale,
        "the sshd tcp check is unaffected by the g4-http outage"
    );
    // Restart: both checks recover.
    ssh(&ssh_key, "sudo systemctl start g4-http.service");
    // Process start: the python3 process is back.
    let python_after_start = wait_until(
        "the python3 selector to measure >= 1 after the service start",
        Duration::from_secs(150),
        || async {
            manager
                .current_dimensioned("vm.guest.process.count", "process_selector", "python3")
                .await
                .and_then(|s| s.integer_value.or(s.value.map(|v| v as i64)))
                .filter(|v| *v >= 1)
        },
    )
    .await;
    let _ = python_after_start;
    checkpoint("process start observed: python3 selector recovered");
    wait_until(
        "service:g4-http.service to recover to ok",
        Duration::from_secs(240),
        || async {
            manager
                .find_check("service:g4-http.service")
                .await
                .filter(|c| c.status == CheckStatus::Ok && !c.stale)
        },
    )
    .await;
    wait_until(
        "http:app to recover to ok",
        Duration::from_secs(120),
        || async {
            manager
                .find_check("http:app")
                .await
                .filter(|c| c.status == CheckStatus::Ok && !c.stale)
        },
    )
    .await;
    let status_points = manager.guest_points("check.status").await;
    assert!(
        status_points >= 3,
        "the check.status trend accumulated points across the outage ({status_points})"
    );
    checkpoint("real outage flipped and recovered both checks; trend recorded");

    // Phase 7 — enabling plugins is an explicit local action: flip the
    // config, restart the agent; only then does the pinned plugin's
    // check appear and report ok.
    ssh(
        &ssh_key,
        "sudo sed -i 's/^enabled = false$/enabled = true/' /etc/chv-monitor/agent.toml \
         && sudo systemctl restart chv-monitor-agent",
    );
    let plugin_ok = wait_until(
        "plugin:g4-http-health to appear and report ok after enabling",
        Duration::from_secs(240),
        || async {
            manager
                .find_check(PLUGIN_CHECK_ID)
                .await
                .filter(|c| c.status == CheckStatus::Ok && !c.stale)
        },
    )
    .await;
    assert!(plugin_ok.service_key.is_none());
    checkpoint("plugin check reported ok after explicit enable");

    // Phase 8 — the tamper: replace the plugin executable with a
    // rogue that would touch /tmp/g4-rogue-ran and report ok. The
    // digest pin must refuse to run it: the check degrades (unknown /
    // never ok), the marker never appears, and the agent keeps
    // reporting everything else.
    let rogue_b64 = base64_of(ROGUE_PLUGIN);
    ssh(
        &ssh_key,
        &format!(
            "echo {rogue_b64} | base64 -d | sudo tee {GUEST_PLUGIN_DIR}/g4-http-health.py >/dev/null \
             && sudo chmod 0755 {GUEST_PLUGIN_DIR}/g4-http-health.py"
        ),
    );
    // Two manifest intervals of slack: the engine re-verifies before
    // every run, so the degraded record appears on the next due tick.
    wait_until(
        "the tampered plugin's check to degrade (never ok)",
        Duration::from_secs(240),
        || async {
            manager.find_check(PLUGIN_CHECK_ID).await.filter(|c| {
                c.status == CheckStatus::Unknown || (c.status != CheckStatus::Ok && c.stale)
            })
        },
    )
    .await;
    tokio::time::sleep(Duration::from_secs(70)).await; // one more interval
    let rogue_marker = ssh(
        &ssh_key,
        "test -f /tmp/g4-rogue-ran && echo RAN || echo NOT_RAN",
    );
    assert_eq!(
        rogue_marker.trim(),
        "NOT_RAN",
        "the rogue executable never ran"
    );
    let degraded = manager
        .find_check(PLUGIN_CHECK_ID)
        .await
        .expect("the tampered plugin's record still exists");
    assert_ne!(
        degraded.status,
        CheckStatus::Ok,
        "the tampered plugin never reports ok"
    );
    checkpoint("tampered plugin degraded without ever being executed");
    // The agent itself is unharmed: the other checks keep flowing.
    let ssh_still_ok = manager
        .find_check("service:ssh.service")
        .await
        .expect("service:ssh.service still recorded after the tamper");
    assert_eq!(ssh_still_ok.status, CheckStatus::Ok);
    // Nothing arrived from the manager: the allowlist directory still
    // holds exactly the two installed files, root-owned, and the
    // manifest is byte-identical to what the seed installed.
    let plugin_ls_after = ssh(&ssh_key, &format!("ls -l {GUEST_PLUGIN_DIR}/"));
    assert!(
        plugin_ls_after.contains("root root"),
        "plugin files stay root-owned: {plugin_ls_after}"
    );
    let file_count = plugin_ls_after
        .lines()
        .filter(|l| l.contains("g4-http-health"))
        .count();
    assert_eq!(
        file_count, 2,
        "exactly the two installed plugin files: {plugin_ls_after}"
    );
    let manifest_digest_after = ssh(
        &ssh_key,
        &format!("sha256sum {GUEST_PLUGIN_DIR}/g4-http-health.json"),
    );
    assert_eq!(
        manifest_digest_in_guest, manifest_digest_after,
        "the manifest was never modified by anything the manager sent"
    );
    checkpoint("allowlist directory integrity intact end to end");

    // Success path cleanup: graceful VM stop + delete. The VMM is gone
    // by now, so release the guard's vm_dir reference — its Drop must
    // neither SIGKILL a possibly-recycled PID nor dump diagnostics.
    let _ = runtime.stop_vm(VM_ID, false, None).await;
    let _ = runtime.delete_vm(VM_ID, None).await;
    cleanup.vm_dir = None;
    checkpoint("vm stopped and deleted");
}
