//! `/proc/net/dev` + `/sys/class/net` + `/proc/net/snmp` network
//! family (G4).

use crate::CollectedSample;

pub(crate) const NET_RX_BYTES: &str = "vm.guest.net.rx_bytes_total";
pub(crate) const NET_TX_BYTES: &str = "vm.guest.net.tx_bytes_total";
pub(crate) const NET_RX_ERRORS: &str = "vm.guest.net.rx_errors_total";
pub(crate) const NET_TX_ERRORS: &str = "vm.guest.net.tx_errors_total";
pub(crate) const NET_RX_DROPS: &str = "vm.guest.net.rx_drops_total";
pub(crate) const NET_TX_DROPS: &str = "vm.guest.net.tx_drops_total";
pub(crate) const NET_LINK_UP: &str = "vm.guest.net.link_up";
pub(crate) const NET_TCP_ESTABLISHED: &str = "vm.guest.net.tcp_established";

/// Discovery bound: /proc/net/dev with more interfaces (a container
/// with hundreds of veths) is truncated, not exploded. The 9th
/// interface is skipped. 8 interfaces × 7 samples + the other
/// families' worst case stay inside the ingestion contract's
/// 512-samples-per-batch ceiling (see the agent's family budget).
const MAX_INTERFACES: usize = 8;
const MAX_INTERFACE_ID_BYTES: usize = 128;

/// One `/proc/net/dev` line — the fields this family emits. The
/// parse validates the full 16-field shape (rx and tx: bytes,
/// packets, errs, drop, fifo, frame/colls, compressed, multicast)
/// even though only bytes/errors/drops are emitted.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InterfaceStats {
    pub name: String,
    pub rx_bytes: u64,
    pub rx_errors: u64,
    pub rx_drop: u64,
    pub tx_bytes: u64,
    pub tx_errors: u64,
    pub tx_drop: u64,
}

/// Pure parse of `/proc/net/dev`: skip the two header lines, then
/// `name: rx[8] tx[8]` per line, bounded to 8 interfaces.
/// Malformed lines are skipped.
pub(crate) fn parse_net_dev(net_dev: &str) -> Vec<InterfaceStats> {
    let mut out = Vec::new();
    for line in net_dev.lines().skip(2) {
        if out.len() >= MAX_INTERFACES {
            break;
        }
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let Some(fields) = rest
            .split_whitespace()
            .map(|f| f.parse::<u64>().ok())
            .collect::<Option<Vec<u64>>>()
        else {
            continue;
        };
        if fields.len() < 16 {
            continue;
        }
        out.push(InterfaceStats {
            name: name.to_string(),
            rx_bytes: fields[0],
            rx_errors: fields[2],
            rx_drop: fields[3],
            tx_bytes: fields[8],
            tx_errors: fields[10],
            tx_drop: fields[11],
        });
    }
    out
}

/// Interface names from /proc/net/dev are kernel-provided, but they
/// are still validated before any `/sys` path is built from them —
/// a malformed read must not turn into path traversal. Kernel
/// names are `[A-Za-z0-9._-]{1,15}` (IFNAMSIZ is 16 with the NUL).
pub(crate) fn valid_interface_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 15
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// `/sys/class/net/<if>/operstate` for a validated name.
pub(crate) fn operstate_path(sys_class_net: &str, name: &str) -> String {
    format!("{}/{}", sys_class_net.trim_end_matches('/'), name)
}

/// Classify an interface for the `interface_id` dimension token.
/// `phys_names` is the injectable set of names with a
/// `/sys/class/net/<if>/device` dir (the caller probes it).
pub(crate) fn classify_interface(name: &str, phys_names: &[String]) -> &'static str {
    if name == "lo" {
        return "loopback";
    }
    // Virtual names split by type: bridge-type prefixes vs
    // veth/overlay prefixes.
    const BRIDGE_PREFIXES: &[&str] = &["br-", "br", "virbr", "tap", "vbr"];
    const VIRTUAL_PREFIXES: &[&str] = &["veth", "docker", "flannel", "cni", "cali"];
    if BRIDGE_PREFIXES.iter().any(|p| name.starts_with(p)) {
        return "bridge";
    }
    if VIRTUAL_PREFIXES.iter().any(|p| name.starts_with(p)) {
        return "virtual";
    }
    if phys_names.iter().any(|n| n == name) {
        return "phys";
    }
    "other"
}

/// Map one interface to samples with its classification-derived
/// `interface_id` dimension. An invalid name produces nothing at
/// all (the caller must not have probed `/sys` for it either); an
/// unreadable operstate leaves only `link_up` absent.
pub(crate) fn emit_interface(
    stats: &InterfaceStats,
    operstate: Option<&str>,
    phys_names: &[String],
) -> Vec<CollectedSample> {
    if !valid_interface_name(&stats.name) {
        return Vec::new();
    }
    let class = classify_interface(&stats.name, phys_names);
    let interface_id = format!("{class}:{}", stats.name);
    if interface_id.len() > MAX_INTERFACE_ID_BYTES {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(7);
    let mut push = |metric_id, value| {
        out.push(CollectedSample::integer_with_dimension(
            metric_id,
            value,
            "interface_id",
            interface_id.clone(),
        ));
    };
    push(NET_RX_BYTES, stats.rx_bytes);
    push(NET_TX_BYTES, stats.tx_bytes);
    push(NET_RX_ERRORS, stats.rx_errors);
    push(NET_TX_ERRORS, stats.tx_errors);
    push(NET_RX_DROPS, stats.rx_drop);
    push(NET_TX_DROPS, stats.tx_drop);
    if let Some(state) = operstate {
        push(NET_LINK_UP, u64::from(state == "up"));
    }
    out
}

/// `CurrEstab` from the `/proc/net/snmp` `Tcp:` header + values
/// line pair, as a count.
pub(crate) fn parse_tcp_curr_estab(snmp: &str) -> Option<u64> {
    let mut header: Vec<&str> = Vec::new();
    let mut have_header = false;
    for line in snmp.lines() {
        let Some(rest) = line.strip_prefix("Tcp:") else {
            continue;
        };
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if !have_header {
            header = fields;
            have_header = true;
            continue;
        }
        let idx = header.iter().position(|f| *f == "CurrEstab")?;
        return fields.get(idx).copied()?.parse::<u64>().ok();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const NET_DEV: &str = "Inter-|   Receive                                                |  Transmit\n\
                           face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed\n\
                           lo: 1234567    100    0    0    0     0          0         0  1234567    100    0    0    0     0     0       0\n\
                           eth0: 987654321 54321    7    3    0     0          0         0  123456789 43210    2    1    0     0     0       0\n";

    #[test]
    fn parses_net_dev_fixture() {
        let ifaces = parse_net_dev(NET_DEV);
        assert_eq!(ifaces.len(), 2);
        assert_eq!(ifaces[0].name, "lo");
        assert_eq!(ifaces[0].rx_bytes, 1234567);
        let eth0 = &ifaces[1];
        assert_eq!(
            eth0,
            &InterfaceStats {
                name: "eth0".into(),
                rx_bytes: 987654321,
                rx_errors: 7,
                rx_drop: 3,
                tx_bytes: 123456789,
                tx_errors: 2,
                tx_drop: 1,
            }
        );
    }

    #[test]
    fn garbage_is_absence() {
        assert!(parse_net_dev("").is_empty());
        assert!(parse_net_dev("header\nheader\nbroken no colon\n").is_empty());
        // Non-numeric or short fields skip the line.
        assert!(parse_net_dev("h\nh\neth0: 1 2 3\n").is_empty());
        assert!(parse_net_dev("h\nh\neth0: x 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n").is_empty());
    }

    #[test]
    fn bounds_at_8_interfaces() {
        let mut text = String::from("h\nh\n");
        for i in 0..9 {
            text.push_str(&format!("if{i}: 1 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0\n"));
        }
        assert_eq!(parse_net_dev(&text).len(), 8, "9th interface is skipped");
    }

    #[test]
    fn interface_name_validation_rejects_traversal() {
        assert!(valid_interface_name("eth0"));
        assert!(valid_interface_name("br-abc123"));
        assert!(valid_interface_name("enp0s3.100"));
        assert!(!valid_interface_name("../../proc/1"), "traversal");
        assert!(!valid_interface_name("eth/0"), "path separator");
        assert!(!valid_interface_name("eth 0"), "space");
        assert!(!valid_interface_name(""), "empty");
        assert!(!valid_interface_name(&"x".repeat(16)), "IFNAMSIZ-1 is 15");
    }

    #[test]
    fn classification_tokens() {
        let phys = vec!["eno1".to_string()];
        assert_eq!(classify_interface("lo", &phys), "loopback");
        assert_eq!(classify_interface("br-abc", &phys), "bridge");
        assert_eq!(classify_interface("br0", &phys), "bridge");
        assert_eq!(classify_interface("virbr0", &phys), "bridge");
        assert_eq!(classify_interface("tap0", &phys), "bridge");
        assert_eq!(classify_interface("vbr0", &phys), "bridge");
        assert_eq!(classify_interface("veth0", &phys), "virtual");
        assert_eq!(classify_interface("docker0", &phys), "virtual");
        assert_eq!(classify_interface("cni0", &phys), "virtual");
        assert_eq!(classify_interface("cali1234abcd", &phys), "virtual");
        assert_eq!(classify_interface("flannel.1", &phys), "virtual");
        assert_eq!(classify_interface("eno1", &phys), "phys");
        assert_eq!(classify_interface("wlan0", &phys), "other");
    }

    #[test]
    fn emit_maps_counters_with_interface_id() {
        let stats = InterfaceStats {
            name: "eth0".into(),
            rx_bytes: 100,
            rx_errors: 1,
            rx_drop: 2,
            tx_bytes: 200,
            tx_errors: 3,
            tx_drop: 4,
        };
        let phys = vec!["eth0".to_string()];
        let out = emit_interface(&stats, Some("up"), &phys);
        assert_eq!(out.len(), 7);
        for s in &out {
            let (k, v) = s.dimension.as_ref().unwrap();
            assert_eq!((*k, v.as_str()), ("interface_id", "phys:eth0"));
            assert!(matches!(s.value, crate::SampleValue::Integer(_)));
        }
        assert_eq!(out[0].metric_id, NET_RX_BYTES);
        assert_eq!(out[0].value, crate::SampleValue::Integer(100));
        assert_eq!(out[5].metric_id, NET_TX_DROPS);
        assert_eq!(out[5].value, crate::SampleValue::Integer(4));
        // operstate "up" => link_up 1; anything else => 0.
        assert_eq!(out[6].metric_id, NET_LINK_UP);
        assert_eq!(out[6].value, crate::SampleValue::Integer(1));
        let down = emit_interface(&stats, Some("down"), &phys);
        assert_eq!(
            down.iter()
                .find(|s| s.metric_id == NET_LINK_UP)
                .unwrap()
                .value,
            crate::SampleValue::Integer(0)
        );
    }

    #[test]
    fn unreadable_operstate_leaves_only_link_up_absent() {
        let stats = InterfaceStats {
            name: "eth0".into(),
            rx_bytes: 1,
            rx_errors: 0,
            rx_drop: 0,
            tx_bytes: 1,
            tx_errors: 0,
            tx_drop: 0,
        };
        let out = emit_interface(&stats, None, &[]);
        assert_eq!(out.len(), 6, "counters still emit");
        assert!(!out.iter().any(|s| s.metric_id == NET_LINK_UP));
    }

    #[test]
    fn invalid_name_emits_nothing() {
        let stats = InterfaceStats {
            name: "../../proc/1".into(),
            rx_bytes: 1,
            rx_errors: 0,
            rx_drop: 0,
            tx_bytes: 1,
            tx_errors: 0,
            tx_drop: 0,
        };
        assert!(emit_interface(&stats, None, &[]).is_empty());
    }

    const SNMP: &str = "Ip: Forwarding DefaultTTL InReceives ...\n\
                        Ip: 1 64 12345 ...\n\
                        Tcp: RtoAlgorithm RtoMin RtoMax MaxConn ActiveOpens PassiveOpens AttemptFails EstabResets CurrEstab InSegs OutSegs RetransSegs\n\
                        Tcp: 1 200 120000 -1 8721 654 12 98 17 99123 88456 432\n";

    #[test]
    fn parses_tcp_curr_estab() {
        assert_eq!(parse_tcp_curr_estab(SNMP), Some(17));
    }

    #[test]
    fn tcp_garbage_is_absence() {
        assert_eq!(parse_tcp_curr_estab(""), None);
        assert_eq!(parse_tcp_curr_estab("Tcp: only a header\n"), None);
        // Missing CurrEstab column or a truncated values line.
        assert_eq!(parse_tcp_curr_estab("Tcp: A B\nTcp: 1 2\n"), None);
        assert_eq!(parse_tcp_curr_estab("Tcp: CurrEstab\nTcp: oops\n"), None);
    }
}
