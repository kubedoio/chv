use async_trait::async_trait;
use chv_errors::ChvError;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::process::Child;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::adapter::{CloudHypervisorAdapter, VmConfig};

struct VmProcess {
    api_socket: std::path::PathBuf,
    child: Child,
}

pub struct ProcessCloudHypervisorAdapter {
    chv_binary: std::path::PathBuf,
    vms: Arc<Mutex<HashMap<String, VmProcess>>>,
}

impl ProcessCloudHypervisorAdapter {
    pub fn new(chv_binary: impl Into<std::path::PathBuf>) -> Self {
        Self {
            chv_binary: chv_binary.into(),
            vms: Arc::new(Mutex::new(HashMap::new())),
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

        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await.map_err(|e| ChvError::Io {
            path: socket.to_string_lossy().to_string(),
            source: e,
        })?;

        let status_code = parse_http_status(&buf[..n]).unwrap_or(0);
        Ok(status_code)
    }

    fn validate_vm_config(&self, config: &VmConfig) -> Result<(), ChvError> {
        if !self.chv_binary.exists() {
            return Err(ChvError::InvalidArgument {
                field: "chv_binary_path".to_string(),
                reason: format!("binary not found: {}", self.chv_binary.display()),
            });
        }
        if !config.kernel_path.exists() {
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

        let mut cmd = tokio::process::Command::new(&self.chv_binary);
        cmd.arg("--api-socket").arg(&config.api_socket_path);
        cmd.arg("--cpus").arg(format!("boot={}", config.cpus));
        cmd.arg("--memory")
            .arg(format!("size={}", config.memory_bytes));
        cmd.arg("--kernel").arg(&config.kernel_path);
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
        cmd.stdout(Stdio::null()).stderr(Stdio::null());

        info!(
            vm_id = %config.vm_id,
            socket = %config.api_socket_path.display(),
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

        let mut map = self.vms.lock().await;
        map.insert(
            config.vm_id.clone(),
            VmProcess {
                api_socket: config.api_socket_path.clone(),
                child,
            },
        );
        Ok(config.vm_id.clone())
    }

    async fn start_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let map = self.vms.lock().await;
        let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
            resource: "vm".to_string(),
            id: vm_id.to_string(),
        })?;

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "booting vm via ch api");

        // Auto-boot via CLI means VM is already running; send boot API for completeness.
        let status =
            Self::ch_api_request(&proc.api_socket, "PUT", "/api/v1/vmm.boot", None).await?;
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
        let mut map = self.vms.lock().await;
        let proc = map.get_mut(vm_id).ok_or_else(|| ChvError::NotFound {
            resource: "vm".to_string(),
            id: vm_id.to_string(),
        })?;

        info!(vm_id = %vm_id, force = force, op = operation_id.unwrap_or("-"), "stopping vm");

        if force {
            let _ = proc.child.start_kill();
        } else {
            let status =
                Self::ch_api_request(&proc.api_socket, "PUT", "/api/v1/vmm.shutdown", None).await?;
            if status != 200 && status != 204 {
                warn!(
                    status = status,
                    "graceful shutdown failed, falling back to kill"
                );
                let _ = proc.child.start_kill();
            }
        }
        Ok(())
    }

    async fn delete_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let mut map = self.vms.lock().await;
        let mut proc = map.remove(vm_id).ok_or_else(|| ChvError::NotFound {
            resource: "vm".to_string(),
            id: vm_id.to_string(),
        })?;

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "deleting vm");

        let _ = proc.child.start_kill();
        let _ = proc.child.wait().await;
        let _ = tokio::fs::remove_file(&proc.api_socket).await;
        Ok(())
    }

    async fn reboot_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        let map = self.vms.lock().await;
        let proc = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
            resource: "vm".to_string(),
            id: vm_id.to_string(),
        })?;

        info!(vm_id = %vm_id, op = operation_id.unwrap_or("-"), "rebooting vm via ch api");

        let status =
            Self::ch_api_request(&proc.api_socket, "PUT", "/api/v1/vmm.reboot", None).await?;
        if status != 200 && status != 204 {
            warn!(status = status, "unexpected status from vmm.reboot");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ProcessCloudHypervisorAdapter;
    use super::parse_http_status;
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
            disks: vec![],
            nics: vec![],
            api_socket_path: PathBuf::from("/tmp/chv-vm-1.sock"),
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
            disks: vec![VmDiskConfig {
                path: disk,
                read_only: false,
            }],
            nics: vec![],
            api_socket_path: PathBuf::from("/tmp/chv-vm-1.sock"),
        };
        adapter.validate_vm_config(&cfg).unwrap();
    }
}
