use async_trait::async_trait;
use chv_errors::ChvError;
use std::collections::HashMap;
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
    _vm_id: String,
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
            request.push_str(&format!("Content-Length: {}\r\n", b.len()));
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

        let response = String::from_utf8_lossy(&buf[..n]);
        let status_line = response.lines().next().unwrap_or("");
        let parts: Vec<&str> = status_line.split_whitespace().collect();
        let status_code = parts
            .get(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);
        Ok(status_code)
    }
}

#[async_trait]
impl CloudHypervisorAdapter for ProcessCloudHypervisorAdapter {
    async fn create_vm(
        &self,
        config: &VmConfig,
        operation_id: Option<&str>,
    ) -> Result<String, ChvError> {
        let mut cmd = tokio::process::Command::new(&self.chv_binary);
        cmd.arg("--api-socket").arg(&config.api_socket_path);
        cmd.arg("--cpus").arg(format!("boot={}", config.cpus));
        cmd.arg("--memory")
            .arg(format!("size={}", config.memory_bytes));
        cmd.arg("--kernel").arg(&config.kernel_path);
        for disk in &config.disk_paths {
            cmd.arg("--disk").arg(format!("path={}", disk.display()));
        }
        cmd.stdout(Stdio::null()).stderr(Stdio::null());

        info!(
            vm_id = %config.vm_id,
            socket = %config.api_socket_path.display(),
            op = operation_id.unwrap_or("-"),
            "spawning cloud-hypervisor"
        );

        let child = cmd.spawn().map_err(|e| ChvError::Io {
            path: self.chv_binary.to_string_lossy().to_string(),
            source: e,
        })?;

        Self::wait_for_socket(&config.api_socket_path, std::time::Duration::from_secs(10)).await?;

        let mut map = self.vms.lock().await;
        map.insert(
            config.vm_id.clone(),
            VmProcess {
                _vm_id: config.vm_id.clone(),
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
}
