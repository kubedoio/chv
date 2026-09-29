use async_trait::async_trait;
use chv_errors::ChvError;
use std::collections::HashMap;
use std::io::{Read as _, Seek, SeekFrom};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::process::Child;
use tracing::{debug, info, warn};

use crate::core_runtime::is_safe_resource_id;
use chv_hypervisor_api::resources::{vm_config_file, vm_pid_file};

/// RAII guard that records VM lifecycle RED metrics when it drops.
///
/// - `chv_vm_ops_total{op, result}` counter — `result` is `"ok"` or `"err"`
/// - `chv_vm_op_duration_seconds{op}` histogram (recorded on BOTH paths so
///   dashboards can visualise error latency)
///
/// Mark `succeeded = true` only when the operation returns `Ok`. Any early
/// return via `?` will trigger Drop with the default `succeeded = false`.
struct VmOpGuard {
    op: &'static str,
    start: std::time::Instant,
    succeeded: bool,
}

impl VmOpGuard {
    fn new(op: &'static str) -> Self {
        Self {
            op,
            start: std::time::Instant::now(),
            succeeded: false,
        }
    }
}

impl Drop for VmOpGuard {
    fn drop(&mut self) {
        let label = if self.succeeded { "ok" } else { "err" };
        metrics::counter!(
            chv_observability::CHV_VM_OPS_TOTAL,
            "op" => self.op,
            "result" => label
        )
        .increment(1);
        metrics::histogram!(
            chv_observability::CHV_VM_OP_DURATION_SECONDS,
            "op" => self.op
        )
        .record(self.start.elapsed().as_secs_f64());
    }
}

use crate::adapter::{
    AddDiskParams, AddNetParams, CloudHypervisorAdapter, VmConfig, VmCounters, VmInfo,
};
use crate::ch_api::{parse_http_status, CloudHypervisorApiClient};

const CONSOLE_SCROLLBACK_BYTES: usize = 256 * 1024;
const CONSOLE_LOG_MAX_BYTES: u64 = 10 * 1024 * 1024;

struct AliveGuard(Arc<AtomicBool>);

impl Drop for AliveGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// RAII guard closing the orphan window between spawning a CH child and
/// registering it in the vm process map. Several awaits separate those two
/// points; if an async cancellation (e.g. the core executor's bounded-drain
/// abort) drops the future in between, an *armed* guard SIGKILLs the child so
/// no unaccounted VMM survives. The guard is disarmed exactly when the child is
/// handed to the vm process map, which owns lifecycle from then on (stop/delete
/// call `start_kill` explicitly, and teardown deliberately leaves running VMs
/// untouched).
struct ChildGuard {
    child: Option<Child>,
    armed: bool,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self {
            child: Some(child),
            armed: true,
        }
    }

    /// Hand the child to its long-lived owner. From here an ordinary drop must
    /// NOT kill the VMM, so disarm before moving the child out of the guard.
    fn disarm(mut self) -> Child {
        debug_assert!(self.armed);
        self.armed = false;
        self.child.take().expect("child is present while armed")
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.armed {
            if let Some(mut child) = self.child.take() {
                let _ = child.start_kill();
            }
        }
    }
}

impl std::ops::Deref for ChildGuard {
    type Target = Child;
    fn deref(&self) -> &Child {
        self.child.as_ref().expect("child is present until disarm")
    }
}

impl std::ops::DerefMut for ChildGuard {
    fn deref_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("child is present until disarm")
    }
}

/// How the guest serial console is attached. Kept alongside the connected
/// fd so lifecycle paths can distinguish "reconnect to the listener" from
/// "re-dup the pty slave" when the console broadcaster needs reviving.
#[derive(Clone, Debug)]
enum SerialTransport {
    /// Unix-stream client of cloud-hypervisor's serial listener at this
    /// path. cloud-hypervisor keeps the listener for the VMM process
    /// lifetime, so a lost connection can be re-established by
    /// reconnecting (and the stored fd swapped for the live one).
    Socket(std::path::PathBuf),
    /// Pty slave allocated by cloud-hypervisor (explicit tuning). The
    /// kernel keeps the pty pair alive across guest reboots; consumers
    /// re-dup the stored slave fd.
    Pty,
}

/// The cloud-hypervisor process backing a tracked VM.
///
/// Normally the agent owns the process end-to-end (`Owned`). After an
/// agent restart, a still-running VMM is a re-parented orphan with no
/// `Child` handle: it is tracked by pid (`Adopted`) and every signal is
/// guarded by a `/proc/<pid>/cmdline` re-validation against the VM's
/// api-socket path so a recycled pid can never be killed by mistake.
/// A dead VMM is not an error state: cloud-hypervisor v43 exits with
/// the guest (the VMM control loop's Exit dispatch runs `vmm_shutdown`),
/// so a stopped VM's entry legitimately points at an exited process —
/// `start_vm` re-spawns it from the persisted creation payload.
enum VmmChild {
    Owned(Child),
    Adopted(u32),
}

/// Deterministic liveness for the re-spawn decision: `prove_exited`
/// distinguishes a proven-exited process from a proven-live one instead of
/// `has_exited`'s safe-but-lossy "errors mean gone" default — a re-spawn
/// may never run while any doubt remains that the old VMM is dead, or two
/// VMMs would own one VM (and one disk).
enum Liveness {
    Alive,
    Exited,
    Unknown(String),
}

impl VmmChild {
    /// Best-effort SIGKILL. For an adopted pid this re-validates the
    /// process identity first and refuses to signal a mismatching (e.g.
    /// recycled) pid rather than risk killing an unrelated process. An
    /// `Owned` child is our own spawn — no identity doubt, `expected_exe`
    /// is unused for it.
    fn kill(&mut self, api_socket: &Path, expected_exe: Option<&std::ffi::OsStr>) {
        match self {
            VmmChild::Owned(child) => {
                let _ = child.start_kill();
            }
            VmmChild::Adopted(pid) => {
                if !pid_is_cloud_hypervisor(*pid, api_socket, expected_exe) {
                    warn!(
                        pid = pid,
                        socket = %api_socket.display(),
                        "refusing to signal adopted pid: identity mismatch or process gone"
                    );
                    return;
                }
                let _ = nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(*pid as i32),
                    nix::sys::signal::Signal::SIGKILL,
                );
            }
        }
    }

    /// Waits for the process to be gone. `Owned` reaps the child; an
    /// adopted orphan is parented to init and reaped there, so this only
    /// polls `/proc` (bounded — a SIGKILLed VMM disappears promptly, and
    /// a hung wait must not pin the caller forever).
    async fn wait(&mut self) {
        match self {
            VmmChild::Owned(child) => {
                let _ = child.wait().await;
            }
            VmmChild::Adopted(pid) => {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while std::time::Instant::now() < deadline {
                    if !pid_exists(*pid) {
                        return;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            }
        }
    }

    /// Strict liveness for the re-spawn decision. Unlike a lossy
    /// "errors mean gone" default, probe errors surface as `Unknown`:
    /// `start_vm` refuses to re-spawn on `Unknown` rather than risk a
    /// second VMM on the same disk. An `Adopted` pid that is alive but no
    /// longer proves our identity (recycled) counts as exited — it is not
    /// ours to boot against, and re-spawn takes over the runtime dir.
    fn prove_exited(&mut self, api_socket: &Path) -> Liveness {
        match self {
            VmmChild::Owned(child) => match child.try_wait() {
                Ok(Some(_)) => Liveness::Exited,
                Ok(None) => Liveness::Alive,
                Err(e) => Liveness::Unknown(e.to_string()),
            },
            VmmChild::Adopted(pid) => {
                if !pid_exists(*pid) {
                    Liveness::Exited
                } else if pid_is_cloud_hypervisor(*pid, api_socket, None) {
                    Liveness::Alive
                } else {
                    Liveness::Exited
                }
            }
        }
    }
}

/// Reads `/proc/<pid>/cmdline` (NUL-separated) as a space-joined string.
fn proc_cmdline(pid: u32) -> Option<String> {
    std::fs::read(format!("/proc/{pid}/cmdline"))
        .ok()
        .map(|raw| String::from_utf8_lossy(&raw).replace('\0', " "))
}

/// Whether `/proc/<pid>` shows a live process — liveness for adopted
/// orphans. A zombie or dead process has already exited; it only lingers
/// in /proc until its parent reaps it, so it counts as gone. Malformed
/// stat data also counts as gone (the safe default for lifecycle paths).
fn pid_exists(pid: u32) -> bool {
    let stat = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat,
        Err(_) => return false,
    };
    // The state field follows the comm field in parentheses, and comm can
    // itself contain spaces and parentheses — parse after the LAST ')'.
    let Some((_, tail)) = stat.rsplit_once(')') else {
        return false;
    };
    match tail.split_whitespace().next() {
        Some(state) => state != "Z" && state != "X",
        None => false,
    }
}

/// Identity check for an adopted VMM pid: the process must exist, its
/// command line must reference both the api-socket flag and this VM's
/// socket path (unique per VM), and — when an expected executable name is
/// given — `/proc/<pid>/exe` must resolve to it. The exe cross-check
/// closes the argv-spoofing hole on the SIGKILL authorization path: a
/// crafted argv alone must never be enough to be signalled. Failure to
/// read either `/proc` file counts as unproven identity (false).
fn pid_is_cloud_hypervisor(
    pid: u32,
    api_socket: &Path,
    expected_exe: Option<&std::ffi::OsStr>,
) -> bool {
    let Some(cmdline) = proc_cmdline(pid) else {
        return false;
    };
    let socket_path = api_socket.to_string_lossy();
    if !(cmdline.contains("--api-socket") && cmdline.contains(socket_path.as_ref())) {
        return false;
    }
    if let Some(expected) = expected_exe {
        match std::fs::read_link(format!("/proc/{pid}/exe")) {
            Ok(exe) => {
                if exe.file_name() != Some(expected) {
                    return false;
                }
            }
            Err(_) => return false,
        }
    }
    true
}

struct VmProcess {
    api_socket: std::path::PathBuf,
    child: VmmChild,
    /// Guest serial-console I/O fd — a unix-stream socket connected to
    /// cloud-hypervisor's serial listener (default Socket transport) or the
    /// pty slave under explicit Pty tuning. Guest output is read (via the
    /// broadcaster) and user keystrokes written through it.
    console_io: OwnedFd,
    /// Which transport `console_io` speaks; drives respawn semantics.
    serial_transport: SerialTransport,
    pty_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
    pty_scrollback: Arc<tokio::sync::RwLock<Vec<u8>>>,
    broadcaster_alive: Arc<AtomicBool>,
    last_cpu_seconds: f64,
    last_cpu_at: Option<std::time::Instant>,
}

pub struct ProcessCloudHypervisorAdapter {
    chv_binary: std::path::PathBuf,
    vms: Arc<tokio::sync::RwLock<HashMap<String, VmProcess>>>,
    /// Per-VM lifecycle serialization. Core-managed mode already
    /// single-flights operations per VM in the executor, but legacy-mode
    /// callers (gRPC handlers, the reconciler) do not, and even core mode
    /// must survive a rare liveness-probe error without forking a VM:
    /// two concurrent lifecycle ops on one VM (notably two `start`s over
    /// an exited VMM) must never both re-spawn a process for it. Entries
    /// are never removed — dropping a key while an operation still holds
    /// its mutex would let a fresh key bypass that holder's serialization.
    lifecycle_locks: std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// One-way latch set by [`Self::drain_and_close_consoles`] during
    /// graceful agent shutdown. Console healing (the broadcaster's
    /// reconnect loop and `respawn_broadcaster_if_dead`) consults it and
    /// stands down: a connection minted during shutdown would be
    /// abortively closed by process exit — unread receive-queue data
    /// makes the kernel reset the connection, which kills cloud-
    /// hypervisor v43's serial-manager thread and can freeze the guest
    /// (see `drain_and_close_consoles` for the full defect chain). Never
    /// cleared: shutdown is one-way.
    console_draining: Arc<AtomicBool>,
}

/// Console.log write mode. `Fresh` (create) truncates — a new VM
/// instance starts a clean log. `Append` (VMM re-spawn, adoption)
/// preserves prior history: the stop path already truncates by design,
/// so appending after a stop starts clean, while a crash leaves boot
/// history worth keeping.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ConsoleLogMode {
    Fresh,
    Append,
}

impl ProcessCloudHypervisorAdapter {
    pub fn new(chv_binary: impl Into<std::path::PathBuf>) -> Self {
        Self {
            chv_binary: chv_binary.into(),
            vms: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            lifecycle_locks: std::sync::Mutex::new(HashMap::new()),
            console_draining: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The per-VM lifecycle mutex (see `lifecycle_locks`). Lifecycle ops
    /// hold it for their whole duration; see the field doc for why keys
    /// are never removed.
    fn vm_op_lock(&self, vm_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self
            .lifecycle_locks
            .lock()
            .expect("lifecycle lock map poisoned");
        locks
            .entry(vm_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// The executable name this adapter's VMMs must prove when an adopted
    /// pid is identity-checked (see `pid_is_cloud_hypervisor`).
    fn expected_vmm_exe(&self) -> Option<&std::ffi::OsStr> {
        self.chv_binary.file_name()
    }

    /// Refuses a create that would run a second VMM for a VM id whose
    /// runtime dir already hosts a live one — unlinking its sockets and
    /// spawning another process would fork the VM (two VMMs, one disk).
    /// Liveness is proven two ways, covering both a pidfile-identifiable
    /// VMM (the normal post-restart case: legacy-mode reconcilers
    /// re-create desired VMs after an agent restart while the adopted
    /// orphan still runs) and a pidfile-less VMM whose api socket still
    /// answers.
    async fn ensure_no_live_vmm(&self, config: &VmConfig) -> Result<(), ChvError> {
        if let Some(vm_dir) = config.api_socket_path.parent() {
            if let Ok(raw) = std::fs::read_to_string(vm_pid_file(vm_dir)) {
                if let Ok(pid) = raw.trim().parse::<u32>() {
                    if pid_is_cloud_hypervisor(
                        pid,
                        &config.api_socket_path,
                        self.expected_vmm_exe(),
                    ) {
                        return Err(ChvError::Internal {
                            reason: format!(
                                "cannot create vm {}: a cloud-hypervisor for this vm is already running (pid {pid}); stop and delete it first (an agent restart re-adopts it)",
                                config.vm_id
                            ),
                        });
                    }
                }
            }
        }
        if config.api_socket_path.exists() {
            // The socket file exists. A stale file (SIGKILLed VMM) must be
            // removable or the new bind fails — but a LIVE socket answers
            // vm.info, meaning a VMM without a pidfile is running: refuse
            // rather than fork it.
            if Self::ch_api_request(&config.api_socket_path, "GET", "/api/v1/vm.info", None)
                .await
                .is_ok()
            {
                return Err(ChvError::Internal {
                    reason: format!(
                        "cannot create vm {}: its api socket {} is answered by a running cloud-hypervisor (no pidfile); stop and delete it first",
                        config.vm_id,
                        config.api_socket_path.display()
                    ),
                });
            }
        }
        Ok(())
    }

    async fn wait_for_socket(socket: &Path, timeout: std::time::Duration) -> Result<(), ChvError> {
        let start = std::time::Instant::now();
        loop {
            if socket.exists() {
                return Ok(());
            }
            if start.elapsed() >= timeout {
                return Err(ChvError::Internal {
                    reason: format!("CH api socket did not appear: {}", socket.display()),
                });
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    async fn get_vm_socket(&self, vm_id: &str) -> Result<std::path::PathBuf, ChvError> {
        let vms = self.vms.read().await;
        let proc = vms.get(vm_id).ok_or_else(|| ChvError::NotFound {
            resource: "vm".to_string(),
            id: vm_id.to_string(),
        })?;
        Ok(proc.api_socket.clone())
    }

    fn expect_status(status: u16, endpoint: &str) -> Result<(), ChvError> {
        if status != 200 && status != 204 {
            return Err(ChvError::Internal {
                reason: format!("{} returned unexpected status {}", endpoint, status),
            });
        }
        Ok(())
    }

    async fn ch_api_request(
        socket: &Path,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<u16, ChvError> {
        CloudHypervisorApiClient::default()
            .request(socket, method, path, body)
            .await
    }

    async fn ch_api_request_with_body(
        socket: &Path,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<(u16, String), ChvError> {
        CloudHypervisorApiClient::default()
            .request_with_body(socket, method, path, body)
            .await
    }

    fn validate_vm_config(&self, config: &VmConfig) -> Result<(), ChvError> {
        if !self.chv_binary.exists() {
            return Err(ChvError::InvalidArgument {
                field: "chv_binary_path".to_string(),
                reason: format!("binary not found: {}", self.chv_binary.display()),
            });
        }
        if config.api_socket_path.as_os_str().is_empty() {
            return Err(ChvError::InvalidArgument {
                field: "api_socket_path".to_string(),
                reason: "api_socket_path is empty".to_string(),
            });
        }
        if config.api_socket_path.parent().is_none() {
            return Err(ChvError::InvalidArgument {
                field: "api_socket_path".to_string(),
                reason: format!(
                    "api_socket_path has no parent directory: {}",
                    config.api_socket_path.display()
                ),
            });
        }
        if let Some(ref fw) = config.firmware_path {
            if !fw.exists() {
                return Err(ChvError::InvalidArgument {
                    field: "firmware_path".to_string(),
                    reason: format!("firmware not found: {}", fw.display()),
                });
            }
        } else if !config.kernel_path.exists() {
            return Err(ChvError::InvalidArgument {
                field: "kernel_path".to_string(),
                reason: format!("kernel not found: {}", config.kernel_path.display()),
            });
        }
        for disk in &config.disks {
            if !disk.path.exists() {
                return Err(ChvError::InvalidArgument {
                    field: "disk_path".to_string(),
                    reason: format!("disk not found: {}", disk.path.display()),
                });
            }
        }
        Ok(())
    }
}

fn read_stderr_tail(path: &std::path::Path) -> String {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let read_from = len.saturating_sub(4096);
    if read_from > 0 {
        let _ = file.seek(SeekFrom::Start(read_from));
    }
    let mut buf = Vec::new();
    let _ = file.read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf);
    // Return all non-empty lines from the tail, joined, so we see the full
    // error context instead of just the last line (which is often "try --help").
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return "<empty>".to_string();
    }
    lines.join(" | ")
}

fn build_cpus_config(config: &VmConfig) -> serde_json::Value {
    let hv = config.hypervisor_overrides.as_ref();
    let mut cpus = serde_json::json!({
        "boot_vcpus": config.cpus,
        "max_vcpus": config.cpus,
    });
    if let Some(true) = hv.and_then(|h| h.cpu_amx) {
        cpus["features"] = serde_json::json!({ "amx": true });
    }
    if let Some(true) = hv.and_then(|h| h.cpu_nested) {
        cpus["topology"] = serde_json::json!({
            "threads_per_core": 1,
            "cores_per_die": config.cpus,
            "dies_per_package": 1,
            "packages": 1,
        });
    }
    cpus
}

async fn build_cloud_init_seed(
    vm_dir: &Path,
    vm_id: &str,
    userdata: Option<&str>,
    nics: &[crate::adapter::VmNicConfig],
) -> Result<std::path::PathBuf, ChvError> {
    let seed_dir = vm_dir.join("seed");
    tokio::fs::create_dir_all(&seed_dir)
        .await
        .map_err(|e| ChvError::Io {
            path: seed_dir.to_string_lossy().to_string(),
            source: e,
        })?;

    let meta_data = format!("instance-id: {}\nlocal-hostname: {}\n", vm_id, vm_id);
    tokio::fs::write(seed_dir.join("meta-data"), meta_data.as_bytes())
        .await
        .map_err(|e| ChvError::Io {
            path: seed_dir.join("meta-data").to_string_lossy().to_string(),
            source: e,
        })?;

    let default_userdata = "#cloud-config\nusers:\n  - name: ubuntu\n    sudo: ALL=(ALL) NOPASSWD:ALL\n    lock_passwd: true\n    ssh_authorized_keys: []\nssh_pwauth: false\n";
    let user_data = userdata.unwrap_or(default_userdata);
    tokio::fs::write(seed_dir.join("user-data"), user_data.as_bytes())
        .await
        .map_err(|e| ChvError::Io {
            path: seed_dir.join("user-data").to_string_lossy().to_string(),
            source: e,
        })?;

    // Generate network-config v2 with MAC-matched static IPs from the control plane IPAM.
    let mut network_config = String::from("version: 2\nethernets:\n");
    for (idx, nic) in nics.iter().enumerate() {
        if nic.mac_address.is_empty() || nic.ip_address.is_empty() {
            continue;
        }
        let prefix = nic.cidr.split_once('/').map(|(_, p)| p).unwrap_or("24");
        let gateway = if nic.gateway.is_empty() {
            let parts: Vec<&str> = nic.ip_address.split('.').collect();
            if parts.len() == 4 {
                format!("{}.{}.{}.1", parts[0], parts[1], parts[2])
            } else {
                String::new()
            }
        } else {
            nic.gateway.clone()
        };

        let iface_name = format!("id{}", idx);
        network_config.push_str(&format!(
            "  {}:\n    match:\n      macaddress: \"{}\"\n    dhcp4: false\n    addresses:\n      - {}/{}\n",
            iface_name, nic.mac_address, nic.ip_address, prefix
        ));
        if !gateway.is_empty() {
            network_config.push_str(&format!(
                "    routes:\n      - to: default\n        via: {}\n",
                gateway
            ));
        }
    }

    if network_config.lines().count() <= 2 {
        // No valid NICs with IPAM data — fallback to DHCP with virtio matching.
        network_config.push_str("  id0:\n    match:\n      driver: virtio*\n    dhcp4: true\n");
    }

    tokio::fs::write(seed_dir.join("network-config"), network_config.as_bytes())
        .await
        .map_err(|e| ChvError::Io {
            path: seed_dir
                .join("network-config")
                .to_string_lossy()
                .to_string(),
            source: e,
        })?;

    let seed_iso = vm_dir.join("seed.iso");
    let output = run_genisoimage(&seed_iso, &seed_dir).await?;

    if !output.status.success() {
        return Err(ChvError::Internal {
            reason: format!(
                "genisoimage failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    Ok(seed_iso)
}

/// Runs `genisoimage` to build the cloud-init seed ISO. Resolved through
/// PATH first; if PATH lookup reports NotFound, falls back to the canonical
/// install location (`install.sh` and the systemd unit guarantee /usr/bin,
/// so this covers a stripped environment such as a minimal container).
async fn run_genisoimage(
    seed_iso: &std::path::Path,
    seed_dir: &std::path::Path,
) -> Result<std::process::Output, ChvError> {
    let command = |binary: &str| {
        let mut command = tokio::process::Command::new(binary);
        command
            .arg("-output")
            .arg(seed_iso)
            .arg("-volid")
            .arg("cidata")
            .arg("-joliet")
            .arg("-rock")
            .arg(seed_dir.join("user-data"))
            .arg(seed_dir.join("meta-data"))
            .arg(seed_dir.join("network-config"));
        command
    };
    match command("genisoimage").output().await {
        Ok(output) => Ok(output),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            command("/usr/bin/genisoimage")
                .output()
                .await
                .map_err(|e| ChvError::Internal {
                    reason: format!("failed to run genisoimage: {}", e),
                })
        }
        Err(error) => Err(ChvError::Internal {
            reason: format!("failed to run genisoimage: {}", error),
        }),
    }
}

/// Borrowed console fan-out pair (broadcast channel + scrollback) used
/// by the serial abandonment paths to preserve drained guest output.
type ConsoleFanout<'a> = (
    &'a tokio::sync::broadcast::Sender<Vec<u8>>,
    &'a Arc<tokio::sync::RwLock<Vec<u8>>>,
);

impl ProcessCloudHypervisorAdapter {
    /// How long a graceful stop waits, after pressing the ACPI power
    /// button (`vm.power-button`), for the guest OS to power itself off
    /// before falling back to a force kill.
    ///
    /// Measured on the pinned noble guest image (M2.5 qualification,
    /// isolation experiment 2026-09-28): a healthy, fully-booted guest
    /// needs ~32 s from button press to poweroff — `snapd.service` alone
    /// stops for 28-31 s — and a button pressed before `systemd-logind`
    /// has subscribed to the input device is silently dropped by the
    /// guest (no shutdown begins at all). The previous 10 s window
    /// therefore force-killed routine guest shutdowns mid-flight, which
    /// removed the runtime map entry and console.log and cascaded into
    /// spurious start failures after an otherwise successful stop. 60 s
    /// gives ~2x headroom over the measured shutdown while still
    /// bounding the worst case for a wedged guest.
    const GRACEFUL_STOP_WINDOW: std::time::Duration = std::time::Duration::from_secs(60);

    /// Builds the cloud-hypervisor serial-console config object for the
    /// VM creation payload. `Socket` (the default transport) points the
    /// serial console at a unix-stream listener inside the VM runtime
    /// directory, which cloud-hypervisor binds while creating the VM and
    /// the agent connects to right after; every other mode is passed
    /// through as the bare mode string, matching the cloud-hypervisor
    /// REST schema (`ConsoleConfig`).
    fn serial_console_config(serial_mode: &str, socket_path: &Path) -> serde_json::Value {
        if serial_mode == chv_common::hypervisor::DEFAULT_SERIAL_MODE {
            serde_json::json!({
                "mode": "Socket",
                "socket": socket_path.to_string_lossy(),
            })
        } else {
            serde_json::json!({ "mode": serial_mode })
        }
    }

    /// Connects once to cloud-hypervisor's serial listener and returns a
    /// blocking, FD_CLOEXEC fd. Fails if the listener is unreachable or
    /// the peer-identity check fails.
    async fn connect_serial_socket_once(socket_path: &Path) -> std::io::Result<OwnedFd> {
        let stream = tokio::net::UnixStream::connect(socket_path).await?;
        let std_stream = stream.into_std()?;
        // tokio connects in non-blocking mode; restore blocking
        // mode so the broadcaster's and console server's plain
        // reads/writes block instead of failing with EAGAIN.
        if let Ok(flags) = nix::fcntl::fcntl(&std_stream, nix::fcntl::F_GETFL) {
            let blocking =
                nix::fcntl::OFlag::from_bits_retain(flags) & !nix::fcntl::OFlag::O_NONBLOCK;
            if let Err(e) = nix::fcntl::fcntl(&std_stream, nix::fcntl::F_SETFL(blocking)) {
                // A fd left non-blocking makes every "blocking" console
                // read spin on EAGAIN; not fatal (connect still
                // succeeded), but it must be visible in the logs.
                warn!(
                    error = %e,
                    "failed to restore blocking mode on the serial socket"
                );
            }
        }
        if let Err(e) = nix::fcntl::fcntl(
            &std_stream,
            nix::fcntl::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
        ) {
            // Without FD_CLOEXEC this descriptor leaks into every
            // subsequently spawned cloud-hypervisor child.
            warn!(
                error = %e,
                "failed to set FD_CLOEXEC on the serial socket"
            );
        }
        // Defense-in-depth peer-identity check, mirroring this crate's
        // trust-walk discipline: the listener at this path was bound by
        // the cloud-hypervisor child this agent spawned — the same
        // effective uid — and the kernel reports the listener owner's
        // credentials to a connecting client via SO_PEERCRED. Anything
        // else answering on this path means the socket is not ours; fail
        // closed rather than pipe a stranger's bytes into the guest
        // console.
        match nix::sys::socket::getsockopt(&std_stream, nix::sys::socket::sockopt::PeerCredentials)
        {
            Ok(creds) if creds.uid() == nix::unistd::geteuid().as_raw() => {}
            other => {
                return Err(std::io::Error::other(format!(
                    "serial socket peer identity mismatch (peer={other:?}, expected euid {})",
                    nix::unistd::geteuid()
                )));
            }
        }
        Ok(std_stream.into())
    }

    /// Connects to the guest serial-console unix socket (Socket
    /// transport). cloud-hypervisor binds the listener while handling
    /// `vm.create`, so the socket exists by the time this runs; the
    /// bounded retry only guards a slow bind racing the connect. The fd is
    /// returned in blocking mode with FD_CLOEXEC set — the console
    /// broadcaster and console server use blocking reads and writes on it.
    /// On failure the spawned child is killed: a console-less VM is never
    /// left running.
    async fn connect_serial_socket(
        socket_path: &Path,
        child: &mut Child,
        vm_id: &str,
        operation_id: Option<&str>,
    ) -> Result<OwnedFd, ChvError> {
        let path_display = socket_path.to_string_lossy().to_string();
        let mut last_err = None;
        for attempt in 0..10u32 {
            match Self::connect_serial_socket_once(socket_path).await {
                Ok(fd) => {
                    info!(
                        vm_id = vm_id,
                        socket = %path_display,
                        op = operation_id.unwrap_or("-"),
                        "chv serial socket connected"
                    );
                    return Ok(fd);
                }
                Err(e) => {
                    last_err = Some(e);
                    if attempt < 9 {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
        }
        let _ = child.start_kill();
        let _ = child.wait().await;
        Err(ChvError::Io {
            path: path_display,
            source: last_err.unwrap_or_else(|| std::io::Error::other("connect failed")),
        })
    }

    /// Reconnects to the guest serial-console listener for an already
    /// running VM (broadcaster respawn): the stored connection may have
    /// been closed by the VMM side, so a fresh one is established. No
    /// child to fail closed on — the caller degrades gracefully (console
    /// capture stays down, retried on the next start_vm). Checks the
    /// `draining` latch on every attempt and gives up immediately once
    /// graceful agent shutdown has begun: a connection minted during
    /// shutdown would be abortively closed by process exit (see
    /// [`Self::drain_and_close_consoles`]).
    async fn reconnect_serial_socket(socket_path: &Path, draining: &AtomicBool) -> Option<OwnedFd> {
        for attempt in 0..10u32 {
            if draining.load(Ordering::SeqCst) {
                return None;
            }
            if let Ok(fd) = Self::connect_serial_socket_once(socket_path).await {
                return Some(fd);
            }
            if attempt < 9 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
        None
    }

    /// Reads a serial-socket descriptor's kernel receive queue until it
    /// would block and returns everything drained, restoring the
    /// descriptor's prior status flags. The fd is forced non-blocking for
    /// the drain so it cannot block on a stream that is still receiving
    /// data. Bytes read here are removed from the queue — which is what
    /// makes the descriptor's eventual close a clean FIN instead of an
    /// RST (the kernel resets a unix-stream connection on close when its
    /// receive queue still holds unread data).
    fn drain_serial_receive_queue(fd: &OwnedFd) -> Vec<u8> {
        let saved = nix::fcntl::fcntl(fd, nix::fcntl::F_GETFL)
            .ok()
            .and_then(nix::fcntl::OFlag::from_bits);
        if let Some(saved) = saved {
            let _ = nix::fcntl::fcntl(
                fd,
                nix::fcntl::F_SETFL(saved | nix::fcntl::OFlag::O_NONBLOCK),
            );
        }
        let mut drained = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            match nix::unistd::read(fd, &mut buf) {
                Ok(0) => break, // peer closed its write side
                Ok(n) => drained.extend_from_slice(&buf[..n]),
                Err(_) => break, // EAGAIN (queue empty) or peer gone
            }
        }
        if let Some(saved) = saved {
            let _ = nix::fcntl::fcntl(fd, nix::fcntl::F_SETFL(saved));
        }
        drained
    }

    /// Cleanly abandons a live serial-console connection the agent has
    /// decided not to keep (heal superseded by a map replacement, failed
    /// dup, VM leaving the map). Consumes and closes the descriptor.
    ///
    /// Why this exists: cloud-hypervisor v43's socket-serial path
    /// mishandles an *abortively* closed client. If the client's receive
    /// queue still holds unread guest output when its descriptor closes,
    /// the kernel resets the connection; the serial-manager thread then
    /// dies silently on the ECONNRESET, `out` keeps pointing at the dead
    /// socket, every guest UART byte's write fails EPIPE and skips the
    /// THRE interrupt, and the guest's interrupt-driven tty tx stalls —
    /// a frozen guest while `vm.info` keeps reporting Running (root
    /// cause proven in the M2.5 qualification, isolation experiments
    /// e9-kernel/e9-quiet). A *clean* close is handled correctly: the
    /// manager logs "Remote end closed serial socket", detaches the
    /// client, the guest is unaffected, and the listener keeps
    /// accepting. This function therefore takes exactly the clean path:
    ///
    /// 1. half-close the write side (`shutdown(SHUT_WR)`) — the manager
    ///    observes the same clean EOF a well-behaved client produces and
    ///    detaches, which also stops it writing guest output into the
    ///    connection;
    /// 2. after a settle delay, drain the receive queue (forwarding any
    ///    already-buffered guest output into the console fan-out when
    ///    one is given, so console.log keeps the evidence);
    /// 3. drop the descriptor — with an empty receive queue the close
    ///    is a FIN, not an RST.
    ///
    /// `shutdown(SHUT_RD)` is deliberately NOT used anywhere on a live
    /// connection: it makes the peer's writes fail EPIPE while the
    /// connection is still open, which is itself a freeze trigger on
    /// v43 (the skipped-THRE path above).
    async fn abandon_serial_connection(
        vm_id: &str,
        fd: OwnedFd,
        fanout: Option<ConsoleFanout<'_>>,
    ) {
        if let Err(e) =
            nix::sys::socket::shutdown(fd.as_raw_fd(), nix::sys::socket::Shutdown::Write)
        {
            debug!(vm_id = %vm_id, error = %e, "serial half-close failed (peer already gone?)");
        }
        // Settle: give the serial manager a moment to observe the EOF
        // and stop writing guest output into the connection, so the
        // drain below converges to an empty queue.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let drained = Self::drain_serial_receive_queue(&fd);
        if drained.is_empty() {
            return;
        }
        if let Some((tx, scrollback)) = fanout {
            let mut sb = scrollback.write().await;
            sb.extend_from_slice(&drained);
            if sb.len() > CONSOLE_SCROLLBACK_BYTES {
                let excess = sb.len() - CONSOLE_SCROLLBACK_BYTES;
                sb.drain(0..excess);
            }
            drop(sb);
            let _ = tx.send(drained.clone());
        }
        info!(
            vm_id = %vm_id,
            bytes = drained.len(),
            "drained buffered console output from an abandoned serial connection"
        );
    }

    /// Graceful-shutdown hook: converts the agent's process exit from an
    /// abortive close of every live serial-console connection into the
    /// clean close cloud-hypervisor v43 handles correctly (see
    /// [`Self::abandon_serial_connection`] for the defect chain — a
    /// client closing with unread receive-queue data resets the
    /// connection, silently kills the serial-manager thread, and can
    /// freeze a still-running guest; VMs outlive the agent).
    ///
    /// Call this as the LAST act of a graceful shutdown — after the
    /// core owner and supervisor shutdown, so the executor's bounded
    /// drain (up to 60 s of still-running lifecycle ops and console
    /// traffic) has finished first. Idempotent: the multiple signal and
    /// error exit paths can each call it.
    ///
    /// 1. latch `console_draining` — console healing stands down instead
    ///    of minting connections that process exit would abortively
    ///    close;
    /// 2. half-close every live Socket-transport connection — each
    ///    serial manager observes the clean EOF it handles correctly and
    ///    detaches, which also stops it writing guest output into the
    ///    connection;
    /// 3. after a settle delay, drain each receive queue, forwarding the
    ///    buffered guest output into the console fan-out (scrollback +
    ///    console.log) so the descriptors' close at process exit is a
    ///    FIN, not an RST;
    /// 4. yield once more so any healer that was mid-reconnect when the
    ///    latch was set can acquire the (now released) vm-map lock, see
    ///    the latch, and cleanly abandon its fresh connection before
    ///    the process goes away.
    ///
    /// Residual risk, documented: guest output landing in a receive
    /// queue between the drain and the process-exit close (a
    /// microsecond-scale window on a loaded host), or a healer starved
    /// past the final yield. A hit degrades to the pre-fix behavior —
    /// recoverable by `vm.reboot`, which re-creates the serial manager.
    pub async fn drain_and_close_consoles(&self) {
        if self.console_draining.swap(true, Ordering::SeqCst) {
            return; // a prior exit path already drained
        }
        let mut socket_vm_ids: Vec<String> = Vec::new();
        {
            // Hold the vm-map write lock across both passes: lifecycle
            // endpoint swaps and the broadcaster heal's swap are blocked
            // behind it, and the heal re-checks the draining latch after
            // acquiring it. (Lock order vms -> pty_scrollback matches
            // every other holder.)
            let mut vms = self.vms.write().await;
            for (vm_id, proc) in vms.iter() {
                if !matches!(proc.serial_transport, SerialTransport::Socket(_)) {
                    continue;
                }
                socket_vm_ids.push(vm_id.clone());
                if let Err(e) = nix::sys::socket::shutdown(
                    proc.console_io.as_raw_fd(),
                    nix::sys::socket::Shutdown::Write,
                ) {
                    debug!(vm_id = %vm_id, error = %e, "console half-close failed (VMM gone?)");
                }
            }
            if !socket_vm_ids.is_empty() {
                info!(
                    vms = socket_vm_ids.len(),
                    "draining serial-console connections for agent shutdown"
                );
            }
            // Settle: let every serial manager observe the EOF, detach
            // its client, and stop writing guest output into the
            // connection — the drain below then converges to an empty
            // queue that stays empty.
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            for vm_id in &socket_vm_ids {
                let Some(proc) = vms.get_mut(vm_id) else {
                    continue;
                };
                let drained = Self::drain_serial_receive_queue(&proc.console_io);
                if drained.is_empty() {
                    continue;
                }
                let mut sb = proc.pty_scrollback.write().await;
                sb.extend_from_slice(&drained);
                if sb.len() > CONSOLE_SCROLLBACK_BYTES {
                    let excess = sb.len() - CONSOLE_SCROLLBACK_BYTES;
                    sb.drain(0..excess);
                }
                drop(sb);
                let _ = proc.pty_tx.send(drained.clone());
                info!(
                    vm_id = %vm_id,
                    bytes = drained.len(),
                    "drained buffered console output during agent shutdown"
                );
            }
        }
        // Final yield: healers blocked on the vm-map lock when the drain
        // began release here, observe the latch, and cleanly abandon
        // their fresh connection. Without this yield the process could
        // exit first and abortively close that connection — recreating
        // the very defect this hook exists to prevent.
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }

    /// Duplicates a console fd for a secondary consumer (broadcaster,
    /// console server). The dup inherits the source fd's blocking mode —
    /// a property of the open file description — while FD_CLOEXEC is
    /// per-descriptor and must be set on each dup.
    fn dup_cloexec(fd: &OwnedFd) -> std::io::Result<OwnedFd> {
        let dup = nix::unistd::dup(fd).map_err(|e| std::io::Error::other(e.to_string()))?;
        if let Err(e) = nix::fcntl::fcntl(&dup, nix::fcntl::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC))
        {
            // FD_CLOEXEC is per-descriptor; without it this dup leaks
            // into every subsequently spawned cloud-hypervisor child.
            // Not fatal, but it must be visible in the logs.
            warn!(error = %e, "failed to set FD_CLOEXEC on a console dup");
        }
        Ok(dup)
    }

    /// Revives the console broadcaster if it died while the VM was
    /// running (outer safety net; the broadcaster itself self-heals
    /// across VMM-side connection cycles — see `spawn_pty_broadcaster`).
    /// Obtains a FRESH endpoint, not a dup of the stored fd: for
    /// the Socket transport the stored connection may itself be dead (the
    /// VMM side can close it across serial reconfiguration or shutdown
    /// cycles), and a broadcaster on a dup of a dead connection dies
    /// instantly — silent, unrecoverable console loss. Socket transport
    /// reconnects and swaps the stored connection for the live one; the
    /// pty slave survives guest reboots, so it is re-dup'ed. A failed
    /// reconnect is non-fatal: console capture stays down and is retried
    /// on the next start_vm.
    async fn respawn_broadcaster_if_dead(
        &self,
        vm_id: &str,
        serial_transport: &SerialTransport,
        pty_tx: &tokio::sync::broadcast::Sender<Vec<u8>>,
        pty_scrollback: &Arc<tokio::sync::RwLock<Vec<u8>>>,
        broadcaster_alive: &Arc<AtomicBool>,
    ) {
        if broadcaster_alive.load(Ordering::SeqCst) {
            return;
        }
        // Graceful agent shutdown: never mint a fresh connection — process
        // exit would abortively close it (see `drain_and_close_consoles`).
        if self.console_draining.load(Ordering::SeqCst) {
            return;
        }
        info!(vm_id = %vm_id, "respawning console broadcaster");
        let broadcaster_fd: Option<OwnedFd> = match serial_transport {
            SerialTransport::Socket(path) => {
                match Self::reconnect_serial_socket(path, &self.console_draining).await {
                    Some(fresh) => {
                        let dup = match Self::dup_cloexec(&fresh) {
                            Ok(d) => d,
                            Err(e) => {
                                warn!(
                                    vm_id = %vm_id,
                                    error = %e,
                                    "serial respawn dup failed; abandoning the fresh connection"
                                );
                                Self::abandon_serial_connection(vm_id, fresh, None).await;
                                return;
                            }
                        };
                        let mut vms = self.vms.write().await;
                        // Post-lock double check of the shutdown latch
                        // (mirrors the broadcaster heal): the drain hook
                        // holds this lock across its passes — if it
                        // latched while this respawn was reconnecting,
                        // abandon the fresh connection cleanly and exit
                        // rather than let process exit abortively close
                        // it (an abortive close resets the connection
                        // and can kill the serial manager).
                        if self.console_draining.load(Ordering::SeqCst) {
                            drop(vms);
                            drop(dup);
                            Self::abandon_serial_connection(vm_id, fresh, None).await;
                            return;
                        }
                        match vms.get_mut(vm_id) {
                            // Supersede check (mirrors the broadcaster
                            // heal): if the entry was replaced — fresh
                            // channel from a VMM re-spawn or re-adoption —
                            // a replacement broadcaster owns the console
                            // now; swapping our endpoint under it would
                            // split the byte stream between two readers of
                            // one socket. Abandon the fresh connection
                            // cleanly instead of dropping it with
                            // possibly-unread data (an abortive close
                            // resets the connection and can kill the
                            // serial manager).
                            Some(proc) if proc.pty_tx.same_channel(pty_tx) => {
                                proc.console_io = fresh;
                                Some(dup)
                            }
                            superseded => {
                                let fanout = superseded
                                    .map(|proc| (proc.pty_tx.clone(), proc.pty_scrollback.clone()));
                                drop(vms);
                                drop(dup);
                                match fanout {
                                    Some((tx, sb)) => {
                                        Self::abandon_serial_connection(
                                            vm_id,
                                            fresh,
                                            Some((&tx, &sb)),
                                        )
                                        .await;
                                    }
                                    None => {
                                        Self::abandon_serial_connection(vm_id, fresh, None).await;
                                    }
                                }
                                None
                            }
                        }
                    }
                    None => {
                        warn!(
                            vm_id = %vm_id,
                            socket = %path.display(),
                            "serial socket reconnect failed; console capture stays down until the next start_vm"
                        );
                        None
                    }
                }
            }
            SerialTransport::Pty => {
                let vms = self.vms.read().await;
                vms.get(vm_id)
                    .and_then(|proc| Self::dup_cloexec(&proc.console_io).ok())
            }
        };
        if let Some(broadcaster_fd) = broadcaster_fd {
            broadcaster_alive.store(true, Ordering::SeqCst);
            Self::spawn_pty_broadcaster(
                self.vms.clone(),
                vm_id.to_string(),
                broadcaster_fd,
                serial_transport.clone(),
                pty_tx.clone(),
                pty_scrollback.clone(),
                broadcaster_alive.clone(),
                self.console_draining.clone(),
            );
        }
    }

    /// Opens the pty slave cloud-hypervisor allocated for the guest serial
    /// console (Pty transport, explicit tuning only). The slave path comes
    /// from `vm.info`; the pty is set to raw mode so the host line
    /// discipline does not buffer or echo keystrokes. On failure the
    /// spawned child is killed.
    async fn open_serial_pty(
        api_socket_path: &Path,
        child: &mut Child,
        vm_id: &str,
        operation_id: Option<&str>,
    ) -> Result<OwnedFd, ChvError> {
        // Query CHV for the PTY slave path it created for the serial device.
        let (info_status, info_body) =
            Self::ch_api_request_with_body(api_socket_path, "GET", "/api/v1/vm.info", None).await?;
        let slave_path = if info_status == 200 {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&info_body) {
                v.pointer("/config/serial/file")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        } else {
            None
        };
        let slave_path = match slave_path {
            Some(path) => path,
            None => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(ChvError::Internal {
                    reason: format!("vm.info did not return serial PTY path for vm {}", vm_id),
                });
            }
        };

        // Open the PTY slave that CHV created.  This is the I/O endpoint
        // for the guest serial console — we read guest output from it and
        // write user keystrokes to it.
        let pty_slave = match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(nix::libc::O_NOCTTY)
            .open(&slave_path)
        {
            Ok(slave) => slave,
            Err(e) => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(ChvError::Io {
                    path: slave_path.clone(),
                    source: e,
                });
            }
        };
        // Set raw mode so the host line discipline doesn't buffer or echo
        // keystrokes — we want every byte forwarded to the guest immediately.
        if let Ok(mut term) = nix::sys::termios::tcgetattr(&pty_slave) {
            nix::sys::termios::cfmakeraw(&mut term);
            let _ =
                nix::sys::termios::tcsetattr(&pty_slave, nix::sys::termios::SetArg::TCSANOW, &term);
        }
        let pty_master: OwnedFd = pty_slave.into();
        let _ = nix::fcntl::fcntl(
            &pty_master,
            nix::fcntl::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
        );

        info!(
            vm_id = vm_id,
            pty = %slave_path,
            op = operation_id.unwrap_or("-"),
            "chv serial pty ready"
        );
        Ok(pty_master)
    }

    /// Spawns the console.log persistence task: subscribes to the VM's
    /// console fan-out channel and writes output to `log_path`, capped at
    /// [`CONSOLE_LOG_MAX_BYTES`] with wraparound (in-memory scrollback keeps
    /// the recent tail either way). The task ends when every sender for the
    /// channel drops — i.e. when the VM entry (and its broadcaster) are
    /// replaced or removed.
    fn spawn_console_log_writer(
        vm_id: &str,
        pty_tx: &tokio::sync::broadcast::Sender<Vec<u8>>,
        log_path: std::path::PathBuf,
        mode: ConsoleLogMode,
    ) {
        let vm_id_log = vm_id.to_string();
        let mut pty_rx_log = pty_tx.subscribe();
        tokio::spawn(async move {
            let mut options = tokio::fs::OpenOptions::new();
            options.create(true);
            match mode {
                ConsoleLogMode::Fresh => {
                    options.truncate(true).write(true);
                }
                ConsoleLogMode::Append => {
                    options.append(true);
                }
            }
            let log_file = options.open(&log_path).await;
            let mut writer = match log_file {
                Ok(f) => f,
                Err(e) => {
                    tracing::debug!(vm_id = %vm_id_log, error = %e, "failed to open console.log");
                    return;
                }
            };
            let mut written: u64 = 0;
            loop {
                match pty_rx_log.recv().await {
                    Ok(data) => {
                        if written + data.len() as u64 > CONSOLE_LOG_MAX_BYTES {
                            // Safety cap: truncate and continue. In-memory scrollback
                            // preserves the last 256 KiB so recent output is still
                            // visible via WebSocket.
                            let _ = writer.set_len(0).await;
                            let _ = writer.seek(SeekFrom::Start(0)).await;
                            written = 0;
                        }
                        if writer.write_all(&data).await.is_ok() {
                            written += data.len() as u64;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // If lagged, just continue reading
                    }
                }
            }
        });
    }

    /// Spawns the console broadcaster for a VM: reads guest serial output
    /// from the console fd and fans it out to the scrollback buffer and
    /// subscribers. For the Socket transport the broadcaster is
    /// SELF-HEALING: whenever the current connection ends it reconnects
    /// to the listener at the same path, swaps the stored `console_io`
    /// for the live endpoint, and keeps streaming. Connection cycles
    /// arise in two ways: (a) the cloud-hypervisor process dies — every
    /// descriptor it held closes and the reader observes a genuine EOF;
    /// (b) `vm.reboot` tears the VM down and re-creates it, re-binding
    /// the serial listener at the same path — v43's serial manager leaks
    /// the accepted descriptor at thread exit (it is handed to epoll via
    /// `into_raw_fd()` and never closed), so the old connection delivers
    /// neither data nor EOF and `reboot_vm` force-rotates it with
    /// `shutdown(SHUT_RD)` to produce the EOF this loop reacts to. The
    /// VMM-side connection replacement also plays a role: the serial
    /// manager shuts its previous client down when a new one connects, so
    /// reconnecting clients are always the live endpoint. Console capture
    /// therefore survives reboots and VMM death without lifecycle-path
    /// coupling. The Pty transport exits on EOF instead (a reboot
    /// allocates a NEW pty at a different path; re-attachment is a
    /// recorded follow-up) and revival is left to the next `start_vm`
    /// respawn.
    #[allow(clippy::too_many_arguments)]
    fn spawn_pty_broadcaster(
        vms: Arc<tokio::sync::RwLock<HashMap<String, VmProcess>>>,
        vm_id: String,
        pty_fd: OwnedFd,
        transport: SerialTransport,
        pty_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
        pty_scrollback: Arc<tokio::sync::RwLock<Vec<u8>>>,
        broadcaster_alive: Arc<AtomicBool>,
        console_draining: Arc<AtomicBool>,
    ) {
        tokio::spawn(async move {
            let _guard = AliveGuard(broadcaster_alive);
            // The canonical console endpoint this broadcaster reads from;
            // the heal path replaces it with the fresh connection.
            let mut current = pty_fd;
            loop {
                // Each connection cycle gets its own dup of `current` for
                // its reader; dropping the cycle's reader closes that dup
                // (and with it the old connection) when the cycle ends.
                let cycle_fd = match Self::dup_cloexec(&current) {
                    Ok(d) => d,
                    Err(_) => break,
                };
                let std_file = std::fs::File::from(cycle_fd);
                let mut reader = tokio::io::BufReader::new(tokio::fs::File::from_std(std_file));
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf).await {
                        Ok(0) | Err(_) => break, // endpoint lost
                        Ok(n) => {
                            let data = buf[..n].to_vec();
                            {
                                let mut sb = pty_scrollback.write().await;
                                sb.extend_from_slice(&data);
                                if sb.len() > CONSOLE_SCROLLBACK_BYTES {
                                    let excess = sb.len() - CONSOLE_SCROLLBACK_BYTES;
                                    sb.drain(0..excess);
                                }
                            }
                            let _ = pty_tx.send(data);
                        }
                    }
                }
                // Endpoint lost. Socket transport: re-establish the
                // connection (bounded retry — the VMM re-binds the
                // listener within the same API call that closed the old
                // one), swap the stored endpoint, and keep streaming.
                // Anything else (reconnect exhausted, VM left the map,
                // Pty transport) ends the broadcaster.
                match transport {
                    SerialTransport::Socket(ref path) => {
                        // Graceful agent shutdown: stand down instead of
                        // minting a connection that process exit would
                        // abortively close (see `drain_and_close_consoles`).
                        if console_draining.load(Ordering::SeqCst) {
                            break;
                        }
                        match Self::reconnect_serial_socket(path, &console_draining).await {
                            Some(fresh) => {
                                let next = match Self::dup_cloexec(&fresh) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        warn!(
                                            vm_id = %vm_id,
                                            error = %e,
                                            "serial heal dup failed; abandoning the fresh connection"
                                        );
                                        Self::abandon_serial_connection(&vm_id, fresh, None).await;
                                        break;
                                    }
                                };
                                let mut map = vms.write().await;
                                // Post-lock double check of the shutdown
                                // latch: the drain hook holds this lock
                                // across its passes — if it latched while
                                // this heal was connecting, abandon the
                                // fresh connection cleanly and exit rather
                                // than let process exit abortively close
                                // it.
                                if console_draining.load(Ordering::SeqCst) {
                                    drop(map);
                                    drop(next);
                                    Self::abandon_serial_connection(&vm_id, fresh, None).await;
                                    break;
                                }
                                match map.get_mut(&vm_id) {
                                    Some(proc) => {
                                        // Supersede check: a VMM re-spawn or
                                        // re-adoption replaces the map entry
                                        // (fresh channel, fresh broadcaster).
                                        // If this entry's channel is no
                                        // longer ours, a replacement
                                        // broadcaster owns the console now —
                                        // exit instead of swapping endpoints
                                        // under it. This also drops the last
                                        // Sender of the old channel, so the
                                        // old console.log writer observes
                                        // Closed and exits (no duplicate
                                        // appends). The fresh connection is
                                        // abandoned CLEANLY — dropping it
                                        // with unread receive-queue data
                                        // would reset the connection and
                                        // can kill the (new) serial manager
                                        // (see `abandon_serial_connection`).
                                        if !proc.pty_tx.same_channel(&pty_tx) {
                                            let (tx, sb) =
                                                (proc.pty_tx.clone(), proc.pty_scrollback.clone());
                                            drop(map);
                                            drop(next);
                                            Self::abandon_serial_connection(
                                                &vm_id,
                                                fresh,
                                                Some((&tx, &sb)),
                                            )
                                            .await;
                                            break;
                                        }
                                        proc.console_io = fresh;
                                    }
                                    None => {
                                        // The VM left the map (force stop /
                                        // delete) — the VMM is being torn
                                        // down, but abandon cleanly anyway:
                                        // the same listener can already
                                        // belong to a replacement VM.
                                        drop(map);
                                        drop(next);
                                        Self::abandon_serial_connection(&vm_id, fresh, None).await;
                                        break;
                                    }
                                }
                                drop(map);
                                current = next;
                                continue;
                            }
                            None => break,
                        }
                    }
                    SerialTransport::Pty => break,
                }
            }
            tracing::debug!(vm_id = %vm_id, "pty broadcaster exited");
        });
    }

    /// Re-spawns the cloud-hypervisor process for a tracked VM whose VMM
    /// has exited, then re-creates and boots the VM from the payload
    /// persisted at create time. cloud-hypervisor v43 exits with the
    /// guest (VMM control-loop Exit dispatch → `vmm_shutdown`), so a
    /// start after a graceful stop has no daemon to `vm.boot` against —
    /// the start contract ("make the VM running") is honored by
    /// rebuilding the process. Mirrors `create_vm`'s post-spawn tail:
    /// fail-closed child guard, fresh console endpoint, fresh fan-out
    /// channel; console.log is APPENDED (the stop path already truncated
    /// it by design, and a crash leaves boot history worth keeping).
    async fn respawn_vmm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let (api_socket, serial_transport) = {
            let vms = self.vms.read().await;
            let proc = vms.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            (proc.api_socket.clone(), proc.serial_transport.clone())
        };
        let vm_dir = api_socket
            .parent()
            .expect("api socket path must have a parent directory")
            .to_path_buf();

        // Stale sockets from an unclean VMM death would fail both binds
        // (api socket, serial listener). Only ever files this adapter
        // owns.
        let _ = tokio::fs::remove_file(&api_socket).await;
        let _ = tokio::fs::remove_file(vm_dir.join("serial.sock")).await;

        let config_path = vm_config_file(&vm_dir);
        let body = tokio::fs::read_to_string(&config_path).await.map_err(|e| {
            ChvError::Internal {
                reason: format!(
                    "cannot restart vm {}: no persisted cloud-hypervisor config at {} ({}); re-create the VM",
                    vm_id,
                    config_path.display(),
                    e
                ),
            }
        })?;

        let mut cmd = tokio::process::Command::new(&self.chv_binary);
        cmd.arg("--api-socket").arg(&api_socket);
        cmd.stdout(Stdio::null());
        let stderr_log_path = vm_dir.join("cloud-hypervisor.stderr.log");
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stderr_log_path)
        {
            Ok(f) => {
                cmd.stderr(Stdio::from(f));
            }
            Err(_) => {
                cmd.stderr(Stdio::null());
            }
        }

        info!(
            vm_id = %vm_id,
            socket = %api_socket.display(),
            binary = %self.chv_binary.display(),
            op = operation_id.unwrap_or("-"),
            "re-spawning cloud-hypervisor for stopped vm"
        );
        let child = cmd.spawn().map_err(|e| ChvError::Io {
            path: self.chv_binary.to_string_lossy().to_string(),
            source: e,
        })?;
        // Same cancellation discipline as create_vm: the armed guard
        // SIGKILLs the child if this future is dropped before the map
        // hand-off.
        let mut child = ChildGuard::new(child);
        if let Some(pid) = child.id() {
            let _ = std::fs::write(vm_pid_file(&vm_dir), format!("{pid}"));
        }

        if let Err(e) = Self::wait_for_socket(&api_socket, std::time::Duration::from_secs(10)).await
        {
            let _ = child.start_kill();
            let _ = child.wait().await;
            let stderr_hint = read_stderr_tail(&stderr_log_path);
            return Err(ChvError::Internal {
                reason: format!(
                    "failed to re-start cloud-hypervisor for vm {}: {} stderr: {}",
                    vm_id, e, stderr_hint
                ),
            });
        }
        if let Ok(Some(exit_status)) = child.try_wait() {
            let stderr_hint = read_stderr_tail(&stderr_log_path);
            return Err(ChvError::Internal {
                reason: format!(
                    "cloud-hypervisor exited immediately for vm {} re-spawn with {}: {}",
                    vm_id, exit_status, stderr_hint
                ),
            });
        }

        let (create_status, create_body) =
            Self::ch_api_request_with_body(&api_socket, "PUT", "/api/v1/vm.create", Some(&body))
                .await?;
        if create_status != 200 && create_status != 204 {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(ChvError::Internal {
                reason: format!(
                    "vm.create returned status {} for vm {} re-spawn: {}",
                    create_status, vm_id, create_body
                ),
            });
        }

        // Fresh console endpoint on the new VMM. Socket transport
        // reconnects to the listener vm.create just bound; Pty transport
        // opens the new pty slave reported by vm.info. Both fail closed
        // (the child is killed) exactly like the create path.
        let console_io: OwnedFd = match &serial_transport {
            SerialTransport::Socket(path) => {
                Self::connect_serial_socket(path, &mut child, vm_id, operation_id).await?
            }
            SerialTransport::Pty => {
                Self::open_serial_pty(&api_socket, &mut child, vm_id, operation_id).await?
            }
        };

        let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
        let pty_scrollback = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let broadcaster_fd = Self::dup_cloexec(&console_io).ok();
        // Claim capture alive only when a broadcaster will actually run.
        let broadcaster_alive = Arc::new(AtomicBool::new(broadcaster_fd.is_some()));

        // Hand-off under the write lock with no intervening await, then
        // swap the entry: the replaced entry's fan-out channel closes
        // (the old console.log writer and any lingering broadcaster exit),
        // and lifecycle ownership moves to the new child.
        let mut map = self.vms.write().await;
        let child = child.disarm();
        map.insert(
            vm_id.to_string(),
            VmProcess {
                api_socket: api_socket.clone(),
                child: VmmChild::Owned(child),
                console_io,
                serial_transport: serial_transport.clone(),
                pty_tx: pty_tx.clone(),
                pty_scrollback: pty_scrollback.clone(),
                broadcaster_alive: broadcaster_alive.clone(),
                last_cpu_seconds: 0.0,
                last_cpu_at: None,
            },
        );
        drop(map);

        if let Some(broadcaster_fd) = broadcaster_fd {
            Self::spawn_pty_broadcaster(
                self.vms.clone(),
                vm_id.to_string(),
                broadcaster_fd,
                serial_transport,
                pty_tx.clone(),
                pty_scrollback.clone(),
                broadcaster_alive,
                self.console_draining.clone(),
            );
        }
        Self::spawn_console_log_writer(
            vm_id,
            &pty_tx,
            vm_dir.join("console.log"),
            ConsoleLogMode::Append,
        );

        let status = Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.boot", None).await?;
        if status != 200 && status != 204 {
            warn!(vm_id = %vm_id, status = status, "vm.boot returned non-success after re-spawn (VM may have auto-booted)");
        }
        Ok(())
    }
}

/// What `start_vm` should do for a VM whose `vm.info` reported `state`.
///
/// Both `Running` and `Paused` map to an idempotent no-op: a paused VM is
/// deliberately left paused. The migration path owns resume-after-pause (see
/// `PausedVmGuard` in chv-agent-core), and user-initiated pauses must survive
/// the reconciler's periodic `start_vm` calls.
enum StartVmAction {
    /// VM is already running (or deliberately paused): idempotent no-op.
    AlreadyRunning,
    /// Any other state: boot the VM.
    Boot,
}

fn start_vm_action(state: &str) -> StartVmAction {
    match state {
        "Running" | "Paused" => StartVmAction::AlreadyRunning,
        _ => StartVmAction::Boot,
    }
}

#[async_trait]
impl CloudHypervisorAdapter for ProcessCloudHypervisorAdapter {
    async fn create_vm(
        &self,
        config: &VmConfig,
        operation_id: Option<&str>,
    ) -> Result<String, ChvError> {
        let mut __guard = VmOpGuard::new("create");
        // Serialize with the other lifecycle ops for this VM (see
        // `lifecycle_locks`): a concurrent stop/delete/start while this
        // create runs must not interleave process spawns and socket
        // cleanups for the same runtime dir.
        let _lifecycle = self.vm_op_lock(&config.vm_id).lock_owned().await;
        if !std::path::Path::new("/dev/kvm").exists() {
            return Err(ChvError::Internal {
                reason: "Host does not have KVM capability (/dev/kvm missing). VMs require hardware virtualization.".into(),
            });
        }
        self.validate_vm_config(config)?;
        self.ensure_no_live_vmm(config).await?;
        if config.api_socket_path.exists() {
            tokio::fs::remove_file(&config.api_socket_path)
                .await
                .map_err(|e| ChvError::Io {
                    path: config.api_socket_path.to_string_lossy().to_string(),
                    source: e,
                })?;
        }
        // A SIGKILLed VMM leaves its serial socket file behind (clean exit
        // removes it); a stale file would make cloud-hypervisor's
        // `vm.create` listener bind fail with EADDRINUSE. Best-effort: the
        // file can only exist when no live VMM owns it.
        if let Some(vm_dir) = config.api_socket_path.parent() {
            let _ = tokio::fs::remove_file(vm_dir.join("serial.sock")).await;
        }

        let mut cmd = tokio::process::Command::new(&self.chv_binary);
        cmd.arg("--api-socket").arg(&config.api_socket_path);

        let vm_runtime_dir = config
            .api_socket_path
            .parent()
            .expect("api_socket_path must have a parent directory");

        // Ensure the VM runtime directory exists so CHV can create its API socket
        // and we can capture stderr logs even when cloud-init seeding is skipped.
        if let Err(e) = tokio::fs::create_dir_all(vm_runtime_dir).await {
            warn!(vm_id = %config.vm_id, error = %e, path = %vm_runtime_dir.display(), "failed to create vm runtime dir");
        }

        // Build cloud-init seed ISO for every VM that has NICs or explicit userdata.
        // The ISO carries the control-plane-assigned static IP configuration so
        // cloud-init images (e.g. Ubuntu cloud images) come up on the correct network.
        let has_nics = !config.nics.is_empty();
        let has_userdata = config
            .cloud_init_userdata
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        let seed_iso_path = if has_nics || has_userdata {
            let userdata = config.cloud_init_userdata.as_deref();
            match build_cloud_init_seed(vm_runtime_dir, &config.vm_id, userdata, &config.nics).await
            {
                Ok(path) => Some(path),
                Err(e) => {
                    warn!(vm_id = %config.vm_id, error = %e, "failed to build cloud-init seed ISO, continuing without it");
                    None
                }
            }
        } else {
            None
        };

        cmd.stdout(Stdio::null());

        let stderr_log_path = vm_runtime_dir.join("cloud-hypervisor.stderr.log");
        let stderr_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stderr_log_path);
        match stderr_file {
            Ok(f) => {
                cmd.stderr(Stdio::from(f));
            }
            Err(e) => {
                warn!(error = %e, path = %stderr_log_path.display(), "failed to open stderr log, falling back to null");
                cmd.stderr(Stdio::null());
            }
        }

        // Build VM config JSON and create VM via REST API (supports multiple disks)
        let mut disks_json = Vec::new();
        for disk in &config.disks {
            let mut disk_obj = serde_json::Map::new();
            disk_obj.insert(
                "path".into(),
                serde_json::Value::from(disk.path.to_string_lossy().to_string()),
            );
            if disk.read_only {
                disk_obj.insert("readonly".into(), serde_json::Value::from(true));
            }
            let image_type = if disk.path.extension().map(|e| e == "qcow2").unwrap_or(false) {
                "Qcow2"
            } else {
                "Raw"
            };
            disk_obj.insert("image_type".into(), serde_json::Value::from(image_type));
            disks_json.push(serde_json::Value::Object(disk_obj));
        }
        if let Some(ref seed_path) = seed_iso_path {
            let mut disk_obj = serde_json::Map::new();
            disk_obj.insert(
                "path".into(),
                serde_json::Value::from(seed_path.to_string_lossy().to_string()),
            );
            disk_obj.insert("readonly".into(), serde_json::Value::from(true));
            disk_obj.insert("image_type".into(), serde_json::Value::from("Raw"));
            disks_json.push(serde_json::Value::Object(disk_obj));
        }

        let mut net_json = Vec::new();
        for nic in &config.nics {
            if nic.tap_name.is_empty() {
                warn!(mac = %nic.mac_address, "skipping NIC with empty tap_name");
                continue;
            }
            let mut net_obj = serde_json::Map::new();
            net_obj.insert(
                "mac".into(),
                serde_json::Value::from(nic.mac_address.clone()),
            );
            net_obj.insert("tap".into(), serde_json::Value::from(nic.tap_name.clone()));
            net_json.push(serde_json::Value::Object(net_obj));
        }

        let mut payload = serde_json::Map::new();
        if let Some(ref fw) = config.firmware_path {
            payload.insert(
                "firmware".into(),
                serde_json::Value::from(fw.to_string_lossy().to_string()),
            );
        } else {
            payload.insert(
                "kernel".into(),
                serde_json::Value::from(config.kernel_path.to_string_lossy().to_string()),
            );
        }

        let hv = config.hypervisor_overrides.as_ref();

        let cpus = build_cpus_config(config);

        let mut memory = serde_json::json!({ "size": config.memory_bytes });
        if let Some(v) = hv.and_then(|h| h.memory_mergeable) {
            memory["mergeable"] = serde_json::json!(v);
        }
        if let Some(v) = hv.and_then(|h| h.memory_hugepages) {
            memory["hugepages"] = serde_json::json!(v);
        }
        if let Some(v) = hv.and_then(|h| h.memory_shared) {
            memory["shared"] = serde_json::json!(v);
        }
        if let Some(v) = hv.and_then(|h| h.memory_prefault) {
            memory["prefault"] = serde_json::json!(v);
        }

        // Serial transport: cloud-hypervisor v43 gates Pty-mode output
        // until input arrives on the pty (vmm/src/serial_manager.rs starts
        // the serial output gate closed and only an epoll input event on
        // the pty master opens it), so a passive consumer — exactly what
        // the agent's console capture is — receives nothing and console.log
        // stays empty forever. Socket mode streams output to a connected
        // unix-stream client with no gate, so it is the default transport
        // (DEFAULT_SERIAL_MODE, shared with the control plane's global
        // hypervisor settings); an explicit serial_mode tuning is still
        // honored (with the Pty gate semantics that mode implies).
        let serial_mode = hv
            .and_then(|h| h.serial_mode.as_deref())
            .unwrap_or(chv_common::hypervisor::DEFAULT_SERIAL_MODE);
        let console_mode = hv.and_then(|h| h.console_mode.as_deref()).unwrap_or("Off");
        let serial_socket_path = vm_runtime_dir.join("serial.sock");
        let serial_json = Self::serial_console_config(serial_mode, &serial_socket_path);

        let mut vm_config_json = serde_json::json!({
            "cpus": cpus,
            "memory": memory,
            "payload": payload,
            "disks": disks_json,
            "net": net_json,
            "serial": serial_json,
            "console": { "mode": console_mode },
        });

        if let Some(true) = hv.and_then(|h| h.cpu_kvm_hyperv) {
            vm_config_json["platform"] = serde_json::json!({ "kvm_hyperv": true });
        }
        if let Some(v) = hv.and_then(|h| h.iommu) {
            vm_config_json["iommu"] = serde_json::json!(v);
        }
        if let Some(v) = hv.and_then(|h| h.watchdog) {
            vm_config_json["watchdog"] = serde_json::json!(v);
        }
        if let Some(v) = hv.and_then(|h| h.pvpanic) {
            vm_config_json["pvpanic"] = serde_json::json!(v);
        }
        if let Some(v) = hv.and_then(|h| h.landlock_enable) {
            vm_config_json["landlock"] = serde_json::json!(v);
        }
        if let Some(ref src) = hv.and_then(|h| h.rng_src.as_ref()) {
            vm_config_json["rng"] = serde_json::json!({ "src": src });
        }
        if let Some(ref tpm_type) = hv.and_then(|h| h.tpm_type.as_ref()) {
            let tpm_socket = hv
                .and_then(|h| h.tpm_socket_path.as_ref())
                .cloned()
                .unwrap_or_else(|| {
                    vm_runtime_dir
                        .join("tpm.sock")
                        .to_string_lossy()
                        .to_string()
                });
            vm_config_json["tpm"] = serde_json::json!({
                "type": tpm_type,
                "socket": tpm_socket,
            });
        }

        let body = vm_config_json.to_string();

        info!(
            vm_id = %config.vm_id,
            socket = %config.api_socket_path.display(),
            binary = %self.chv_binary.display(),
            op = operation_id.unwrap_or("-"),
            "spawning cloud-hypervisor"
        );

        let child = cmd.spawn().map_err(|e| ChvError::Io {
            path: self.chv_binary.to_string_lossy().to_string(),
            source: e,
        })?;
        // Close the orphan window: `child` is not yet registered in the vm
        // process map, and several awaits separate this spawn from the
        // registration. If an async cancellation (e.g. the core executor's
        // bounded-drain abort) drops this future mid-way, the child must not
        // survive as an unaccounted VMM — the armed `ChildGuard` SIGKILLs it.
        // It is disarmed immediately before the child is handed to the vm
        // process map, which then owns lifecycle.
        let mut child = ChildGuard::new(child);

        // Persist BOTH the VMM pid and the exact vm.create payload BEFORE
        // any await toward the API: the window between spawn and create
        // success is precisely where an agent crash would otherwise strand
        // a VM that adoption can see but start can never re-create (the
        // payload is what `respawn_vmm` needs). Writing the pid before the
        // VM exists trades a small, manageable misclassification — a crash
        // between spawn and vm.create adopts a live VMM with no VM inside,
        // where stop treats it as stopped and delete reaps it by identity —
        // for full coverage of the much larger window. Both writes are
        // best-effort with a warning: without the pidfile, adoption of
        // this VM is skipped; without the config, a later start degrades
        // to a clear re-create-required error.
        if let Some(pid) = child.id() {
            let _ = std::fs::write(vm_pid_file(vm_runtime_dir), format!("{pid}"));
        }
        if let Err(e) = std::fs::write(vm_config_file(vm_runtime_dir), &body) {
            warn!(
                vm_id = %config.vm_id,
                error = %e,
                "failed to persist vm-config.json; vm restart after a VMM exit will require re-create"
            );
        }

        if let Err(e) =
            Self::wait_for_socket(&config.api_socket_path, std::time::Duration::from_secs(10)).await
        {
            let _ = child.start_kill();
            let _ = child.wait().await;
            let stderr_hint = read_stderr_tail(&stderr_log_path);
            warn!(
                vm_id = %config.vm_id,
                stderr = %stderr_hint,
                "cloud-hypervisor failed to create api socket within 10s"
            );
            return Err(ChvError::Internal {
                reason: format!(
                    "failed to start cloud-hypervisor for vm {}: {} stderr: {}",
                    config.vm_id, e, stderr_hint
                ),
            });
        }

        if let Ok(Some(exit_status)) = child.try_wait() {
            let stderr_hint = read_stderr_tail(&stderr_log_path);
            let _ = tokio::fs::remove_file(&config.api_socket_path).await;
            return Err(ChvError::Internal {
                reason: format!(
                    "cloud-hypervisor exited immediately for vm {} with {}: {}",
                    config.vm_id, exit_status, stderr_hint
                ),
            });
        }

        let (create_status, create_body) = Self::ch_api_request_with_body(
            &config.api_socket_path,
            "PUT",
            "/api/v1/vm.create",
            Some(&body),
        )
        .await?;

        if create_status != 200 && create_status != 204 {
            let _ = child.start_kill();
            let _ = child.wait().await;
            let _ = tokio::fs::remove_file(&config.api_socket_path).await;
            return Err(ChvError::Internal {
                reason: format!(
                    "vm.create returned status {} for vm {}: {}",
                    create_status, config.vm_id, create_body
                ),
            });
        }

        // Acquire the guest serial-console I/O endpoint — guest output is
        // read from it and user keystrokes are written to it. Socket mode
        // (the default) connects to the unix listener cloud-hypervisor
        // bound while creating the VM; Pty mode (explicit tuning only)
        // opens the pty slave cloud-hypervisor allocated and reported in
        // vm.info. Both helpers fail closed: on error they kill the
        // spawned child so a console-less VM is never left running.
        let console_io: OwnedFd = if serial_mode == chv_common::hypervisor::DEFAULT_SERIAL_MODE {
            Self::connect_serial_socket(
                &serial_socket_path,
                &mut child,
                &config.vm_id,
                operation_id,
            )
            .await?
        } else {
            Self::open_serial_pty(
                &config.api_socket_path,
                &mut child,
                &config.vm_id,
                operation_id,
            )
            .await?
        };

        let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
        let pty_scrollback = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let serial_transport = if serial_mode == chv_common::hypervisor::DEFAULT_SERIAL_MODE {
            SerialTransport::Socket(serial_socket_path.clone())
        } else {
            SerialTransport::Pty
        };

        // Duplicate the descriptor for the broadcaster while retaining the
        // owned console descriptor for the VM process map.
        let broadcaster_fd = Self::dup_cloexec(&console_io).ok();
        // Claim capture alive only when a broadcaster will actually run.
        let broadcaster_alive = Arc::new(AtomicBool::new(broadcaster_fd.is_some()));

        // Acquire the vm-process-map lock while the guard is still armed so a
        // cancellation landing on this await still SIGKILLs the child; then
        // disarm and insert with no intervening await. From here the map owns
        // lifecycle (stop/delete call start_kill explicitly) and a normal drop
        // of a registered VmProcess (e.g. runtime teardown) does not SIGKILL a
        // VM that is deliberately left running.
        let mut map = self.vms.write().await;
        let child = child.disarm();
        map.insert(
            config.vm_id.clone(),
            VmProcess {
                api_socket: config.api_socket_path.clone(),
                child: VmmChild::Owned(child),
                // The guest serial-console I/O fd: a unix-stream socket to
                // cloud-hypervisor's serial listener by default (Socket
                // transport), or the pty slave under explicit Pty tuning.
                console_io,
                serial_transport: serial_transport.clone(),
                pty_tx: pty_tx.clone(),
                pty_scrollback: pty_scrollback.clone(),
                broadcaster_alive: broadcaster_alive.clone(),
                last_cpu_seconds: 0.0,
                last_cpu_at: None,
            },
        );
        drop(map);

        // Spawn background broadcaster: read PTY output and fan out via broadcast channel
        if let Some(broadcaster_fd) = broadcaster_fd {
            Self::spawn_pty_broadcaster(
                self.vms.clone(),
                config.vm_id.clone(),
                broadcaster_fd,
                serial_transport,
                pty_tx.clone(),
                pty_scrollback.clone(),
                broadcaster_alive.clone(),
                self.console_draining.clone(),
            );
        }

        // Subscribe to broadcast channel and persist to console.log
        Self::spawn_console_log_writer(
            &config.vm_id,
            &pty_tx,
            vm_runtime_dir.join("console.log"),
            ConsoleLogMode::Fresh,
        );

        __guard.succeeded = true;
        Ok(config.vm_id.clone())
    }

    async fn start_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let mut __guard = VmOpGuard::new("start");
        // Serialize with the other lifecycle ops for this VM (see
        // `lifecycle_locks`): two concurrent starts over an exited VMM
        // must never both re-spawn a process for it.
        let _lifecycle = self.vm_op_lock(vm_id).lock_owned().await;

        // cloud-hypervisor v43 exits when the guest exits (VMM control
        // loop Exit dispatch → vmm_shutdown), so a start after a graceful
        // stop — or any guest-side poweroff — has no daemon to boot
        // against. The start contract is "make the VM running": re-spawn
        // the VMM and re-create the VM from the payload persisted at
        // create time instead of failing on a dead api socket. Liveness
        // is proven, not assumed: an indeterminate probe result refuses
        // the re-spawn (a second VMM on one disk is never acceptable)
        // with an error the caller can retry.
        let liveness = {
            let mut vms = self.vms.write().await;
            let Some(proc) = vms.get_mut(vm_id) else {
                return Err(ChvError::NotFound {
                    resource: "vm".to_string(),
                    id: vm_id.to_string(),
                });
            };
            let api_socket = proc.api_socket.clone();
            proc.child.prove_exited(&api_socket)
        };
        match liveness {
            Liveness::Exited => {
                info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "vmm process exited, re-spawning");
                self.respawn_vmm(vm_id, operation_id).await?;
                __guard.succeeded = true;
                return Ok(());
            }
            Liveness::Unknown(e) => {
                warn!(vm_id = %vm_id, error = %e, "cannot determine vmm liveness; refusing to re-spawn");
                return Err(ChvError::Internal {
                    reason: format!(
                        "cannot determine whether the cloud-hypervisor process for vm {vm_id} is still running ({e}); refusing to re-spawn — retry the operation"
                    ),
                });
            }
            Liveness::Alive => {}
        }

        let (api_socket, serial_transport, pty_tx, pty_scrollback, broadcaster_alive) = {
            let vms = self.vms.read().await;
            let proc = vms.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            (
                proc.api_socket.clone(),
                proc.serial_transport.clone(),
                proc.pty_tx.clone(),
                proc.pty_scrollback.clone(),
                proc.broadcaster_alive.clone(),
            )
        };

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "booting vm via ch api");

        let (info_status, info_body) =
            Self::ch_api_request_with_body(&api_socket, "GET", "/api/v1/vm.info", None).await?;
        let mut state = String::new();
        let action = if info_status == 200 {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&info_body) {
                state = v
                    .get("state")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
                start_vm_action(&state)
            } else {
                start_vm_action("")
            }
        } else {
            start_vm_action("")
        };

        // Respawn broadcaster if it died while the VM was running.
        // This must happen BEFORE the idempotent early return so console
        // output doesn't stall when start_vm is called on an already-running VM.
        self.respawn_broadcaster_if_dead(
            vm_id,
            &serial_transport,
            &pty_tx,
            &pty_scrollback,
            &broadcaster_alive,
        )
        .await;

        match action {
            StartVmAction::AlreadyRunning => {
                info!(vm_id = %vm_id, state = %state, "vm already booted, skipping vm.boot");
                // Idempotent success: VM is already in the desired state.
                // The guard's succeeded flag must be set on every Ok return
                // so RED metrics classify this as ok, not err.
                __guard.succeeded = true;
                return Ok(());
            }
            StartVmAction::Boot => {}
        }

        let status = Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.boot", None).await?;
        if status != 200 && status != 204 {
            warn!(vm_id = %vm_id, status = status, "vm.boot returned non-success (VM may have auto-booted)");
        }

        __guard.succeeded = true;
        Ok(())
    }

    async fn stop_vm(
        &self,
        vm_id: &str,
        force: bool,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let mut __guard = VmOpGuard::new("stop");
        // Serialize with the other lifecycle ops for this VM (see
        // `lifecycle_locks`).
        let _lifecycle = self.vm_op_lock(vm_id).lock_owned().await;
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(vm_id = %vm_id, force = force, op = operation_id.unwrap_or("-"), "stopping vm");

        if force {
            let removed = {
                let mut map = self.vms.write().await;
                map.remove(vm_id)
            };
            let log_path = if let Some(mut proc) = removed {
                // Clear in-memory scrollback before dropping the process.
                {
                    let mut sb = proc.pty_scrollback.write().await;
                    sb.clear();
                }
                let log_path = proc.api_socket.parent().map(|p| p.join("console.log"));
                // INVARIANT — kill and reap the VMM BEFORE `proc` (and its
                // console_io descriptor, plus any dup the broadcaster still
                // holds) drops: an agent-side close of a serial connection
                // whose receive queue holds unread data resets the
                // connection, which kills cloud-hypervisor v43's serial
                // manager (and with a live manager in the blast radius, can
                // freeze the guest — see `abandon_serial_connection`).
                // With the VMM already dead there is no peer left to reset.
                // Any future reordering of these steps must preserve this.
                proc.child.kill(&proc.api_socket, self.expected_vmm_exe());
                // Reap before dropping the entry: the agent is the parent
                // of an Owned child, and an unreaped exit would linger as
                // a zombie for the agent's lifetime.
                proc.child.wait().await;
                log_path
            } else {
                None
            };
            if let Some(path) = log_path {
                let _ = tokio::fs::remove_file(&path).await;
                info!(vm_id = %vm_id, path = %path.display(), "removed console.log on force stop");
            }
        } else {
            // Graceful stop: send the ACPI power button so the guest OS can
            // shut itself down cleanly. cloud-hypervisor v43 exits WITH the
            // guest (the VMM control loop's Exit dispatch runs vmm_shutdown),
            // so once the guest reaches Shutdown the VMM process is gone —
            // there is no daemon to keep alive. The VmProcess entry
            // deliberately STAYS in the map (with its exited child) so a
            // later start re-spawns the VMM and re-creates the VM from the
            // payload persisted at create time (see `respawn_vmm`), and
            // stop/delete remain idempotent against the dead process. Poll
            // vm.info for the graceful window (see `GRACEFUL_STOP_WINDOW`)
            // waiting for a non-running terminal state (Shutdown or
            // Created).
            let _ = Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.power-button", None).await;
            // Poll vm.info waiting for the VM to reach a non-running
            // terminal state (Shutdown or Created). A guest that powers
            // itself off takes the process with it (v43 exits with the
            // guest), so the request failing mid-window is the normal
            // completion signal too.
            let start = std::time::Instant::now();
            let mut graceful_shutdown = false;
            while start.elapsed() < Self::GRACEFUL_STOP_WINDOW {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if let Ok((200, body)) =
                    Self::ch_api_request_with_body(&api_socket, "GET", "/api/v1/vm.info", None)
                        .await
                {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                        let state = v.get("state").and_then(|s| s.as_str()).unwrap_or("");
                        if state == "Shutdown" || state == "Created" {
                            graceful_shutdown = true;
                            break;
                        }
                    }
                } else {
                    // CH process disappeared — treat as stopped
                    graceful_shutdown = true;
                    break;
                }
            }

            if !graceful_shutdown {
                tracing::warn!(vm_id = %vm_id, "VM did not shut down gracefully within timeout, force-killing");
                let removed = {
                    let mut map = self.vms.write().await;
                    map.remove(vm_id)
                };
                let log_path = if let Some(mut proc) = removed {
                    {
                        let mut sb = proc.pty_scrollback.write().await;
                        sb.clear();
                    }
                    let log_path = proc.api_socket.parent().map(|p| p.join("console.log"));
                    proc.child.kill(&proc.api_socket, self.expected_vmm_exe());
                    // Reap before dropping the entry (zombie prevention).
                    proc.child.wait().await;
                    log_path
                } else {
                    None
                };
                if let Some(path) = log_path {
                    let _ = tokio::fs::remove_file(&path).await;
                    info!(vm_id = %vm_id, path = %path.display(), "removed console.log on force stop after graceful timeout");
                }
                // Force-kill fallback after graceful-stop timeout is still a
                // successful stop from the caller's point of view: the VM is no
                // longer in the runtime map. Mark the guard before the early
                // return so RED metrics classify this as ok.
                __guard.succeeded = true;
                return Ok(());
            }

            // Clear console caches after graceful shutdown. The VmProcess stays
            // in the map so a later vm.boot can restart it; we truncate the
            // on-disk log so the existing writer task can continue appending.
            let (pty_scrollback, log_path) = {
                let vms = self.vms.read().await;
                let proc = vms.get(vm_id);
                (
                    proc.map(|p| p.pty_scrollback.clone()),
                    proc.and_then(|p| p.api_socket.parent().map(|d| d.join("console.log"))),
                )
            };
            if let Some(sb) = pty_scrollback {
                let mut buf = sb.write().await;
                buf.clear();
            }
            if let Some(path) = log_path {
                match tokio::fs::OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(&path)
                    .await
                {
                    Ok(_) => {
                        info!(vm_id = %vm_id, path = %path.display(), "truncated console.log on graceful stop")
                    }
                    Err(e) => {
                        warn!(vm_id = %vm_id, path = %path.display(), error = %e, "failed to truncate console.log")
                    }
                }
            }
        }
        __guard.succeeded = true;
        Ok(())
    }

    async fn delete_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let mut __guard = VmOpGuard::new("delete");
        // Serialize with the other lifecycle ops for this VM (see
        // `lifecycle_locks`).
        let _lifecycle = self.vm_op_lock(vm_id).lock_owned().await;
        let mut proc = {
            let mut map = self.vms.write().await;
            map.remove(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?
        };

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "deleting vm");

        proc.child.kill(&proc.api_socket, self.expected_vmm_exe());
        proc.child.wait().await;
        // Remove the runtime artifacts this adapter owns: the api socket,
        // the pid file and the persisted creation payload. Disk images and
        // the VM directory itself belong to the storage/authority layers.
        let _ = tokio::fs::remove_file(&proc.api_socket).await;
        if let Some(vm_dir) = proc.api_socket.parent() {
            let _ = tokio::fs::remove_file(vm_pid_file(vm_dir)).await;
            let _ = tokio::fs::remove_file(vm_config_file(vm_dir)).await;
        }
        __guard.succeeded = true;
        Ok(())
    }

    async fn reboot_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        // Serialize with the other lifecycle ops for this VM (see
        // `lifecycle_locks`).
        let _lifecycle = self.vm_op_lock(vm_id).lock_owned().await;
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "rebooting vm via ch api");

        // Snapshot the pre-reboot console connection BEFORE the API call.
        // cloud-hypervisor v43's vm.reboot tears the VM down and re-creates
        // it, re-binding the serial listener at the same path — but the OLD
        // accepted connection is orphaned without EOF: the serial manager's
        // accept path hands its descriptor to epoll via `into_raw_fd()` and
        // never closes it, so the broadcaster parked on it would never wake.
        // The forced EOF must land on exactly this connection — the one
        // that predates the reboot — not on whatever a concurrent heal may
        // have swapped into the map meanwhile, so a dup is taken up front:
        // it pins the open file description, making the raw fd impossible
        // to recycle under us. Socket transport only (the pty transport
        // has no socket to shut down; its reboot behavior is a recorded
        // follow-up).
        let pre_reboot_console: Option<OwnedFd> = {
            let vms = self.vms.read().await;
            match vms.get(vm_id) {
                Some(proc) if matches!(proc.serial_transport, SerialTransport::Socket(_)) => {
                    Self::dup_cloexec(&proc.console_io).ok()
                }
                _ => None,
            }
        };

        let status = Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.reboot", None).await?;
        if status == 0 {
            warn!(
                vm_id = %vm_id,
                "unparseable response from vm.reboot; serial connection not rotated"
            );
            return Ok(());
        }
        if status != 200 && status != 204 {
            warn!(status = status, "unexpected status from vm.reboot");
            return Ok(());
        }
        // A successful vm.reboot re-bound the listener; rotate the pinned
        // connection. shutdown(2) acts on the open file description, so it
        // wakes every blocked reader on it (the broadcaster's dup
        // included): the broadcaster observes EOF and its self-heal
        // reconnects to the re-bound listener, swapping the stored
        // endpoint and streaming the new boot. Queued bytes are NOT
        // discarded by SHUT_RD (reads still drain them, then EOF), and
        // the subsequent close of the rotated descriptors is clean even
        // with data queued — SHUT_RD neutralizes the reset-on-close
        // heuristic. It does make the peer's writes fail EPIPE while the
        // connection is nominally open, which is exactly why SHUT_RD is
        // used ONLY here, on a connection whose VMM peer is already torn
        // down by the reboot — never on a live connection (see
        // `abandon_serial_connection`). Best-effort: a failure means the
        // connection was already gone, which the broadcaster's own EOF
        // handling covers.
        if let Some(fd) = pre_reboot_console {
            if let Err(e) =
                nix::sys::socket::shutdown(fd.as_raw_fd(), nix::sys::socket::Shutdown::Read)
            {
                warn!(
                    vm_id = %vm_id,
                    error = %e,
                    "serial read-side shutdown after vm.reboot failed"
                );
            }
        }
        Ok(())
    }

    async fn resize_vm(
        &self,
        vm_id: &str,
        cpus: Option<u32>,
        memory_bytes: Option<u64>,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(vm_id = %vm_id, ?cpus, ?memory_bytes, op = operation_id.unwrap_or("-"), "resizing vm via ch api");

        let mut obj = serde_json::Map::new();
        if let Some(c) = cpus {
            obj.insert("desired_vcpus".to_string(), serde_json::Value::from(c));
        }
        if let Some(m) = memory_bytes {
            obj.insert("desired_ram".to_string(), serde_json::Value::from(m));
        }
        let body = serde_json::Value::Object(obj).to_string();

        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.resize", Some(&body)).await?;
        Self::expect_status(status, "vm.resize")?;
        Ok(())
    }

    async fn vm_info(&self, vm_id: &str) -> Result<VmInfo, ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        let (status, body) =
            Self::ch_api_request_with_body(&api_socket, "GET", "/api/v1/vm.info", None).await?;
        if status != 200 {
            return Err(ChvError::Internal {
                reason: format!("vm.info returned unexpected status {}", status),
            });
        }

        let response_json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| ChvError::Internal {
                reason: format!("failed to parse vm.info response: {}", e),
            })?;

        let state = response_json
            .get("state")
            .and_then(|s| s.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let cpus = response_json
            .pointer("/config/cpus/boot_vcpus")
            .and_then(|c| c.as_u64())
            .unwrap_or(0) as u32;
        let memory_bytes = response_json
            .pointer("/config/memory/size")
            .and_then(|m| m.as_u64())
            .unwrap_or(0);

        Ok(VmInfo {
            state,
            cpus,
            memory_bytes,
        })
    }

    async fn vm_counters(&self, vm_id: &str) -> Result<VmCounters, ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        let (status, body) =
            Self::ch_api_request_with_body(&api_socket, "GET", "/api/v1/vm.counters", None).await?;
        if status != 200 {
            return Err(ChvError::Internal {
                reason: format!("vm.counters returned unexpected status {}", status),
            });
        }

        let response_json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| ChvError::Internal {
                reason: format!("failed to parse vm.counters response: {}", e),
            })?;

        // CPU usage is reported in seconds; compute percentage from delta across ticks.
        let cpu_seconds = response_json
            .pointer("/cpus/usage/cpu_seconds")
            .and_then(|c| c.as_f64())
            .unwrap_or(0.0);

        let mut cpu_percent = 0.0;
        {
            let mut map = self.vms.write().await;
            if let Some(proc) = map.get_mut(vm_id) {
                if let Some(last_at) = proc.last_cpu_at {
                    let delta_secs = cpu_seconds - proc.last_cpu_seconds;
                    let elapsed = last_at.elapsed().as_secs_f64();
                    if elapsed > 0.0 && delta_secs >= 0.0 {
                        // CH reports CPU time across all vCPUs.
                        // Normalize to a percentage of wall-clock time.
                        cpu_percent = (delta_secs / elapsed) * 100.0;
                        // Clamp to a sane max (e.g. 100% per vCPU is unrealistic for long
                        // intervals, but possible for short ones). Let downstream clamp if
                        // they want per-vCPU percentages.
                    }
                }
                proc.last_cpu_seconds = cpu_seconds;
                proc.last_cpu_at = Some(std::time::Instant::now());
            }
        }

        let mut net_rx = 0u64;
        let mut net_tx = 0u64;
        if let Some(net) = response_json.get("net").and_then(|n| n.as_object()) {
            for (_iface, counters) in net {
                if let Some(obj) = counters.as_object() {
                    net_rx += obj.get("rx_bytes").and_then(|x| x.as_u64()).unwrap_or(0);
                    net_tx += obj.get("tx_bytes").and_then(|x| x.as_u64()).unwrap_or(0);
                }
            }
        }

        let mut disk_read = 0u64;
        let mut disk_written = 0u64;
        if let Some(block) = response_json.get("block").and_then(|b| b.as_object()) {
            for (_dev, counters) in block {
                if let Some(obj) = counters.as_object() {
                    disk_read += obj.get("read_bytes").and_then(|x| x.as_u64()).unwrap_or(0);
                    disk_written += obj.get("write_bytes").and_then(|x| x.as_u64()).unwrap_or(0);
                }
            }
        }

        // Memory counters are not exposed by vm.counters; use vm.info config as total
        // and report 0 for used (CH does not expose guest memory usage).
        let memory_total = response_json
            .pointer("/memory/available")
            .and_then(|m| m.as_u64())
            .unwrap_or(0);

        Ok(VmCounters {
            cpu_percent,
            memory_bytes_used: 0,
            memory_bytes_total: memory_total,
            disk_bytes_read: disk_read,
            disk_bytes_written: disk_written,
            net_bytes_rx: net_rx,
            net_bytes_tx: net_tx,
        })
    }

    async fn snapshot_vm(
        &self,
        vm_id: &str,
        destination: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(vm_id = %vm_id, destination = %destination, op = operation_id.unwrap_or("-"), "snapshotting vm via ch api");

        let body =
            serde_json::json!({"destination_url": format!("file://{}", destination)}).to_string();
        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.snapshot", Some(&body)).await?;
        Self::expect_status(status, "vm.snapshot")?;
        Ok(())
    }

    async fn restore_snapshot(
        &self,
        vm_id: &str,
        source: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(vm_id = %vm_id, source = %source, op = operation_id.unwrap_or("-"), "restoring snapshot via ch api");

        let body = serde_json::json!({"source_url": format!("file://{}", source)}).to_string();
        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.restore", Some(&body)).await?;
        Self::expect_status(status, "vm.restore")?;
        Ok(())
    }

    async fn pause_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let mut __guard = VmOpGuard::new("pause");
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "pausing vm via ch api");

        let status = Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.pause", None).await?;
        Self::expect_status(status, "vm.pause")?;
        __guard.succeeded = true;
        Ok(())
    }

    async fn resume_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let mut __guard = VmOpGuard::new("resume");
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "resuming vm via ch api");

        let status = Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.resume", None).await?;
        Self::expect_status(status, "vm.resume")?;
        __guard.succeeded = true;
        Ok(())
    }

    async fn power_button(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "sending ACPI power button via ch api");

        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.power-button", None).await?;
        Self::expect_status(status, "vm.power-button")?;
        Ok(())
    }

    async fn add_disk(
        &self,
        vm_id: &str,
        params: &AddDiskParams,
        operation_id: Option<&str>,
    ) -> Result<String, ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(vm_id = %vm_id, path = %params.path.display(), op = operation_id.unwrap_or("-"), "hot-adding disk via ch api");

        let mut obj = serde_json::Map::new();
        obj.insert(
            "path".to_string(),
            serde_json::Value::from(params.path.to_string_lossy().to_string()),
        );
        obj.insert(
            "readonly".to_string(),
            serde_json::Value::from(params.read_only),
        );
        if let Some(ref id) = params.id {
            obj.insert("id".to_string(), serde_json::Value::from(id.clone()));
        }
        let body = serde_json::Value::Object(obj).to_string();

        let (status, response_body) =
            Self::ch_api_request_with_body(&api_socket, "PUT", "/api/v1/vm.add-disk", Some(&body))
                .await?;
        Self::expect_status(status, "vm.add-disk")?;
        Ok(response_body)
    }

    async fn remove_device(
        &self,
        vm_id: &str,
        device_id: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(vm_id = %vm_id, device_id = %device_id, op = operation_id.unwrap_or("-"), "hot-removing device via ch api");

        let body = serde_json::json!({"id": device_id}).to_string();
        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.remove-device", Some(&body))
                .await?;
        Self::expect_status(status, "vm.remove-device")?;
        Ok(())
    }

    async fn add_net(
        &self,
        vm_id: &str,
        params: &AddNetParams,
        operation_id: Option<&str>,
    ) -> Result<String, ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(vm_id = %vm_id, tap = %params.tap_name, mac = %params.mac_address, op = operation_id.unwrap_or("-"), "hot-adding net via ch api");

        let mut obj = serde_json::Map::new();
        obj.insert(
            "tap".to_string(),
            serde_json::Value::from(params.tap_name.clone()),
        );
        obj.insert(
            "mac".to_string(),
            serde_json::Value::from(params.mac_address.clone()),
        );
        if let Some(ref id) = params.id {
            obj.insert("id".to_string(), serde_json::Value::from(id.clone()));
        }
        let body = serde_json::Value::Object(obj).to_string();

        let (status, response_body) =
            Self::ch_api_request_with_body(&api_socket, "PUT", "/api/v1/vm.add-net", Some(&body))
                .await?;
        Self::expect_status(status, "vm.add-net")?;
        Ok(response_body)
    }

    async fn resize_disk(
        &self,
        vm_id: &str,
        disk_id: &str,
        new_size_bytes: u64,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(vm_id = %vm_id, disk_id = %disk_id, new_size = new_size_bytes, op = operation_id.unwrap_or("-"), "resizing disk via ch api");

        let body = format!(r#"{{"id":"{}","new_size":{}}}"#, disk_id, new_size_bytes);
        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.resize-zone", Some(&body)).await?;
        Self::expect_status(status, "vm.resize-zone")?;
        Ok(())
    }

    async fn ping(&self, vm_id: &str) -> Result<bool, ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        match Self::ch_api_request(&api_socket, "GET", "/api/v1/vmm.ping", None).await {
            Ok(status) => Ok(status == 200),
            Err(_) => Ok(false),
        }
    }

    async fn send_migration(
        &self,
        vm_id: &str,
        destination_url: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(
            vm_id = %vm_id,
            destination_url = %destination_url,
            op = operation_id.unwrap_or("-"),
            "initiating send-migration via ch api"
        );

        let body = serde_json::json!({"destination_url": destination_url}).to_string();
        // send-migration is a long-running blocking call; use a custom long timeout.
        let (status, response_body) = {
            let mut stream = tokio::net::UnixStream::connect(&api_socket)
                .await
                .map_err(|e| ChvError::Io {
                    path: api_socket.to_string_lossy().to_string(),
                    source: e,
                })?;

            let request = format!(
                "PUT /api/v1/vm.send-migration HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
                body.len(),
                body
            );

            stream
                .write_all(request.as_bytes())
                .await
                .map_err(|e| ChvError::Io {
                    path: api_socket.to_string_lossy().to_string(),
                    source: e,
                })?;

            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            let read_fut = async {
                loop {
                    let n = stream.read(&mut tmp).await.map_err(|e| ChvError::Io {
                        path: api_socket.to_string_lossy().to_string(),
                        source: e,
                    })?;
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(header_end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header_bytes = &buf[..header_end];
                        let headers = String::from_utf8_lossy(header_bytes);
                        let content_length = headers
                            .lines()
                            .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
                            .and_then(|l| l.split_once(':').map(|x| x.1))
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        let body_start = header_end + 4;
                        if buf.len() >= body_start + content_length {
                            break;
                        }
                    }
                }
                Ok::<(), ChvError>(())
            };
            // Migration can take minutes; allow up to 10 minutes for this blocking call.
            tokio::time::timeout(std::time::Duration::from_secs(600), read_fut)
                .await
                .map_err(|_| ChvError::Internal {
                    reason: format!("send-migration timed out for vm {}", vm_id),
                })??;

            let raw = String::from_utf8_lossy(&buf);
            let status_code = parse_http_status(raw.as_bytes()).unwrap_or(0);
            let resp_body = if let Some(idx) = raw.find("\r\n\r\n") {
                raw[idx + 4..].to_string()
            } else {
                String::new()
            };
            (status_code, resp_body)
        };

        if status != 200 && status != 204 {
            return Err(ChvError::Internal {
                reason: format!(
                    "vm.send-migration returned status {} for vm {}: {}",
                    status, vm_id, response_body
                ),
            });
        }
        Ok(())
    }

    async fn receive_migration(
        &self,
        vm_id: &str,
        receiver_url: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(
            vm_id = %vm_id,
            receiver_url = %receiver_url,
            op = operation_id.unwrap_or("-"),
            "initiating receive-migration via ch api"
        );

        let body = serde_json::json!({"receiver_url": receiver_url}).to_string();
        // receive-migration is also a long-running blocking call.
        let (status, response_body) = {
            let mut stream = tokio::net::UnixStream::connect(&api_socket)
                .await
                .map_err(|e| ChvError::Io {
                    path: api_socket.to_string_lossy().to_string(),
                    source: e,
                })?;

            let request = format!(
                "PUT /api/v1/vm.receive-migration HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
                body.len(),
                body
            );

            stream
                .write_all(request.as_bytes())
                .await
                .map_err(|e| ChvError::Io {
                    path: api_socket.to_string_lossy().to_string(),
                    source: e,
                })?;

            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            let read_fut = async {
                loop {
                    let n = stream.read(&mut tmp).await.map_err(|e| ChvError::Io {
                        path: api_socket.to_string_lossy().to_string(),
                        source: e,
                    })?;
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(header_end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header_bytes = &buf[..header_end];
                        let headers = String::from_utf8_lossy(header_bytes);
                        let content_length = headers
                            .lines()
                            .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
                            .and_then(|l| l.split_once(':').map(|x| x.1))
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        let body_start = header_end + 4;
                        if buf.len() >= body_start + content_length {
                            break;
                        }
                    }
                }
                Ok::<(), ChvError>(())
            };
            // Migration can take minutes; allow up to 10 minutes.
            tokio::time::timeout(std::time::Duration::from_secs(600), read_fut)
                .await
                .map_err(|_| ChvError::Internal {
                    reason: format!("receive-migration timed out for vm {}", vm_id),
                })??;

            let raw = String::from_utf8_lossy(&buf);
            let status_code = parse_http_status(raw.as_bytes()).unwrap_or(0);
            let resp_body = if let Some(idx) = raw.find("\r\n\r\n") {
                raw[idx + 4..].to_string()
            } else {
                String::new()
            };
            (status_code, resp_body)
        };

        if status != 200 && status != 204 {
            return Err(ChvError::Internal {
                reason: format!(
                    "vm.receive-migration returned status {} for vm {}: {}",
                    status, vm_id, response_body
                ),
            });
        }
        Ok(())
    }

    async fn get_vm_state(&self, vm_id: &str) -> Result<String, ChvError> {
        let info = self.vm_info(vm_id).await?;
        Ok(info.state)
    }

    async fn coredump(
        &self,
        vm_id: &str,
        destination: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(vm_id = %vm_id, destination = %destination, op = operation_id.unwrap_or("-"), "generating coredump via ch api");

        let body = format!(r#"{{"destination_url":"file://{}"}}"#, destination);
        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.coredump", Some(&body)).await?;
        Self::expect_status(status, "vm.coredump")?;
        Ok(())
    }

    async fn pty_master(&self, vm_id: &str) -> Option<OwnedFd> {
        let map = self.vms.read().await;
        let proc = map.get(vm_id)?;
        Self::dup_cloexec(&proc.console_io).ok()
    }

    async fn pty_output_rx(
        &self,
        vm_id: &str,
    ) -> Option<tokio::sync::broadcast::Receiver<Vec<u8>>> {
        let map = self.vms.read().await;
        let proc = map.get(vm_id)?;
        Some(proc.pty_tx.subscribe())
    }

    async fn pty_scrollback(&self, vm_id: &str) -> Option<Vec<u8>> {
        let map = self.vms.read().await;
        let proc = map.get(vm_id)?;
        let sb = proc.pty_scrollback.read().await;
        Some(sb.clone())
    }
}

use cellhv_core_runtime_ownership::ProcessIdentity;

#[derive(Debug, Clone)]
pub struct AdoptedVmHandle {
    pub vm_id: String,
    pub api_socket: std::path::PathBuf,
    pub identity: ProcessIdentity,
    pub proc_path: std::path::PathBuf,
    pub runtime_root: std::path::PathBuf,
}

impl AdoptedVmHandle {
    pub fn new(vm_id: String, api_socket: std::path::PathBuf, identity: ProcessIdentity) -> Self {
        Self {
            vm_id,
            api_socket,
            identity,
            proc_path: std::path::PathBuf::from("/proc"),
            runtime_root: std::path::PathBuf::from("/"),
        }
    }

    pub fn with_runtime_root(mut self, path: std::path::PathBuf) -> Self {
        self.runtime_root = path;
        self
    }

    pub fn with_proc_path(mut self, path: std::path::PathBuf) -> Self {
        self.proc_path = path;
        self
    }

    pub fn revalidate(&self) -> Result<(), ChvError> {
        let vm_id = cellhv_core_types::VmId::new(&self.vm_id).map_err(|_| ChvError::Internal {
            reason: "invalid vm_id".to_string(),
        })?;
        let observer = crate::linux_observation::LinuxOwnershipObservation::open_with_proc(
            &self.runtime_root,
            &self.proc_path,
            self.identity.pid,
            vm_id,
        )
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to open observer: {}", e),
        })?;

        let current_identity = observer
            .process(self.identity.pid)
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to get process identity: {}", e),
            })?
            .ok_or_else(|| ChvError::Internal {
                reason: "process exited".to_string(),
            })?;

        if current_identity.pid != self.identity.pid
            || current_identity.start_ticks != self.identity.start_ticks
            || current_identity.boot_id != self.identity.boot_id
            || current_identity.executable != self.identity.executable
            || current_identity.uid != self.identity.uid
            || current_identity.gid != self.identity.gid
            || current_identity.cgroup_fingerprint != self.identity.cgroup_fingerprint
        {
            return Err(ChvError::Internal {
                reason: "process identity changed".to_string(),
            });
        }

        Ok(())
    }

    pub async fn api_request(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<u16, ChvError> {
        self.revalidate()?;
        crate::ch_api::CloudHypervisorApiClient::default()
            .request(&self.api_socket, method, path, body)
            .await
    }

    pub async fn api_request_with_body(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<(u16, String), ChvError> {
        self.revalidate()?;
        crate::ch_api::CloudHypervisorApiClient::default()
            .request_with_body(&self.api_socket, method, path, body)
            .await
    }
}

impl ProcessCloudHypervisorAdapter {
    /// Rebuilds the in-memory VM map from on-disk runtime state after an
    /// agent restart.
    ///
    /// The map is agent-process state; the VMMs it describes are not. A
    /// restart while VMs are running leaves re-parented orphans the new
    /// agent cannot see — every lifecycle op then fails with NotFound
    /// while the VMs keep running (proven by M2.5 qualification run 7:
    /// the post-restart stop and delete both failed this way). This scan
    /// rebuilds one entry per `{runtime_root}/vms/<vm_id>/` directory:
    ///
    /// - liveness and identity come from the persisted `ch.pid` plus a
    ///   `/proc/<pid>/cmdline` re-validation against the VM's api-socket
    ///   path (`pid_is_cloud_hypervisor`) — pid recycling can never
    ///   redirect a kill;
    /// - a live orphan gets its serial console re-attached (Socket
    ///   transport: reconnect to the listener the orphan still serves;
    ///   the broadcaster and console.log writer resume in append mode,
    ///   preserving pre-restart history) and is tracked as
    ///   [`VmmChild::Adopted`];
    /// - a dead VMM (cloud-hypervisor v43 exits with the guest) still
    ///   gets an entry — stop/delete are then idempotent and the next
    ///   start re-spawns from the persisted payload;
    /// - entries without a parseable `ch.pid` are skipped with a warning
    ///   (ops on them keep today's NotFound semantics rather than
    ///   guessing at process identity);
    /// - Pty-transport VMs are adopted for lifecycle only: the console
    ///   endpoint cannot be re-derived without a live `vm.info` and is a
    ///   recorded follow-up.
    ///
    /// Failures are per-directory (warn + skip); only an unreadable base
    /// directory fails the call.
    pub async fn adopt_running_vms(&self, runtime_root: &std::path::Path) -> Result<(), ChvError> {
        let vms_base = runtime_root.join("vms");
        let entries = match std::fs::read_dir(&vms_base) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // No VMs have ever been created on this node.
                return Ok(());
            }
            Err(e) => {
                return Err(ChvError::Io {
                    path: vms_base.to_string_lossy().to_string(),
                    source: e,
                });
            }
        };

        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let vm_id = entry.file_name().to_string_lossy().to_string();
            // Layer-B path-safety: the directory name becomes a map key
            // and (via start) a re-spawn source; never build entries for
            // ids the authority layer would reject.
            if !is_safe_resource_id(&vm_id) {
                warn!(vm_id = %vm_id, "skipping vm runtime dir with unsafe id during adoption");
                continue;
            }
            let vm_dir = entry.path();
            let api_socket = vm_dir.join("vm.sock");

            // Never adopt over a tracked entry: a second adoption pass
            // (or a split-brain peer agent on the same runtime root) must
            // not replace an Owned child with an Adopted pid — dropping
            // the Child would orphan a VMM this agent is responsible for
            // reaping.
            if self.vms.read().await.contains_key(&vm_id) {
                warn!(
                    vm_id = %vm_id,
                    "skipping adoption: vm is already tracked by this agent"
                );
                continue;
            }

            let pid = std::fs::read_to_string(vm_pid_file(&vm_dir))
                .ok()
                .and_then(|raw| raw.trim().parse::<u32>().ok());
            let Some(pid) = pid else {
                warn!(
                    vm_id = %vm_id,
                    "skipping vm runtime dir without a parseable ch.pid during adoption"
                );
                continue;
            };

            let vmm_alive = pid_is_cloud_hypervisor(pid, &api_socket, self.expected_vmm_exe());
            let serial_sock = vm_dir.join("serial.sock");
            let serial_transport = if serial_sock.exists() {
                SerialTransport::Socket(serial_sock)
            } else {
                SerialTransport::Pty
            };

            // Re-attach the console only for a live VMM on the Socket
            // transport; anything else gets the EOF placeholder (an
            // honest "no console" endpoint — the broadcaster, if ever
            // spawned on it, exits immediately).
            let (console_io, console_live) = if vmm_alive {
                match &serial_transport {
                    SerialTransport::Socket(path) => {
                        match Self::connect_serial_socket_once(path).await {
                            Ok(fd) => (fd, true),
                            Err(e) => {
                                warn!(
                                    vm_id = %vm_id,
                                    error = %e,
                                    "serial re-attach failed during adoption; console capture stays down"
                                );
                                (Self::eof_placeholder_fd(), false)
                            }
                        }
                    }
                    SerialTransport::Pty => (Self::eof_placeholder_fd(), false),
                }
            } else {
                (Self::eof_placeholder_fd(), false)
            };

            let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
            let pty_scrollback = Arc::new(tokio::sync::RwLock::new(Vec::new()));
            // Take the broadcaster's endpoint BEFORE the entry consumes
            // console_io, so the map never needs to be re-read on the
            // just-inserted entry. Capture is only claimed alive when the
            // dup (and with it the broadcaster spawn) actually happens.
            let broadcaster_fd = if console_live {
                Self::dup_cloexec(&console_io).ok()
            } else {
                None
            };
            let broadcaster_alive = Arc::new(AtomicBool::new(broadcaster_fd.is_some()));

            {
                let mut map = self.vms.write().await;
                map.insert(
                    vm_id.clone(),
                    VmProcess {
                        api_socket: api_socket.clone(),
                        child: VmmChild::Adopted(pid),
                        console_io,
                        serial_transport: serial_transport.clone(),
                        pty_tx: pty_tx.clone(),
                        pty_scrollback: pty_scrollback.clone(),
                        broadcaster_alive: broadcaster_alive.clone(),
                        last_cpu_seconds: 0.0,
                        last_cpu_at: None,
                    },
                );
            }

            if let Some(broadcaster_fd) = broadcaster_fd {
                Self::spawn_pty_broadcaster(
                    self.vms.clone(),
                    vm_id.clone(),
                    broadcaster_fd,
                    serial_transport,
                    pty_tx.clone(),
                    pty_scrollback.clone(),
                    broadcaster_alive,
                    self.console_draining.clone(),
                );
                Self::spawn_console_log_writer(
                    &vm_id,
                    &pty_tx,
                    vm_dir.join("console.log"),
                    ConsoleLogMode::Append,
                );
                info!(
                    vm_id = %vm_id,
                    pid = pid,
                    "adopted running vm after agent restart; console capture resumed"
                );
            } else if vmm_alive {
                info!(
                    vm_id = %vm_id,
                    pid = pid,
                    "adopted running vm after agent restart (console not re-attached)"
                );
            } else {
                info!(
                    vm_id = %vm_id,
                    "adopted stopped vm runtime dir after agent restart"
                );
            }
        }
        Ok(())
    }

    /// A read-only fd whose reads return EOF immediately — the
    /// placeholder console endpoint for adopted entries with no live
    /// connection. Lifecycle paths see an honest "no console" state
    /// instead of a guessed-at endpoint. Reachability notes: `reboot_vm`'s
    /// forced rotation can never touch it (rotation happens only after a
    /// 2xx `vm.reboot`, which a dead VMM's missing api socket can never
    /// answer), and console keystrokes written to it fail with EPIPE —
    /// an honest error for a VM whose console is down.
    fn eof_placeholder_fd() -> OwnedFd {
        match nix::unistd::pipe() {
            // Dropping the write end makes the read end return EOF.
            Ok((read_end, write_end)) => {
                drop(write_end);
                read_end
            }
            Err(_) => {
                // /dev/null reads EOF forever — the last-resort stand-in.
                std::fs::File::open("/dev/null")
                    .expect("/dev/null must open")
                    .into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::build_cpus_config;
    use super::parse_http_status;
    use super::ProcessCloudHypervisorAdapter;
    use super::SerialTransport;
    use super::VmProcess;
    use super::VmmChild;
    use crate::adapter::{CloudHypervisorAdapter, VmConfig, VmDiskConfig};
    use chv_common::hypervisor::HypervisorOverrides;
    use chv_errors::ChvError;
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn parse_http_status_extracts_200() {
        let bytes = b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
        assert_eq!(parse_http_status(bytes), Some(200));
    }

    #[test]
    fn parse_http_status_extracts_204() {
        let bytes = b"HTTP/1.1 204 No Content\r\n";
        assert_eq!(parse_http_status(bytes), Some(204));
    }

    #[test]
    fn parse_http_status_returns_none_for_garbage() {
        assert_eq!(parse_http_status(b"garbage"), None);
    }

    #[test]
    fn parse_http_status_handles_empty() {
        assert_eq!(parse_http_status(b""), None);
    }

    #[test]
    fn serial_console_config_socket_mode_carries_listener_path() {
        // Socket is the default transport: the config must point
        // cloud-hypervisor at the per-VM listener the agent connects to —
        // a bare mode string would silently fall back to the pty, whose
        // v43 output gate swallows passive console capture.
        let value = ProcessCloudHypervisorAdapter::serial_console_config(
            "Socket",
            std::path::Path::new("/run/chv/vms/vm-1/serial.sock"),
        );
        assert_eq!(value["mode"], serde_json::json!("Socket"));
        assert_eq!(
            value["socket"],
            serde_json::json!("/run/chv/vms/vm-1/serial.sock")
        );
    }

    #[test]
    fn serial_console_config_passes_other_modes_through() {
        for mode in ["Pty", "File", "Null", "Off"] {
            let value = ProcessCloudHypervisorAdapter::serial_console_config(
                mode,
                std::path::Path::new("/run/chv/vms/vm-1/serial.sock"),
            );
            assert_eq!(value["mode"], serde_json::json!(mode));
            assert!(value.get("socket").is_none());
        }
    }

    #[tokio::test]
    async fn connect_serial_socket_returns_blocking_cloexec_fd() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let socket_path = dir.path().join("serial.sock");
        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
        let mut child = tokio::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();

        let fd = ProcessCloudHypervisorAdapter::connect_serial_socket(
            &socket_path,
            &mut child,
            "vm-test",
            None,
        )
        .await
        .expect("connect must succeed against a bound listener");

        // The broadcaster and console server issue plain blocking
        // reads/writes; a non-blocking fd would EAGAIN and silently kill
        // console capture.
        let status_flags =
            nix::fcntl::fcntl(&fd, nix::fcntl::F_GETFL).expect("fd flags must be readable");
        assert_eq!(
            nix::fcntl::OFlag::from_bits_retain(status_flags) & nix::fcntl::OFlag::O_NONBLOCK,
            nix::fcntl::OFlag::empty(),
            "serial socket fd must be blocking"
        );
        let fd_flags =
            nix::fcntl::fcntl(&fd, nix::fcntl::F_GETFD).expect("fd flags must be readable");
        assert_ne!(
            fd_flags & nix::fcntl::FdFlag::FD_CLOEXEC.bits(),
            0,
            "serial socket fd must be CLOEXEC"
        );
        // The connect contract keeps the spawned VMM alive on success —
        // only the failure path kills the child.
        assert!(
            child.try_wait().unwrap().is_none(),
            "child must still be running after a successful serial connect"
        );
        // The accepted listener end must be drained by the helper's
        // connect having completed the handshake (no deadlock risk from an
        // unread backlog is asserted implicitly by the connect succeeding).
        drop(listener);
        let _ = child.start_kill();
        let _ = child.wait().await;
    }

    #[tokio::test]
    async fn connect_serial_socket_fails_closed_and_kills_child_without_listener() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let socket_path = dir.path().join("serial.sock");
        let mut child = tokio::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();

        let err = ProcessCloudHypervisorAdapter::connect_serial_socket(
            &socket_path,
            &mut child,
            "vm-test",
            None,
        )
        .await
        .expect_err("connect must fail with no listener");

        assert!(matches!(err, ChvError::Io { .. }));
        let status = child.try_wait().unwrap().expect("child must be reaped");
        assert!(!status.success(), "the spawned child must be killed");
    }

    #[test]
    fn dup_cloexec_sets_cloexec_and_carries_data() {
        use std::io::Write as _;

        let (a, mut b) = std::os::unix::net::UnixStream::pair().unwrap();
        let owned: OwnedFd = a.into();
        let dup = ProcessCloudHypervisorAdapter::dup_cloexec(&owned).expect("dup must succeed");

        // FD_CLOEXEC is per-descriptor: the dup must carry it.
        let fd_flags = nix::fcntl::fcntl(&dup, nix::fcntl::F_GETFD).unwrap();
        assert_ne!(
            fd_flags & nix::fcntl::FdFlag::FD_CLOEXEC.bits(),
            0,
            "dup must be CLOEXEC"
        );

        // The dup is a live view of the same open file description: data
        // written to the peer arrives through the dup...
        b.write_all(b"x").unwrap();
        let mut buf = [0u8; 1];
        let n = nix::unistd::read(&dup, &mut buf).unwrap();
        assert_eq!((n, buf[0]), (1, b'x'));

        // ...and the source fd is still open and usable (the dup did not
        // consume it).
        b.write_all(b"y").unwrap();
        let mut buf = [0u8; 1];
        let n = nix::unistd::read(&owned, &mut buf).unwrap();
        assert_eq!((n, buf[0]), (1, b'y'));
    }

    #[tokio::test]
    async fn respawn_broadcaster_reconnects_and_swaps_dead_socket() {
        use std::io::Write as _;

        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let vm_dir = dir.path().join("vm-respawn");
        std::fs::create_dir_all(&vm_dir).unwrap();
        let sock_path = dir.path().join("serial.sock");

        // Stand in for cloud-hypervisor's serial listener: accept one
        // connection and immediately stream a probe byte (guest output).
        let listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
        let acceptor = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().expect("accept");
            conn.write_all(b"probe").expect("write probe");
            std::thread::sleep(std::time::Duration::from_millis(1500));
        });

        let adapter = ProcessCloudHypervisorAdapter::new(dir.path().join("chv"));
        let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
        let pty_scrollback = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let broadcaster_alive = Arc::new(AtomicBool::new(false));
        let mut child = tokio::process::Command::new("true").spawn().unwrap();
        let _ = child.wait().await;
        // A dead stand-in for a VMM-closed serial connection.
        let dead_console_io: OwnedFd = std::fs::File::open("/dev/null").unwrap().into();
        let dead_fd = dead_console_io.as_raw_fd();

        {
            let mut map = adapter.vms.write().await;
            map.insert(
                "vm-respawn".to_string(),
                VmProcess {
                    api_socket: vm_dir.join("vm.sock"),
                    child: VmmChild::Owned(child),
                    console_io: dead_console_io,
                    serial_transport: SerialTransport::Socket(sock_path.clone()),
                    pty_tx: pty_tx.clone(),
                    pty_scrollback: pty_scrollback.clone(),
                    broadcaster_alive: broadcaster_alive.clone(),
                    last_cpu_seconds: 0.0,
                    last_cpu_at: None,
                },
            );
        }

        // Subscribe before the respawn so the broadcaster's fan-out is
        // observable.
        let mut rx = pty_tx.subscribe();
        adapter
            .respawn_broadcaster_if_dead(
                "vm-respawn",
                &SerialTransport::Socket(sock_path),
                &pty_tx,
                &pty_scrollback,
                &broadcaster_alive,
            )
            .await;

        assert!(
            broadcaster_alive.load(Ordering::SeqCst),
            "broadcaster must be alive after respawn"
        );

        // The stored connection was swapped for the live one (the dead
        // fd stays owned by the map entry until the swap, so the numbers
        // must differ).
        let map = adapter.vms.read().await;
        let proc = map.get("vm-respawn").unwrap();
        assert_ne!(
            proc.console_io.as_raw_fd(),
            dead_fd,
            "stored console fd must be swapped for the reconnected one"
        );
        drop(map);

        // The respawned broadcaster streams guest output from the fresh
        // connection through the fan-out channel.
        let got = tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv())
            .await
            .expect("probe must arrive through the fan-out")
            .expect("channel must be live");
        assert_eq!(got, b"probe".to_vec());

        // The reconnect handshake completed on the listener side too.
        acceptor.join().expect("acceptor thread");
    }

    #[tokio::test]
    async fn respawn_broadcaster_redups_pty_transport() {
        use std::io::Write as _;

        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let vm_dir = dir.path().join("vm-respawn-pty");
        std::fs::create_dir_all(&vm_dir).unwrap();

        let adapter = ProcessCloudHypervisorAdapter::new(dir.path().join("chv"));
        let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
        let pty_scrollback = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let broadcaster_alive = Arc::new(AtomicBool::new(false));
        let mut child = tokio::process::Command::new("true").spawn().unwrap();
        let _ = child.wait().await;
        let (a, mut b) = std::os::unix::net::UnixStream::pair().unwrap();
        let console_io: OwnedFd = a.into();
        let original_fd = console_io.as_raw_fd();

        {
            let mut map = adapter.vms.write().await;
            map.insert(
                "vm-respawn-pty".to_string(),
                VmProcess {
                    api_socket: vm_dir.join("vm.sock"),
                    child: VmmChild::Owned(child),
                    console_io,
                    serial_transport: SerialTransport::Pty,
                    pty_tx: pty_tx.clone(),
                    pty_scrollback: pty_scrollback.clone(),
                    broadcaster_alive: broadcaster_alive.clone(),
                    last_cpu_seconds: 0.0,
                    last_cpu_at: None,
                },
            );
        }

        let mut rx = pty_tx.subscribe();
        adapter
            .respawn_broadcaster_if_dead(
                "vm-respawn-pty",
                &SerialTransport::Pty,
                &pty_tx,
                &pty_scrollback,
                &broadcaster_alive,
            )
            .await;

        assert!(
            broadcaster_alive.load(Ordering::SeqCst),
            "broadcaster must be alive after respawn"
        );

        // The pty transport re-dups the stored slave; the stored fd is NOT
        // swapped (the kernel keeps the pty pair alive across reboots).
        let map = adapter.vms.read().await;
        let proc = map.get("vm-respawn-pty").unwrap();
        assert_eq!(
            proc.console_io.as_raw_fd(),
            original_fd,
            "pty transport must keep the stored slave fd"
        );
        drop(map);

        // The broadcaster reads through the dup: output written to the
        // peer end arrives through the fan-out channel.
        b.write_all(b"probe").unwrap();
        let got = tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv())
            .await
            .expect("probe must arrive through the fan-out")
            .expect("channel must be live");
        assert_eq!(got, b"probe".to_vec());
    }

    #[tokio::test]
    async fn broadcaster_self_heals_across_listener_rebind() {
        use std::io::Write as _;

        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let vm_dir = dir.path().join("vm-heal");
        std::fs::create_dir_all(&vm_dir).unwrap();
        let sock_path = dir.path().join("serial.sock");

        // Stand in for cloud-hypervisor's serial lifecycle across a
        // vm.reboot: SerialManager::drop closes the accepted connection
        // AND removes the socket file while dropping the listener, then
        // pre_create_console_devices re-binds a fresh listener at the same
        // path and the new guest's output must flow to a reconnecting
        // client.
        let rebind_path = sock_path.clone();
        let listener1 = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut conn1, _) = listener1.accept().expect("accept 1");
            conn1.write_all(b"first").expect("write first");
            // vm.reboot: tear the serial device down — the listener first
            // (so no reconnect can land in a doomed accept backlog), then
            // the accepted connection (whose close is what signals EOF to
            // the broadcaster) — and re-bind a fresh listener at the same
            // path, exactly as pre_create_console_devices does after
            // SerialManager::drop removed the socket file.
            drop(listener1);
            drop(conn1);
            std::fs::remove_file(&rebind_path).unwrap();
            let listener2 = std::os::unix::net::UnixListener::bind(&rebind_path).unwrap();
            let (mut conn2, _) = listener2.accept().expect("accept 2");
            conn2.write_all(b"second").expect("write second");
            // Hold the new connection open while the test asserts.
            std::thread::sleep(std::time::Duration::from_millis(2500));
        });

        let adapter = ProcessCloudHypervisorAdapter::new(dir.path().join("chv"));
        let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
        let pty_scrollback = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let broadcaster_alive = Arc::new(AtomicBool::new(false));
        let mut child = tokio::process::Command::new("true").spawn().unwrap();
        let _ = child.wait().await;
        // A dead stand-in for a VMM-closed serial connection.
        let dead_console_io: OwnedFd = std::fs::File::open("/dev/null").unwrap().into();
        let dead_fd = dead_console_io.as_raw_fd();

        {
            let mut map = adapter.vms.write().await;
            map.insert(
                "vm-heal".to_string(),
                VmProcess {
                    api_socket: vm_dir.join("vm.sock"),
                    child: VmmChild::Owned(child),
                    console_io: dead_console_io,
                    serial_transport: SerialTransport::Socket(sock_path.clone()),
                    pty_tx: pty_tx.clone(),
                    pty_scrollback: pty_scrollback.clone(),
                    broadcaster_alive: broadcaster_alive.clone(),
                    last_cpu_seconds: 0.0,
                    last_cpu_at: None,
                },
            );
        }

        let mut rx = pty_tx.subscribe();
        adapter
            .respawn_broadcaster_if_dead(
                "vm-heal",
                &SerialTransport::Socket(sock_path.clone()),
                &pty_tx,
                &pty_scrollback,
                &broadcaster_alive,
            )
            .await;
        assert!(
            broadcaster_alive.load(Ordering::SeqCst),
            "broadcaster must be alive after respawn"
        );

        // First probe arrives through the first connection, and the dead
        // stored fd was swapped for the live one.
        let got = tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv())
            .await
            .expect("first probe must arrive through the fan-out")
            .expect("channel must be live");
        assert_eq!(got, b"first".to_vec());
        let fd_after_first = {
            let map = adapter.vms.read().await;
            let proc = map.get("vm-heal").unwrap();
            assert_ne!(
                proc.console_io.as_raw_fd(),
                dead_fd,
                "stored console fd must be swapped for the reconnected one"
            );
            proc.console_io.as_raw_fd()
        };

        // Across the simulated reboot (connection closed, listener dropped
        // and re-bound at the same path), the broadcaster SELF-HEALS: the
        // second probe arrives through the new connection, the stored fd
        // is swapped again, and the broadcaster never dies.
        let got = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .expect("second probe must arrive after the self-heal")
            .expect("channel must be live");
        assert_eq!(got, b"second".to_vec());
        assert!(
            broadcaster_alive.load(Ordering::SeqCst),
            "broadcaster must survive the connection cycle without respawning"
        );
        {
            let map = adapter.vms.read().await;
            let proc = map.get("vm-heal").unwrap();
            assert_ne!(
                proc.console_io.as_raw_fd(),
                fd_after_first,
                "stored console fd must be swapped across the self-heal"
            );
        }

        server.join().expect("server thread");
    }

    /// Pins the exact AF_UNIX kernel semantics the clean-disconnect
    /// machinery relies on. Root cause of the M2.5 guest freeze: an
    /// agent-side close with unread receive-queue data RESETS the
    /// connection, and cloud-hypervisor v43's serial manager dies
    /// silently on the resulting ECONNRESET. If a kernel behavior change
    /// ever breaks these assertions, the drain-before-close discipline
    /// must be re-evaluated.
    #[tokio::test]
    async fn serial_close_semantics_regression() {
        use std::io::{Read as _, Write as _};

        // Negative control — the defect model itself: the "serial
        // manager" writes a burst, the client closes WITHOUT draining,
        // and the manager's next read observes ECONNRESET (not EOF).
        {
            let (mut manager, client) = std::os::unix::net::UnixStream::pair().unwrap();
            manager.write_all(b"guest boot burst").unwrap();
            // Let the bytes land in the client's receive queue.
            std::thread::sleep(std::time::Duration::from_millis(50));
            drop(client); // abortive: unread data still queued
            let mut buf = [0u8; 64];
            let err = manager.read(&mut buf).unwrap_err();
            assert_eq!(
                err.kind(),
                std::io::ErrorKind::ConnectionReset,
                "close with unread receive-queue data must reset the connection (defect model)"
            );
        }

        // The fix's semantics: half-close, settle, drain, then drop —
        // the manager observes a clean EOF instead.
        {
            let (mut manager, client) = std::os::unix::net::UnixStream::pair().unwrap();
            manager.write_all(b"guest boot burst").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(50));
            let client_fd: OwnedFd = client.into();
            ProcessCloudHypervisorAdapter::abandon_serial_connection(
                "vm-semantics",
                client_fd,
                None,
            )
            .await;
            let mut buf = [0u8; 64];
            let n = manager
                .read(&mut buf)
                .unwrap_or_else(|e| panic!("manager must observe a clean EOF, not {e:?}"));
            assert_eq!(n, 0, "manager must observe EOF after a clean abandon");
        }
    }

    /// `drain_and_close_consoles` converts agent shutdown into the clean
    /// close cloud-hypervisor handles correctly: the serial-manager
    /// stand-in observes a clean EOF — never the ECONNRESET an undrained
    /// close produces — and the buffered guest output is preserved
    /// through the console fan-out (scrollback + console.log evidence).
    #[tokio::test]
    async fn drain_and_close_consoles_closes_cleanly() {
        use std::io::{Read as _, Write as _};

        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let vm_dir = dir.path().join("vm-drain");
        std::fs::create_dir_all(&vm_dir).unwrap();
        let sock_path = dir.path().join("serial.sock");

        // Stand in for cloud-hypervisor's serial manager: one accepted
        // client connection that streams guest output.
        let listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().expect("accept");
            // Simulate a mid-burst guest: console output the agent has
            // not read yet when shutdown begins.
            conn.write_all(b"late guest output").expect("write burst");
            conn
        });

        let adapter = ProcessCloudHypervisorAdapter::new(dir.path().join("chv"));
        let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
        let pty_scrollback = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let broadcaster_alive = Arc::new(AtomicBool::new(false));
        let mut child = tokio::process::Command::new("true").spawn().unwrap();
        let _ = child.wait().await;

        let client = std::os::unix::net::UnixStream::connect(&sock_path).unwrap();
        let console_io: OwnedFd = client.into();
        {
            let mut map = adapter.vms.write().await;
            map.insert(
                "vm-drain".to_string(),
                VmProcess {
                    api_socket: vm_dir.join("vm.sock"),
                    child: VmmChild::Owned(child),
                    console_io,
                    serial_transport: SerialTransport::Socket(sock_path.clone()),
                    pty_tx: pty_tx.clone(),
                    pty_scrollback: pty_scrollback.clone(),
                    broadcaster_alive: broadcaster_alive.clone(),
                    last_cpu_seconds: 0.0,
                    last_cpu_at: None,
                },
            );
        }

        // Subscribe BEFORE the drain: broadcast sends without a
        // receiver are dropped, and the drained bytes are the assertion
        // target.
        let mut rx = pty_tx.subscribe();
        // Let the burst land in the receive queue, then run the
        // graceful-shutdown drain.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        adapter.drain_and_close_consoles().await;

        // The buffered guest output was preserved through the fan-out.
        let got = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("drained output must reach the console fan-out")
            .expect("channel must be live");
        assert_eq!(got, b"late guest output".to_vec());
        assert!(pty_scrollback.read().await.ends_with(b"late guest output"));

        // The serial manager observes a clean EOF — never a reset.
        let mut conn = server.join().expect("server thread");
        let mut buf = [0u8; 64];
        let n = conn
            .read(&mut buf)
            .unwrap_or_else(|e| panic!("serial manager must observe EOF, not {e:?}"));
        assert_eq!(n, 0, "serial manager must observe a clean EOF");

        // Idempotent: the multiple process-exit paths can each call it.
        adapter.drain_and_close_consoles().await;
    }

    /// While the shutdown drain latch is set, a broadcaster whose
    /// connection ends must stand down instead of reconnecting: a
    /// connection minted during shutdown would be abortively closed by
    /// process exit — recreating the very freeze the drain prevents. The
    /// serial-manager stand-in keeps its listener bound and must observe
    /// no second connection.
    #[tokio::test]
    async fn drain_latch_suppresses_broadcaster_reconnect() {
        use std::io::Write as _;

        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let vm_dir = dir.path().join("vm-latch");
        std::fs::create_dir_all(&vm_dir).unwrap();
        let sock_path = dir.path().join("serial.sock");

        let listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
        listener.set_nonblocking(true).unwrap();

        let adapter = ProcessCloudHypervisorAdapter::new(dir.path().join("chv"));
        let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
        let pty_scrollback = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let broadcaster_alive = Arc::new(AtomicBool::new(false));
        let mut child = tokio::process::Command::new("true").spawn().unwrap();
        let _ = child.wait().await;

        let client = std::os::unix::net::UnixStream::connect(&sock_path).unwrap();
        let console_io: OwnedFd = client.into();
        let broadcaster_fd = ProcessCloudHypervisorAdapter::dup_cloexec(&console_io).unwrap();
        {
            let mut map = adapter.vms.write().await;
            map.insert(
                "vm-latch".to_string(),
                VmProcess {
                    api_socket: vm_dir.join("vm.sock"),
                    child: VmmChild::Owned(child),
                    console_io,
                    serial_transport: SerialTransport::Socket(sock_path.clone()),
                    pty_tx: pty_tx.clone(),
                    pty_scrollback: pty_scrollback.clone(),
                    broadcaster_alive: broadcaster_alive.clone(),
                    last_cpu_seconds: 0.0,
                    last_cpu_at: None,
                },
            );
        }
        // The alive flag is set by the spawner (the guard only clears it
        // on exit) — mirror the production call sites.
        broadcaster_alive.store(true, Ordering::SeqCst);
        ProcessCloudHypervisorAdapter::spawn_pty_broadcaster(
            adapter.vms.clone(),
            "vm-latch".to_string(),
            broadcaster_fd,
            SerialTransport::Socket(sock_path.clone()),
            pty_tx.clone(),
            pty_scrollback.clone(),
            broadcaster_alive.clone(),
            adapter.console_draining.clone(),
        );
        // Subscribe before writing the probe so the send has a receiver.
        let mut rx = pty_tx.subscribe();

        // Accept the agent's connection (retry: the listener is
        // non-blocking) and stream one probe through it.
        let mut conn = loop {
            match listener.accept() {
                Ok((conn, _)) => break conn,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
                Err(e) => panic!("accept: {e}"),
            }
        };
        conn.write_all(b"probe").unwrap();
        let got = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("probe must arrive through the fan-out")
            .expect("channel must be live");
        assert_eq!(got, b"probe".to_vec());
        assert!(
            broadcaster_alive.load(Ordering::SeqCst),
            "broadcaster must be alive and streaming"
        );

        // Begin the graceful-shutdown drain, then end the manager side
        // of the connection: the broadcaster wakes with EOF and its
        // heal must stand down — the listener stays bound, so a wrongful
        // reconnect would be observable as a pending connection.
        adapter.drain_and_close_consoles().await;
        drop(conn);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while broadcaster_alive.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        assert!(
            !broadcaster_alive.load(Ordering::SeqCst),
            "broadcaster must stand down during the shutdown drain instead of reconnecting"
        );

        // No second connection may land on the still-bound listener.
        let watch_until = std::time::Instant::now() + std::time::Duration::from_millis(300);
        while std::time::Instant::now() < watch_until {
            match listener.accept() {
                Ok((conn, _)) => {
                    drop(conn);
                    panic!("broadcaster reconnected during the shutdown drain");
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => panic!("accept: {e}"),
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    }

    /// `vm.reboot` in cloud-hypervisor v43 tears the VM down and
    /// re-creates it, re-binding the serial listener at the same path —
    /// but the OLD connection is orphaned without an EOF: the serial
    /// manager's accept path hands the descriptor to epoll via
    /// `into_raw_fd()` and never closes it, so a reader parked on it sees
    /// neither data nor EOF. `reboot_vm` must force-rotate the connection
    /// (`shutdown(SHUT_RD)` — description-level, so it also wakes the
    /// broadcaster's dup) after a successful reboot; the broadcaster's
    /// self-heal then reconnects to the re-bound listener and console
    /// capture follows the new boot.
    #[tokio::test]
    async fn reboot_vm_rotates_zombie_serial_connection() {
        use std::io::{Read as _, Write as _};

        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let vm_dir = dir.path().join("vm-reboot-rotate");
        std::fs::create_dir_all(&vm_dir).unwrap();
        let sock_path = dir.path().join("serial.sock");
        let api_sock_path = vm_dir.join("vm.sock");

        // Fake cloud-hypervisor API socket: answer vm.reboot with 204.
        let api_listener = std::os::unix::net::UnixListener::bind(&api_sock_path).unwrap();
        let api_server = std::thread::spawn(move || {
            let (mut conn, _) = api_listener.accept().expect("api accept");
            let mut request = [0u8; 1024];
            let _ = conn.read(&mut request);
            conn.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
                .expect("api respond");
        });

        // The zombie's peer is still open for the whole test — nobody
        // closed it — proving the rotation did not rely on the VMM
        // closing the connection.
        let (_zombie_peer_stays_open, zombie_agent_end) =
            std::os::unix::net::UnixStream::pair().unwrap();

        // The re-bound listener vm.reboot "left behind": the broadcaster's
        // self-heal must connect here and receive the new boot's output.
        let listener2 = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
        let rebind_server = std::thread::spawn(move || {
            let (mut conn, _) = listener2.accept().expect("rebind accept");
            conn.write_all(b"second-boot").expect("write second boot");
            // Hold the connection open while the test asserts.
            std::thread::sleep(std::time::Duration::from_millis(2500));
        });

        let adapter = ProcessCloudHypervisorAdapter::new(dir.path().join("chv"));
        let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
        let pty_scrollback = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let broadcaster_alive = Arc::new(AtomicBool::new(true));
        let mut child = tokio::process::Command::new("true").spawn().unwrap();
        let _ = child.wait().await;
        let zombie_console_io: OwnedFd = zombie_agent_end.into();
        let zombie_fd = zombie_console_io.as_raw_fd();

        {
            let mut map = adapter.vms.write().await;
            map.insert(
                "vm-reboot-rotate".to_string(),
                VmProcess {
                    api_socket: api_sock_path.clone(),
                    child: VmmChild::Owned(child),
                    console_io: zombie_console_io,
                    serial_transport: SerialTransport::Socket(sock_path.clone()),
                    pty_tx: pty_tx.clone(),
                    pty_scrollback: pty_scrollback.clone(),
                    broadcaster_alive: broadcaster_alive.clone(),
                    last_cpu_seconds: 0.0,
                    last_cpu_at: None,
                },
            );
        }

        // The broadcaster is ALIVE and parked on the zombie connection —
        // the exact production state after create + reboot. Without the
        // rotation it would stay parked forever (this is the bug).
        let broadcaster_fd = ProcessCloudHypervisorAdapter::dup_cloexec(
            &adapter
                .vms
                .read()
                .await
                .get("vm-reboot-rotate")
                .unwrap()
                .console_io,
        )
        .unwrap();
        ProcessCloudHypervisorAdapter::spawn_pty_broadcaster(
            adapter.vms.clone(),
            "vm-reboot-rotate".to_string(),
            broadcaster_fd,
            SerialTransport::Socket(sock_path.clone()),
            pty_tx.clone(),
            pty_scrollback.clone(),
            broadcaster_alive.clone(),
            adapter.console_draining.clone(),
        );
        let mut rx = pty_tx.subscribe();

        adapter.reboot_vm("vm-reboot-rotate", None).await.unwrap();
        api_server.join().expect("api server thread");

        // The broadcaster observed the forced EOF, reconnected to the
        // re-bound listener, and streams the new boot's output.
        let got = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .expect("second-boot probe must arrive through the fan-out")
            .expect("channel must be live");
        assert_eq!(got, b"second-boot".to_vec());
        assert!(
            broadcaster_alive.load(Ordering::SeqCst),
            "broadcaster must survive the forced rotation"
        );
        {
            let map = adapter.vms.read().await;
            let proc = map.get("vm-reboot-rotate").unwrap();
            assert_ne!(
                proc.console_io.as_raw_fd(),
                zombie_fd,
                "stored console fd must be swapped for the reconnected one"
            );
        }

        rebind_server.join().expect("rebind server thread");
    }

    /// A failed `vm.reboot` (non-2xx) must NOT rotate the serial
    /// connection — the listener was not re-bound, so a forced EOF would
    /// kill console capture for a VM that is still running on the old
    /// connection.
    #[tokio::test]
    async fn reboot_vm_failure_leaves_serial_connection_alone() {
        use std::io::{Read as _, Write as _};

        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let vm_dir = dir.path().join("vm-reboot-fail");
        std::fs::create_dir_all(&vm_dir).unwrap();
        let api_sock_path = vm_dir.join("vm.sock");

        // Fake cloud-hypervisor API socket: answer vm.reboot with 500.
        let api_listener = std::os::unix::net::UnixListener::bind(&api_sock_path).unwrap();
        let api_server = std::thread::spawn(move || {
            let (mut conn, _) = api_listener.accept().expect("api accept");
            let mut request = [0u8; 1024];
            let _ = conn.read(&mut request);
            conn.write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n")
                .expect("api respond");
        });

        // A healthy, live connection: the peer stays open and responsive.
        let (mut peer, agent_end) = std::os::unix::net::UnixStream::pair().unwrap();
        peer.write_all(b"still-streaming").unwrap();
        let console_io: OwnedFd = agent_end.into();
        let original_fd = console_io.as_raw_fd();

        let adapter = ProcessCloudHypervisorAdapter::new(dir.path().join("chv"));
        let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
        let pty_scrollback = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let broadcaster_alive = Arc::new(AtomicBool::new(true));
        let mut child = tokio::process::Command::new("true").spawn().unwrap();
        let _ = child.wait().await;

        {
            let mut map = adapter.vms.write().await;
            map.insert(
                "vm-reboot-fail".to_string(),
                VmProcess {
                    api_socket: api_sock_path.clone(),
                    child: VmmChild::Owned(child),
                    console_io,
                    serial_transport: SerialTransport::Socket(vm_dir.join("serial.sock")),
                    pty_tx: pty_tx.clone(),
                    pty_scrollback: pty_scrollback.clone(),
                    broadcaster_alive: broadcaster_alive.clone(),
                    last_cpu_seconds: 0.0,
                    last_cpu_at: None,
                },
            );
        }

        let mut rx = pty_tx.subscribe();

        // The broadcaster streams from the live connection.
        let broadcaster_fd = ProcessCloudHypervisorAdapter::dup_cloexec(
            &adapter
                .vms
                .read()
                .await
                .get("vm-reboot-fail")
                .unwrap()
                .console_io,
        )
        .unwrap();
        ProcessCloudHypervisorAdapter::spawn_pty_broadcaster(
            adapter.vms.clone(),
            "vm-reboot-fail".to_string(),
            broadcaster_fd,
            SerialTransport::Socket(vm_dir.join("serial.sock")),
            pty_tx.clone(),
            pty_scrollback.clone(),
            broadcaster_alive.clone(),
            adapter.console_draining.clone(),
        );
        let got = tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv())
            .await
            .expect("probe must arrive before the reboot attempt")
            .expect("channel must be live");
        assert_eq!(got, b"still-streaming".to_vec());

        adapter.reboot_vm("vm-reboot-fail", None).await.unwrap();
        api_server.join().expect("api server thread");

        // The connection was NOT rotated: the stored fd is unchanged and
        // the connection is still fully functional end-to-end — new peer
        // output flows through the unchanged broadcaster.
        {
            let map = adapter.vms.read().await;
            let proc = map.get("vm-reboot-fail").unwrap();
            assert_eq!(
                proc.console_io.as_raw_fd(),
                original_fd,
                "failed vm.reboot must not rotate the serial connection"
            );
        }
        peer.write_all(b"post-reboot").unwrap();
        let got = tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv())
            .await
            .expect("post-reboot probe must arrive through the untouched connection")
            .expect("channel must be live");
        assert_eq!(got, b"post-reboot".to_vec());
    }

    /// Between `spawn()` returning and the child's `execve` completing there
    /// is a small window in which `/proc/<pid>/cmdline` still reads empty;
    /// tests that assert on process identity wait for it to be observable.
    async fn wait_for_cmdline(pid: u32) -> String {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if let Some(cmdline) = super::proc_cmdline(pid) {
                if !cmdline.is_empty() {
                    return cmdline;
                }
            }
            assert!(
                std::time::Instant::now() < deadline,
                "cmdline for pid {pid} never became observable"
            );
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }

    /// VmmChild semantics: an Owned child is killed and reaped through
    /// kill/wait; an Adopted pid is only signalled when /proc/<pid>/cmdline
    /// AND the executable name still prove identity against the VM's
    /// api-socket path — a mismatching (e.g. recycled or argv-spoofed) pid
    /// is refused, and a vanished pid is absorbed without panicking.
    #[tokio::test]
    async fn vmm_child_owned_and_adopted_semantics() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let api_socket = dir.path().join("vm.sock");

        // Owned: spawn → live → kill → wait → exited.
        let owned_child = tokio::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .unwrap();
        let mut owned = VmmChild::Owned(owned_child);
        assert!(matches!(
            owned.prove_exited(&api_socket),
            super::Liveness::Alive
        ));
        owned.kill(&api_socket, None);
        owned.wait().await;
        assert!(matches!(
            owned.prove_exited(&api_socket),
            super::Liveness::Exited
        ));

        // Adopted with a matching command line. `sh -c "sleep 5; true"`
        // never execs (compound command), so the process keeps its argv —
        // including the api-socket flag and path — for its lifetime.
        let mut adopted_proc = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("sleep 5; true")
            .arg("--api-socket")
            .arg(&api_socket)
            .spawn()
            .unwrap();
        let adopted_pid = adopted_proc.id().expect("freshly spawned child has a pid");
        wait_for_cmdline(adopted_pid).await;
        // The stand-in's real executable — what the exe cross-check
        // compares against (the adapter passes its chv_binary's file
        // name in production).
        let adopted_exe = std::fs::read_link(format!("/proc/{adopted_pid}/exe")).unwrap();
        let adopted_exe_name = adopted_exe.file_name().unwrap().to_owned();
        assert!(
            super::pid_is_cloud_hypervisor(
                adopted_pid,
                &api_socket,
                Some(adopted_exe_name.as_os_str())
            ),
            "argv + exe identity must match for the live stand-in"
        );
        // A correct argv with a WRONG expected executable is refused:
        // crafted argv alone must never authorize a signal.
        assert!(!super::pid_is_cloud_hypervisor(
            adopted_pid,
            &api_socket,
            Some(std::ffi::OsStr::new("definitely-not-cloud-hypervisor"))
        ));
        let mut adopted = VmmChild::Adopted(adopted_pid);
        assert!(matches!(
            adopted.prove_exited(&api_socket),
            super::Liveness::Alive
        ));
        adopted.kill(&api_socket, Some(adopted_exe_name.as_os_str()));
        adopted.wait().await;
        assert!(matches!(
            adopted.prove_exited(&api_socket),
            super::Liveness::Exited
        ));
        let _ = adopted_proc.wait().await;

        // Adopted with a NON-matching command line: the kill must be
        // refused — an unrelated process must never be signalled.
        let mut unrelated = tokio::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let unrelated_pid = unrelated.id().expect("freshly spawned child has a pid");
        let unrelated_cmdline = wait_for_cmdline(unrelated_pid).await;
        assert!(
            unrelated_cmdline.contains("sleep"),
            "stand-in must be running"
        );
        assert!(!super::pid_is_cloud_hypervisor(
            unrelated_pid,
            &api_socket,
            None
        ));
        let mut mismatched = VmmChild::Adopted(unrelated_pid);
        mismatched.kill(&api_socket, None);
        // The refusal left the process alive.
        assert!(
            super::pid_exists(unrelated_pid),
            "kill must be refused for a pid without identity proof"
        );
        // Liveness for the re-spawn decision is about OUR VMM: a live but
        // unproven pid (recycled) counts as exited — the runtime dir is
        // ours to take over, the unrelated process is not ours to signal.
        assert!(matches!(
            mismatched.prove_exited(&api_socket),
            super::Liveness::Exited
        ));
        let _ = unrelated.start_kill();
        let _ = unrelated.wait().await;

        // Adopted with a vanished pid: no panic, absorbed as exited.
        let mut vanished = VmmChild::Adopted(4_000_000);
        assert!(matches!(
            vanished.prove_exited(&api_socket),
            super::Liveness::Exited
        ));
        vanished.kill(&api_socket, None);
        vanished.wait().await;
    }

    /// The adoption scan rebuilds the in-memory map from on-disk runtime
    /// state: a live orphan (pid + cmdline identity + serial listener)
    /// is re-attached with console capture resumed and console.log
    /// APPENDED (pre-restart history preserved); a dead VMM still gets
    /// an entry (idempotent stop/delete, re-spawnable start); dirs
    /// without a pidfile or with unsafe ids are skipped.
    #[tokio::test]
    async fn adopt_rebuilds_map_from_runtime_dir() {
        use std::io::Write as _;

        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let runtime_root = dir.path().join("runtime");
        let vms_base = runtime_root.join("vms");

        // vm-live: a live orphan with a serial listener and prior history.
        let live_dir = vms_base.join("vm-live");
        std::fs::create_dir_all(&live_dir).unwrap();
        let live_api_socket = live_dir.join("vm.sock");
        let serial_sock = live_dir.join("serial.sock");
        let mut orphan = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("sleep 5; true")
            .arg("--api-socket")
            .arg(&live_api_socket)
            .spawn()
            .unwrap();
        let orphan_pid = orphan.id().expect("freshly spawned child has a pid");
        wait_for_cmdline(orphan_pid).await;
        std::fs::write(live_dir.join("ch.pid"), format!("{orphan_pid}")).unwrap();
        std::fs::write(live_dir.join("vm-config.json"), "{}").unwrap();
        std::fs::write(live_dir.join("console.log"), "history\n").unwrap();
        // The orphan's serial listener: accept the agent's reconnect and
        // stream a probe (with a small delay so the test can subscribe to
        // the fan-out channel before the bytes flow).
        let serial_listener = std::os::unix::net::UnixListener::bind(&serial_sock).unwrap();
        let acceptor = std::thread::spawn(move || {
            let (mut conn, _) = serial_listener.accept().expect("serial accept");
            std::thread::sleep(std::time::Duration::from_millis(500));
            conn.write_all(b"resumed").expect("write resumed probe");
            std::thread::sleep(std::time::Duration::from_millis(2500));
        });

        // vm-dead: a reaped pid, no serial socket.
        let dead_dir = vms_base.join("vm-dead");
        std::fs::create_dir_all(&dead_dir).unwrap();
        let mut dead_child = tokio::process::Command::new("true").spawn().unwrap();
        let dead_pid = dead_child.id().expect("freshly spawned child has a pid");
        let _ = dead_child.wait().await;
        std::fs::write(dead_dir.join("ch.pid"), format!("{dead_pid}")).unwrap();

        // vm-nopid: no pidfile → skipped with today's NotFound semantics.
        std::fs::create_dir_all(vms_base.join("vm-nopid")).unwrap();

        // Unsafe id (contains a path separator character): skipped.
        let unsafe_dir = vms_base.join("bad\\id");
        std::fs::create_dir_all(&unsafe_dir).unwrap();
        std::fs::write(unsafe_dir.join("ch.pid"), "1").unwrap();

        // The adapter's chv_binary file name is what the exe cross-check
        // expects adopted VMMs to run — point it at the stand-in's real
        // executable so identity validation runs in its production shape.
        let orphan_exe = std::fs::read_link(format!("/proc/{orphan_pid}/exe")).unwrap();
        let adapter = ProcessCloudHypervisorAdapter::new(orphan_exe.clone());
        adapter.adopt_running_vms(&runtime_root).await.unwrap();

        let mut live_rx;
        {
            let vms = adapter.vms.read().await;
            let live = vms.get("vm-live").expect("live orphan must be adopted");
            let VmmChild::Adopted(live_pid) = &live.child else {
                panic!("live orphan must be tracked as Adopted");
            };
            assert_eq!(*live_pid, orphan_pid);
            assert!(super::pid_is_cloud_hypervisor(
                *live_pid,
                &live_api_socket,
                orphan_exe.file_name()
            ));
            assert!(
                live.broadcaster_alive.load(Ordering::SeqCst),
                "console capture must resume for the adopted orphan"
            );
            live_rx = live.pty_tx.subscribe();

            let dead = vms.get("vm-dead").expect("dead VMM dir must be adopted");
            let VmmChild::Adopted(dead_tracked_pid) = &dead.child else {
                panic!("dead VMM must be tracked as Adopted");
            };
            assert_eq!(*dead_tracked_pid, dead_pid);
            assert!(
                !super::pid_is_cloud_hypervisor(*dead_tracked_pid, &dead_dir.join("vm.sock"), None),
                "dead VMM must classify as exited"
            );
            assert!(
                !dead.broadcaster_alive.load(Ordering::SeqCst),
                "no broadcaster may be spawned for a dead VMM"
            );

            assert!(
                !vms.contains_key("vm-nopid"),
                "dir without pidfile must be skipped"
            );
            assert!(!vms.contains_key("bad\\id"), "unsafe id must be skipped");
        }

        // Console capture actually resumed: the serial probe flows through
        // the fan-out channel and console.log gains it WITHOUT losing the
        // pre-restart history (append semantics).
        let got = tokio::time::timeout(std::time::Duration::from_secs(5), live_rx.recv())
            .await
            .expect("resumed probe must arrive through the adopted console")
            .expect("channel must be live");
        assert_eq!(got, b"resumed".to_vec());

        let log_path = live_dir.join("console.log");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let contents = std::fs::read_to_string(&log_path).unwrap_or_default();
            if contents.contains("history") && contents.contains("resumed") {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "console.log must contain history + resumed, got: {contents:?}"
            );
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        let _ = orphan.start_kill();
        let _ = orphan.wait().await;
        acceptor.join().expect("acceptor thread");
    }

    /// start_vm on a tracked VM whose VMM has exited routes to the
    /// re-spawn path: without a persisted config it fails fast with a
    /// clear re-create-required error while PRESERVING the entry
    /// (stop/delete stay functional); with a config but a missing binary
    /// the spawn failure surfaces.
    #[tokio::test]
    async fn start_vm_respawns_dead_vmm() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let vm_dir = dir.path().join("vm-rs");
        std::fs::create_dir_all(&vm_dir).unwrap();

        // The chv binary does not exist: any re-spawn attempt surfaces the
        // spawn failure immediately instead of hanging.
        let adapter = ProcessCloudHypervisorAdapter::new(dir.path().join("chv-missing"));
        let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
        let mut child = tokio::process::Command::new("true").spawn().unwrap();
        let _ = child.wait().await;
        {
            let mut map = adapter.vms.write().await;
            map.insert(
                "vm-rs".to_string(),
                VmProcess {
                    api_socket: vm_dir.join("vm.sock"),
                    child: VmmChild::Owned(child),
                    console_io: std::fs::File::open("/dev/null").unwrap().into(),
                    serial_transport: SerialTransport::Socket(vm_dir.join("serial.sock")),
                    pty_tx,
                    pty_scrollback: Arc::new(tokio::sync::RwLock::new(Vec::new())),
                    broadcaster_alive: Arc::new(AtomicBool::new(false)),
                    last_cpu_seconds: 0.0,
                    last_cpu_at: None,
                },
            );
        }

        // No persisted config → clear re-create-required error, entry kept.
        let err = adapter.start_vm("vm-rs", None).await.unwrap_err();
        match &err {
            ChvError::Internal { reason } => {
                assert!(
                    reason.contains("re-create"),
                    "error must tell the operator to re-create: {reason}"
                );
            }
            other => panic!("expected Internal, got {other:?}"),
        }
        assert!(
            adapter.vms.read().await.contains_key("vm-rs"),
            "a failed re-spawn must preserve the tracked entry"
        );

        // Persisted config + missing binary → the spawn failure surfaces.
        std::fs::write(vm_dir.join("vm-config.json"), r#"{"cpus":1}"#).unwrap();
        let err = adapter.start_vm("vm-rs", None).await.unwrap_err();
        assert!(
            matches!(err, ChvError::Io { .. }),
            "expected Io from the failed spawn, got {err:?}"
        );
        assert!(adapter.vms.read().await.contains_key("vm-rs"));
    }

    /// delete_vm removes the adapter-owned runtime artifacts (api socket,
    /// ch.pid, vm-config.json) alongside the VMM process.
    #[tokio::test]
    async fn delete_vm_removes_runtime_artifacts() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let vm_dir = dir.path().join("vm-del");
        std::fs::create_dir_all(&vm_dir).unwrap();
        let api_socket = vm_dir.join("vm.sock");
        std::fs::write(&api_socket, b"").unwrap();
        std::fs::write(vm_dir.join("ch.pid"), "12345").unwrap();
        std::fs::write(vm_dir.join("vm-config.json"), "{}").unwrap();

        let adapter = ProcessCloudHypervisorAdapter::new(dir.path().join("chv"));
        let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
        let mut child = tokio::process::Command::new("true").spawn().unwrap();
        let _ = child.wait().await;
        {
            let mut map = adapter.vms.write().await;
            map.insert(
                "vm-del".to_string(),
                VmProcess {
                    api_socket: api_socket.clone(),
                    child: VmmChild::Owned(child),
                    console_io: std::fs::File::open("/dev/null").unwrap().into(),
                    serial_transport: SerialTransport::Socket(vm_dir.join("serial.sock")),
                    pty_tx,
                    pty_scrollback: Arc::new(tokio::sync::RwLock::new(Vec::new())),
                    broadcaster_alive: Arc::new(AtomicBool::new(false)),
                    last_cpu_seconds: 0.0,
                    last_cpu_at: None,
                },
            );
        }

        adapter.delete_vm("vm-del", None).await.unwrap();
        assert!(!api_socket.exists(), "api socket must be removed");
        assert!(!vm_dir.join("ch.pid").exists(), "pidfile must be removed");
        assert!(
            !vm_dir.join("vm-config.json").exists(),
            "persisted config must be removed"
        );
        assert!(!adapter.vms.read().await.contains_key("vm-del"));
    }

    /// Lifecycle operations serialize per VM: with the per-VM mutex held
    /// by an external holder, a lifecycle op on the same VM must block
    /// instead of running concurrently (two concurrent starts over an
    /// exited VMM would otherwise both re-spawn a process for it). An op
    /// on a DIFFERENT VM must not be blocked by the first holder.
    #[tokio::test]
    async fn lifecycle_ops_serialize_per_vm() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let adapter = ProcessCloudHypervisorAdapter::new(dir.path().join("chv"));

        let held = adapter.vm_op_lock("vm-a");
        let _guard = held.lock_owned().await;

        // Same VM: must block — without serialization it would complete
        // immediately with NotFound (the map is empty).
        let start_a = adapter.start_vm("vm-a", None);
        match tokio::time::timeout(std::time::Duration::from_millis(200), start_a).await {
            Err(_elapsed) => {} // still blocked — serialization holds
            Ok(res) => panic!("start on vm-a ran while its lifecycle lock was held: {res:?}"),
        }

        // Different VM: must not be blocked by vm-a's holder (it fails
        // fast with NotFound instead).
        let start_b = adapter.start_vm("vm-b", None);
        match tokio::time::timeout(std::time::Duration::from_millis(200), start_b).await {
            Ok(Err(ChvError::NotFound { .. })) => {}
            other => panic!("start on vm-b should fail fast with NotFound, got {other:?}"),
        }
    }

    /// create refuses to run a second VMM for a VM whose runtime dir
    /// already hosts a live one — by pidfile identity, and by its api
    /// socket answering — while a dead pid and a stale (non-listening)
    /// socket file do not block a create.
    #[tokio::test]
    async fn create_refuses_over_live_vmm() {
        use std::io::Write as _;

        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();

        // --- Case 1: live VMM identified by pidfile + cmdline + exe. ---
        let live_dir = dir.path().join("vms").join("vm-live");
        std::fs::create_dir_all(&live_dir).unwrap();
        let live_api_socket = live_dir.join("vm.sock");
        let mut orphan = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("sleep 5; true")
            .arg("--api-socket")
            .arg(&live_api_socket)
            .spawn()
            .unwrap();
        let orphan_pid = orphan.id().expect("freshly spawned child has a pid");
        wait_for_cmdline(orphan_pid).await;
        std::fs::write(live_dir.join("ch.pid"), format!("{orphan_pid}")).unwrap();
        let orphan_exe = std::fs::read_link(format!("/proc/{orphan_pid}/exe")).unwrap();

        let adapter = ProcessCloudHypervisorAdapter::new(orphan_exe.clone());
        let cfg = VmConfig {
            vm_id: "vm-live".to_string(),
            cpus: 1,
            memory_bytes: 512 * 1024 * 1024,
            kernel_path: dir.path().join("kernel"), // unused by the guard
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: live_api_socket.clone(),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        };
        let err = adapter.ensure_no_live_vmm(&cfg).await.unwrap_err();
        match &err {
            ChvError::Internal { reason } => {
                assert!(
                    reason.contains("already running"),
                    "unexpected error: {reason}"
                );
            }
            other => panic!("expected Internal, got {other:?}"),
        }

        let _ = orphan.start_kill();
        let _ = orphan.wait().await;

        // --- Case 2: dead pid + stale non-listening socket: no refusal. ---
        std::fs::write(&live_api_socket, b"").unwrap(); // stale file
        adapter.ensure_no_live_vmm(&cfg).await.unwrap();

        // --- Case 3: no pidfile, but the api socket answers: refusal. ---
        let anon_dir = dir.path().join("vms").join("vm-anon");
        std::fs::create_dir_all(&anon_dir).unwrap();
        let anon_api_socket = anon_dir.join("vm.sock");
        let responder = std::os::unix::net::UnixListener::bind(&anon_api_socket).unwrap();
        let server = std::thread::spawn(move || {
            if let Ok((mut conn, _)) = responder.accept() {
                let _ = conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
                // Hold the connection open until the test ends.
                std::thread::sleep(std::time::Duration::from_millis(2500));
            }
        });
        let cfg_anon = VmConfig {
            vm_id: "vm-anon".to_string(),
            cpus: 1,
            memory_bytes: 512 * 1024 * 1024,
            kernel_path: dir.path().join("kernel"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: anon_api_socket.clone(),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        };
        let err = adapter.ensure_no_live_vmm(&cfg_anon).await.unwrap_err();
        match &err {
            ChvError::Internal { reason } => {
                assert!(
                    reason.contains("answered by a running cloud-hypervisor"),
                    "unexpected error: {reason}"
                );
            }
            other => panic!("expected Internal, got {other:?}"),
        }
        server.join().expect("responder thread");
    }

    #[test]
    fn validate_vm_config_rejects_missing_kernel() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let chv_bin = dir.path().join("cloud-hypervisor");
        std::fs::write(&chv_bin, b"#!/bin/true").unwrap();
        let adapter = ProcessCloudHypervisorAdapter::new(chv_bin);
        let cfg = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 1,
            memory_bytes: 512 * 1024 * 1024,
            kernel_path: dir.path().join("missing-kernel"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: PathBuf::from("/tmp/chv/vms/vm-1/vm.sock"),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        };
        let err = adapter.validate_vm_config(&cfg).unwrap_err();
        assert!(matches!(err, ChvError::InvalidArgument { field, .. } if field == "kernel_path"));
    }

    #[test]
    fn validate_vm_config_accepts_existing_paths() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let chv_bin = dir.path().join("cloud-hypervisor");
        let kernel = dir.path().join("vmlinux");
        let disk = dir.path().join("root.img");
        std::fs::write(&chv_bin, b"#!/bin/true").unwrap();
        std::fs::write(&kernel, b"kernel").unwrap();
        std::fs::write(&disk, b"disk").unwrap();
        let adapter = ProcessCloudHypervisorAdapter::new(chv_bin);
        let cfg = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 1,
            memory_bytes: 512 * 1024 * 1024,
            kernel_path: kernel,
            firmware_path: None,
            disks: vec![VmDiskConfig {
                path: disk,
                read_only: false,
                id: None,
            }],
            nics: vec![],
            api_socket_path: PathBuf::from("/tmp/chv/vms/vm-1/vm.sock"),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        };
        adapter.validate_vm_config(&cfg).unwrap();
    }

    #[test]
    fn nested_cpu_config_includes_complete_topology_for_cloud_hypervisor_v51() {
        let cfg = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 2,
            memory_bytes: 512 * 1024 * 1024,
            kernel_path: PathBuf::from("/tmp/kernel"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: PathBuf::from("/tmp/chv/vms/vm-1/vm.sock"),
            cloud_init_userdata: None,
            hypervisor_overrides: Some(HypervisorOverrides {
                cpu_nested: Some(true),
                ..Default::default()
            }),
        };

        let cpus = build_cpus_config(&cfg);

        assert_eq!(cpus["boot_vcpus"], 2);
        assert_eq!(cpus["max_vcpus"], 2);
        assert_eq!(cpus["topology"]["threads_per_core"], 1);
        assert_eq!(cpus["topology"]["cores_per_die"], 2);
        assert_eq!(cpus["topology"]["dies_per_package"], 1);
        assert_eq!(cpus["topology"]["packages"], 1);
    }

    #[tokio::test]
    async fn stop_vm_force_clears_console_cache() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let vm_dir = dir.path().join("vm-test");
        std::fs::create_dir_all(&vm_dir).unwrap();

        // Create a dummy console.log with some content.
        let console_log = vm_dir.join("console.log");
        std::fs::write(&console_log, b"boot log line 1\nboot log line 2\n").unwrap();

        // Spawn a short-lived child process that we can kill.
        let child = tokio::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .unwrap();

        // Create a fake VmProcess directly in the adapter's map.
        let adapter = ProcessCloudHypervisorAdapter::new(dir.path().join("chv"));
        let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
        let pty_scrollback = Arc::new(tokio::sync::RwLock::new(Vec::from(b"scrollback data")));
        let console_io = std::fs::File::open("/dev/null").unwrap().into();

        {
            let mut map = adapter.vms.write().await;
            map.insert(
                "vm-test".to_string(),
                VmProcess {
                    api_socket: vm_dir.join("vm.sock"),
                    child: VmmChild::Owned(child),
                    console_io,
                    serial_transport: SerialTransport::Pty,
                    pty_tx: pty_tx.clone(),
                    pty_scrollback: pty_scrollback.clone(),
                    broadcaster_alive: Arc::new(AtomicBool::new(false)),
                    last_cpu_seconds: 0.0,
                    last_cpu_at: None,
                },
            );
        }

        // Pre-stop assertions.
        assert_eq!(
            adapter.pty_scrollback("vm-test").await.unwrap(),
            b"scrollback data"
        );
        assert!(console_log.exists());

        // Force stop should clear scrollback and remove console.log.
        adapter
            .stop_vm("vm-test", true, Some("op-test"))
            .await
            .unwrap();

        // Post-stop assertions.
        assert!(adapter.pty_scrollback("vm-test").await.is_none());
        assert!(
            !console_log.exists(),
            "console.log should be removed on force stop"
        );
    }

    #[tokio::test]
    async fn stop_vm_graceful_clears_console_cache() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let vm_dir = dir.path().join("vm-test");
        std::fs::create_dir_all(&vm_dir).unwrap();

        // Create a dummy console.log with some content.
        let console_log = vm_dir.join("console.log");
        std::fs::write(&console_log, b"boot log line 1\nboot log line 2\n").unwrap();

        // For graceful stop we need a real CHV process or we skip the API part.
        // Since we can't spawn real CHV, we test the cache-clear path by
        // simulating what happens after the API shutdown succeeds: the VmProcess
        // stays in the map and we clear its scrollback and truncate the log.
        // We test this by manually exercising the internal logic via the adapter.
        let adapter = ProcessCloudHypervisorAdapter::new(dir.path().join("chv"));
        let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
        let pty_scrollback = Arc::new(tokio::sync::RwLock::new(Vec::from(b"scrollback data")));
        let console_io = std::fs::File::open("/dev/null").unwrap().into();

        // Spawn a child that exits immediately so the graceful shutdown loop
        // breaks early (CH process disappeared).
        let mut child = tokio::process::Command::new("true").spawn().unwrap();
        let _ = child.wait().await;

        {
            let mut map = adapter.vms.write().await;
            map.insert(
                "vm-test".to_string(),
                VmProcess {
                    api_socket: vm_dir.join("vm.sock"),
                    child: VmmChild::Owned(child),
                    console_io,
                    serial_transport: SerialTransport::Pty,
                    pty_tx: pty_tx.clone(),
                    pty_scrollback: pty_scrollback.clone(),
                    broadcaster_alive: Arc::new(AtomicBool::new(false)),
                    last_cpu_seconds: 0.0,
                    last_cpu_at: None,
                },
            );
        }

        // Pre-stop assertions.
        assert_eq!(
            adapter.pty_scrollback("vm-test").await.unwrap(),
            b"scrollback data"
        );
        assert!(console_log.exists());
        let pre_size = std::fs::metadata(&console_log).unwrap().len();
        assert!(pre_size > 0);

        // Graceful stop: the CH API calls will fail (no real socket), so the
        // loop breaks with "CH process disappeared" and then clears caches.
        adapter
            .stop_vm("vm-test", false, Some("op-test"))
            .await
            .unwrap();

        // Post-stop assertions: scrollback cleared, log truncated.
        assert_eq!(adapter.pty_scrollback("vm-test").await.unwrap(), b"");
        assert!(
            console_log.exists(),
            "console.log should still exist after graceful stop"
        );
        let post_size = std::fs::metadata(&console_log).unwrap().len();
        assert_eq!(
            post_size, 0,
            "console.log should be truncated on graceful stop"
        );
    }

    #[test]
    fn adopted_vm_handle_revalidates_successfully() {
        use super::AdoptedVmHandle;
        use std::fs;

        let temp = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let proc_path = temp.path().join("proc");
        fs::create_dir(&proc_path).unwrap();

        let pid = 1234;
        let pid_path = proc_path.join(pid.to_string());
        fs::create_dir(&pid_path).unwrap();

        fs::write(pid_path.join("stat"), b"1234 (bash) S 1 1 1 0 -1 4210944 1 0 0 0 0 0 0 0 20 0 1 0 12345 0 0 18446744073709551615 0 0 0 0 0 0 0 2147483647 0 0 0 0 17 0 0 0 0 0 0 0 0 0 0 0 0 0 0").unwrap();

        fs::write(
            pid_path.join("status"),
            b"Uid:\t1000\t1000\t1000\t1000\nGid:\t1000\t1000\t1000\t1000\n",
        )
        .unwrap();
        fs::write(pid_path.join("cgroup"), b"0::/test\n").unwrap();

        let sys_kernel_random = proc_path.join("sys").join("kernel").join("random");
        fs::create_dir_all(&sys_kernel_random).unwrap();
        fs::write(
            sys_kernel_random.join("boot_id"),
            b"00000000-0000-4000-8000-000000000001\n",
        )
        .unwrap();

        let fake_exe = temp.path().join("fake_exe");
        fs::write(&fake_exe, b"").unwrap();
        std::os::unix::fs::symlink(&fake_exe, pid_path.join("exe")).unwrap();

        use std::os::unix::fs::MetadataExt;
        let exe_meta = fs::metadata(&fake_exe).unwrap();
        let identity = cellhv_core_runtime_ownership::ProcessIdentity {
            pid,
            start_ticks: 12345,
            boot_id: "00000000-0000-4000-8000-000000000001".to_string(),
            executable: cellhv_core_runtime_ownership::FileIdentity {
                device: exe_meta.dev(),
                inode: exe_meta.ino(),
            },
            uid: 1000,
            gid: 1000,
            cgroup_fingerprint: "/test".to_string(),
        };

        let handle = AdoptedVmHandle::new(
            "vm-test".to_string(),
            temp.path().join("vm.sock"),
            identity.clone(),
        )
        .with_proc_path(proc_path.clone())
        .with_runtime_root(temp.path().to_path_buf());

        let r = handle.revalidate();
        assert!(r.is_ok());

        fs::write(pid_path.join("stat"), b"1234 (bash) S 1 1 1 0 -1 4210944 1 0 0 0 0 0 0 0 20 0 1 0 99999 0 0 18446744073709551615 0 0 0 0 0 0 0 2147483647 0 0 0 0 17 0 0 0 0 0 0 0 0 0 0 0 0 0 0").unwrap();
        assert!(handle.revalidate().is_err());
    }

    #[test]
    fn start_vm_action_paused_is_already_running() {
        assert!(matches!(
            super::start_vm_action("Paused"),
            super::StartVmAction::AlreadyRunning
        ));
    }

    #[test]
    fn start_vm_action_running_is_already_running() {
        assert!(matches!(
            super::start_vm_action("Running"),
            super::StartVmAction::AlreadyRunning
        ));
    }

    #[test]
    fn start_vm_action_other_states_map_to_boot() {
        for state in ["", "Created", "Booted", "Failed", "garbage"] {
            assert!(
                matches!(super::start_vm_action(state), super::StartVmAction::Boot),
                "state {state:?} should map to Boot"
            );
        }
    }

    /// Linux process state from /proc/<pid>/stat ('S' = sleeping, 'Z' = zombie
    /// i.e. terminated-but-not-yet-reaped, None = fully gone).
    fn proc_state(pid: u32) -> Option<char> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // Format: `pid (comm) state ...`. comm can contain spaces/parens, so
        // the state is the token immediately after the last ')'.
        let close = stat.rfind(')')?;
        stat.as_bytes().get(close + 2).copied().map(char::from)
    }

    /// Empty spawn helper mirroring the CH spawn shape (null stdio, long sleep).
    fn spawn_sleep_child() -> (tokio::process::Child, u32) {
        let child = tokio::process::Command::new("sleep")
            .arg("300")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id().unwrap();
        (child, pid)
    }

    /// Proves the orphan-window guard in the CH spawn path: while a CH child
    /// is spawned but not yet registered in the vm process map, dropping an
    /// *armed* `ChildGuard` must terminate the VMM — otherwise an aborted
    /// create (e.g. a bounded-drain cancellation) leaves an unaccounted live
    /// process.
    #[tokio::test]
    async fn child_guard_armed_drop_terminates_child() {
        let (child, pid) = spawn_sleep_child();
        assert!(
            proc_state(pid).is_some(),
            "child should be alive before drop"
        );
        drop(super::ChildGuard::new(child));
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if matches!(proc_state(pid), None | Some('Z')) {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "child was not terminated by armed ChildGuard drop"
            );
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }

    /// Proves the counterpart: after registration the vm process map owns the
    /// child, so `disarm()` must return a live child whose ordinary drop does
    /// NOT kill the VMM (lifecycle is explicit via start_kill on stop/delete).
    #[tokio::test]
    async fn child_guard_disarm_keeps_child_alive() {
        let (child, pid) = spawn_sleep_child();
        let mut child = super::ChildGuard::new(child).disarm();
        assert!(
            !matches!(proc_state(pid), None | Some('Z')),
            "child must remain running after disarm"
        );
        // Clean up and reap this test's child so it does not linger on the host.
        child.start_kill().unwrap();
        child.wait().await.unwrap();
    }
}
