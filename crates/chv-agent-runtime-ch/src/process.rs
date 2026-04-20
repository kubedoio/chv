use async_trait::async_trait;
use chv_errors::ChvError;
use std::collections::HashMap;
use std::fmt::Write;
use std::io::{Read as _, Seek, SeekFrom};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::process::Child;
use tracing::{info, warn};

use crate::adapter::{AddDiskParams, AddNetParams, CloudHypervisorAdapter, VmConfig, VmInfo};

struct VmProcess {
    api_socket: std::path::PathBuf,
    child: Child,
    pty_master: OwnedFd,
}

pub struct ProcessCloudHypervisorAdapter {
    chv_binary: std::path::PathBuf,
    vms: Arc<std::sync::Mutex<HashMap<String, VmProcess>>>,
}

impl ProcessCloudHypervisorAdapter {
    pub fn new(chv_binary: impl Into<std::path::PathBuf>) -> Self {
        Self {
            chv_binary: chv_binary.into(),
            vms: Arc::new(std::sync::Mutex::new(HashMap::new())),
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

    async fn ch_api_request(
        socket: &Path,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<u16, ChvError> {
        let (status, _body) = Self::ch_api_request_with_body(socket, method, path, body).await?;
        Ok(status)
    }

    async fn ch_api_request_with_body(
        socket: &Path,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<(u16, String), ChvError> {
        let mut stream = UnixStream::connect(socket)
            .await
            .map_err(|e| ChvError::Io {
                path: socket.to_string_lossy().to_string(),
                source: e,
            })?;

        let mut request = format!("{} {} HTTP/1.1\r\nHost: localhost\r\n", method, path);
        if let Some(b) = body {
            let _ = write!(&mut request, "Content-Length: {}\r\n", b.len());
            request.push_str("Content-Type: application/json\r\n");
            request.push_str("\r\n");
            request.push_str(b);
        } else {
            request.push_str("Content-Length: 0\r\n");
            request.push_str("\r\n");
        }

        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|e| ChvError::Io {
                path: socket.to_string_lossy().to_string(),
                source: e,
            })?;

        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        let read_fut = async {
            loop {
                let n = stream.read(&mut tmp).await.map_err(|e| ChvError::Io {
                    path: socket.to_string_lossy().to_string(),
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
                        .and_then(|l| l.splitn(2, ':').nth(1))
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
        tokio::time::timeout(std::time::Duration::from_secs(30), read_fut)
            .await
            .map_err(|_| ChvError::Io {
                path: socket.to_string_lossy().to_string(),
                source: std::io::Error::new(std::io::ErrorKind::TimedOut, "socket read timed out"),
            })??;

        let raw = String::from_utf8_lossy(&buf);
        let status_code = parse_http_status(raw.as_bytes()).unwrap_or(0);
        let response_body = if let Some(idx) = raw.find("\r\n\r\n") {
            raw[idx + 4..].to_string()
        } else {
            String::new()
        };
        Ok((status_code, response_body))
    }

    fn validate_vm_config(&self, config: &VmConfig) -> Result<(), ChvError> {
        if !self.chv_binary.exists() {
            return Err(ChvError::InvalidArgument {
                field: "chv_binary_path".to_string(),
                reason: format!("binary not found: {}", self.chv_binary.display()),
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
    text.lines()
        .last()
        .unwrap_or("<empty>")
        .to_string()
}

async fn build_cloud_init_seed(
    vm_dir: &Path,
    vm_id: &str,
    userdata: &str,
) -> Result<std::path::PathBuf, ChvError> {
    let seed_dir = vm_dir.join("seed");
    tokio::fs::create_dir_all(&seed_dir).await.map_err(|e| ChvError::Io {
        path: seed_dir.to_string_lossy().to_string(),
        source: e,
    })?;

    let meta_data = format!("instance-id: {}\nlocal-hostname: {}\n", vm_id, vm_id);
    tokio::fs::write(seed_dir.join("meta-data"), meta_data.as_bytes()).await.map_err(|e| ChvError::Io {
        path: seed_dir.join("meta-data").to_string_lossy().to_string(),
        source: e,
    })?;

    tokio::fs::write(seed_dir.join("user-data"), userdata.as_bytes()).await.map_err(|e| ChvError::Io {
        path: seed_dir.join("user-data").to_string_lossy().to_string(),
        source: e,
    })?;

    let network_config = "version: 2\nethernets:\n  id0:\n    match:\n      driver: virtio*\n    dhcp4: true\n";
    tokio::fs::write(seed_dir.join("network-config"), network_config.as_bytes()).await.map_err(|e| ChvError::Io {
        path: seed_dir.join("network-config").to_string_lossy().to_string(),
        source: e,
    })?;

    let seed_iso = vm_dir.join("seed.iso");
    let output = tokio::process::Command::new("genisoimage")
        .arg("-output").arg(&seed_iso)
        .arg("-volid").arg("cidata")
        .arg("-joliet").arg("-rock")
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
            reason: format!("genisoimage failed: {}", String::from_utf8_lossy(&output.stderr)),
        });
    }

    Ok(seed_iso)
}

fn parse_http_status(response_bytes: &[u8]) -> Option<u16> {
    let response = String::from_utf8_lossy(response_bytes);
    let status_line = response.lines().next()?;
    let parts: Vec<&str> = status_line.split_whitespace().collect();
    parts.get(1)?.parse::<u16>().ok()
}

#[async_trait]
impl CloudHypervisorAdapter for ProcessCloudHypervisorAdapter {
    async fn create_vm(
        &self,
        config: &VmConfig,
        operation_id: Option<&str>,
    ) -> Result<String, ChvError> {
        self.validate_vm_config(config)?;
        if config.api_socket_path.exists() {
            tokio::fs::remove_file(&config.api_socket_path)
                .await
                .map_err(|e| ChvError::Io {
                    path: config.api_socket_path.to_string_lossy().to_string(),
                    source: e,
                })?;
        }

        let pty_master = nix::pty::posix_openpt(nix::fcntl::OFlag::O_RDWR).map_err(|e| ChvError::Io {
            path: "pty".to_string(),
            source: std::io::Error::from_raw_os_error(e as i32),
        })?;
        nix::pty::grantpt(&pty_master).map_err(|e| ChvError::Io {
            path: "pty".to_string(),
            source: std::io::Error::from_raw_os_error(e as i32),
        })?;
        nix::pty::unlockpt(&pty_master).map_err(|e| ChvError::Io {
            path: "pty".to_string(),
            source: std::io::Error::from_raw_os_error(e as i32),
        })?;
        let slave_path = unsafe { nix::pty::ptsname(&pty_master) }.map_err(|e| ChvError::Io {
            path: "pty".to_string(),
            source: std::io::Error::from_raw_os_error(e as i32),
        })?;

        let mut cmd = tokio::process::Command::new(&self.chv_binary);
        cmd.arg("--api-socket").arg(&config.api_socket_path);

        let vm_runtime_dir = config.api_socket_path
            .parent()
            .expect("api_socket_path must have a parent directory");

        // Build cloud-init seed ISO if userdata is provided
        let seed_iso_path = if let Some(ref userdata) = config.cloud_init_userdata {
            if !userdata.trim().is_empty() {
                match build_cloud_init_seed(vm_runtime_dir, &config.vm_id, userdata).await {
                    Ok(path) => Some(path),
                    Err(e) => {
                        warn!(vm_id = %config.vm_id, error = %e, "failed to build cloud-init seed ISO, continuing without it");
                        None
                    }
                }
            } else { None }
        } else { None };

        cmd.stdout(Stdio::null());

        let stderr_log_path = vm_runtime_dir.join("cloud-hypervisor.stderr.log");
        let stderr_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stderr_log_path);
        match stderr_file {
            Ok(f) => { cmd.stderr(Stdio::from(f)); }
            Err(e) => {
                warn!(error = %e, path = %stderr_log_path.display(), "failed to open stderr log, falling back to null");
                cmd.stderr(Stdio::null());
            }
        }

        info!(
            vm_id = %config.vm_id,
            socket = %config.api_socket_path.display(),
            pty = %slave_path,
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

        let pty_fd_raw = pty_master.into_raw_fd();

        // Build VM config JSON and create VM via REST API (supports multiple disks)
        let mut disks_json = Vec::new();
        for disk in &config.disks {
            let mut d = serde_json::Map::new();
            d.insert("path".into(), serde_json::Value::from(disk.path.to_string_lossy().to_string()));
            if disk.read_only {
                d.insert("readonly".into(), serde_json::Value::from(true));
            }
            disks_json.push(serde_json::Value::Object(d));
        }
        if let Some(ref seed_path) = seed_iso_path {
            let mut d = serde_json::Map::new();
            d.insert("path".into(), serde_json::Value::from(seed_path.to_string_lossy().to_string()));
            d.insert("readonly".into(), serde_json::Value::from(true));
            disks_json.push(serde_json::Value::Object(d));
        }

        let mut net_json = Vec::new();
        for nic in &config.nics {
            if nic.tap_name.is_empty() {
                warn!(mac = %nic.mac_address, "skipping NIC with empty tap_name");
                continue;
            }
            let mut n = serde_json::Map::new();
            n.insert("mac".into(), serde_json::Value::from(nic.mac_address.clone()));
            n.insert("tap".into(), serde_json::Value::from(nic.tap_name.clone()));
            net_json.push(serde_json::Value::Object(n));
        }

        let mut payload = serde_json::Map::new();
        if let Some(ref fw) = config.firmware_path {
            payload.insert("firmware".into(), serde_json::Value::from(fw.to_string_lossy().to_string()));
        } else {
            payload.insert("kernel".into(), serde_json::Value::from(config.kernel_path.to_string_lossy().to_string()));
        }

        let vm_config_json = serde_json::json!({
            "cpus": { "boot_vcpus": config.cpus, "max_vcpus": config.cpus },
            "memory": { "size": config.memory_bytes },
            "payload": payload,
            "disks": disks_json,
            "net": net_json,
            "serial": { "mode": "Tty" },
            "console": { "mode": "Off" }
        });

        let body = vm_config_json.to_string();
        let (create_status, create_body) = Self::ch_api_request_with_body(
            &config.api_socket_path, "PUT", "/api/v1/vm.create", Some(&body),
        ).await?;

        if create_status != 200 && create_status != 204 {
            let _ = child.start_kill();
            let _ = child.wait().await;
            let _ = tokio::fs::remove_file(&config.api_socket_path).await;
            return Err(ChvError::Internal {
                reason: format!("vm.create returned status {} for vm {}: {}", create_status, config.vm_id, create_body),
            });
        }

        let mut map = self.vms.lock().unwrap();
        map.insert(
            config.vm_id.clone(),
            VmProcess {
                api_socket: config.api_socket_path.clone(),
                child,
                pty_master: unsafe { OwnedFd::from_raw_fd(pty_fd_raw) },
            },
        );
        drop(map);

        // Spawn background logger: tee PTY output to console.log
        let log_path = vm_runtime_dir.join("console.log");
        let logger_fd = unsafe { nix::libc::dup(pty_fd_raw) };
        if logger_fd >= 0 {
            let vm_id_log = config.vm_id.clone();
            tokio::spawn(async move {
                let std_file = unsafe { std::fs::File::from_raw_fd(logger_fd) };
                let mut reader = tokio::io::BufReader::new(tokio::fs::File::from_std(std_file));
                let log_file = tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path)
                    .await;
                let mut writer = match log_file {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::debug!(vm_id = %vm_id_log, error = %e, "failed to open console.log");
                        return;
                    }
                };
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let _ = writer.write_all(&buf[..n]).await;
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        Ok(config.vm_id.clone())
    }

    async fn start_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "booting vm via ch api");

        let (info_status, info_body) =
            Self::ch_api_request_with_body(&api_socket, "GET", "/api/v1/vm.info", None).await?;
        if info_status == 200 {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&info_body) {
                let state = v.get("state").and_then(|s| s.as_str()).unwrap_or("");
                if state == "Running" || state == "Paused" {
                    info!(vm_id = %vm_id, state = %state, "vm already booted, skipping vm.boot");
                    return Ok(());
                }
            }
        }

        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.boot", None).await?;
        if status != 200 && status != 204 {
            warn!(vm_id = %vm_id, status = status, "vm.boot returned non-success (VM may have auto-booted)");
        }
        Ok(())
    }

    async fn stop_vm(
        &self,
        vm_id: &str,
        force: bool,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        info!(vm_id = %vm_id, force = force, op = operation_id.unwrap_or("-"), "stopping vm");

        if force {
            let mut map = self.vms.lock().unwrap();
            if let Some(mut proc) = map.remove(vm_id) {
                let _ = proc.child.start_kill();
                let _ = proc.child.try_wait();
            }
        } else {
            let _ = Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.shutdown", None).await;
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let _ = Self::ch_api_request(&api_socket, "PUT", "/api/v1/vmm.shutdown", None).await;
            let mut map = self.vms.lock().unwrap();
            if let Some(mut proc) = map.remove(vm_id) {
                let _ = proc.child.start_kill();
                let _ = proc.child.try_wait();
            }
        }
        Ok(())
    }

    async fn delete_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let mut proc = {
            let mut map = self.vms.lock().unwrap();
            map.remove(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?
        };

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "deleting vm");

        let _ = proc.child.start_kill();
        let _ = proc.child.wait().await;
        let _ = tokio::fs::remove_file(&proc.api_socket).await;
        Ok(())
    }

    async fn reboot_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "rebooting vm via ch api");

        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.reboot", None).await?;
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
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

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
        if status != 200 && status != 204 {
            return Err(ChvError::Internal {
                reason: format!("vm.resize returned unexpected status {}", status),
            });
        }
        Ok(())
    }

    async fn vm_info(&self, vm_id: &str) -> Result<VmInfo, ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        let (status, body) =
            Self::ch_api_request_with_body(&api_socket, "GET", "/api/v1/vm.info", None).await?;
        if status != 200 {
            return Err(ChvError::Internal {
                reason: format!("vm.info returned unexpected status {}", status),
            });
        }

        let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| ChvError::Internal {
            reason: format!("failed to parse vm.info response: {}", e),
        })?;

        let state = v
            .get("state")
            .and_then(|s| s.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let cpus = v
            .pointer("/config/cpus/boot_vcpus")
            .and_then(|c| c.as_u64())
            .unwrap_or(0) as u32;
        let memory_bytes = v
            .pointer("/config/memory/size")
            .and_then(|m| m.as_u64())
            .unwrap_or(0);

        Ok(VmInfo {
            state,
            cpus,
            memory_bytes,
        })
    }

    async fn snapshot_vm(
        &self,
        vm_id: &str,
        destination: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        info!(vm_id = %vm_id, destination = %destination, op = operation_id.unwrap_or("-"), "snapshotting vm via ch api");

        let body = format!(r#"{{"destination_url":"file://{}"}}"#, destination);
        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.snapshot", Some(&body)).await?;
        if status != 200 && status != 204 {
            return Err(ChvError::Internal {
                reason: format!("vm.snapshot returned unexpected status {}", status),
            });
        }
        Ok(())
    }

    async fn restore_snapshot(
        &self,
        vm_id: &str,
        source: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        info!(vm_id = %vm_id, source = %source, op = operation_id.unwrap_or("-"), "restoring snapshot via ch api");

        let body = format!(r#"{{"source_url":"file://{}"}}"#, source);
        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.restore", Some(&body)).await?;
        if status != 200 && status != 204 {
            return Err(ChvError::Internal {
                reason: format!("vm.restore returned unexpected status {}", status),
            });
        }
        Ok(())
    }

    async fn pause_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "pausing vm via ch api");

        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.pause", None).await?;
        if status != 200 && status != 204 {
            return Err(ChvError::Internal {
                reason: format!("vm.pause returned unexpected status {}", status),
            });
        }
        Ok(())
    }

    async fn resume_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "resuming vm via ch api");

        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.resume", None).await?;
        if status != 200 && status != 204 {
            return Err(ChvError::Internal {
                reason: format!("vm.resume returned unexpected status {}", status),
            });
        }
        Ok(())
    }

    async fn power_button(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "sending ACPI power button via ch api");

        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.power-button", None).await?;
        if status != 200 && status != 204 {
            return Err(ChvError::Internal {
                reason: format!("vm.power-button returned unexpected status {}", status),
            });
        }
        Ok(())
    }

    async fn add_disk(
        &self,
        vm_id: &str,
        params: &AddDiskParams,
        operation_id: Option<&str>,
    ) -> Result<String, ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        info!(vm_id = %vm_id, path = %params.path.display(), op = operation_id.unwrap_or("-"), "hot-adding disk via ch api");

        let mut obj = serde_json::Map::new();
        obj.insert("path".to_string(), serde_json::Value::from(params.path.to_string_lossy().to_string()));
        obj.insert("readonly".to_string(), serde_json::Value::from(params.read_only));
        if let Some(ref id) = params.id {
            obj.insert("id".to_string(), serde_json::Value::from(id.clone()));
        }
        let body = serde_json::Value::Object(obj).to_string();

        let (status, response_body) =
            Self::ch_api_request_with_body(&api_socket, "PUT", "/api/v1/vm.add-disk", Some(&body)).await?;
        if status != 200 && status != 204 {
            return Err(ChvError::Internal {
                reason: format!("vm.add-disk returned unexpected status {}", status),
            });
        }
        Ok(response_body)
    }

    async fn remove_device(
        &self,
        vm_id: &str,
        device_id: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        info!(vm_id = %vm_id, device_id = %device_id, op = operation_id.unwrap_or("-"), "hot-removing device via ch api");

        let body = format!(r#"{{"id":"{}"}}"#, device_id);
        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.remove-device", Some(&body)).await?;
        if status != 200 && status != 204 {
            return Err(ChvError::Internal {
                reason: format!("vm.remove-device returned unexpected status {}", status),
            });
        }
        Ok(())
    }

    async fn add_net(
        &self,
        vm_id: &str,
        params: &AddNetParams,
        operation_id: Option<&str>,
    ) -> Result<String, ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        info!(vm_id = %vm_id, tap = %params.tap_name, mac = %params.mac_address, op = operation_id.unwrap_or("-"), "hot-adding net via ch api");

        let mut obj = serde_json::Map::new();
        obj.insert("tap".to_string(), serde_json::Value::from(params.tap_name.clone()));
        obj.insert("mac".to_string(), serde_json::Value::from(params.mac_address.clone()));
        if let Some(ref id) = params.id {
            obj.insert("id".to_string(), serde_json::Value::from(id.clone()));
        }
        let body = serde_json::Value::Object(obj).to_string();

        let (status, response_body) =
            Self::ch_api_request_with_body(&api_socket, "PUT", "/api/v1/vm.add-net", Some(&body)).await?;
        if status != 200 && status != 204 {
            return Err(ChvError::Internal {
                reason: format!("vm.add-net returned unexpected status {}", status),
            });
        }
        Ok(response_body)
    }

    async fn resize_disk(
        &self,
        vm_id: &str,
        disk_id: &str,
        new_size_bytes: u64,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        info!(vm_id = %vm_id, disk_id = %disk_id, new_size = new_size_bytes, op = operation_id.unwrap_or("-"), "resizing disk via ch api");

        let body = format!(r#"{{"id":"{}","new_size":{}}}"#, disk_id, new_size_bytes);
        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.resize-zone", Some(&body)).await?;
        if status != 200 && status != 204 {
            return Err(ChvError::Internal {
                reason: format!("vm.resize-zone returned unexpected status {}", status),
            });
        }
        Ok(())
    }

    async fn ping(&self, vm_id: &str) -> Result<bool, ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        match Self::ch_api_request(&api_socket, "GET", "/api/v1/vmm.ping", None).await {
            Ok(status) => Ok(status == 200),
            Err(_) => Ok(false),
        }
    }

    async fn coredump(
        &self,
        vm_id: &str,
        destination: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let api_socket = {
            let map = self.vms.lock().unwrap();
            let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
                resource: "vm".to_string(),
                id: vm_id.to_string(),
            })?;
            proc.api_socket.clone()
        };

        info!(vm_id = %vm_id, destination = %destination, op = operation_id.unwrap_or("-"), "generating coredump via ch api");

        let body = format!(r#"{{"destination_url":"file://{}"}}"#, destination);
        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vm.coredump", Some(&body)).await?;
        if status != 200 && status != 204 {
            return Err(ChvError::Internal {
                reason: format!("vm.coredump returned unexpected status {}", status),
            });
        }
        Ok(())
    }

    fn pty_master(&self, vm_id: &str) -> Option<OwnedFd> {
        let map = self.vms.lock().ok()?;
        let proc = map.get(vm_id)?;
        let fd = nix::unistd::dup(proc.pty_master.as_raw_fd()).ok()?;
        Some(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

#[cfg(test)]
mod tests {
    use super::parse_http_status;
    use super::ProcessCloudHypervisorAdapter;
    use crate::adapter::{VmConfig, VmDiskConfig};
    use chv_errors::ChvError;
    use std::path::PathBuf;

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
        let dir = tempfile::tempdir().unwrap();
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
        };
        let err = adapter.validate_vm_config(&cfg).unwrap_err();
        assert!(matches!(err, ChvError::InvalidArgument { field, .. } if field == "kernel_path"));
    }

    #[test]
    fn validate_vm_config_accepts_existing_paths() {
        let dir = tempfile::tempdir().unwrap();
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
            }],
            nics: vec![],
            api_socket_path: PathBuf::from("/tmp/chv/vms/vm-1/vm.sock"),
            cloud_init_userdata: None,
        };
        adapter.validate_vm_config(&cfg).unwrap();
    }
}
