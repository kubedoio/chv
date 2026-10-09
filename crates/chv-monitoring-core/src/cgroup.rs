//! Read-only cgroup v2 probes for verified runtime-owned cgroups
//! (prompt 01 task 4).
//!
//! Ownership fencing is explicit and layered:
//!
//! 1. the caller establishes that the pid is a process it owns (the
//!    runtime's `ProcessIdentity` machinery);
//! 2. [`ProcessFence`] re-validates that the pid has not been recycled
//!    (same pid, same `start_ticks`, same host `boot_id`) immediately
//!    before any read — a recycled pid must never lend its cgroup to a
//!    VM's samples;
//! 3. the cgroup path itself comes only from the kernel's
//!    `/proc/<pid>/cgroup` (never caller input), and is constrained to
//!    stay under the cgroup root.
//!
//! If ownership cannot be established, the caller must report
//! `unsupported` — never guess a process or open arbitrary paths. No
//! guest paths are ever opened by this module.

use std::fs;
use std::path::{Path, PathBuf};

use crate::process_probe::{read_boot_id, read_proc_stat, ProbeError};

/// Errors from cgroup probing. `Unsupported` is a **configuration**
/// verdict (no cgroup v2, or ownership unprovable), not a failure: the
/// corresponding samples must be reported `unsupported`, not `zero` and
/// not `unavailable`.
#[derive(Debug, thiserror::Error)]
pub enum CgroupProbeError {
    #[error("cgroup v2 is not usable on this host: {0}")]
    Unsupported(String),
    #[error("process fence failed: pid {0} is gone or recycled")]
    FenceFailed(u32),
    #[error("cgroup probe I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("malformed cgroup data: {0}")]
    Malformed(&'static str),
}

/// Pid + start-ticks + boot-id fence: the minimum identity that makes a
/// `/proc/<pid>` reading attributable to one process incarnation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessFence {
    pub pid: u32,
    pub start_ticks: u64,
    pub boot_id: String,
}

impl ProcessFence {
    /// Capture the fence for a live pid (reads its `stat` and the host
    /// boot id).
    pub fn capture(proc_root: &Path, pid: u32) -> Result<Self, ProbeError> {
        let stat = read_proc_stat(proc_root, pid)?;
        Ok(ProcessFence {
            pid,
            start_ticks: stat.start_ticks,
            boot_id: read_boot_id(proc_root)?,
        })
    }

    /// Re-validate: the pid must still exist with the same start ticks
    /// on the same boot. `false` means the reading must not be
    /// attributed to the original process (gone or recycled).
    pub fn verify(&self, proc_root: &Path) -> bool {
        if read_boot_id(proc_root)
            .map(|b| b != self.boot_id)
            .unwrap_or(true)
        {
            return false;
        }
        match read_proc_stat(proc_root, self.pid) {
            Ok(stat) => stat.start_ticks == self.start_ticks,
            Err(_) => false,
        }
    }

    /// The identity-epoch string for sample fencing.
    pub fn identity_epoch(&self) -> String {
        format!("pid-{}-start-{}", self.pid, self.start_ticks)
    }
}

/// One fenced cgroup v2 reading for a process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CgroupReading {
    /// Total CPU time consumed by the cgroup (user + system), from
    /// `cpu.stat` `usage_usec`.
    pub cpu_usage_usec: u64,
    /// `memory.current` — the cgroup's current resident memory, the
    /// host-accounted VM memory the metrics contract defines.
    pub memory_current_bytes: u64,
    /// The verified cgroup path the reading came from.
    pub cgroup_path: PathBuf,
}

/// Read-only cgroup v2 probe rooted at `/sys/fs/cgroup` (configurable
/// for tests).
pub struct CgroupV2Probe {
    cgroup_root: PathBuf,
    proc_root: PathBuf,
}

impl CgroupV2Probe {
    /// Probe against the real host roots.
    pub fn new() -> Self {
        CgroupV2Probe {
            cgroup_root: PathBuf::from("/sys/fs/cgroup"),
            proc_root: PathBuf::from("/proc"),
        }
    }

    /// Probe against explicit roots (tests and hermetic environments).
    pub fn with_roots(cgroup_root: impl Into<PathBuf>, proc_root: impl Into<PathBuf>) -> Self {
        CgroupV2Probe {
            cgroup_root: cgroup_root.into(),
            proc_root: proc_root.into(),
        }
    }

    /// Resolve the process's cgroup v2 path from the kernel's
    /// `/proc/<pid>/cgroup`. Only the unified-hierarchy entry
    /// (`0::<path>`) is accepted; a hybrid or v1 layout yields
    /// `Unsupported`.
    pub fn resolve_cgroup_path(&self, pid: u32) -> Result<PathBuf, CgroupProbeError> {
        let raw = fs::read_to_string(self.proc_root.join(pid.to_string()).join("cgroup"))?;
        let mut unified: Option<String> = None;
        for line in raw.lines() {
            let mut parts = line.splitn(3, ':');
            let hierarchy = parts.next().unwrap_or("");
            let controllers = parts.next().unwrap_or("");
            let path = parts.next().unwrap_or("");
            if hierarchy == "0" && controllers.is_empty() {
                unified = Some(path.to_string());
                break;
            }
        }
        let rel = unified.ok_or(CgroupProbeError::Unsupported(
            "no cgroup v2 unified hierarchy entry".to_string(),
        ))?;
        // Defense in depth: the path comes from the kernel, but a
        // relative escape must still never leave the cgroup root.
        if rel.split('/').any(|c| c == "..") {
            return Err(CgroupProbeError::Malformed("cgroup path escapes root"));
        }
        Ok(self.cgroup_root.join(rel.trim_start_matches('/')))
    }

    /// Read `cpu.stat` `usage_usec` from a cgroup path.
    pub fn read_cpu_usage_usec(&self, path: &Path) -> Result<u64, CgroupProbeError> {
        let raw = fs::read_to_string(path.join("cpu.stat"))?;
        for line in raw.lines() {
            if let Some(rest) = line.strip_prefix("usage_usec ") {
                return rest
                    .trim()
                    .parse::<u64>()
                    .map_err(|_| CgroupProbeError::Malformed("cpu.stat usage_usec"));
            }
        }
        Err(CgroupProbeError::Malformed("cpu.stat has no usage_usec"))
    }

    /// Read `memory.current` (bytes) from a cgroup path.
    pub fn read_memory_current(&self, path: &Path) -> Result<u64, CgroupProbeError> {
        let raw = fs::read_to_string(path.join("memory.current"))?;
        raw.trim()
            .parse::<u64>()
            .map_err(|_| CgroupProbeError::Malformed("memory.current"))
    }

    /// Fenced read: verify the fence, resolve the cgroup path from the
    /// kernel, and read `cpu.stat` + `memory.current`. A failed fence is
    /// an error (the caller reports `unsupported`/`unavailable`, never a
    /// guessed process's numbers).
    pub fn read_fenced(&self, fence: &ProcessFence) -> Result<CgroupReading, CgroupProbeError> {
        if !fence.verify(&self.proc_root) {
            return Err(CgroupProbeError::FenceFailed(fence.pid));
        }
        let path = self.resolve_cgroup_path(fence.pid)?;
        Ok(CgroupReading {
            cpu_usage_usec: self.read_cpu_usage_usec(&path)?,
            memory_current_bytes: self.read_memory_current(&path)?,
            cgroup_path: path,
        })
    }
}

impl Default for CgroupV2Probe {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_host(
        cgroup_root: &Path,
        proc_root: &Path,
        pid: u32,
        start_ticks: u64,
        cgroup_line: &str,
        cpu_stat: &str,
        memory_current: &str,
    ) {
        let pdir = proc_root.join(pid.to_string());
        fs::create_dir_all(&pdir).unwrap();
        // tokens after the comm: [0]=R, [1..=18]=1..18, [19]=start_ticks
        fs::write(
            pdir.join("stat"),
            format!("{pid} (cloud-hypervis) R 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 {start_ticks} 21 22"),
        )
        .unwrap();
        fs::write(pdir.join("cgroup"), cgroup_line).unwrap();
        let bdir = proc_root.join("sys/kernel/random");
        fs::create_dir_all(&bdir).unwrap();
        fs::write(bdir.join("boot_id"), "boot-1\n").unwrap();

        let cdir = cgroup_root.join("chv.slice/vm-1.slice");
        fs::create_dir_all(&cdir).unwrap();
        fs::write(cdir.join("cpu.stat"), cpu_stat).unwrap();
        fs::write(cdir.join("memory.current"), memory_current).unwrap();
    }

    #[test]
    fn fenced_read_resolves_and_reads() {
        let cg = tempfile::tempdir().unwrap();
        let pr = tempfile::tempdir().unwrap();
        fake_host(
            cg.path(),
            pr.path(),
            100,
            5000,
            "0::/chv.slice/vm-1.slice\n",
            "usage_usec 1234567\nuser_usec 1000000\nsystem_usec 234567\n",
            "536870912\n",
        );
        let probe = CgroupV2Probe::with_roots(cg.path(), pr.path());
        let fence = ProcessFence::capture(pr.path(), 100).unwrap();
        assert_eq!(fence.start_ticks, 5000);
        let reading = probe.read_fenced(&fence).unwrap();
        assert_eq!(reading.cpu_usage_usec, 1_234_567);
        assert_eq!(reading.memory_current_bytes, 536_870_912);
        assert_eq!(reading.cgroup_path, cg.path().join("chv.slice/vm-1.slice"));
    }

    #[test]
    fn recycled_pid_fails_the_fence() {
        let cg = tempfile::tempdir().unwrap();
        let pr = tempfile::tempdir().unwrap();
        fake_host(
            cg.path(),
            pr.path(),
            100,
            5000,
            "0::/chv.slice/vm-1.slice\n",
            "usage_usec 1\n",
            "1\n",
        );
        let probe = CgroupV2Probe::with_roots(cg.path(), pr.path());
        let fence = ProcessFence::capture(pr.path(), 100).unwrap();
        // The pid is "recycled": same pid, different start ticks.
        fs::write(
            pr.path().join("100/stat"),
            "100 (other-proc) R 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 9999 21 22",
        )
        .unwrap();
        assert!(matches!(
            probe.read_fenced(&fence),
            Err(CgroupProbeError::FenceFailed(100))
        ));
    }

    #[test]
    fn gone_pid_fails_the_fence() {
        let cg = tempfile::tempdir().unwrap();
        let pr = tempfile::tempdir().unwrap();
        fake_host(
            cg.path(),
            pr.path(),
            100,
            5000,
            "0::/\n",
            "usage_usec 1\n",
            "1\n",
        );
        let fence = ProcessFence::capture(pr.path(), 100).unwrap();
        fs::remove_dir_all(pr.path().join("100")).unwrap();
        assert!(!fence.verify(pr.path()));
    }

    #[test]
    fn boot_change_fails_the_fence() {
        let cg = tempfile::tempdir().unwrap();
        let pr = tempfile::tempdir().unwrap();
        fake_host(
            cg.path(),
            pr.path(),
            100,
            5000,
            "0::/\n",
            "usage_usec 1\n",
            "1\n",
        );
        let fence = ProcessFence::capture(pr.path(), 100).unwrap();
        fs::write(pr.path().join("sys/kernel/random/boot_id"), "boot-2\n").unwrap();
        assert!(!fence.verify(pr.path()));
    }

    #[test]
    fn hybrid_layout_is_unsupported() {
        let cg = tempfile::tempdir().unwrap();
        let pr = tempfile::tempdir().unwrap();
        fake_host(
            cg.path(),
            pr.path(),
            100,
            1,
            "2:cpu:/\n1:name=systemd:/init.scope\n",
            "usage_usec 1\n",
            "1\n",
        );
        let probe = CgroupV2Probe::with_roots(cg.path(), pr.path());
        assert!(matches!(
            probe.resolve_cgroup_path(100),
            Err(CgroupProbeError::Unsupported(_))
        ));
    }

    #[test]
    fn escaping_path_is_rejected() {
        let cg = tempfile::tempdir().unwrap();
        let pr = tempfile::tempdir().unwrap();
        fake_host(
            cg.path(),
            pr.path(),
            100,
            1,
            "0::/../..\n",
            "usage_usec 1\n",
            "1\n",
        );
        let probe = CgroupV2Probe::with_roots(cg.path(), pr.path());
        assert!(matches!(
            probe.resolve_cgroup_path(100),
            Err(CgroupProbeError::Malformed(_))
        ));
    }

    #[test]
    fn real_host_unified_hierarchy_resolves_for_self() {
        // On the real host (cgroup v2 unified), this process must resolve
        // to a real cgroup path and read cpu.stat. Skipped when the test
        // host itself is not cgroup v2 (CI containers may differ).
        let probe = CgroupV2Probe::new();
        match probe.resolve_cgroup_path(std::process::id()) {
            Ok(path) => {
                assert!(path.starts_with("/sys/fs/cgroup"));
                assert!(probe.read_cpu_usage_usec(&path).is_ok());
            }
            Err(CgroupProbeError::Unsupported(_)) => { /* host layout not v2-only */ }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }
}
