//! Read-only guest collectors for `chv-monitor-agent` (ADR-026,
//! campaign #602: prompt 03 baseline set + prompt 04 part 1 — the
//! G4 filesystem, network and process families).
//!
//! Scope rules:
//! - Every collector reads well-known read-only files under
//!   `/proc`, `/sys` or `/etc` and derives a number. No writes, no
//!   arbitrary command execution. The G3 baseline set is always
//!   collected; the G4 families (filesystem, network, process) are
//!   opt-in sub-collectors — this crate parses and samples, the
//!   agent binary owns config defaults and per-family CADENCE
//!   scheduling.
//! - Parsers are pure functions over file contents so tests inject
//!   text instead of fixtures on disk (the process scan is the one
//!   exception: it walks `/proc` itself and is tested against a
//!   synthetic tree).
//! - Failure is an honest absence: a collector that cannot read or
//!   parse its source contributes no sample, never a fabricated
//!   value. A measured zero is still a sample (a process selector
//!   that ran and matched nothing reports count 0). The CPU and
//!   process utilization samplers need a previous observation to
//!   compute a delta, so their first collection is an absence by
//!   design.
//! - The process collector reads exactly `/proc/<pid>/stat` and
//!   `/proc/<pid>/status` — never `cmdline` or `environ` (privacy
//!   contract).
//! - Metric ids are the registry ids (`chv-monitoring-core`
//!   `registry::lookup` is the authority for kind/unit wire strings;
//!   this crate deliberately does not duplicate them).

mod cpu;
mod filesystem;
mod loadavg;
mod memory;
mod network;
mod os_release;
mod process;
mod uptime;

pub use cpu::CpuSampler;
pub use os_release::OsIdentity;
pub use process::{ProcSnapshot, ProcessSampler};

/// One collected value. Byte and packet counters stay integers end
/// to end (contract: integer precision); ratios and gauges over
/// measured quantities are floats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SampleValue {
    Float(f64),
    Integer(u64),
}

impl SampleValue {
    /// The value as `f64` (lossy for very large integers — for
    /// tests and logging; the wire keeps `Integer` precision).
    pub fn as_f64(&self) -> f64 {
        match *self {
            SampleValue::Float(v) => v,
            SampleValue::Integer(v) => v as f64,
        }
    }
}

impl From<f64> for SampleValue {
    fn from(v: f64) -> Self {
        SampleValue::Float(v)
    }
}

impl From<u64> for SampleValue {
    fn from(v: u64) -> Self {
        SampleValue::Integer(v)
    }
}

/// One collected numeric value, tagged with its registry metric id
/// and at most one dimension — every G4 family uses exactly one
/// (`mount_id`, `interface_id` or `process_selector`).
#[derive(Debug, Clone, PartialEq)]
pub struct CollectedSample {
    pub metric_id: &'static str,
    pub value: SampleValue,
    pub dimension: Option<(&'static str, String)>,
}

impl CollectedSample {
    /// A dimensionless float sample (the G3 shape).
    pub fn float(metric_id: &'static str, value: f64) -> Self {
        Self {
            metric_id,
            value: SampleValue::Float(value),
            dimension: None,
        }
    }

    /// A dimensionless integer sample.
    pub fn integer(metric_id: &'static str, value: u64) -> Self {
        Self {
            metric_id,
            value: SampleValue::Integer(value),
            dimension: None,
        }
    }

    /// A float sample with its single dimension.
    pub fn float_with_dimension(
        metric_id: &'static str,
        value: f64,
        key: &'static str,
        dimension: String,
    ) -> Self {
        Self {
            metric_id,
            value: SampleValue::Float(value),
            dimension: Some((key, dimension)),
        }
    }

    /// An integer sample with its single dimension.
    pub fn integer_with_dimension(
        metric_id: &'static str,
        value: u64,
        key: &'static str,
        dimension: String,
    ) -> Self {
        Self {
            metric_id,
            value: SampleValue::Integer(value),
            dimension: Some((key, dimension)),
        }
    }
}

/// Where the collectors read. Overridable wholesale for tests; every
/// default is a read-only procfs/sysfs/etcs path.
#[derive(Debug, Clone)]
pub struct ProcPaths {
    pub stat: String,
    pub loadavg: String,
    pub meminfo: String,
    pub uptime: String,
    pub os_release: String,
    pub kernel_release: String,
    pub boot_id: String,
    /// PID 1's mounts file (G4 filesystem family) — the init mount
    /// namespace, not the agent's own. The agent's systemd unit
    /// sandboxes it into a private mount namespace (`ProtectSystem=strict`
    /// remounts `/` read-only there, `PrivateTmp` adds a `/tmp` that
    /// does not exist system-wide), so `/proc/self/mounts` would
    /// misreport the guest's filesystems. `/proc/1/mounts` is the
    /// live, unsandboxed truth — world-readable by default; on
    /// hidepid-hardened guests the family degrades to an honest
    /// absence. Mounts made after the agent started still reach the
    /// family's statvfs step through the unit's slave propagation.
    pub mounts: String,
    /// `/proc/net/dev` (G4 network family).
    pub net_dev: String,
    /// `/proc/net/snmp` (G4 network family, `Tcp:` lines).
    pub net_snmp: String,
    /// `/sys/class/net` root for operstate/device probing.
    pub sys_class_net: String,
    /// `/proc` root for the process scan
    /// (`<proc>/<pid>/stat` and `<proc>/<pid>/status` only).
    pub proc: String,
    /// `sysconf(_SC_CLK_TCK)` — the agent hardcodes it via libc
    /// (100 on every Linux it targets); parameterized for tests so
    /// process CPU math is deterministic.
    pub clock_ticks_per_sec: u64,
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
            mounts: "/proc/1/mounts".into(),
            net_dev: "/proc/net/dev".into(),
            net_snmp: "/proc/net/snmp".into(),
            sys_class_net: "/sys/class/net".into(),
            proc: "/proc".into(),
            clock_ticks_per_sec: 100,
        }
    }
}

/// The collector facade. Holds only sampler state (CPU deltas,
/// per-selector process deltas); everything else is stateless.
pub struct GuestCollectors {
    paths: ProcPaths,
    cpu: CpuSampler,
    /// G4 opt-ins, OFF by default: the agent binary decides config
    /// defaults. Per-family CADENCE scheduling is likewise the
    /// agent binary's job, not this crate's.
    filesystems_enabled: bool,
    network_enabled: bool,
    process_selectors: Vec<String>,
    /// One stateful sampler per selector, parallel to
    /// `process_selectors`.
    process_samplers: Vec<ProcessSampler>,
}

impl GuestCollectors {
    pub fn new() -> Self {
        Self::with_paths(ProcPaths::default())
    }

    pub fn with_paths(paths: ProcPaths) -> Self {
        Self {
            cpu: CpuSampler::new(),
            paths,
            filesystems_enabled: false,
            network_enabled: false,
            process_selectors: Vec::new(),
            process_samplers: Vec::new(),
        }
    }

    /// Opt in to the filesystem family (`collect_filesystems`).
    pub fn enable_filesystems(mut self) -> Self {
        self.filesystems_enabled = true;
        self
    }

    /// Opt in to the network family (`collect_network`).
    pub fn enable_network(mut self) -> Self {
        self.network_enabled = true;
        self
    }

    /// Opt in to the process family with explicit selectors. Each
    /// selector matches the `/proc/<pid>/status` `Name:` field
    /// exactly (case-sensitive). Selectors are trimmed; empty and
    /// over-128-byte selectors are dropped (the dimension bound).
    /// The count bound (16) is the caller's to enforce.
    pub fn enable_processes(mut self, selectors: Vec<String>) -> Self {
        self.process_selectors = selectors
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s.len() <= 128)
            .collect();
        self.process_samplers = self
            .process_selectors
            .iter()
            .map(|_| ProcessSampler::new(self.paths.clock_ticks_per_sec))
            .collect();
        self
    }

    /// Collect one round of baseline (G3) samples. This keeps its
    /// exact G3 meaning — baseline only, `SampleValue::Float`, no
    /// dimensions — so existing callers keep their cadence and wire
    /// shape; the G4 families have their own methods
    /// (`collect_filesystems`, `collect_network`,
    /// `collect_processes`) for the agent to schedule. Collectors
    /// whose source is unreadable or unparseable contribute nothing
    /// (honest absence), so the result may be shorter than the full
    /// set — and on the very first call CPU utilization is absent
    /// by design.
    pub fn collect(&mut self) -> Vec<CollectedSample> {
        let mut out = Vec::with_capacity(4);
        if let Some(v) = self.cpu.sample(&read(&self.paths.stat)) {
            out.push(CollectedSample::float(cpu::CPU_UTILIZATION, v));
        }
        if let Some(v) = loadavg::parse(&read(&self.paths.loadavg)) {
            out.push(CollectedSample::float(loadavg::LOAD1, v));
        }
        if let Some(v) = memory::parse_available_bytes(&read(&self.paths.meminfo)) {
            out.push(CollectedSample::float(memory::MEM_AVAILABLE, v));
        }
        if let Some(v) = uptime::parse_seconds(&read(&self.paths.uptime)) {
            out.push(CollectedSample::float(uptime::UPTIME_SECONDS, v));
        }
        out
    }

    /// Filesystem family: per-mount available/total bytes, inode
    /// utilization and read-only state (dimension `mount_id`).
    /// Empty unless the family was enabled — opting in is explicit.
    pub fn collect_filesystems(&mut self) -> Vec<CollectedSample> {
        if !self.filesystems_enabled {
            return Vec::new();
        }
        let entries = filesystem::parse_mounts(&read(&self.paths.mounts));
        let mut out = Vec::new();
        for entry in &entries {
            // statvfs is the family's only impure step; its failure
            // skips the stat-dependent fields for that mount (the
            // options-derived read_only still emits).
            let stat = filesystem::statvfs_mount(&entry.mountpoint);
            out.extend(filesystem::emit_mount(entry, stat));
        }
        out
    }

    /// Network family: per-interface counters (dimension
    /// `interface_id`), link state and the host-wide TCP
    /// established count. Empty unless the family was enabled —
    /// opting in is explicit.
    pub fn collect_network(&mut self) -> Vec<CollectedSample> {
        if !self.network_enabled {
            return Vec::new();
        }
        let interfaces = network::parse_net_dev(&read(&self.paths.net_dev));
        // Names with a /sys/class/net/<if>/device dir are physical;
        // the probe only runs on validated names.
        let phys: Vec<String> = interfaces
            .iter()
            .filter(|s| network::valid_interface_name(&s.name))
            .filter(|s| {
                std::path::Path::new(&self.paths.sys_class_net)
                    .join(&s.name)
                    .join("device")
                    .exists()
            })
            .map(|s| s.name.clone())
            .collect();
        let mut out = Vec::new();
        for stats in &interfaces {
            // The name is kernel-provided but still validated before
            // any /sys path is built from it; an invalid name skips
            // the whole interface (honest absence).
            let operstate = network::valid_interface_name(&stats.name).then(|| {
                read(&network::operstate_path(
                    &self.paths.sys_class_net,
                    &stats.name,
                ))
            });
            let operstate = operstate
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            out.extend(network::emit_interface(stats, operstate.as_deref(), &phys));
        }
        if let Some(v) = network::parse_tcp_curr_estab(&read(&self.paths.net_snmp)) {
            out.push(CollectedSample::integer(network::NET_TCP_ESTABLISHED, v));
        }
        out
    }

    /// Process family: per selector (dimension `process_selector`)
    /// the matched process count, summed RSS bytes and the stateful
    /// CPU utilization ratio. `now_ms` is the caller's wall clock
    /// (epoch ms). Empty unless selectors were configured.
    pub fn collect_processes(&mut self, now_ms: u64) -> Vec<CollectedSample> {
        if self.process_selectors.is_empty() {
            return Vec::new();
        }
        let snapshots = self.scan_processes();
        let ncpus = process::parse_cpu_count(&read(&self.paths.stat));
        let mut out = Vec::new();
        for (i, selector) in self.process_selectors.iter().enumerate() {
            let procs: Vec<ProcSnapshot> = snapshots
                .iter()
                .filter(|p| p.name == *selector)
                .cloned()
                .collect();
            out.push(CollectedSample::integer_with_dimension(
                process::PROCESS_COUNT,
                procs.len() as u64,
                "process_selector",
                selector.clone(),
            ));
            let rss_bytes: u64 = procs
                .iter()
                .map(|p| p.rss_kb)
                .sum::<u64>()
                .saturating_mul(1024);
            out.push(CollectedSample::integer_with_dimension(
                process::PROCESS_RSS_BYTES,
                rss_bytes,
                "process_selector",
                selector.clone(),
            ));
            if let Some(ratio) = self.process_samplers[i].sample(&procs, now_ms, ncpus) {
                out.push(CollectedSample::float_with_dimension(
                    process::PROCESS_CPU_UTILIZATION,
                    ratio,
                    "process_selector",
                    selector.clone(),
                ));
            }
        }
        out
    }

    /// Scan `/proc` for numeric pid dirs and snapshot each readable
    /// process. Bounded to 4096 pids (the 4097th is skipped);
    /// unreadable dirs (permission, racing exit) are skipped
    /// silently — normal churn. NEVER reads `cmdline` or `environ`:
    /// the privacy contract allows exactly `stat` and `status`.
    fn scan_processes(&self) -> Vec<ProcSnapshot> {
        let mut out = Vec::new();
        let Ok(dir) = std::fs::read_dir(&self.paths.proc) else {
            return out;
        };
        let mut pids: Vec<i32> = dir
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                name.parse::<i32>().ok()
            })
            .collect();
        pids.sort_unstable();
        pids.dedup();
        // The bound counts SCANNED pids (not only readable ones): a
        // guest with tens of thousands of unreadable pid dirs must
        // not turn the scan into an unbounded walk.
        for pid in pids.into_iter().take(4096) {
            let stat = read(&format!("{}/{}/stat", self.paths.proc, pid));
            let status = read(&format!("{}/{}/status", self.paths.proc, pid));
            let (Some(stat), Some((name, rss_kb))) = (
                process::parse_pid_stat(&stat),
                process::parse_status(&status),
            ) else {
                continue;
            };
            out.push(ProcSnapshot {
                pid: stat.pid,
                starttime: stat.starttime,
                name,
                rss_kb,
                cpu_ticks: stat.cpu_ticks,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static DIR_SEQ: AtomicU32 = AtomicU32::new(0);

    /// A scratch directory unique per test run (no dev-dependency
    /// on tempfile; the crate stays dependency-minimal).
    fn scratch_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "chv-monitor-collectors-{tag}-{}",
            DIR_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Write a synthetic /proc tree: one dir per
    /// `(pid, name, rss_kb, cpu_ticks)` plus a `stat` file with
    /// `ncpus` `cpuN` lines.
    fn fake_proc(entries: &[(i32, &str, u64, u64)], ncpus: usize, tag: &str) -> std::path::PathBuf {
        let root = scratch_dir(tag);
        let mut stat = String::from("cpu  100 0 100 700 50 0 0 50 0 0\n");
        for i in 0..ncpus {
            stat.push_str(&format!("cpu{i} 100 0 100 700 50 0 0 50 0 0\n"));
        }
        std::fs::write(root.join("stat"), stat).unwrap();
        for &(pid, name, rss_kb, ticks) in entries {
            let dir = root.join(pid.to_string());
            std::fs::create_dir_all(&dir).unwrap();
            // tail after the comm parens: state(3) .. cmajflt(13),
            // utime(14)=ticks, stime(15)=0, filler, starttime(22)=pid.
            let stat_line = format!(
                "{pid} ({name}) S 1 1 1 1 1 1 1 1 1 1 {ticks} 0 0 0 0 1 0 0 {pid} \
                 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1\n"
            );
            std::fs::write(dir.join("stat"), stat_line).unwrap();
            std::fs::write(
                dir.join("status"),
                format!("Name:\t{name}\nVmRSS:\t{rss_kb} kB\n"),
            )
            .unwrap();
        }
        root
    }

    #[test]
    fn collect_keeps_g3_baseline_shape() {
        let dir = scratch_dir("g3");
        std::fs::write(dir.join("stat"), "cpu  100 0 100 700 50 0 0 50 0 0\n").unwrap();
        std::fs::write(dir.join("loadavg"), "0.42 0.50 0.55 1/382 12345\n").unwrap();
        std::fs::write(dir.join("meminfo"), "MemAvailable:   10485760 kB\n").unwrap();
        std::fs::write(dir.join("uptime"), "12345.67 23456.78\n").unwrap();
        let mut c = GuestCollectors::with_paths(ProcPaths {
            stat: dir.join("stat").to_str().unwrap().into(),
            loadavg: dir.join("loadavg").to_str().unwrap().into(),
            meminfo: dir.join("meminfo").to_str().unwrap().into(),
            uptime: dir.join("uptime").to_str().unwrap().into(),
            ..ProcPaths::default()
        });
        let samples = c.collect();
        // First observation: CPU utilization is absent by design;
        // the other three are floats with no dimension.
        assert_eq!(samples.len(), 3);
        for s in &samples {
            assert_eq!(s.dimension, None);
            assert!(matches!(s.value, SampleValue::Float(_)), "{s:?}");
        }
        let ids: Vec<&str> = samples.iter().map(|s| s.metric_id).collect();
        assert_eq!(
            ids,
            vec![
                loadavg::LOAD1,
                memory::MEM_AVAILABLE,
                uptime::UPTIME_SECONDS
            ]
        );
    }

    #[test]
    fn g4_families_are_off_by_default() {
        let mut c = GuestCollectors::new();
        assert!(c.collect_filesystems().is_empty());
        assert!(c.collect_network().is_empty());
        assert!(c.collect_processes(1_000).is_empty());
    }

    #[test]
    fn filesystems_default_source_is_the_init_mount_namespace() {
        // G4 real-VM lesson (found by the evidence rig): the agent's
        // systemd unit sandboxes it with ProtectSystem=strict, which
        // remounts / read-only IN THE AGENT'S OWN mount namespace —
        // /proc/self/mounts reported the guest's writable root as
        // read-only. The family must read PID 1's mounts: the live,
        // unsandboxed view of the filesystems it reports on.
        assert_eq!(ProcPaths::default().mounts, "/proc/1/mounts");
    }

    #[test]
    fn collect_processes_reports_counts_rss_and_first_tick_absence() {
        let root = fake_proc(
            &[(1, "bash", 100, 50), (2, "nginx", 200, 60)],
            2,
            "proc-basics",
        );
        let mut c = GuestCollectors::with_paths(ProcPaths {
            proc: root.to_str().unwrap().into(),
            ..ProcPaths::default()
        })
        .enable_processes(vec!["bash".into(), "nomatch".into()]);

        let first = c.collect_processes(1_000);
        let get = |samples: &[CollectedSample], metric: &str, dim: &str| {
            samples
                .iter()
                .find(|s| {
                    s.metric_id == metric
                        && s.dimension
                            .as_ref()
                            .is_some_and(|(k, v)| *k == "process_selector" && v == dim)
                })
                .unwrap_or_else(|| panic!("missing {metric} for {dim}"))
                .value
        };
        // First tick: counts and RSS are measured; CPU is absent.
        assert_eq!(
            get(&first, process::PROCESS_COUNT, "bash"),
            SampleValue::Integer(1)
        );
        assert_eq!(
            get(&first, process::PROCESS_RSS_BYTES, "bash"),
            SampleValue::Integer(100 * 1024)
        );
        assert!(
            !first
                .iter()
                .any(|s| s.metric_id == process::PROCESS_CPU_UTILIZATION),
            "first observation must be absent"
        );
        // Zero matches are a measured zero, not an absence.
        assert_eq!(
            get(&first, process::PROCESS_COUNT, "nomatch"),
            SampleValue::Integer(0)
        );
        assert_eq!(
            get(&first, process::PROCESS_RSS_BYTES, "nomatch"),
            SampleValue::Integer(0)
        );

        // Second tick over an unchanged tree: utilization is a
        // measured 0.0 (no tick delta).
        let second = c.collect_processes(2_000);
        assert_eq!(
            get(&second, process::PROCESS_CPU_UTILIZATION, "bash"),
            SampleValue::Float(0.0)
        );
    }

    #[test]
    fn process_scan_bounds_at_4096_pids() {
        let entries: Vec<(i32, &str, u64, u64)> = (1..=4097).map(|pid| (pid, "x", 1, 1)).collect();
        let root = fake_proc(&entries, 1, "proc-bound");
        let mut c = GuestCollectors::with_paths(ProcPaths {
            proc: root.to_str().unwrap().into(),
            ..ProcPaths::default()
        })
        .enable_processes(vec!["x".into()]);
        let samples = c.collect_processes(1_000);
        let count = samples
            .iter()
            .find(|s| s.metric_id == process::PROCESS_COUNT)
            .unwrap()
            .value;
        assert_eq!(count, SampleValue::Integer(4096), "4097th pid is skipped");
    }

    #[test]
    fn enable_processes_sanitizes_selectors() {
        let root = fake_proc(&[(1, "bash", 10, 10)], 1, "proc-selectors");
        let mut c = GuestCollectors::with_paths(ProcPaths {
            proc: root.to_str().unwrap().into(),
            ..ProcPaths::default()
        })
        .enable_processes(vec!["  bash  ".into(), String::new(), "x".repeat(129)]);
        let samples = c.collect_processes(1_000);
        let dims: Vec<&String> = samples
            .iter()
            .filter_map(|s| s.dimension.as_ref().map(|(_, v)| v))
            .collect();
        assert!(dims.iter().all(|d| *d == "bash"), "{dims:?}");
        assert!(!dims.is_empty());
    }

    #[test]
    fn collect_filesystems_emits_per_mount() {
        let dir = scratch_dir("fs");
        // A tmpfs mount at the scratch dir itself, so statvfs
        // succeeds on a real filesystem.
        let mounts = format!(
            "proc /proc proc rw,nosuid 0 0\ntmpfs {} tmpfs rw 0 0\n",
            dir.display()
        );
        std::fs::write(dir.join("mounts"), mounts).unwrap();
        let mut c = GuestCollectors::with_paths(ProcPaths {
            mounts: dir.join("mounts").to_str().unwrap().into(),
            ..ProcPaths::default()
        })
        .enable_filesystems();
        let samples = c.collect_filesystems();
        let ids: Vec<&str> = samples.iter().map(|s| s.metric_id).collect();
        assert_eq!(
            ids,
            vec![
                filesystem::FS_READ_ONLY,
                filesystem::FS_AVAILABLE_BYTES,
                filesystem::FS_TOTAL_BYTES,
                filesystem::FS_INODES_UTILIZATION,
            ]
        );
        for s in &samples {
            let (k, v) = s.dimension.as_ref().unwrap();
            assert_eq!(*k, "mount_id");
            assert_eq!(v, &format!("tmpfs:{}", dir.display()));
        }
    }
}
