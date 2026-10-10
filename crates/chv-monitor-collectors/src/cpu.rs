//! `/proc/stat` CPU utilization (aggregate `cpu` line).
//!
//! Utilization is a ratio of deltas between two observations of the
//! aggregate CPU time line. The first observation establishes the
//! baseline and produces no sample — an honest absence, not a zero.

pub(crate) const CPU_UTILIZATION: &str = "vm.guest.cpu.utilization_ratio";

/// Aggregate CPU times, in units of USER_HZ (typically 1/100 s).
#[derive(Debug, Clone, Copy, PartialEq)]
struct CpuTimes {
    idle: f64,
    total: f64,
}

/// Stateful sampler over `/proc/stat` aggregate `cpu` lines.
#[derive(Debug, Default)]
pub struct CpuSampler {
    prev: Option<CpuTimes>,
}

impl CpuSampler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one `/proc/stat` read; `None` means "no sample" (first
    /// observation, unreadable source, or a zero-length interval).
    pub fn sample(&mut self, stat: &str) -> Option<f64> {
        let now = parse_aggregate(stat)?;
        let prev = self.prev.replace(now)?;
        let d_total = now.total - prev.total;
        if d_total <= 0.0 {
            return None;
        }
        let d_idle = (now.idle - prev.idle).max(0.0);
        // Clamp: kernel accounting anomalies must not produce a ratio
        // outside [0, 1].
        Some((1.0 - d_idle / d_total).clamp(0.0, 1.0))
    }
}

/// Parse the aggregate `cpu` line of `/proc/stat`:
/// `cpu  user nice system idle iowait irq softirq steal ...`.
/// Idle-adjacent counters (idle + iowait) count as not-busy; the
/// total is the sum of all fields after the label.
fn parse_aggregate(stat: &str) -> Option<CpuTimes> {
    let line = stat.lines().next()?;
    let mut fields = line.split_whitespace();
    if fields.next()? != "cpu" {
        return None;
    }
    let values: Vec<f64> = fields
        .map(|f| f.parse::<f64>().ok())
        .collect::<Option<_>>()?;
    if values.len() < 4 {
        return None;
    }
    let idle = values.get(3).copied()? + values.get(4).copied().unwrap_or(0.0);
    let total = values.iter().sum();
    Some(CpuTimes { idle, total })
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: &str = "cpu  100 0 100 700 50 0 0 50 0 0\n";
    const T1: &str = "cpu  150 0 150 750 50 0 0 50 0 0\n";

    #[test]
    fn first_observation_is_absence() {
        let mut s = CpuSampler::new();
        assert_eq!(s.sample(T0), None);
    }

    #[test]
    fn delta_computes_busy_ratio() {
        // T0: total 1000, idle 750. T1: total 1150, idle 800.
        // busy delta 150 / total delta 150 = 1.0? No: total delta
        // 150, idle delta 50 -> busy 100/150 = 0.667.
        let mut s = CpuSampler::new();
        s.sample(T0);
        let v = s.sample(T1).unwrap();
        assert!((v - 100.0 / 150.0).abs() < 1e-9, "{v}");
    }

    #[test]
    fn counter_reset_yields_absence_not_noise() {
        let mut s = CpuSampler::new();
        s.sample(T1);
        assert_eq!(s.sample(T0), None, "backwards clock must be absent");
    }

    #[test]
    fn clamps_anomalous_idle_growth() {
        let mut s = CpuSampler::new();
        s.sample("cpu  10 0 10 80 0 0 0 0\n");
        // idle grows more than total — impossible; must clamp, not
        // produce a negative ratio.
        let v = s.sample("cpu  10 0 10 90 0 0 0 0\n").unwrap();
        assert_eq!(v, 0.0);
    }

    #[test]
    fn garbage_is_absence() {
        let mut s = CpuSampler::new();
        assert_eq!(s.sample(""), None);
        assert_eq!(s.sample("meminfo: not this file\n"), None);
        assert_eq!(s.sample("cpu  1 2 3\n"), None, "too few fields");
    }
}
