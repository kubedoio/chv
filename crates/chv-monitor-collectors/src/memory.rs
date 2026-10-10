//! `/proc/meminfo` `MemAvailable` (bytes).

pub(crate) const MEM_AVAILABLE: &str = "vm.memory.guest_available_bytes";

/// Parse `MemAvailable:` from `/proc/meminfo` (kB units).
pub(crate) fn parse_available_bytes(meminfo: &str) -> Option<f64> {
    for line in meminfo.lines() {
        let mut parts = line.split_whitespace();
        if parts.next()? == "MemAvailable:" {
            let kb: f64 = parts.next()?.parse().ok()?;
            if !kb.is_finite() || kb < 0.0 {
                return None;
            }
            return Some(kb * 1024.0);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMINFO: &str = "MemTotal:       16384000 kB\nMemFree:         8192000 kB\nMemAvailable:   10485760 kB\nBuffers:          512000 kB\n";

    #[test]
    fn parses_mem_available() {
        assert_eq!(parse_available_bytes(MEMINFO), Some(10_485_760.0 * 1024.0));
    }

    #[test]
    fn missing_field_is_absence() {
        assert_eq!(parse_available_bytes("MemTotal: 100 kB\n"), None);
        assert_eq!(parse_available_bytes(""), None);
    }

    #[test]
    fn negative_or_garbage_is_absence() {
        assert_eq!(parse_available_bytes("MemAvailable: -5 kB\n"), None);
        assert_eq!(parse_available_bytes("MemAvailable: oops kB\n"), None);
    }
}
