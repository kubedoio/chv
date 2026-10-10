//! Read-only guest collectors for `chv-monitor-agent` (ADR-026,
//! campaign #602 prompt 03 — baseline set).
//!
//! Scope rules:
//! - Every collector reads one well-known read-only file under
//!   `/proc` or `/etc` and derives a number. No writes, no process
//!   enumeration, no arbitrary command execution — the G3 gate keeps
//!   the agent to exactly this surface (filesystem, process and
//!   service collectors are G4/prompt 04 and are not present here).
//! - Parsers are pure functions over file contents so tests inject
//!   text instead of fixtures on disk.
//! - Failure is an honest absence: a collector that cannot read or
//!   parse its source contributes no sample, never a zero and never a
//!   fabricated value. The CPU utilization sampler additionally needs
//!   a previous observation to compute a delta, so its first
//!   collection is an absence by design.
//! - Metric ids are the registry ids (`chv-monitoring-core`
//!   `registry::lookup` is the authority for kind/unit wire strings;
//!   this crate deliberately does not duplicate them).

mod cpu;
mod loadavg;
mod memory;
mod os_release;
mod uptime;

pub use cpu::CpuSampler;
pub use os_release::OsIdentity;

/// One collected numeric value, tagged with its registry metric id.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectedSample {
    pub metric_id: &'static str,
    pub value: f64,
}

/// Where the collectors read. Overridable wholesale for tests; every
/// default is a read-only procfs/etcs path.
#[derive(Debug, Clone)]
pub struct ProcPaths {
    pub stat: String,
    pub loadavg: String,
    pub meminfo: String,
    pub uptime: String,
    pub os_release: String,
    pub kernel_release: String,
    pub boot_id: String,
}

impl Default for ProcPaths {
    fn default() -> Self {
        Self {
            stat: "/proc/stat".into(),
            loadavg: "/proc/loadavg".into(),
            meminfo: "/proc/meminfo".into(),
            uptime: "/proc/uptime".into(),
            os_release: "/etc/os-release".into(),
            kernel_release: "/proc/sys/kernel/osrelease".into(),
            boot_id: "/proc/sys/kernel/random/boot_id".into(),
        }
    }
}

/// The baseline collector set. Holds only the CPU sampler's previous
/// observation; everything else is stateless.
pub struct GuestCollectors {
    paths: ProcPaths,
    cpu: CpuSampler,
}

impl GuestCollectors {
    pub fn new() -> Self {
        Self::with_paths(ProcPaths::default())
    }

    pub fn with_paths(paths: ProcPaths) -> Self {
        Self {
            cpu: CpuSampler::new(),
            paths,
        }
    }

    /// Collect one round of baseline samples. Collectors whose source
    /// is unreadable or unparseable contribute nothing (honest
    /// absence), so the result may be shorter than the full set — and
    /// on the very first call CPU utilization is absent by design.
    pub fn collect(&mut self) -> Vec<CollectedSample> {
        let mut out = Vec::with_capacity(4);
        if let Some(v) = self.cpu.sample(&read(&self.paths.stat)) {
            out.push(CollectedSample {
                metric_id: cpu::CPU_UTILIZATION,
                value: v,
            });
        }
        if let Some(v) = loadavg::parse(&read(&self.paths.loadavg)) {
            out.push(CollectedSample {
                metric_id: loadavg::LOAD1,
                value: v,
            });
        }
        if let Some(v) = memory::parse_available_bytes(&read(&self.paths.meminfo)) {
            out.push(CollectedSample {
                metric_id: memory::MEM_AVAILABLE,
                value: v,
            });
        }
        if let Some(v) = uptime::parse_seconds(&read(&self.paths.uptime)) {
            out.push(CollectedSample {
                metric_id: uptime::UPTIME_SECONDS,
                value: v,
            });
        }
        out
    }

    /// OS identity for the batch envelope (allowlisted fields, sent
    /// once per batch, bounded to 64 bytes per field by the manager).
    pub fn os_identity(&self) -> OsIdentity {
        os_release::parse_identity(
            &read(&self.paths.os_release),
            &read(&self.paths.kernel_release),
        )
    }

    /// The kernel boot id: the dedup epoch for the sequence counter.
    /// Absorbs surrounding whitespace; empty when unreadable (the
    /// caller substitutes a persisted fallback rather than reuse
    /// sequences across boots).
    pub fn boot_id(&self) -> String {
        read(&self.paths.boot_id).trim().to_string()
    }
}

impl Default for GuestCollectors {
    fn default() -> Self {
        Self::new()
    }
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}
