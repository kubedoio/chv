use async_trait::async_trait;
use chv_errors::ChvError;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

pub mod resources;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VmInfo {
    pub state: String,
    pub cpus: u32,
    pub memory_bytes: u64,
}

/// Point-in-time runtime counters for one VM (legacy VmStateReport
/// transport shape).
///
/// Field semantics after the G1 sampler repair (native monitoring
/// campaign, #602; measured on the qualified v53.0 pin — see
/// `docs/evidence/native-monitoring/g0b/`):
///
/// - `cpu_percent` — percent of one core across all vCPUs, from
///   identity-fenced VMM-process CPU time deltas (the pinned
///   `vm.counters` API exposes no CPU counter). `0` also covers
///   "no valid interval yet" and "counter epoch reset" — the legacy
///   transport has no quality field; the v1 sample path replaces this
///   with quality-carrying samples.
/// - `memory_bytes_used` — **host-accounted** VMM-process RSS, not the
///   guest working set. `memory_bytes_total` is the configured guest
///   memory; the two are not comparable (the native spec's host/guest
///   memory split).
/// - `disk_*`/`net_*` — cumulative device counters summed from the
///   pinned `vm.counters` flat device map. A field whose sum is
///   unavailable (a device missing it) is reported as `0` here — again
///   a legacy-transport flattening the sample path replaces.
#[derive(Debug, Clone, Default)]
pub struct VmCounters {
    pub cpu_percent: f64,
    pub memory_bytes_used: u64,
    pub memory_bytes_total: u64,
    pub disk_bytes_read: u64,
    pub disk_bytes_written: u64,
    pub net_bytes_rx: u64,
    pub net_bytes_tx: u64,
    /// Whether `cpu_percent` is a real measurement this cycle (a
    /// process interval elapsed, the VMM identity held, and no epoch
    /// reset intervened). `false` ⇒ the legacy transport flattens to
    /// `0`; the v1 sample path must emit a non-valid quality sample,
    /// never a zero.
    pub cpu_percent_measured: bool,
    /// Whether `memory_bytes_used` is a real reading (the VMM process
    /// identity held for the `/proc/<pid>/stat` read). Same honesty
    /// rule as `cpu_percent_measured`.
    pub memory_measured: bool,
    /// Per-device cumulative counters keyed by the VMM's device id
    /// (`_disk0`, `_net1`, …), straight from the pinned flat
    /// `vm.counters` map — never summed across devices. The summed
    /// `disk_*`/`net_*` fields stay for the legacy transport.
    pub disk_read_by_device: std::collections::BTreeMap<String, u64>,
    pub disk_write_by_device: std::collections::BTreeMap<String, u64>,
    pub net_rx_by_device: std::collections::BTreeMap<String, u64>,
    pub net_tx_by_device: std::collections::BTreeMap<String, u64>,
    /// `(boot_id, vmm start ticks)` identity of the VMM process that
    /// produced these counters — the epoch fence for every counter in
    /// this struct. `None` ⇒ the process identity could not be
    /// established this cycle, and counter samples must not be emitted
    /// (a counter without its epoch cannot be delta-subtracted safely
    /// across a VMM restart).
    pub counter_epoch: Option<(String, String)>,
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
    pub hypervisor_overrides: Option<chv_common::hypervisor::HypervisorOverrides>,
    /// Optional virtio-vsock device (G5 monitoring transport). Strictly
    /// opt-in: `None` — the default — attaches no device and changes
    /// nothing about the VM. The CID is node-scoped (unique among the
    /// running VMs of one host) and assigned by the node authority, not
    /// the control plane; the socket path is the host-side Unix socket
    /// the VMM proxies guest connections to, owned by the node agent —
    /// never a VMM API or control socket.
    pub vsock: Option<VmVsockConfig>,
}

/// The virtio-vsock device contract (cloud-hypervisor `VsockConfig`:
/// `cid` must be `>= 3`; `socket` is the host-side proxy path).
#[derive(Debug, Clone)]
pub struct VmVsockConfig {
    /// Guest CID. `>= 3` (0–2 are reserved: null, hypervisor, host).
    pub cid: u32,
    /// Host-side Unix socket path (under the VM's runtime dir).
    pub socket_path: PathBuf,
}

/// Node-side vsock device preparation shared by every authority path
/// (the Core runtime, the legacy reconciler, and the legacy direct RPC):
/// allocate a durable node-scoped CID from the node's cursor and derive
/// the device socket path under the VM's runtime dir. The caller
/// supplies the node runtime dir (cursor home, e.g.
/// `/var/lib/chv/agent`) and the VM's runtime dir (device socket home).
pub fn prepare_vm_vsock_device(
    node_runtime_dir: &Path,
    vm_dir: &Path,
) -> std::io::Result<VmVsockConfig> {
    let cid = allocate_vsock_cid(&node_runtime_dir.join("vsock-cid-cursor"))?;
    Ok(VmVsockConfig {
        cid,
        socket_path: vm_dir.join("vsock.sock"),
    })
}

/// The fixed guest→host port of the monitoring transport over vsock
/// (the guest connects to CID 2 / `VMADDR_CID_HOST` on this port).
///
/// This is a cross-component contract constant: the pinned
/// cloud-hypervisor v53.0 vsock muxer maps a guest connection to host
/// port `P` onto the Unix socket path `"<device_socket>_<P>"`, so the
/// node-side relay listener filename and the guest's dial target must
/// agree exactly. It is deliberately NOT configurable in v1 — a
/// guest-configured port the relay does not mirror would silently
/// RST every connection. The value is mirrored as a constant in
/// `cmd/chv-monitor-agent` (the guest crate cannot depend on this
/// one); the g5 rig asserts both sides agree.
pub const VSOCK_MONITOR_PORT: u32 = 1024;

/// First CID the allocator hands out (above the reserved 0–2 and any
/// well-known low CIDs, leaving headroom for manual/operator use).
const VSOCK_CID_FLOOR: u32 = 1000;
/// Last CID the allocator hands out; allocation fails loudly beyond it
/// rather than wrapping into reuse.
const VSOCK_CID_CEILING: u32 = 4_000_000_000;

/// Durable, monotonic guest-CID allocation for one node (G5, prompt-06
/// task 2: CID allocation, uniqueness, lifecycle and restart ownership
/// must be explicit).
///
/// The pinned cloud-hypervisor v53.0 does NOT register guest CIDs with
/// the host kernel and does not validate them — uniqueness is a CHV
/// convention, not a kernel hazard (see ADR-026 and the vsock contract).
/// The allocator still makes the convention explicit, durable and
/// restart-stable:
///
/// - CIDs come from a persisted cursor file (durable node state, e.g.
///   `/var/lib/chv/agent/vsock-cid-cursor`) and are strictly monotonic:
///   no two `vm.create` calls on one node ever get the same CID, and
///   CIDs are never recycled after a VM is deleted.
/// - A VMM respawn keeps a VM's CID (it is persisted in the VM's
///   `vm-config.json`); only a fresh create allocates.
/// - A missing cursor file means a fresh node (allocation starts at the
///   floor); a cursor that EXISTS but is malformed fails loudly —
///   guessing could reuse a CID a running VM still carries in its
///   `vm-config.json`. Recovery is explicit: restore the file to a
///   value at or above every allocated CID.
///
/// The cursor is guarded by an exclusive `flock` so concurrent creates
/// on one node cannot interleave the read-modify-write.
pub fn allocate_vsock_cid(cursor_path: &Path) -> std::io::Result<u32> {
    use std::io::{Read, Seek, SeekFrom, Write};

    if let Some(parent) = cursor_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(cursor_path)?;
    file.lock()?;
    let outcome = (|| -> std::io::Result<u32> {
        let mut buf = String::new();
        let mut f = &file;
        f.read_to_string(&mut buf)?;
        // Decide from the content UNDER the lock (a pre-lock existence
        // check races concurrent first allocations). Empty means no
        // allocation has ever committed — either a fresh node, or a
        // crash before the very first write — so starting at the floor
        // is correct and re-use-free. Non-empty and unparseable means
        // a torn write; guessing could reuse a CID a running VM still
        // carries in its vm-config.json, so that fails loudly.
        let last = if buf.trim().is_empty() {
            VSOCK_CID_FLOOR - 1
        } else {
            match buf.trim().parse::<u32>() {
                Ok(v) => v,
                Err(_) => {
                    return Err(std::io::Error::other(format!(
                        "vsock CID cursor at {} is malformed (content {:?}); refusing to \
                         guess — restore it to a value at or above every allocated CID \
                         (each VM's vm-config.json records its vsock cid)",
                        cursor_path.display(),
                        buf.trim()
                    )));
                }
            }
        };
        let next = last
            .checked_add(1)
            .filter(|n| *n <= VSOCK_CID_CEILING)
            .ok_or_else(|| {
                std::io::Error::other(format!(
                    "vsock CID cursor at {} is exhausted (ceiling {}); the node cannot \
                     allocate another vsock CID",
                    cursor_path.display(),
                    VSOCK_CID_CEILING
                ))
            })?;
        // Commit order matters for crash safety: write the new value at
        // offset 0, THEN truncate to its length. A crash between the two
        // leaves a torn (unparseable) file, which the next allocation
        // rejects loudly; a crash before the write leaves the previous
        // value intact. The value is only returned after sync_all.
        f.seek(SeekFrom::Start(0))?;
        write!(f, "{next}")?;
        f.set_len(next.to_string().len() as u64)?;
        f.sync_all()?;
        Ok(next)
    })();
    file.unlock()?;
    outcome
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
pub trait HypervisorAdapter: Send + Sync + 'static {
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

    // --- Migration ---
    /// Initiate sending VM memory/state to a destination.
    /// Calls PUT /api/v1/vm.send-migration with {"destination_url": "tcp://{host}:{port}"}
    async fn send_migration(
        &self,
        vm_id: &str,
        destination_url: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;

    /// Open a migration receiving socket.
    /// Calls PUT /api/v1/vm.receive-migration with {"receiver_url": "tcp://0.0.0.0:{port}"}
    async fn receive_migration(
        &self,
        vm_id: &str,
        receiver_url: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;

    /// Check VM state (useful for monitoring migration progress).
    /// Returns the current VM state string from GET /api/v1/vm.info.
    async fn get_vm_state(&self, vm_id: &str) -> Result<String, ChvError>;

    // --- Diagnostics ---
    async fn coredump(
        &self,
        vm_id: &str,
        destination: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;

    // --- PTY ---
    async fn pty_master(&self, _vm_id: &str) -> Option<OwnedFd> {
        None
    }

    async fn pty_output_rx(
        &self,
        _vm_id: &str,
    ) -> Option<tokio::sync::broadcast::Receiver<Vec<u8>>> {
        None
    }

    async fn pty_scrollback(&self, _vm_id: &str) -> Option<Vec<u8>> {
        None
    }
}

pub use resources::AlwaysInUse;
pub use resources::NoObservedAttachments;
pub use resources::{
    ensure_vm_runtime_dir, nic_id, vm_api_socket, vm_runtime_dir, HostResourceController,
    NetworkUsageLookup, ObservedAttachmentSource, ObservedVmAttachments, DEFAULT_NIC_CIDR,
};
// Relocated to chv-common (#356 N5) so nwd's derived-names teardown fallback
// shares the definition; re-exported here so existing imports keep working.
pub use chv_common::bridge_name_for_network;

#[cfg(test)]
mod vsock_tests {
    use super::{allocate_vsock_cid, VSOCK_CID_FLOOR};

    #[test]
    fn vsock_cid_allocation_is_monotonic_and_durable() {
        let dir = tempfile::tempdir().unwrap();
        let cursor = dir.path().join("vsock-cid-cursor");
        // Fresh node: first allocation is the floor.
        assert_eq!(allocate_vsock_cid(&cursor).unwrap(), VSOCK_CID_FLOOR);
        // Strictly monotonic across calls, and persisted (new call, same file).
        assert_eq!(allocate_vsock_cid(&cursor).unwrap(), VSOCK_CID_FLOOR + 1);
        assert_eq!(allocate_vsock_cid(&cursor).unwrap(), VSOCK_CID_FLOOR + 2);
        // The cursor file records the last allocated value.
        assert_eq!(
            std::fs::read_to_string(&cursor).unwrap().trim(),
            (VSOCK_CID_FLOOR + 2).to_string()
        );
    }

    #[test]
    fn vsock_cid_allocation_starts_at_floor_for_existing_cursor_value() {
        // A pre-seeded cursor (e.g. operator-restored after corruption)
        // resumes above it — never below.
        let dir = tempfile::tempdir().unwrap();
        let cursor = dir.path().join("vsock-cid-cursor");
        std::fs::write(&cursor, "5000").unwrap();
        assert_eq!(allocate_vsock_cid(&cursor).unwrap(), 5001);
    }

    #[test]
    fn vsock_cid_allocation_fails_loudly_on_malformed_cursor() {
        // Guessing could reuse a CID a running VM still carries; the
        // allocator must refuse, not reset.
        let dir = tempfile::tempdir().unwrap();
        let cursor = dir.path().join("vsock-cid-cursor");
        std::fs::write(&cursor, "not-a-number").unwrap();
        let err = allocate_vsock_cid(&cursor).unwrap_err();
        assert!(err.to_string().contains("malformed"), "{err}");
    }

    #[test]
    fn vsock_cid_allocation_fails_loudly_on_exhaustion() {
        let dir = tempfile::tempdir().unwrap();
        let cursor = dir.path().join("vsock-cid-cursor");
        std::fs::write(&cursor, u32::MAX.to_string()).unwrap();
        let err = allocate_vsock_cid(&cursor).unwrap_err();
        assert!(err.to_string().contains("exhausted"), "{err}");
    }

    #[test]
    fn vsock_cid_allocation_concurrent_calls_stay_monotonic() {
        // The flock must serialize concurrent read-modify-write cycles:
        // N parallel allocations yield N distinct, gap-free values.
        let dir = tempfile::tempdir().unwrap();
        let cursor = dir.path().join("vsock-cid-cursor");
        let cursor = std::sync::Arc::new(cursor);
        let mut handles = Vec::new();
        for _ in 0..16 {
            let cursor = std::sync::Arc::clone(&cursor);
            handles.push(std::thread::spawn(move || {
                allocate_vsock_cid(&cursor).unwrap()
            }));
        }
        let mut values: Vec<u32> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        values.sort_unstable();
        let expected: Vec<u32> = (VSOCK_CID_FLOOR..VSOCK_CID_FLOOR + 16).collect();
        assert_eq!(values, expected);
    }
}
