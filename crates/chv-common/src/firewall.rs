//! The network firewall-rule vocabulary (#355/#368): ONE definition shared
//! by every layer that speaks rules — the BFF's save-time validation and
//! nwd's enforcement engine.
//!
//! Why this module exists: the UI's firewall editor and the (undispatched)
//! `/v1/firewall-rules` CRUD invented a parallel dialect
//! (`ingress`/`egress`, `allow`/`deny`, `port_range`), while nwd's engine
//! has always required `inbound`/`outbound`, `accept`/`drop`/`reject`,
//! `dest_port`. Pre-#355 nothing dispatched network policy to nodes, so
//! the split was invisible. Once #355 rode the persisted ruleset on the
//! VM spec, a dialect rule detonated at attach time (`attach_vm_nic`'s
//! policy-guard refresh rejected it) and terminally failed the VM create
//! with an opaque RUNTIME_UNAVAILABLE — the M4.4 re-qualification's N7.
//!
//! The engine vocabulary is the contract. Producers validate at save time
//! (fail fast, where the operator sees it); nwd stays strict (defense in
//! depth: a ruleset that somehow bypasses the save gate still fails loudly
//! at the enforcement point, never silently).

/// Rule directions nwd's engine accepts. The UI-era aliases
/// `ingress`/`egress` are NOT valid.
pub const DIRECTIONS: &[&str] = &["inbound", "outbound"];

/// Rule protocols nwd's engine accepts.
pub const PROTOCOLS: &[&str] = &["tcp", "udp", "icmp", "sctp", "all"];

/// Rule actions nwd's engine accepts. The UI-era aliases `allow`/`deny`
/// are NOT valid.
pub const ACTIONS: &[&str] = &["accept", "drop", "reject"];

/// The exact set of keys a rule object may carry (nwd's `FirewallRule`
/// fields). Anything else — `source`, `port_range`, `priority`,
/// `description` — is rejected at save time so a silently-dropped field
/// can never narrow a rule the operator meant to be scoped.
pub const RULE_KEYS: &[&str] = &[
    "direction",
    "protocol",
    "action",
    "source_cidr",
    "dest_port",
];

/// Is `cidr` a syntactically valid IPv4/IPv6 CIDR block (nwd's engine
/// semantics, shared with the BFF's save-time gate)?
pub fn is_valid_cidr(cidr: &str) -> bool {
    let parts: Vec<&str> = cidr.splitn(2, '/').collect();
    if parts.len() != 2 {
        return false;
    }
    match parts[0].parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(_)) => parts[1].parse::<u8>().map(|p| p <= 32).unwrap_or(false),
        Ok(std::net::IpAddr::V6(_)) => parts[1].parse::<u8>().map(|p| p <= 128).unwrap_or(false),
        Err(_) => false,
    }
}

/// Is `port` a valid single port or port range (`443`, `8080-8090`)?
pub fn is_valid_port_spec(port: &str) -> bool {
    if port.contains('-') {
        let parts: Vec<&str> = port.splitn(2, '-').collect();
        parts.len() == 2 && parts[0].parse::<u16>().is_ok() && parts[1].parse::<u16>().is_ok()
    } else {
        port.parse::<u16>().is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cidr_validator_accepts_v4_and_v6_blocks() {
        assert!(is_valid_cidr("10.200.0.0/24"));
        assert!(is_valid_cidr("0.0.0.0/0"));
        assert!(is_valid_cidr("fd00::/8"));
        assert!(!is_valid_cidr("10.200.0.0")); // no prefix
        assert!(!is_valid_cidr("10.200.0.0/33")); // bad prefix
        assert!(!is_valid_cidr("not-an-ip/24"));
    }

    #[test]
    fn port_spec_validator_accepts_ports_and_ranges() {
        assert!(is_valid_port_spec("443"));
        assert!(is_valid_port_spec("8080-8090"));
        assert!(!is_valid_port_spec("80-")); // dangling range
        assert!(!is_valid_port_spec("ssh")); // no service names
        assert!(!is_valid_port_spec("0-99999")); // out of u16
    }
}
