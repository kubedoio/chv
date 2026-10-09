//! Read-only node interface and block-device counters (`/proc/net/dev`,
//! `/proc/diskstats`).
//!
//! Per the metrics contract these are **per-device counters** with
//! registered dimensions (`interface_id` / `block_device_id`) — never
//! summed blindly across bridges or stacked devices; aggregation is a
//! query-time decision over labeled series.
//!
//! `/proc/diskstats` reports sectors; the kernel userspace ABI fixes the
//! sector size at 512 bytes for these counters (`proc(5)`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Bytes per sector in `/proc/diskstats` counters (kernel ABI).
pub const DISKSTATS_SECTOR_BYTES: u64 = 512;

/// One network interface's counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InterfaceCounters {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
}

/// One block device's counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BlockDeviceCounters {
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub reads_completed: u64,
    pub writes_completed: u64,
}

/// Parse `/proc/net/dev` into per-interface counters, excluding
/// loopback (`lo` carries no node load and would double-count
/// localhost traffic).
pub fn parse_net_dev(raw: &str) -> BTreeMap<String, InterfaceCounters> {
    let mut out = BTreeMap::new();
    // Skip the two header lines.
    for line in raw.lines().skip(2) {
        let Some((iface, rest)) = line.split_once(':') else {
            continue;
        };
        let iface = iface.trim();
        if iface.is_empty() || iface == "lo" {
            continue;
        }
        let mut f = rest.split_whitespace().map(|v| v.parse::<u64>().ok());
        // Column order per proc(5): receive bytes, packets, errs, drop,
        // fifo, frame, compressed, multicast, then transmit in the same
        // order.
        let rx_bytes = f.next().flatten().unwrap_or(0);
        let rx_packets = f.next().flatten().unwrap_or(0);
        for _ in 0..6 {
            let _ = f.next();
        }
        let tx_bytes = f.next().flatten().unwrap_or(0);
        let tx_packets = f.next().flatten().unwrap_or(0);
        out.insert(
            iface.to_string(),
            InterfaceCounters {
                rx_bytes,
                tx_bytes,
                rx_packets,
                tx_packets,
            },
        );
    }
    out
}

/// Read and parse the host's `/proc/net/dev`.
pub fn read_net_dev(proc_root: &Path) -> std::io::Result<BTreeMap<String, InterfaceCounters>> {
    Ok(parse_net_dev(&fs::read_to_string(
        proc_root.join("net/dev"),
    )?))
}

/// Parse `/proc/diskstats` into per-device counters, excluding the
/// virtual `loop*` and `ram*` devices (no real node I/O behind them).
pub fn parse_diskstats(raw: &str) -> BTreeMap<String, BlockDeviceCounters> {
    let mut out = BTreeMap::new();
    for line in raw.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        // fields: major minor name reads_completed reads_merged
        // sectors_read ms_reading writes_completed writes_merged
        // sectors_written ms_writing ...
        if f.len() < 10 {
            continue;
        }
        let name = f[2];
        if name.starts_with("loop") || name.starts_with("ram") {
            continue;
        }
        let parse = |i: usize| -> u64 { f[i].parse::<u64>().unwrap_or(0) };
        let sectors_read = parse(5);
        let sectors_written = parse(9);
        out.insert(
            name.to_string(),
            BlockDeviceCounters {
                read_bytes: sectors_read.saturating_mul(DISKSTATS_SECTOR_BYTES),
                write_bytes: sectors_written.saturating_mul(DISKSTATS_SECTOR_BYTES),
                reads_completed: parse(3),
                writes_completed: parse(7),
            },
        );
    }
    out
}

/// Read and parse the host's `/proc/diskstats`.
pub fn read_diskstats(proc_root: &Path) -> std::io::Result<BTreeMap<String, BlockDeviceCounters>> {
    Ok(parse_diskstats(&fs::read_to_string(
        proc_root.join("diskstats"),
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NET_DEV: &str = "Inter-|   Receive                                                |  Transmit\n face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed\n    lo: 1234567     987    0    0    0     0          0         0  1234567     987    0    0    0     0     0          0\n  eth0: 100 5    0    0    0     0          0         0  200 7    0    0    0     0     0          0\n my tap: 42 1 0 0 0 0 0 0  24 2 0 0 0 0 0 0\n";

    #[test]
    fn net_dev_parses_per_interface() {
        let m = parse_net_dev(NET_DEV);
        assert_eq!(m.len(), 2, "lo excluded, eth0 + 'my tap' kept");
        let eth0 = &m["eth0"];
        assert_eq!(eth0.rx_bytes, 100);
        assert_eq!(eth0.tx_bytes, 200);
        assert_eq!(eth0.rx_packets, 5);
        assert_eq!(eth0.tx_packets, 7);
        let tap = &m["my tap"];
        assert_eq!(tap.rx_bytes, 42);
        assert_eq!(tap.tx_bytes, 24);
    }

    const DISKSTATS: &str = "   8       0 sda 111 22 3333 444 55 66 7777 888 0 0 0 0 0 0\n   7       0 loop0 1 2 3 4 5 6 7 8 0 0 0 0 0 0\n   1       0 ram0 1 2 3 4 5 6 7 8 0 0 0 0 0 0\n 259       0 nvme0n1 100 0 200 0 300 0 400 0 0 0 0 0 0 0\n";

    #[test]
    fn diskstats_parses_and_excludes_virtual() {
        let m = parse_diskstats(DISKSTATS);
        assert_eq!(m.len(), 2, "loop0 and ram0 excluded");
        let sda = &m["sda"];
        assert_eq!(sda.reads_completed, 111);
        assert_eq!(sda.read_bytes, 3333 * DISKSTATS_SECTOR_BYTES);
        assert_eq!(sda.writes_completed, 55);
        assert_eq!(sda.write_bytes, 7777 * DISKSTATS_SECTOR_BYTES);
        assert_eq!(m["nvme0n1"].read_bytes, 200 * 512);
    }

    #[test]
    fn real_host_reads() {
        // Real-/proc smoke: parsing must succeed and `lo` must be
        // excluded. The environment may legitimately offer nothing to
        // read — a network namespace with only `lo` yields no
        // interfaces, a masked /proc/diskstats may be empty, and a
        // present-but-idle interface has all-zero counters — so the
        // parser's correctness is pinned by the fixture tests above,
        // not by host state.
        let root = Path::new("/proc");
        let net = read_net_dev(root).unwrap();
        assert!(!net.contains_key("lo"));
        let disks = read_diskstats(root).unwrap();
        for device in disks.keys() {
            assert!(!device.starts_with("loop") && !device.starts_with("ram"));
        }
    }
}
