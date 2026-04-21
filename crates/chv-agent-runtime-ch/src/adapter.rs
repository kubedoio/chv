use async_trait::async_trait;
use chv_errors::ChvError;
use std::os::fd::OwnedFd;
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VmInfo {
    pub state: String,
    pub cpus: u32,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct VmCounters {
    pub cpu_percent: f64,
    pub memory_bytes_used: u64,
    pub memory_bytes_total: u64,
    pub disk_bytes_read: u64,
    pub disk_bytes_written: u64,
    pub net_bytes_rx: u64,
    pub net_bytes_tx: u64,
}

#[derive(Debug, Clone)]
pub struct VmConfig {
    pub vm_id: String,
    pub cpus: u32,
    pub memory_bytes: u64,
    pub kernel_path: PathBuf,
    pub firmware_path: Option<PathBuf>,
    pub disks: Vec<VmDiskConfig>,
    pub nics: Vec<VmNicConfig>,
    pub api_socket_path: PathBuf,
    pub cloud_init_userdata: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VmDiskConfig {
    pub path: PathBuf,
    pub read_only: bool,
    pub id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VmNicConfig {
    pub network_id: String,
    pub mac_address: String,
    pub ip_address: String,
    pub tap_name: String,
    pub cidr: String,
    pub gateway: String,
}

#[derive(Debug, Clone)]
pub struct AddDiskParams {
    pub path: PathBuf,
    pub read_only: bool,
    pub id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AddNetParams {
    pub tap_name: String,
    pub mac_address: String,
    pub id: Option<String>,
}

#[async_trait]
pub trait CloudHypervisorAdapter: Send + Sync + 'static {
    // --- VM Lifecycle ---
    async fn create_vm(
        &self,
        config: &VmConfig,
        operation_id: Option<&str>,
    ) -> Result<String, ChvError>;
    async fn start_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError>;
    async fn stop_vm(
        &self,
        vm_id: &str,
        force: bool,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;
    async fn delete_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError>;
    async fn reboot_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError>;
    async fn pause_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError>;
    async fn resume_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError>;
    async fn power_button(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError>;

    // --- Resource Management ---
    async fn resize_vm(
        &self,
        vm_id: &str,
        cpus: Option<u32>,
        memory_bytes: Option<u64>,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;

    async fn add_disk(
        &self,
        vm_id: &str,
        params: &AddDiskParams,
        operation_id: Option<&str>,
    ) -> Result<String, ChvError>;

    async fn remove_device(
        &self,
        vm_id: &str,
        device_id: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;

    async fn add_net(
        &self,
        vm_id: &str,
        params: &AddNetParams,
        operation_id: Option<&str>,
    ) -> Result<String, ChvError>;

    async fn resize_disk(
        &self,
        vm_id: &str,
        disk_id: &str,
        new_size_bytes: u64,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;

    // --- Introspection ---
    async fn vm_info(&self, vm_id: &str) -> Result<VmInfo, ChvError>;
    async fn vm_counters(&self, vm_id: &str) -> Result<VmCounters, ChvError>;
    async fn ping(&self, vm_id: &str) -> Result<bool, ChvError>;

    // --- Snapshots ---
    async fn snapshot_vm(
        &self,
        vm_id: &str,
        destination: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;

    async fn restore_snapshot(
        &self,
        vm_id: &str,
        source: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;

    // --- Diagnostics ---
    async fn coredump(
        &self,
        vm_id: &str,
        destination: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;

    // --- PTY ---
    fn pty_master(&self, _vm_id: &str) -> Option<OwnedFd> {
        None
    }

    fn pty_output_rx(&self, _vm_id: &str) -> Option<tokio::sync::broadcast::Receiver<Vec<u8>>> {
        None
    }
}
