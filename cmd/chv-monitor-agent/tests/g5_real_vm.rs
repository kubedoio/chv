//! G5 gate evidence: the vsock monitoring transport and VM lifecycle
//! qualification on a REAL VM.
//!
//! Env-gated (CI has no KVM; the real-host record lives in
//! `docs/evidence/native-monitoring/g5/README.md`):
//!
//! ```sh
//! CHV_G1_VMM_BINARY=/tmp/opencode/g0b/cloud-hypervisor \
//! CHV_G1_FIRMWARE=/var/lib/chv/qual/hypervisor-fw \
//! CHV_G1_IMAGE=/var/lib/chv/qual/images/noble-qual-patched.img \
//! CHV_G5_AGENT_DEB=dist/packages/chv-monitor-agent_<ver>_amd64.deb \
//! cargo test -p chv-monitor-agent --test g5_real_vm -- --nocapture
//! ```
//!
//! What this proves, end to end on the production paths (real VMM,
//! real firmware boot, real Ubuntu noble guest with cloud-init, real
//! vsock device attached at vm.create, real package install, real
//! systemd, real bridge networking, real rustls on both ends):
//!
//! 1. **The pipe** — the guest agent with `[transport] type = "vsock"`
//!    enrolls and ingests through the full D1 chain: guest TLS →
//!    loopback forwarder → AF_VSOCK (CID 2, port 1024) → the VMM's
//!    muxer → the node relay's Unix socket (`vsock.sock_1024`) → the
//!    control plane's mTLS listener. Proven by the manager's
//!    connection PEER: over vsock the peer is the node (the relay's
//!    own dial), never the guest's bridge IP.
//! 2. **Transport-switch identity/replay preservation** — the same
//!    credential, the same agent_id and a strictly advancing ingest
//!    sequence survive switching the guest agent tcp ↔ vsock and back
//!    (the credential and replay state are transport-agnostic by
//!    design; SNI and certificate validation keep using server_url's
//!    host on both transports).
//! 3. **Lifecycle matrix** — reboot (same VMM, device re-attached),
//!    pause/resume (traffic stalls and resumes), stop/start (relay
//!    endpoint removed and re-derived, device re-created from the
//!    persisted payload), delete (all sockets and the persisted
//!    payload removed), guest agent restart (state_dir + credential
//!    on the guest disk carry identity across restarts), and a node
//!    (agent) restart simulated at the adapter level: a fresh adapter
//!    with its own relay manager adopts the live VM and re-binds the
//!    relay endpoint without disturbing the guest.
//! 4. **CID authority** — the durable node cursor hands out strictly
//!    increasing, distinct CIDs across VMs; the second VM's create
//!    allocates from the same cursor and gets its own relay endpoint.
//! 5. **Bounds and cleanup** — the relay endpoint exists exactly when
//!    a vsock VM is running; stop and delete remove it; a stale file
//!    never blocks a re-bind.
//!
//! What this deliberately does NOT claim: live migration of a vsock VM
//! (the platform's live migration is itself unqualified — the vsock
//! migration leg is reported BLOCKED, not mocked) and multi-node
//! behavior (the v1 transport is single-node; multi-node monitoring is
//! a separate campaign gate). The per-IP claim rate limit applies to
//! the relay's shared source address: N vsock VMs on one node enrolling
//! within one minute share that node's 10/min claim budget (documented
//! caveat, single enrollment here).

use chv_controlplane_service::api::tls::{build_https_config, serve_tls};
use chv_controlplane_service::monitoring_agent::{
    AgentCertificateIssuer, GuestIngestionLimits, MonitoringAgentService,
};
use chv_controlplane_store::test_util::TestDb;
use chv_controlplane_store::{EventRepository, MonitoringAgentRepository, MonitoringAgentRow};
use chv_monitoring_core::model::{SampleQuality, TargetKind};
use chv_monitoring_store::{MonitoringStore, MonitoringStoreConfig, Resolution};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Short interface names (IFNAMSIZ limit); unique to this rig so a
/// crashed run is visible and cleanable (`ip link del`). Distinct
/// subnet from the g3 (192.168.62.0/24) and g4 (192.168.63.0/24) rigs
/// so all can run on the same host.
const BRIDGE: &str = "br-g5ev";
const TAP: &str = "tap-g5ev";
const BRIDGE_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 64, 1);
const GUEST_IP: &str = "192.168.64.50";
const GUEST_MAC: &str = "52:54:00:64:00:50";
const VM_ID: &str = "g5-vm";
const VM_ID_2: &str = "g5-vm-2";
/// The manager's DNS name — deliberately NOT an IP literal: hyper
/// bypasses the reqwest `.resolve()` override for IP-literal URLs, so
/// a name is required for the vsock dial redirect to take effect. The
/// guest resolves it via an /etc/hosts entry when running tcp; over
/// vsock the name is used for SNI/validation only.
const MANAGER_NAME: &str = "g5-manager.chv.internal";

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn checkpoint(msg: &str) {
    let _ = std::io::Write::write_fmt(
        &mut std::io::stderr(),
        format_args!("g5 checkpoint: {msg}\n"),
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
    run_ip(&["addr", "add", "192.168.64.1/24", "dev", BRIDGE], true);
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
/// must not leak (the VMMs via SIGKILL, the network topology). Owns
/// the VM's tempdir so diagnostics survive unwinding.
struct HostCleanup {
    _dir: Option<tempfile::TempDir>,
    vm_dirs: Vec<PathBuf>,
}

impl HostCleanup {
    fn new() -> Self {
        Self {
            _dir: None,
            vm_dirs: Vec::new(),
        }
    }

    fn keep_dir(&mut self, dir: tempfile::TempDir) {
        self._dir = Some(dir);
    }
}

impl Drop for HostCleanup {
    fn drop(&mut self) {
        for vm_dir in self.vm_dirs.iter() {
            // SIGKILL the VMM if it is still up (graceful stops are
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
            dump_diagnostics(vm_dir);
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
                format_args!("g5 console.log: unreadable ({e})\n"),
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
                    format_args!("g5 {label} (tail):\n{}\n", lines[start..].join("\n")),
                );
            }
            Err(e) => {
                let _ = std::io::Write::write_fmt(
                    &mut std::io::stderr(),
                    format_args!("g5 {label}: unreadable ({e})\n"),
                );
            }
        }
    }
}

/// A self-signed TLS server certificate for the manager's DNS name
/// (the agent trusts exactly this PEM as its manager CA; name-only SAN
/// proves the guest's SNI and validation ride the pipe unchanged).
fn manager_server_cert() -> (String, String) {
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::default();
    params.subject_alt_names = vec![rcgen::SanType::DnsName(MANAGER_NAME.parse().unwrap())];
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
        .push(rcgen::DnType::CommonName, "g5-evidence-agent-ca");
    let cert = params.self_signed(&key).unwrap();
    (cert.pem(), key.serialize_pem())
}

/// The in-process manager: operational DB, monitoring store, agent
/// CA, service and a live TLS listener bound to the BRIDGE IP.
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

        for vm_id in [VM_ID, VM_ID_2] {
            sqlx::query("INSERT INTO vms (vm_id, display_name) VALUES ($1, $1)")
                .bind(vm_id)
                .execute(&ops.pool)
                .await
                .unwrap();
        }

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
            chv_monitoring_store::MonitoringHealth::new(),
            EventRepository::new(ops.pool.clone()),
            limits,
            None,
        ));

        let (server_cert_pem, server_key_pem) = manager_server_cert();
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

    /// The URL the GUEST uses (DNS name: SNI + validation host).
    fn guest_base_url(&self) -> String {
        format!("https://{MANAGER_NAME}:{}", self.port)
    }

    /// The URL the NODE-side relay dials (IP:port — the relay is a raw
    /// byte pipe and needs a routable target, not a name).
    fn relay_target(&self) -> String {
        format!("https://{BRIDGE_IP}:{}", self.port)
    }

    async fn active_agent(&self, vm_id: &str) -> Option<MonitoringAgentRow> {
        self.repo.find_active_agent_by_vm(vm_id).await.unwrap()
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
}

impl Drop for Manager {
    fn drop(&mut self) {
        if let Some(server) = self.server.take() {
            server.abort();
        }
    }
}

/// The peers of the manager's ESTABLISHED TCP connections. This is
/// the transport discriminator: over vsock every manager connection
/// comes from the relay's own local dial (peer = the bridge IP, the
/// node), never from the guest's bridge address; over tcp the peer is
/// the guest itself.
fn manager_peers(port: u16) -> Vec<String> {
    let out = std::process::Command::new("ss")
        .arg("-tn")
        .output()
        .expect("run ss");
    assert!(
        out.status.success(),
        "ss failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let suffix = format!(":{port}");
    text.lines()
        .filter_map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            // State Recv-Q Send-Q Local Address:Port Peer Address:Port
            if cols.len() >= 5 && cols[0] == "ESTAB" && cols[3].ends_with(&suffix) {
                Some(cols[4].to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Run a command in the guest over ssh (the rig's scenario trigger).
/// The noble cloud image's default user has passwordless sudo.
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

/// Best-effort ssh (diagnostics only: no panic on failure).
fn ssh_try(key_path: &Path, command: &str) -> Option<String> {
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
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// In-guest transport diagnostics — the difference between "AF_VSOCK
/// missing in the guest", "device not attached", "forwarder cannot
/// dial" and "TLS refused" is only observable from inside the guest.
fn dump_guest_transport_state(key_path: &Path) {
    for (label, command) in [
        (
            "agent journal",
            "journalctl -u chv-monitor-agent --no-pager -n 40",
        ),
        (
            "vsock modules",
            "lsmod | grep -i vsock || echo NO_VSOCK_MODULES",
        ),
        (
            "vsock dmesg",
            "sudo dmesg | grep -i vsock || echo NO_VSOCK_IN_DMESG",
        ),
        (
            "virtio device modalias",
            "grep -h . /sys/bus/virtio/devices/*/modalias 2>/dev/null || echo NO_VIRTIO_DEVICES",
        ),
        (
            "guest sockets",
            "ss -tnp 2>/dev/null | grep -E 'chv-monitor|127.0.0.1' || echo NO_LOCAL_CONNS",
        ),
    ] {
        match ssh_try(key_path, command) {
            Some(text) => {
                let _ = std::io::Write::write_fmt(
                    &mut std::io::stderr(),
                    format_args!("g5 guest {label}:\n{text}\n"),
                );
            }
            None => {
                let _ = std::io::Write::write_fmt(
                    &mut std::io::stderr(),
                    format_args!("g5 guest {label}: ssh not reachable yet\n"),
                );
            }
        }
    }
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
/// seconds, not sub-second.
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
            panic!("g5: timed out waiting for {what} (budget {budget:?})");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// `wait_until` plus periodic in-guest transport diagnostics: after
/// every 60 s of waiting, dump the guest agent's journal, the vsock
/// module state and the console tail. The VM is destroyed when the
/// test panics, so a phase failure must be diagnosable from the run
/// log alone — there is no second chance to look inside the guest.
async fn wait_until_diag<T, F>(
    what: &str,
    budget: Duration,
    ssh_key: &std::path::Path,
    vm_dir: &std::path::Path,
    mut probe: impl FnMut() -> F,
) -> T
where
    F: std::future::Future<Output = Option<T>>,
{
    let deadline = Instant::now() + budget;
    let mut polls = 0u32;
    loop {
        if let Some(value) = probe().await {
            return value;
        }
        if Instant::now() >= deadline {
            panic!("g5: timed out waiting for {what} (budget {budget:?})");
        }
        polls += 1;
        if polls.is_multiple_of(30) {
            checkpoint(&format!(
                "still waiting for {what} (poll {polls}); diagnostics:"
            ));
            dump_console_tail(vm_dir);
            dump_guest_transport_state(ssh_key);
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

/// The guest agent's G5 config: vsock transport, 5 s interval so the
/// phases fit in evidence time. Collectors stay at their defaults
/// (read-only families on) — the resource family is the telemetry
/// under observation.
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
log_level = "debug"

[transport]
type = "vsock"
"#,
        url = manager.guest_base_url()
    )
}

/// cloud-init user-data: ssh key, the /etc/hosts entry that makes the
/// manager's DNS name resolvable for the tcp phases, and the agent
/// install (deb + config + CA + claim from the seed ISO).
fn guest_userdata(ssh_pubkey: &str) -> String {
    format!(
        r#"#cloud-config
ssh_authorized_keys:
  - {ssh_pubkey}
runcmd:
  - echo "{BRIDGE_IP} {MANAGER_NAME}" | tee -a /etc/hosts
  - mkdir -p /media/cidata
  - mount -o ro "$(findfs LABEL=cidata)" /media/cidata
  - dpkg -i /media/cidata/chv-monitor-agent.deb
  - install -m 0644 /media/cidata/agent.toml /etc/chv-monitor/agent.toml
  - install -m 0644 /media/cidata/manager-ca.pem /etc/chv-monitor/manager-ca.pem
  - install -o chv-monitor -g chv-monitor -m 0600 /media/cidata/claim /var/lib/chv-monitor/claim
  - systemctl enable --now chv-monitor-agent
"#
    )
}

/// Enrich the production-built NoCloud seed with the agent package
/// and its inputs (the production adapter writes user-data from
/// `cloud_init_userdata` — which already carries the ssh key — so
/// this only adds seed files).
async fn enrich_seed(vm_dir: &Path, manager: &Manager, deb: &Path, claim_token: &str) {
    let seed_dir = vm_dir.join("seed");
    std::fs::copy(deb, seed_dir.join("chv-monitor-agent.deb"))
        .expect("copy the agent deb into the seed");
    std::fs::write(seed_dir.join("agent.toml"), guest_agent_toml(manager))
        .expect("write the guest agent config into the seed");
    std::fs::write(seed_dir.join("manager-ca.pem"), &manager.server_cert_pem)
        .expect("write the manager CA into the seed");
    std::fs::write(seed_dir.join("claim"), claim_token).expect("write the claim into the seed");

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
            .arg(seed_dir.join("claim"));
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
    checkpoint("seed enriched with the agent package, config and claim");
}

/// The relay's listening path for a VM (the contract: the device
/// socket from the persisted payload + "_" + the fixed port).
fn relay_socket(vm_dir: &Path) -> PathBuf {
    chv_agent_runtime_ch::vsock_relay::relay_socket_path(
        vm_dir.join("vsock.sock").to_string_lossy().as_ref(),
    )
}

#[tokio::test]
async fn g5_real_vm_vsock_transport_and_lifecycle_matrix() {
    let Ok(vmm_binary) = std::env::var("CHV_G1_VMM_BINARY") else {
        eprintln!("skipping: CHV_G1_VMM_BINARY not set (real-KVM evidence test)");
        return;
    };
    let firmware =
        std::env::var("CHV_G1_FIRMWARE").expect("CHV_G1_FIRMWARE with CHV_G1_VMM_BINARY");
    let image = std::env::var("CHV_G1_IMAGE").expect("CHV_G1_IMAGE with CHV_G1_VMM_BINARY");
    let Ok(agent_deb) = std::env::var("CHV_G5_AGENT_DEB") else {
        eprintln!("skipping: CHV_G5_AGENT_DEB not set (build the guest package first)");
        return;
    };
    let agent_deb = PathBuf::from(agent_deb);
    assert!(
        agent_deb.is_file(),
        "CHV_G5_AGENT_DEB={:?} is not a file",
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
    checkpoint(
        format!(
            "manager listening on {} (guest url {})",
            manager.relay_target(),
            manager.guest_base_url()
        )
        .as_str(),
    );

    // The claim is issued BEFORE the VM exists: it is embedded in the
    // seed, so its TTL covers boot + cloud-init + install.
    let claim = manager
        .service
        .issue_claim(VM_ID, "g5-evidence", now_ms())
        .await
        .expect("issue the one-time claim");

    // The guest agent install WRITES to the guest rootfs (dpkg,
    // systemd enable) — but the pinned qualification image is never
    // written (G0b/G1/G2/G3/G4 discipline). A full sparse copy gives
    // the VMM a writable boot disk.
    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    // The cleanup guard takes ownership of the TempDir; every path
    // below is pre-derived so nothing borrows `dir` after the move.
    let root = dir.path().to_path_buf();
    let (ssh_key, ssh_pubkey) = rig_ssh_key(&root);
    let root_disk = root.join("g5-root.qcow2");
    let cp = std::process::Command::new("cp")
        .arg("--sparse=always")
        .arg(&image)
        .arg(&root_disk)
        .status()
        .expect("run cp for the writable root copy");
    assert!(cp.success(), "cp of the qualified image failed");

    // The node side: the production adapter WITH the G5 relay manager
    // wired (exactly as chv-agent's main wires it — the relay target
    // is the manager's routable address, fail-soft parse like prod).
    let relay_manager = Arc::new(
        chv_agent_runtime_ch::vsock_relay::VsockRelayManager::new(&manager.relay_target())
            .expect("parse the relay target"),
    );
    let adapter =
        chv_agent_runtime_ch::ProcessCloudHypervisorAdapter::new(PathBuf::from(&vmm_binary))
            .with_vsock_relay(Some(Arc::clone(&relay_manager)));
    let runtime = chv_agent_core::vm_runtime::VmRuntime::new(Arc::new(adapter));

    let vm_dir = root.join("vms").join(VM_ID);
    cleanup.vm_dirs.push(vm_dir.clone());
    // The cleanup guard owns the tempdir: on a failed assert it dumps
    // the console/VMM logs BEFORE the directory is deleted.
    cleanup.keep_dir(dir);

    // --- Phase 1: create with the vsock device (the opt-in path) ---
    // Production creates the VM's runtime dir before device prep; the
    // rig drives the adapter directly, so it does the same.
    std::fs::create_dir_all(&vm_dir).expect("create the VM runtime dir");
    let vsock = chv_hypervisor_api::prepare_vm_vsock_device(&root, &vm_dir)
        .expect("allocate a vsock device for the VM");
    let cid_1 = vsock.cid;
    checkpoint(format!("vsock device allocated (cid {cid_1})").as_str());

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
            network_id: "g5-evidence".to_string(),
            mac_address: GUEST_MAC.to_string(),
            ip_address: GUEST_IP.to_string(),
            tap_name: TAP.to_string(),
            cidr: "192.168.64.0/24".to_string(),
            gateway: BRIDGE_IP.to_string(),
        }],
        api_socket_path: vm_dir.join("vm.sock"),
        cloud_init_userdata: Some(guest_userdata(&ssh_pubkey)),
        hypervisor_overrides: None,
        vsock: Some(vsock),
    };

    checkpoint("creating vm (production adapter, vsock device, seed built)");
    runtime
        .create_vm(VM_ID, "g5-1", &config, None)
        .await
        .expect("production create_vm attaches the vsock device");

    // The persisted payload carries the device; the relay endpoint
    // exists from the moment the VM exists (create-time hook); the
    // node cursor is committed at the allocated CID.
    let persisted: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(vm_dir.join("vm-config.json")).expect("persisted payload"),
    )
    .expect("payload json");
    assert_eq!(
        persisted["vsock"]["cid"].as_u64(),
        Some(cid_1 as u64),
        "the persisted payload carries the allocated CID"
    );
    assert!(
        relay_socket(&vm_dir).exists(),
        "the relay endpoint exists right after create"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("vsock-cid-cursor"))
            .expect("node vsock CID cursor")
            .trim(),
        cid_1.to_string(),
        "the cursor is committed at the allocated CID"
    );
    checkpoint("create: payload + relay endpoint + cursor verified");

    enrich_seed(&vm_dir, &manager, &agent_deb, &claim.token).await;
    runtime
        .start_vm(VM_ID, None)
        .await
        .expect("production start_vm boots the enriched seed");

    // Diagnostics: the adapter/relay logs (accept, dial, bounds) are
    // only visible with a subscriber installed.
    let _ = tracing_subscriber::fmt()
        .with_env_filter("chv_agent_runtime_ch=debug")
        .with_writer(std::io::stderr)
        .try_init();

    {
        let pid: u32 = std::fs::read_to_string(vm_dir.join("ch.pid"))
            .expect("the adapter persists the VMM pid")
            .trim()
            .parse()
            .expect("ch.pid content");
        // vm.boot (not vm.create) constructs the devices: the vsock
        // device's host-side socket exists from the moment the VMM
        // boots the VM.
        assert!(
            vm_dir.join("vsock.sock").exists(),
            "the VMM bound the device socket at boot (vm.boot attached the device)"
        );
        wait_until(
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
        checkpoint("guest executing");
    }

    // --- Phase 2: enrollment rides the pipe (D1 end-to-end) ---
    let mut poll_count: u32 = 0;
    let agent = wait_until(
        "the guest agent to enroll over vsock (cloud-init + dpkg + systemd + mTLS through the pipe)",
        Duration::from_secs(420),
        || {
            poll_count += 1;
            if poll_count.is_multiple_of(30) {
                checkpoint(&format!(
                    "still waiting for enrollment (poll {poll_count}); guest console tail:"
                ));
                dump_console_tail(&vm_dir);
                dump_guest_transport_state(&ssh_key);
            }
            async { manager.active_agent(VM_ID).await }
        },
    )
    .await;
    checkpoint("agent enrolled over vsock");
    assert_eq!(agent.vm_id, VM_ID);
    let agent_id = agent.agent_id.clone();

    // The transport discriminator: the manager's connection peer is
    // the relay's local dial (the bridge IP), NOT the guest's bridge
    // address — the traffic physically rode the pipe.
    wait_until(
        "a manager connection from the relay (peer = bridge IP, not the guest)",
        Duration::from_secs(60),
        || async {
            let peers = manager_peers(manager.port);
            (!peers.is_empty()
                && peers
                    .iter()
                    .all(|p| p.starts_with(&format!("{BRIDGE_IP}:"))))
            .then_some(peers)
        },
    )
    .await;
    assert!(
        relay_socket(&vm_dir).exists(),
        "the relay endpoint stays up while the VM runs"
    );
    checkpoint("manager peers are all relay-side: the pipe carried the TLS session");

    // Telemetry flows over the pipe.
    let points_after_pipe = wait_until(
        "valid guest telemetry points over vsock",
        Duration::from_secs(180),
        || async {
            let n = manager.guest_points("vm.guest.cpu.utilization_ratio").await;
            (n > 0).then_some(n)
        },
    )
    .await;
    checkpoint(format!("telemetry over the pipe: {points_after_pipe} points").as_str());
    let sequence_over_pipe = manager
        .active_agent(VM_ID)
        .await
        .expect("agent after pipe phase")
        .last_sequence
        .expect("sequence advances over the pipe");
    assert!(sequence_over_pipe > 0);

    // --- Phase 3: switch the guest to tcp (identity/replay must survive) ---
    ssh(
        &ssh_key,
        // `sed -i` replaces the file via rename; vm.reboot is a HARD
        // reset (no ACPI shutdown, no page-cache flush), so the
        // rewritten config must be on disk before the rig reboots the
        // VM — otherwise the reset can leave a zero-length file (the
        // exact failure the G5 rig caught: the agent crash-looped
        // with "missing field `server_url`" after the reboot).
        "sudo sed -i 's/^type = \"vsock\"$/type = \"tcp\"/' /etc/chv-monitor/agent.toml \
         && sudo sync \
         && sudo systemctl restart chv-monitor-agent",
    );
    wait_until(
        "telemetry to keep flowing over tcp with the SAME agent",
        Duration::from_secs(120),
        || async {
            let agent = manager.active_agent(VM_ID).await?;
            let n = manager.guest_points("vm.guest.cpu.utilization_ratio").await;
            (agent.agent_id == agent_id && n > points_after_pipe)
                .then_some((agent.last_sequence, n))
        },
    )
    .await;
    // Over tcp the manager's peer IS the guest's bridge address — the
    // discriminator in reverse, proving the switch actually switched.
    wait_until(
        "a manager connection from the guest itself (peer = guest IP)",
        Duration::from_secs(60),
        || async {
            manager_peers(manager.port)
                .iter()
                .any(|p| p.starts_with(&format!("{GUEST_IP}:")))
                .then_some(())
        },
    )
    .await;
    checkpoint("tcp phase: same agent, samples flowing, peer is the guest");

    // --- Phase 4: switch back to vsock ---
    let points_before_switch_back = manager.guest_points("vm.guest.cpu.utilization_ratio").await;
    ssh(
        &ssh_key,
        "sudo sed -i 's/^type = \"tcp\"$/type = \"vsock\"/' /etc/chv-monitor/agent.toml \
         && sudo sync \
         && sudo systemctl restart chv-monitor-agent",
    );
    let (sequence_after_switch, points_after_switch_back) = wait_until(
        "telemetry to keep flowing over vsock again with the SAME agent",
        Duration::from_secs(120),
        || async {
            let agent = manager.active_agent(VM_ID).await?;
            let n = manager.guest_points("vm.guest.cpu.utilization_ratio").await;
            (agent.agent_id == agent_id
                && agent.last_sequence.is_some_and(|s| s > sequence_over_pipe)
                && n > points_before_switch_back)
                .then_some((agent.last_sequence, n))
        },
    )
    .await;
    wait_until(
        "the manager peer to be relay-side again",
        Duration::from_secs(60),
        || async {
            let peers = manager_peers(manager.port);
            (!peers.is_empty() && peers.iter().all(|p| p.starts_with(&format!("{BRIDGE_IP}"))))
                .then_some(())
        },
    )
    .await;
    checkpoint(format!(
        "vsock phase restored: sequence advanced past {sequence_over_pipe} to {sequence_after_switch:?}, {points_after_switch_back} points"
    )
    .as_str());

    // --- Phase 5: reboot (same VMM, device re-attached) ---
    runtime.reboot_vm(VM_ID, None).await.expect("vm.reboot");
    let points_before_reboot = manager.guest_points("vm.guest.cpu.utilization_ratio").await;
    wait_until_diag(
        "telemetry to resume after the guest reboot (same agent, no re-enroll)",
        Duration::from_secs(240),
        &ssh_key,
        &vm_dir,
        || async {
            let agent = manager.active_agent(VM_ID).await?;
            let n = manager.guest_points("vm.guest.cpu.utilization_ratio").await;
            (agent.agent_id == agent_id && n > points_before_reboot).then_some(n)
        },
    )
    .await;
    assert!(
        relay_socket(&vm_dir).exists(),
        "the relay endpoint survives the reboot (same VMM)"
    );
    checkpoint("reboot: guest back, same agent, relay endpoint intact");

    // --- Phase 6: pause / resume ---
    runtime.pause_vm(VM_ID, None).await.expect("vm.pause");
    tokio::time::sleep(Duration::from_secs(20)).await;
    runtime.resume_vm(VM_ID, None).await.expect("vm.resume");
    let points_before_resume = manager.guest_points("vm.guest.cpu.utilization_ratio").await;
    wait_until_diag(
        "telemetry to resume after pause/resume",
        Duration::from_secs(240),
        &ssh_key,
        &vm_dir,
        || async {
            let n = manager.guest_points("vm.guest.cpu.utilization_ratio").await;
            (n > points_before_resume).then_some(n)
        },
    )
    .await;
    checkpoint("pause/resume: traffic resumed");

    // --- Phase 7: stop / start (endpoint removed, then re-derived) ---
    runtime
        .stop_vm(VM_ID, false, None)
        .await
        .expect("graceful stop");
    assert!(
        !relay_socket(&vm_dir).exists(),
        "stop removes the relay endpoint"
    );
    assert!(
        !vm_dir.join("vsock.sock").exists(),
        "stop leaves no device socket behind (the VMM unlinks on clean exit)"
    );
    checkpoint("stop: relay endpoint and device socket removed");

    runtime.start_vm(VM_ID, None).await.expect("re-start");
    assert!(
        relay_socket(&vm_dir).exists(),
        "start re-derives the relay endpoint from the persisted payload"
    );
    assert!(
        vm_dir.join("vsock.sock").exists(),
        "the re-spawned VMM re-created the device socket"
    );
    let points_before_restart = manager.guest_points("vm.guest.cpu.utilization_ratio").await;
    wait_until_diag(
        "telemetry to resume after the full stop/start cycle (same agent)",
        Duration::from_secs(420),
        &ssh_key,
        &vm_dir,
        || async {
            let agent = manager.active_agent(VM_ID).await?;
            let n = manager.guest_points("vm.guest.cpu.utilization_ratio").await;
            (agent.agent_id == agent_id && n > points_before_restart).then_some(n)
        },
    )
    .await;
    checkpoint("stop/start: guest back, same agent, endpoint re-derived");

    // --- Phase 8: node (agent) restart — a FRESH adapter adopts the live VM ---
    let adapter2 =
        chv_agent_runtime_ch::ProcessCloudHypervisorAdapter::new(PathBuf::from(&vmm_binary))
            .with_vsock_relay(Some(Arc::new(
                chv_agent_runtime_ch::vsock_relay::VsockRelayManager::new(&manager.relay_target())
                    .expect("parse the relay target"),
            )));
    adapter2
        .adopt_running_vms(&root)
        .await
        .expect("a fresh adapter adopts the running VM");
    assert!(
        relay_socket(&vm_dir).exists(),
        "adoption re-binds the relay endpoint"
    );
    // Force the guest to make a NEW connection (its keep-alive rides
    // the old, unlinked socket inode): restart the agent in-guest and
    // require the new traffic to still land.
    ssh(&ssh_key, "sudo systemctl restart chv-monitor-agent");
    let points_before_adopt = manager.guest_points("vm.guest.cpu.utilization_ratio").await;
    wait_until_diag(
        "telemetry to flow through the ADOPTED relay (fresh manager, rebound endpoint)",
        Duration::from_secs(240),
        &ssh_key,
        &vm_dir,
        || async {
            let agent = manager.active_agent(VM_ID).await?;
            let n = manager.guest_points("vm.guest.cpu.utilization_ratio").await;
            (agent.agent_id == agent_id && n > points_before_adopt).then_some(n)
        },
    )
    .await;
    wait_until(
        "the manager peer to be relay-side after adoption",
        Duration::from_secs(60),
        || async {
            let peers = manager_peers(manager.port);
            (!peers.is_empty() && peers.iter().all(|p| p.starts_with(&format!("{BRIDGE_IP}"))))
                .then_some(())
        },
    )
    .await;
    checkpoint("node restart (adoption): endpoint rebound, traffic flows through the new relay");

    // --- Phase 9: delete (all artifacts gone) ---
    let _ = runtime.stop_vm(VM_ID, false, None).await;
    runtime.delete_vm(VM_ID, None).await.expect("delete");
    assert!(
        !relay_socket(&vm_dir).exists(),
        "delete removes the relay endpoint"
    );
    assert!(
        !vm_dir.join("vsock.sock").exists(),
        "delete removes the device socket"
    );
    assert!(
        !vm_dir.join("vm-config.json").exists(),
        "delete removes the persisted payload"
    );
    checkpoint("delete: all vsock artifacts removed");

    // --- Phase 10: CID uniqueness across VMs (same node cursor) ---
    let vm_dir_2 = root.join("vms").join(VM_ID_2);
    cleanup.vm_dirs.push(vm_dir_2.clone());
    std::fs::create_dir_all(&vm_dir_2).expect("create the VM 2 runtime dir");
    let root_disk_2 = root.join("g5-root-2.qcow2");
    let cp2 = std::process::Command::new("cp")
        .arg("--sparse=always")
        .arg(&image)
        .arg(&root_disk_2)
        .status()
        .expect("run cp for the second root copy");
    assert!(cp2.success(), "cp of the qualified image failed (vm 2)");
    let vsock_2 =
        chv_hypervisor_api::prepare_vm_vsock_device(&root, &vm_dir_2).expect("allocate vm 2");
    let cid_2 = vsock_2.cid;
    assert_ne!(cid_2, cid_1, "the node cursor hands out distinct CIDs");
    assert!(cid_2 > cid_1, "the cursor is monotonic");
    let config_2 = chv_hypervisor_api::VmConfig {
        vm_id: VM_ID_2.to_string(),
        cpus: 1,
        memory_bytes: 256 * 1024 * 1024,
        kernel_path: PathBuf::from("/dev/null"),
        firmware_path: Some(PathBuf::from(&firmware)),
        disks: vec![chv_hypervisor_api::VmDiskConfig {
            path: root_disk_2,
            read_only: false,
            id: None,
        }],
        nics: vec![],
        api_socket_path: vm_dir_2.join("vm.sock"),
        cloud_init_userdata: None,
        hypervisor_overrides: None,
        vsock: Some(vsock_2),
    };
    runtime
        .create_vm(VM_ID_2, "g5-2", &config_2, None)
        .await
        .expect("create the second vsock VM (no boot needed)");
    assert!(
        relay_socket(&vm_dir_2).exists(),
        "the second VM gets its own relay endpoint"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("vsock-cid-cursor"))
            .expect("node vsock CID cursor")
            .trim(),
        cid_2.to_string(),
        "the cursor advanced to the second allocation"
    );
    let _ = runtime.stop_vm(VM_ID_2, false, None).await;
    runtime.delete_vm(VM_ID_2, None).await.expect("delete vm 2");
    assert!(!relay_socket(&vm_dir_2).exists());
    checkpoint(
        format!(
            "cid uniqueness: {} then {} (cursor committed), vm 2 cleaned up",
            cid_1, cid_2
        )
        .as_str(),
    );

    // --- Migration leg: BLOCKED, honestly ---------------------------------
    // The platform's live migration is itself unqualified (no CHV
    // live-migration qualification exists in this campaign or the
    // platform's record). Per prompt-06: report the vsock migration
    // leg BLOCKED rather than substitute a mock. Recorded in
    // docs/evidence/native-monitoring/g5/README.md.
    checkpoint("migration leg: BLOCKED (platform live migration unqualified — not mocked)");

    // Matrix summary (stderr, mirrored into the evidence record).
    checkpoint(
        "matrix: boot=PASS pipe-enroll=PASS pipe-telemetry=PASS switch-tcp=PASS \
                switch-vsock=PASS reboot=PASS pause-resume=PASS stop-start=PASS \
                agent-restart=PASS node-restart-adopt=PASS delete=PASS cid-uniqueness=PASS \
                migration=BLOCKED detach=N/A(v1 create-time only) multi-node=N/A(separate gate)",
    );

    // Success path cleanup: the VMs are already deleted; release the
    // guard's vm_dir references — its Drop must neither SIGKILL a
    // possibly-recycled PID nor dump diagnostics.
    cleanup.vm_dirs.clear();
    checkpoint("g5 rig complete");
}
