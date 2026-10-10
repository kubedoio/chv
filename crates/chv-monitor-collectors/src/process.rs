//! `/proc/<pid>/{stat,status}` process family (G4).
//!
//! PRIVACY CONTRACT: this collector reads exactly two files per
//! pid — `stat` (cpu ticks + starttime) and `status` (Name, VmRSS).
//! It NEVER reads `cmdline` or `environ`; selectors match the
//! kernel-reported `Name:` field only.

use std::collections::HashMap;

pub(crate) const PROCESS_COUNT: &str = "vm.guest.process.count";
pub(crate) const PROCESS_RSS_BYTES: &str = "vm.guest.process.rss_bytes";
pub(crate) const PROCESS_CPU_UTILIZATION: &str = "vm.guest.process.cpu_utilization_ratio";

/// The fields of `/proc/<pid>/stat` the family needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PidStat {
    pub pid: i32,
    /// utime (field 14) + stime (field 15), clock ticks.
    pub cpu_ticks: u64,
    /// starttime (field 22), clock ticks since boot — the pid
    /// identity fence: a reused pid has a different starttime.
    pub starttime: u64,
}

/// Parse `/proc/<pid>/stat`. The comm field (2) is parenthesized
/// and may contain spaces and parens, so numbered fields are taken
/// relative to the LAST `)` in the line (field N is tail field
/// N - 3, where the tail starts at the state, field 3).
pub(crate) fn parse_pid_stat(stat: &str) -> Option<PidStat> {
    let line = stat.lines().next()?;
    let close = line.rfind(')')?;
    let pid = line.split_whitespace().next()?.parse::<i32>().ok()?;
    let fields: Vec<&str> = line[close + 1..].split_whitespace().collect();
    let field = |n: usize| -> Option<u64> { fields.get(n - 3)?.parse().ok() };
    Some(PidStat {
        pid,
        cpu_ticks: field(14)?.saturating_add(field(15)?),
        starttime: field(22)?,
    })
}

/// Parse `Name:` and `VmRSS:` from `/proc/<pid>/status`. `Name` is
/// required; a missing `VmRSS` (kernel threads have none) counts as
/// 0 kB — the kernel genuinely reports no user memory for them.
pub(crate) fn parse_status(status: &str) -> Option<(String, u64)> {
    let mut name = None;
    let mut rss_kb = 0u64;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("Name:") {
            name = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("VmRSS:") {
            // `VmRSS:\t  12345 kB`
            rss_kb = rest.split_whitespace().next()?.parse().ok()?;
        }
    }
    let name = name?;
    (!name.is_empty()).then_some((name, rss_kb))
}

/// One process as seen by a scan pass, already aggregated across
/// the two files.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcSnapshot {
    pub pid: i32,
    pub starttime: u64,
    /// `/proc/<pid>/status` `Name:` — the selector match key.
    pub name: String,
    pub rss_kb: u64,
    pub cpu_ticks: u64,
}

/// Stateful per-selector CPU sampler over matched
/// `(pid, starttime)` pairs. Mirrors `CpuSampler`'s discipline:
/// the first observation is an absence (no delta yet), a
/// zero-length or backwards wall interval is an absence, and a
/// clock-tick counter that goes backwards makes the whole tick
/// absent rather than noisy. A reused pid with a new starttime is a
/// different key — a new process whose accumulator starts empty.
pub struct ProcessSampler {
    clock_ticks_per_sec: u64,
    prev: HashMap<(i32, u64), u64>,
    /// 0 means "no previous observation" (epoch ms is never 0).
    prev_wall_ms: u64,
}

impl ProcessSampler {
    pub fn new(clock_ticks_per_sec: u64) -> Self {
        Self {
            clock_ticks_per_sec,
            prev: HashMap::new(),
            prev_wall_ms: 0,
        }
    }

    /// Feed the matched snapshots for one selector at `now_ms`
    /// (epoch ms) with the guest's cpu count. Returns the aggregate
    /// utilization ratio — summed tick deltas divided by the
    /// interval's tick capacity (wall delta × clock ticks per
    /// second × cpus), clamped to [0, 1] — or `None` for an honest
    /// absence.
    pub fn sample(&mut self, procs: &[ProcSnapshot], now_ms: u64, ncpus: u64) -> Option<f64> {
        let mut next: HashMap<(i32, u64), u64> = HashMap::with_capacity(procs.len());
        for p in procs {
            next.insert((p.pid, p.starttime), p.cpu_ticks);
        }
        let result = self.compute(&next, now_ms, ncpus);
        self.prev = next;
        self.prev_wall_ms = now_ms;
        result
    }

    fn compute(&self, next: &HashMap<(i32, u64), u64>, now_ms: u64, ncpus: u64) -> Option<f64> {
        if self.prev_wall_ms == 0 || now_ms <= self.prev_wall_ms {
            return None; // first observation / zero-length interval
        }
        let mut delta_ticks: u64 = 0;
        for (key, &ticks) in next {
            let Some(&prev_ticks) = self.prev.get(key) else {
                continue; // new process: no delta this tick
            };
            if ticks < prev_ticks {
                return None; // counter reset: absence, not noise
            }
            delta_ticks += ticks - prev_ticks;
        }
        let wall_s = (now_ms - self.prev_wall_ms) as f64 / 1000.0;
        let capacity_ticks = wall_s * self.clock_ticks_per_sec as f64 * ncpus as f64;
        if !capacity_ticks.is_finite() || capacity_ticks <= 0.0 {
            return None;
        }
        Some((delta_ticks as f64 / capacity_ticks).clamp(0.0, 1.0))
    }
}

/// Count `cpuN` lines in `/proc/stat` — the guest's cpu count for
/// process-utilization normalization.
pub(crate) fn parse_cpu_count(stat: &str) -> u64 {
    stat.lines()
        .filter(|l| {
            let Some(rest) = l.strip_prefix("cpu") else {
                return false;
            };
            // The aggregate line is `cpu  ...`; per-cpu lines are
            // `cpu0 ...`, `cpu12 ...`.
            rest.starts_with(|c: char| c.is_ascii_digit())
        })
        .count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    // tail after the comm parens: state(3) ppid(4) pgrp(5)
    // session(6) tty_nr(7) tpgid(8) flags(9) minflt(10) cminflt(11)
    // majflt(12) cmajflt(13) utime(14)=100 stime(15)=50 cutime(16)
    // cstime(17) priority(18) nice(19) threads(20) itrealvalue(21)
    // starttime(22)=999888, then filler.
    const STAT: &str = "4242 (my proc) S 1 2 3 4 5 6 7 8 9 10 100 50 0 20 0 1 0 0 999888 \
                        123456 789 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n";

    #[test]
    fn parse_pid_stat_reads_fields_after_parens() {
        let s = parse_pid_stat(STAT).unwrap();
        assert_eq!(s.pid, 4242);
        assert_eq!(s.cpu_ticks, 150, "utime + stime");
        assert_eq!(s.starttime, 999888);
    }

    #[test]
    fn parse_pid_stat_handles_spaces_and_parens_in_comm() {
        // comm is "a b) (c" — the last ')' is the real closer.
        let line = "7 (a b) (c) S 1 1 1 1 1 1 1 1 1 1 5 3 0 0 0 0 1 0 42 1 1 1 1\n";
        let s = parse_pid_stat(line).unwrap();
        assert_eq!(s.pid, 7);
        assert_eq!(s.cpu_ticks, 8);
        assert_eq!(s.starttime, 42);
    }

    #[test]
    fn parse_pid_stat_garbage_is_absence() {
        assert_eq!(parse_pid_stat(""), None);
        assert_eq!(parse_pid_stat("not a stat file\n"), None);
        assert_eq!(parse_pid_stat("1 (x) S 1\n"), None, "truncated tail");
        assert_eq!(
            parse_pid_stat("1 (x) S 1 1 1 1 1 1 1 1 1 1 -5 -5 0 0 0 0 1 0 0 42 1\n"),
            None,
            "negative ticks"
        );
    }

    #[test]
    fn parse_status_reads_name_and_rss() {
        let status = "Name:\tnginx\nUmask:\t0022\nVmRSS:\t   12345 kB\nThreads:\t4\n";
        assert_eq!(parse_status(status), Some(("nginx".into(), 12345)));
    }

    #[test]
    fn parse_status_kernel_thread_has_no_rss() {
        let status = "Name:\tkworker/0:1\nUmask:\t0000\nThreads:\t1\n";
        assert_eq!(parse_status(status), Some(("kworker/0:1".into(), 0)));
    }

    #[test]
    fn parse_status_garbage_is_absence() {
        assert_eq!(parse_status(""), None);
        assert_eq!(parse_status("VmRSS:\t123 kB\n"), None, "no Name");
        assert_eq!(parse_status("Name:\t\nVmRSS:\t1 kB\n"), None, "empty Name");
        assert_eq!(parse_status("Name:\tx\nVmRSS:\toops kB\n"), None);
    }

    #[test]
    fn parse_cpu_count_counts_cpun_lines() {
        let stat = "cpu  1 2 3 4 5 6 7 8 0 0\ncpu0 1 2 3 4 5 6 7 8\ncpu1 1 2 3 4 5 6 7 8\ncpu12 1 2 3 4\ncpufreq nonsense\nintr 1\n";
        assert_eq!(parse_cpu_count(stat), 3);
        assert_eq!(parse_cpu_count(""), 0);
    }

    fn snap(pid: i32, starttime: u64, ticks: u64) -> ProcSnapshot {
        ProcSnapshot {
            pid,
            starttime,
            name: "x".into(),
            rss_kb: 0,
            cpu_ticks: ticks,
        }
    }

    #[test]
    fn first_observation_is_absence() {
        let mut s = ProcessSampler::new(100);
        assert_eq!(s.sample(&[snap(1, 10, 50)], 1_000, 2), None);
    }

    #[test]
    fn computes_ratio_over_matched_deltas() {
        let mut s = ProcessSampler::new(100);
        s.sample(&[snap(1, 10, 50), snap(2, 20, 0)], 1_000, 2);
        // 1000 ms wall × 100 tps × 2 cpus = 200 ticks capacity;
        // delta = 100 + 50 = 150 ticks -> 0.75.
        let v = s
            .sample(&[snap(1, 10, 150), snap(2, 20, 50)], 2_000, 2)
            .unwrap();
        assert!((v - 0.75).abs() < 1e-9, "{v}");
    }

    #[test]
    fn pid_reuse_with_new_starttime_resets_accumulator() {
        let mut s = ProcessSampler::new(100);
        s.sample(&[snap(1, 10, 200)], 1_000, 1);
        // Same pid, different starttime: a new process — its ticks
        // are a first observation, contributing no delta.
        let v = s.sample(&[snap(1, 999, 300)], 2_000, 1).unwrap();
        assert_eq!(v, 0.0);
    }

    #[test]
    fn counter_reset_yields_absence_not_noise() {
        let mut s = ProcessSampler::new(100);
        s.sample(&[snap(1, 10, 200)], 1_000, 1);
        assert_eq!(s.sample(&[snap(1, 10, 100)], 2_000, 1), None);
    }

    #[test]
    fn zero_or_backwards_wall_interval_is_absence() {
        let mut s = ProcessSampler::new(100);
        s.sample(&[snap(1, 10, 0)], 2_000, 1);
        assert_eq!(s.sample(&[snap(1, 10, 50)], 2_000, 1), None, "zero delta");
        assert_eq!(s.sample(&[snap(1, 10, 50)], 1_999, 1), None, "backwards");
    }

    #[test]
    fn clamps_over_capacity_ratio() {
        let mut s = ProcessSampler::new(100);
        s.sample(&[snap(1, 10, 0)], 1_000, 1);
        // 1000 ms × 100 tps × 1 cpu = 100 ticks capacity, but the
        // process burned 500 — accounting anomaly, clamp to 1.
        let v = s.sample(&[snap(1, 10, 500)], 2_000, 1).unwrap();
        assert_eq!(v, 1.0);
    }

    #[test]
    fn zero_cpus_is_absence() {
        let mut s = ProcessSampler::new(100);
        s.sample(&[snap(1, 10, 0)], 1_000, 0);
        assert_eq!(s.sample(&[snap(1, 10, 50)], 2_000, 0), None);
    }
}
