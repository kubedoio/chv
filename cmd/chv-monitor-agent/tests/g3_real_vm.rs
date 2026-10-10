//! G3 gate evidence: the optional guest monitoring agent on a REAL VM.
//!
//! Env-gated (CI has no KVM; the real-host record lives in
//! `docs/evidence/native-monitoring/g3/README.md`):
//!
//! ```sh
//! CHV_G1_VMM_BINARY=/tmp/opencode/g0b/cloud-hypervisor \
//! CHV_G1_FIRMWARE=/var/lib/chv/qual/hypervisor-fw \
//! CHV_G1_IMAGE=/var/lib/chv/qual/images/noble-qual-patched.img \
//! CHV_G3_AGENT_DEB=dist/packages/chv-monitor-agent_<ver>_amd64.deb \
//! cargo test -p chv-monitor-agent --test g3_real_vm -- --nocapture
//! ```
//!
//! What this proves, end to end on the production paths (real VMM
//! process, real firmware boot, real guest OS with cloud-init, real
//! package install, real systemd service, real bridge networking,
//! real rustls on both ends):
//!
//! 1. **Packaged install in a real guest** — cloud-init in the
//!    qualified image installs the `chv-monitor-agent` .deb from the
//!    NoCloud seed (the seed is enriched between `create_vm` and
//!    `start_vm`; every cloud-init file the production adapter wrote
//!    is preserved byte-for-byte).
//! 2. **Secure enrollment** — the agent redeems the one-time claim
//!    over mutual TLS against a manager listener bound to the bridge
//!    IP; the registry row for the VM becomes active.
//! 3. **Real guest telemetry** — load, uptime, memory, CPU and OS
//!    identity from inside the guest land in the manager's bounded
//!    history store, bound to the correct VM.
//! 4. **Outage durability** — a manager outage spools batches on the
//!    guest disk; on reconnect the spool drains and the sequence
//!    high-water advances past the gap.
//! 5. **Revocation** — after revoke, reporting stops (last_seen and
//!    last_sequence freeze; the agent is 401-blocked).
//!
//! What this deliberately does NOT claim: hardware attestation or
//! VM-image identity — the credential proves only that the holder
//! redeemed an operator-issued claim on this install (ADR-026's
//! recorded trust limitation).

use chv_controlplane_service::api::tls::{build_https_config, serve_tls};
use chv_controlplane_service::monitoring_agent::{
    AgentCertificateIssuer, GuestIngestionLimits, MonitoringAgentService,
};
use chv_controlplane_store::test_util::TestDb;
use chv_controlplane_store::{EventRepository, MonitoringAgentRepository, MonitoringAgentRow};
use chv_monitoring_store::{MonitoringHealth, MonitoringStore, MonitoringStoreConfig};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Short interface names (IFNAMSIZ limit); unique to this rig so a
/// crashed run is visible and cleanable (`ip link del`).
const BRIDGE: &str = "br-g3ev";
const TAP: &str = "tap-g3ev";
const BRIDGE_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 62, 1);
const GUEST_IP: &str = "192.168.62.50";
const GUEST_MAC: &str = "52:54:00:62:00:50";
const VM_ID: &str = "g3-vm";

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn checkpoint(msg: &str) {
    let _ = std::io::Write::write_fmt(
        &mut std::io::stderr(),
        format_args!("g3 checkpoint: {msg}\n"),
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

/// Bridge + tap on the host (test-side topology: production nwd owns
/// this in a real deployment; G3's subject is the guest agent, not
/// nwd). The manager listener binds the bridge IP, the guest reaches
/// it via cloud-init's static IP from the production seed.
fn setup_network() {
    // Idempotent: clear any leftover from a crashed run first.
    run_ip(&["link", "del", TAP], false);
    run_ip(&["link", "del", BRIDGE], false);
    run_ip(&["link", "add", BRIDGE, "type", "bridge"], true);
    run_ip(&["addr", "add", "192.168.62.1/24", "dev", BRIDGE], true);
    run_ip(&["link", "set", BRIDGE, "up"], true);
    run_ip(&["tuntap", "add", "dev", TAP, "mode", "tap"], true);
    run_ip(&["link", "set", TAP, "master", BRIDGE], true);
    run_ip(&["link", "set", TAP, "up"], true);
}

fn teardown_network() {
    run_ip(&["link", "del", TAP], false);
    run_ip(&["link", "del", BRIDGE], false);
}

/// Synchronous best-effort cleanup for the host resources a failed
/// run must not leak: the VMM process (SIGKILL via the adapter's pid
/// file) and the test-side network topology. Owns the VM's tempdir so
/// the console and VMM logs are still readable when diagnostics are
/// dumped (a bare TempDir local would drop — and delete — before this
/// guard during unwinding).
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
                format_args!("g3 console.log: unreadable ({e})\n"),
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
                    format_args!("g3 {label} (tail):\n{}\n", lines[start..].join("\n")),
                );
            }
            Err(e) => {
                let _ = std::io::Write::write_fmt(
                    &mut std::io::stderr(),
                    format_args!("g3 {label}: unreadable ({e})\n"),
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
        .push(rcgen::DnType::CommonName, "g3-evidence-agent-ca");
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
    agent_ca_pem: String,
    server_cert_pem: String,
    server_key_pem: String,
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
        // TTL clock starts at issuance. A full firmware boot + cloud-init
        // + dpkg takes well under the default 10 minutes, but this
        // evidence run must not flake on a loaded host.
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
            agent_ca_pem,
            server_cert_pem,
            server_key_pem,
            port,
            server: Some(server),
            _shutdown_tx: shutdown_tx,
        }
    }

    fn base_url(&self) -> String {
        format!("https://{BRIDGE_IP}:{}", self.port)
    }

    async fn stop_listener(&mut self) {
        if let Some(server) = self.server.take() {
            server.abort();
            let _ = server.await;
        }
    }

    async fn restart_listener(&mut self) {
        self.stop_listener().await;
        let https = build_https_config(
            &self.server_cert_pem,
            &self.server_key_pem,
            Some(&self.agent_ca_pem),
        )
        .unwrap();
        let router = chv_controlplane_service::api::agent_routes::agent_routes(axum::Router::new())
            .layer(axum::Extension(self.service.clone()));
        let listener = tokio::net::TcpListener::bind((BRIDGE_IP, self.port))
            .await
            .expect("rebind the same bridge port");
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());
        self._shutdown_tx = shutdown_tx;
        let server = tokio::spawn(async move {
            let _ = serve_tls(listener, https, router, shutdown_rx).await;
        });
        self.server = Some(server);
    }

    async fn active_agent(&self) -> Option<MonitoringAgentRow> {
        self.repo.find_active_agent_by_vm(VM_ID).await.unwrap()
    }

    async fn guest_points(&self, metric_id: &str) -> usize {
        let now = now_ms() as u64;
        self.store
            .query_history(
                &chv_monitoring_core::model::TargetKind::Vm,
                VM_ID,
                &[metric_id.to_string()],
                None,
                now.saturating_sub(3_600_000),
                now + 3_600_000,
                1000,
                chv_monitoring_store::Resolution::Raw,
            )
            .await
            .unwrap()
            .iter()
            .map(|s| {
                s.points
                    .iter()
                    .filter(|p| p.quality == chv_monitoring_core::model::SampleQuality::Valid)
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

/// The guest agent's config, verbatim what the package documents
/// (`docs/examples/monitor-agent.toml` shape) — 5 s interval so the
/// lifecycle phases fit in evidence time.
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
"#,
        url = manager.base_url()
    )
}

/// cloud-config executed by cloud-init inside the guest. Everything
/// comes from the NoCloud seed (no network fetch needed for the
/// install): the deb, the config, the manager CA and the one-time
/// claim. Order matters — dpkg first (creates the chv-monitor user
/// and the conffile), then overwrite the conffile with the real
/// config (a pre-existing /etc/chv-monitor/agent.toml would trip
/// dpkg's conffile prompt), then place the claim with the agent's
/// ownership, then start the service.
fn guest_userdata() -> String {
    r#"#cloud-config
runcmd:
  - mkdir -p /media/cidata
  - mount -o ro "$(findfs LABEL=cidata)" /media/cidata
  - dpkg -i /media/cidata/chv-monitor-agent.deb
  - install -m 0644 /media/cidata/agent.toml /etc/chv-monitor/agent.toml
  - install -m 0644 /media/cidata/manager-ca.pem /etc/chv-monitor/manager-ca.pem
  - install -o chv-monitor -g chv-monitor -m 0600 /media/cidata/claim /var/lib/chv-monitor/claim
  - systemctl enable --now chv-monitor-agent
"#
    .to_string()
}

/// Enrich the production-built NoCloud seed with the agent package
/// and its inputs. The production adapter writes user-data, meta-data
/// and network-config into `<vm_dir>/seed/` and builds `seed.iso`
/// from exactly those three files; this re-runs the same genisoimage
/// invocation with the extra files added, atomically replacing the
/// ISO between `create_vm` and `start_vm` (the VMM reads disk images
/// at boot). Cloud-init ignores the extra files; runcmd mounts the
/// seed and installs from it.
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
    let output = tokio::process::Command::new("genisoimage")
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
        .output()
        .await
        .expect("run genisoimage for the enriched seed");
    assert!(
        output.status.success(),
        "genisoimage failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::rename(&iso_new, &iso).expect("atomically replace the seed ISO");
    checkpoint("seed enriched with the agent package and inputs");
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
            panic!("g3: timed out waiting for {what} (budget {budget:?})");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

#[tokio::test]
async fn g3_real_vm_guest_agent_enrolls_collects_and_revokes() {
    let Ok(vmm_binary) = std::env::var("CHV_G1_VMM_BINARY") else {
        eprintln!("skipping: CHV_G1_VMM_BINARY not set (real-KVM evidence test)");
        return;
    };
    let firmware =
        std::env::var("CHV_G1_FIRMWARE").expect("CHV_G1_FIRMWARE with CHV_G1_VMM_BINARY");
    let image = std::env::var("CHV_G1_IMAGE").expect("CHV_G1_IMAGE with CHV_G1_VMM_BINARY");
    let Ok(agent_deb) = std::env::var("CHV_G3_AGENT_DEB") else {
        eprintln!("skipping: CHV_G3_AGENT_DEB not set (build the guest package first)");
        return;
    };
    let agent_deb = PathBuf::from(agent_deb);
    assert!(
        agent_deb.is_file(),
        "CHV_G3_AGENT_DEB={:?} is not a file",
        agent_deb
    );
    if !is_root_with_ip() {
        eprintln!("skipping: bridge setup needs root + iproute2 (test-side topology)");
        return;
    }

    let mut cleanup = HostCleanup::new();
    setup_network();
    checkpoint("network up (bridge + tap)");

    let mut manager = Manager::new().await;
    checkpoint(format!("manager listening on {}", manager.base_url()).as_str());

    // The claim is issued BEFORE the VM exists: it is embedded in the
    // seed, so its TTL covers boot + cloud-init + install.
    let claim = manager
        .service
        .issue_claim(VM_ID, "g3-evidence", now_ms())
        .await
        .expect("issue the one-time claim");

    // The guest agent install WRITES to the guest rootfs (dpkg,
    // systemd enable) — but the pinned qualification image is never
    // written (G0b/G1/G2 discipline). A full sparse copy gives the
    // VMM a writable boot disk: every guest write lands in the
    // throwaway copy, the pinned bytes are only ever read. (A qcow2
    // overlay is NOT an option: cloud-hypervisor refuses backing
    // files — "Maximum disk nesting depth exceeded".)
    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let root_disk = dir.path().join("g3-root.qcow2");
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

    // The qualification shape (G0b/G1/G2): 2 vCPU, 512 MiB, firmware
    // boot — plus this rig's writable root overlay (see above), the
    // NIC (the production seed builder turns it into cloud-init
    // static network-config) and the cloud-config userdata.
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
            network_id: "g3-evidence".to_string(),
            mac_address: GUEST_MAC.to_string(),
            ip_address: GUEST_IP.to_string(),
            tap_name: TAP.to_string(),
            cidr: "192.168.62.0/24".to_string(),
            gateway: BRIDGE_IP.to_string(),
        }],
        api_socket_path: vm_dir.join("vm.sock"),
        cloud_init_userdata: Some(guest_userdata()),
        hypervisor_overrides: None,
    };

    checkpoint("creating vm (production adapter, seed built)");
    runtime
        .create_vm(VM_ID, "g3-1", &config, None)
        .await
        .expect("production create_vm builds the cloud-init seed");
    enrich_seed(&vm_dir, &manager, &agent_deb, &claim.token).await;
    runtime
        .start_vm(VM_ID, None)
        .await
        .expect("production start_vm boots the enriched seed");

    // `vm.boot` returning success is NOT proof the guest executes (a
    // payload-load failure can leave a zombie "Running" state — found
    // the hard way with a rejected overlay disk). CPU time is: a
    // booting firmware/kernel burns it immediately.
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
        checkpoint(&format!("guest executing (vmm cpu ticks +{boot_ticks})"));
    }
    checkpoint("vm booted; waiting for cloud-init to install and the agent to enroll");

    // Phase 1 — enrollment: the packaged agent in the real guest
    // redeems the one-time claim over mTLS through the bridge. The
    // wait dumps the guest console tail every ~60 s so a stuck boot,
    // a failed cloud-init step, or an unreachable manager is visible
    // in the test output, not just in a post-mortem.
    let mut poll_count: u32 = 0;
    let agent = wait_until(
        "the guest agent to enroll (cloud-init + dpkg + systemd + mTLS)",
        Duration::from_secs(420),
        || {
            // Mutate the FnMut capture in the closure body (never
            // inside the returned async block — a &mut capture cannot
            // escape into the future).
            poll_count += 1;
            if poll_count % 30 == 0 {
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
    assert_eq!(agent.vm_id, VM_ID, "the credential is scoped to this VM");
    assert_eq!(agent.status, "active");

    // Phase 2 — real guest telemetry: baseline collectors inside the
    // real guest, delivered over the enrolled credential.
    for metric in [
        "vm.guest.load1",
        "vm.guest.uptime_seconds",
        "vm.guest.cpu.utilization_ratio",
        "vm.memory.guest_available_bytes",
    ] {
        let points = wait_until(
            &format!("valid {metric} points from inside the guest"),
            Duration::from_secs(120),
            || async {
                let n = manager.guest_points(metric).await;
                (n > 0).then_some(n)
            },
        )
        .await;
        assert!(points > 0, "{metric} has valid points");
        checkpoint(&format!("{metric}: {points} valid points"));
    }
    let agent = manager.active_agent().await.expect("agent row");
    assert!(
        agent
            .os_name
            .as_deref()
            .is_some_and(|n| n.contains("Ubuntu")),
        "OS identity from inside the guest: {:?}",
        agent.os_name
    );
    assert!(agent.os_kernel_release.is_some(), "kernel release recorded");
    assert!(
        agent.last_seen_at_ms.is_some() && agent.last_sequence.is_some(),
        "ingestion updates the liveness fields"
    );

    // Phase 3 — outage durability: the manager listener goes away;
    // the guest agent must spool on disk and drain on reconnect. The
    // sequence high-water is the durable proof (point counts collapse
    // under query bucketing; the registry high-water does not lie).
    let pre_outage = manager.active_agent().await.unwrap().last_sequence.unwrap();
    manager.stop_listener().await;
    checkpoint("manager outage begins");
    tokio::time::sleep(Duration::from_secs(12)).await;
    let during_outage = manager.active_agent().await.unwrap().last_sequence.unwrap();
    assert_eq!(
        during_outage, pre_outage,
        "no ingestion while the manager is down"
    );
    manager.restart_listener().await;
    checkpoint("manager back; waiting for the spool to drain");
    let drained = wait_until(
        "the spool to drain past the outage gap (>= 2 spooled batches)",
        Duration::from_secs(180),
        || async {
            manager
                .active_agent()
                .await
                .and_then(|a| a.last_sequence)
                .and_then(|seq| (seq >= pre_outage + 2).then_some(seq))
        },
    )
    .await;
    checkpoint(&format!(
        "spool drained: sequence {pre_outage} -> {drained}"
    ));

    // Phase 4 — revocation: the operator revokes the credential;
    // reporting must stop and stay stopped.
    manager
        .repo
        .revoke_agent(&agent.agent_id, "g3-evidence", now_ms())
        .await
        .expect("revoke the agent");
    checkpoint("agent revoked; waiting for the block to settle");
    tokio::time::sleep(Duration::from_secs(15)).await;
    let (seen_at, seq_at) = {
        let row = manager
            .repo
            .find_agent(&agent.agent_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.status, "revoked");
        (row.last_seen_at_ms.unwrap(), row.last_sequence.unwrap())
    };
    tokio::time::sleep(Duration::from_secs(15)).await;
    let row = manager
        .repo
        .find_agent(&agent.agent_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.status, "revoked");
    assert_eq!(
        row.last_seen_at_ms,
        Some(seen_at),
        "last_seen frozen after revocation"
    );
    assert_eq!(
        row.last_sequence,
        Some(seq_at),
        "no batch is accepted after revocation"
    );
    checkpoint("revocation blocks reporting, liveness frozen");

    // Success path cleanup: graceful VM stop + delete, then the Drop
    // guard removes the topology (and dumps nothing — no failure).
    let _ = runtime.stop_vm(VM_ID, false, None).await;
    let _ = runtime.delete_vm(VM_ID, None).await;
    checkpoint("vm stopped and deleted");
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
    // Bridge/tap setup needs CAP_NET_ADMIN (the rig runs as root on
    // the evidence host); `ip` comes from iproute2.
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
