use async_trait::async_trait;
use chv_errors::ChvError;
use std::collections::HashMap;
use std::fmt::Write;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::process::Child;
use tracing::{info, warn};

use crate::adapter::{CloudHypervisorAdapter, VmConfig, VmInfo};

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
        loop {
            let n = stream.read(&mut tmp).await.map_err(|e| ChvError::Io {
                path: socket.to_string_lossy().to_string(),
                source: e,
            })?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            // If we've read at least the status line and headers are done, check if body complete.
            // For simplicity stop after the first non-zero read that includes headers.
            let raw = String::from_utf8_lossy(&buf);
            if raw.contains("\r\n\r\n") {
                break;
            }
        }

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
        cmd.arg("--cpus").arg(format!("boot={}", config.cpus));
        cmd.arg("--memory")
            .arg(format!("size={}", config.memory_bytes));
        if let Some(ref fw) = config.firmware_path {
            cmd.arg("--firmware").arg(fw);
        } else {
            cmd.arg("--kernel").arg(&config.kernel_path);
        }
        for disk in &config.disks {
            let arg = if disk.read_only {
                format!("path={},readonly=on", disk.path.display())
            } else {
                format!("path={}", disk.path.display())
            };
            cmd.arg("--disk").arg(arg);
        }
        for nic in &config.nics {
            if nic.tap_name.is_empty() {
                warn!(mac = %nic.mac_address, "skipping NIC with empty tap_name");
                continue;
            }
            cmd.arg("--net")
                .arg(format!("mac={},tap={}", nic.mac_address, nic.tap_name));
        }
        cmd.arg("--console")
            .arg("off")
            .arg("--serial")
            .arg(format!("tty={}", slave_path));

        if let Some(ref userdata) = config.cloud_init_userdata {
            if !userdata.trim().is_empty() {
                let vm_runtime_dir = config.api_socket_path
                    .parent()
                    .expect("api_socket_path must have a parent directory");
                let userdata_path = vm_runtime_dir.join("user-data.yaml");
                tokio::fs::write(&userdata_path, userdata.as_bytes())
                    .await
                    .map_err(|e| ChvError::Io {
                        path: userdata_path.to_string_lossy().to_string(),
                        source: e,
                    })?;
                cmd.arg("--user-data").arg(&userdata_path);
            }
        }

        cmd.stdout(Stdio::null()).stderr(Stdio::null());

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
            return Err(ChvError::Internal {
                reason: format!(
                    "failed to start cloud-hypervisor for vm {}: {}",
                    config.vm_id, e
                ),
            });
        }

        let mut map = self.vms.lock().unwrap();
        map.insert(
            config.vm_id.clone(),
            VmProcess {
                api_socket: config.api_socket_path.clone(),
                child,
                pty_master: unsafe { OwnedFd::from_raw_fd(pty_master.into_raw_fd()) },
            },
        );
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

        // Auto-boot via CLI means VM is already running; send boot API for completeness.
        let status =
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vmm.boot", None).await?;
        if status != 200 && status != 204 {
            warn!(status = status, "unexpected status from vmm.boot");
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
            if let Some(proc) = map.get_mut(vm_id) {
                let _ = proc.child.start_kill();
            }
        } else {
            let status =
                Self::ch_api_request(&api_socket, "PUT", "/api/v1/vmm.shutdown", None).await?;
            if status != 200 && status != 204 {
                warn!(
                    status = status,
                    "graceful shutdown failed, falling back to kill"
                );
                let mut map = self.vms.lock().unwrap();
                if let Some(proc) = map.get_mut(vm_id) {
                    let _ = proc.child.start_kill();
                }
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
            Self::ch_api_request(&api_socket, "PUT", "/api/v1/vmm.reboot", None).await?;
        if status != 200 && status != 204 {
            warn!(status = status, "unexpected status from vmm.reboot");
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
