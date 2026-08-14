use async_trait::async_trait;
use chv_errors::ChvError;
use std::collections::HashMap;
use std::io::{Read as _, Seek, SeekFrom};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::process::Child;
use tracing::{info, warn};

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

struct VmProcess {
    api_socket: std::path::PathBuf,
    child: Child,
    pty_master: OwnedFd,
    pty_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
    pty_scrollback: Arc<tokio::sync::RwLock<Vec<u8>>>,
    broadcaster_alive: Arc<AtomicBool>,
    last_cpu_seconds: f64,
    last_cpu_at: Option<std::time::Instant>,
}

pub struct ProcessCloudHypervisorAdapter {
    chv_binary: std::path::PathBuf,
    vms: Arc<tokio::sync::RwLock<HashMap<String, VmProcess>>>,
}

impl ProcessCloudHypervisorAdapter {
    pub fn new(chv_binary: impl Into<std::path::PathBuf>) -> Self {
        Self {
            chv_binary: chv_binary.into(),
            vms: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
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
    let output = tokio::process::Command::new("genisoimage")
        .arg("-output")
        .arg(&seed_iso)
        .arg("-volid")
        .arg("cidata")
        .arg("-joliet")
        .arg("-rock")
        .arg(seed_dir.join("user-data"))
        .arg(seed_dir.join("meta-data"))
        .arg(seed_dir.join("network-config"))
        .output()
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to run genisoimage: {}", e),
        })?;

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

impl ProcessCloudHypervisorAdapter {
    fn spawn_pty_broadcaster(
        vm_id: String,
        pty_fd: std::os::fd::RawFd,
        pty_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
        pty_scrollback: Arc<tokio::sync::RwLock<Vec<u8>>>,
        broadcaster_alive: Arc<AtomicBool>,
    ) {
        tokio::spawn(async move {
            let _guard = AliveGuard(broadcaster_alive);
            let std_file = unsafe { std::fs::File::from_raw_fd(pty_fd) };
            let mut reader = tokio::io::BufReader::new(tokio::fs::File::from_std(std_file));
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => break,
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
                    Err(e) => {
                        tracing::debug!(vm_id = %vm_id, error = %e, "pty broadcaster read error");
                        break;
                    }
                }
            }
            tracing::debug!(vm_id = %vm_id, "pty broadcaster exited");
        });
    }
}

/// What `start_vm` should do for a VM whose `vm.info` reported `state`.
///
/// A paused VM must be resumed rather than treated as a successful no-op:
/// a migration failure can leave the VM paused, and the reconciler relies on
/// `start_vm` to drive it back to Running.
enum StartVmAction {
    /// VM is already running: start is an idempotent no-op.
    AlreadyRunning,
    /// VM is paused: resume it (reconciler self-healing).
    Resume,
    /// Any other state: boot the VM.
    Boot,
}

fn start_vm_action(state: &str) -> StartVmAction {
    match state {
        "Running" => StartVmAction::AlreadyRunning,
        "Paused" => StartVmAction::Resume,
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
        if !std::path::Path::new("/dev/kvm").exists() {
            return Err(ChvError::Internal {
                reason: "Host does not have KVM capability (/dev/kvm missing). VMs require hardware virtualization.".into(),
            });
        }
        self.validate_vm_config(config)?;
        if config.api_socket_path.exists() {
            tokio::fs::remove_file(&config.api_socket_path)
                .await
                .map_err(|e| ChvError::Io {
                    path: config.api_socket_path.to_string_lossy().to_string(),
                    source: e,
                })?;
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

        info!(
            vm_id = %config.vm_id,
            socket = %config.api_socket_path.display(),
            binary = %self.chv_binary.display(),
            op = operation_id.unwrap_or("-"),
            "spawning cloud-hypervisor"
        );

        let mut child = cmd.spawn().map_err(|e| ChvError::Io {
            path: self.chv_binary.to_string_lossy().to_string(),
            source: e,
        })?;

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

        let serial_mode = hv.and_then(|h| h.serial_mode.as_deref()).unwrap_or("Pty");
        let console_mode = hv.and_then(|h| h.console_mode.as_deref()).unwrap_or("Off");

        let mut vm_config_json = serde_json::json!({
            "cpus": cpus,
            "memory": memory,
            "payload": payload,
            "disks": disks_json,
            "net": net_json,
            "serial": { "mode": serial_mode },
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

        // Query CHV for the PTY slave path it created for the serial device.
        let (info_status, info_body) =
            Self::ch_api_request_with_body(&config.api_socket_path, "GET", "/api/v1/vm.info", None)
                .await?;
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
        }
        .ok_or_else(|| {
            let _ = child.start_kill();
            ChvError::Internal {
                reason: format!(
                    "vm.info did not return serial PTY path for vm {}",
                    config.vm_id
                ),
            }
        })?;

        // Open the PTY slave that CHV created.  This is the I/O endpoint
        // for the guest serial console — we read guest output from it and
        // write user keystrokes to it.
        let pty_slave = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(nix::libc::O_NOCTTY)
            .open(&slave_path)
            .map_err(|e| ChvError::Io {
                path: slave_path.clone(),
                source: e,
            })?;
        // Set raw mode so the host line discipline doesn't buffer or echo
        // keystrokes — we want every byte forwarded to the guest immediately.
        if let Ok(mut term) = nix::sys::termios::tcgetattr(&pty_slave) {
            nix::sys::termios::cfmakeraw(&mut term);
            let _ =
                nix::sys::termios::tcsetattr(&pty_slave, nix::sys::termios::SetArg::TCSANOW, &term);
        }
        let pty_fd_raw = pty_slave.into_raw_fd();
        let _ = nix::fcntl::fcntl(
            pty_fd_raw,
            nix::fcntl::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
        );

        info!(
            vm_id = %config.vm_id,
            pty = %slave_path,
            op = operation_id.unwrap_or("-"),
            "chv serial pty ready"
        );

        let (pty_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(4096);
        let pty_scrollback = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let broadcaster_alive = Arc::new(AtomicBool::new(true));

        // Dup the fd for the broadcaster BEFORE OwnedFd takes ownership
        let broadcaster_fd = unsafe { nix::libc::dup(pty_fd_raw) };
        if broadcaster_fd >= 0 {
            let _ = nix::fcntl::fcntl(
                broadcaster_fd,
                nix::fcntl::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
            );
        }

        let mut map = self.vms.write().await;
        map.insert(
            config.vm_id.clone(),
            VmProcess {
                api_socket: config.api_socket_path.clone(),
                child,
                pty_master: unsafe { OwnedFd::from_raw_fd(pty_fd_raw) },
                pty_tx: pty_tx.clone(),
                pty_scrollback: pty_scrollback.clone(),
                broadcaster_alive: broadcaster_alive.clone(),
                last_cpu_seconds: 0.0,
                last_cpu_at: None,
            },
        );
        drop(map);

        // Spawn background broadcaster: read PTY output and fan out via broadcast channel
        if broadcaster_fd >= 0 {
            Self::spawn_pty_broadcaster(
                config.vm_id.clone(),
                broadcaster_fd,
                pty_tx.clone(),
                pty_scrollback.clone(),
                broadcaster_alive.clone(),
            );
        }

        // Subscribe to broadcast channel and persist to console.log
        let log_path = vm_runtime_dir.join("console.log");
        let vm_id_log = config.vm_id.clone();
        let mut pty_rx_log = pty_tx.subscribe();
        tokio::spawn(async move {
            let log_file = tokio::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&log_path)
                .await;
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

        __guard.succeeded = true;
        Ok(config.vm_id.clone())
    }

    async fn start_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let mut __guard = VmOpGuard::new("start");
        let (api_socket, pty_master_fd, pty_tx, pty_scrollback, broadcaster_alive) = {
            let vms = self.vms.read().await;
            let proc = vms.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            (
                proc.api_socket.clone(),
                proc.pty_master.as_raw_fd(),
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
        if !broadcaster_alive.load(Ordering::SeqCst) {
            info!(vm_id = %vm_id, "respawning pty broadcaster");
            let broadcaster_fd = unsafe { nix::libc::dup(pty_master_fd) };
            if broadcaster_fd >= 0 {
                let _ = nix::fcntl::fcntl(
                    broadcaster_fd,
                    nix::fcntl::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
                );
                broadcaster_alive.store(true, Ordering::SeqCst);
                Self::spawn_pty_broadcaster(
                    vm_id.to_string(),
                    broadcaster_fd,
                    pty_tx.clone(),
                    pty_scrollback.clone(),
                    broadcaster_alive.clone(),
                );
            }
        }

        match action {
            StartVmAction::Resume => {
                // A migration failure can leave the VM paused; resume it so
                // the reconciler drives it back to Running (self-healing).
                info!(vm_id = %vm_id, state = %state, "vm is paused; resuming via ch api");
                let status =
                    Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.resume", None).await?;
                Self::expect_status(status, "vm.resume")?;
                // Idempotent success: the VM is now in the desired state.
                // The guard's succeeded flag must be set on every Ok return
                // so RED metrics classify this as ok, not err.
                __guard.succeeded = true;
                return Ok(());
            }
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
                let _ = proc.child.start_kill();
                let _ = proc.child.try_wait();
                log_path
            } else {
                None
            };
            if let Some(path) = log_path {
                let _ = tokio::fs::remove_file(&path).await;
                info!(vm_id = %vm_id, path = %path.display(), "removed console.log on force stop");
            }
        } else {
            // Graceful stop: ask the VM to shut down but keep the VMM daemon
            // alive so a subsequent vm.boot can restart it. Do NOT call
            // vmm.shutdown — that would kill the daemon and require a full
            // vm.create + vm.boot sequence to start again.
            // Graceful stop: send ACPI power button so the guest OS can shut
            // itself down cleanly. Then poll vm.info for up to 10s waiting for
            // the VM to reach a non-running terminal state (Shutdown or Created).
            let _ = Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.power-button", None).await;
            // Poll vm.info for up to 10s waiting for the VM to reach a
            // non-running terminal state (Shutdown or Created).
            let start = std::time::Instant::now();
            let mut graceful_shutdown = false;
            while start.elapsed() < std::time::Duration::from_secs(10) {
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
                    let _ = proc.child.start_kill();
                    let _ = proc.child.try_wait();
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
        let mut proc = {
            let mut map = self.vms.write().await;
            map.remove(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?
        };

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "deleting vm");

        let _ = proc.child.start_kill();
        let _ = proc.child.wait().await;
        let _ = tokio::fs::remove_file(&proc.api_socket).await;
        __guard.succeeded = true;
        Ok(())
    }

    async fn reboot_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let api_socket = self.get_vm_socket(vm_id).await?;

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "rebooting vm via ch api");

        let status = Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.reboot", None).await?;
        if status != 200 && status != 204 {
            warn!(status = status, "unexpected status from vm.reboot");
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
        let fd = nix::unistd::dup(proc.pty_master.as_raw_fd()).ok()?;
        Some(unsafe { OwnedFd::from_raw_fd(fd) })
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
    pub async fn adopt_running_vms(&self, _runtime_root: &std::path::Path) -> Result<(), ChvError> {
        // - Discover canonical runtime directories.
        // - Validate OwnerMarkerV1 ownership marker, PID, /proc/<pid>/stat start time, boot ID, executable identity, UID/GID, cgroup identity, socket inode, and peer credentials.
        // - Probe Cloud Hypervisor API and prove no conflicting candidate exists.
        // - Fail-closed classification of owned, foreign, ambiguous, stale, and missing processes.
        // - Adopt exact matches without requiring the original Tokio Child handle.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::build_cpus_config;
    use super::parse_http_status;
    use super::ProcessCloudHypervisorAdapter;
    use super::VmProcess;
    use crate::adapter::{CloudHypervisorAdapter, VmConfig, VmDiskConfig};
    use chv_common::hypervisor::HypervisorOverrides;
    use chv_errors::ChvError;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
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
        let pty_master = std::fs::File::open("/dev/null").unwrap().into();

        {
            let mut map = adapter.vms.write().await;
            map.insert(
                "vm-test".to_string(),
                VmProcess {
                    api_socket: vm_dir.join("vm.sock"),
                    child,
                    pty_master,
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
        let pty_master = std::fs::File::open("/dev/null").unwrap().into();

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
                    child,
                    pty_master,
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
    fn start_vm_action_paused_maps_to_resume() {
        assert!(matches!(
            super::start_vm_action("Paused"),
            super::StartVmAction::Resume
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
}
