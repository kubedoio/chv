use chv_errors::ChvError;
use serde::Deserialize;
use tokio::process::Command;
use tracing::info;

#[derive(Clone, Debug, Deserialize)]
pub struct FirewallRule {
    pub direction: String,
    pub protocol: String,
    pub source_cidr: Option<String>,
    pub dest_port: Option<String>,
    pub action: String,
}

const ALLOWED_PROTOCOLS: &[&str] = &["tcp", "udp", "icmp", "sctp", "all"];
const ALLOWED_ACTIONS: &[&str] = &["accept", "drop", "reject"];
const ALLOWED_DIRECTIONS: &[&str] = &["inbound", "outbound"];

fn validate_rule(rule: &FirewallRule) -> Result<(), ChvError> {
    if !ALLOWED_DIRECTIONS.contains(&rule.direction.as_str()) {
        return Err(ChvError::InvalidArgument {
            field: "direction".to_string(),
            reason: format!(
                "invalid direction '{}': must be inbound or outbound",
                rule.direction
            ),
        });
    }
    if !ALLOWED_PROTOCOLS.contains(&rule.protocol.as_str()) {
        return Err(ChvError::InvalidArgument {
            field: "protocol".to_string(),
            reason: format!(
                "invalid protocol '{}': must be one of tcp, udp, icmp, sctp, all",
                rule.protocol
            ),
        });
    }
    if !ALLOWED_ACTIONS.contains(&rule.action.as_str()) {
        return Err(ChvError::InvalidArgument {
            field: "action".to_string(),
            reason: format!(
                "invalid action '{}': must be accept, drop, or reject",
                rule.action
            ),
        });
    }
    if let Some(ref cidr) = rule.source_cidr {
        if !is_valid_cidr(cidr) {
            return Err(ChvError::InvalidArgument {
                field: "source_cidr".to_string(),
                reason: format!("invalid CIDR: '{}'", cidr),
            });
        }
    }
    if let Some(ref port) = rule.dest_port {
        if !is_valid_port_spec(port) {
            return Err(ChvError::InvalidArgument {
                field: "dest_port".to_string(),
                reason: format!("invalid port spec: '{}'", port),
            });
        }
    }
    Ok(())
}

fn is_valid_cidr(cidr: &str) -> bool {
    let parts: Vec<&str> = cidr.splitn(2, '/').collect();
    if parts.len() != 2 {
        return false;
    }
    parts[0].parse::<std::net::IpAddr>().is_ok()
        && parts[1].parse::<u8>().map(|p| p <= 128).unwrap_or(false)
}

fn is_valid_port_spec(port: &str) -> bool {
    if port.contains('-') {
        let parts: Vec<&str> = port.splitn(2, '-').collect();
        parts.len() == 2 && parts[0].parse::<u16>().is_ok() && parts[1].parse::<u16>().is_ok()
    } else {
        port.parse::<u16>().is_ok()
    }
}

pub async fn apply_firewall_rules(table: &str, policy_json: &[u8]) -> Result<(), ChvError> {
    let rules: Vec<FirewallRule> = if policy_json.is_empty() {
        Vec::new()
    } else {
        serde_json::from_slice(policy_json).map_err(|e| ChvError::InvalidArgument {
            field: "policy_json".to_string(),
            reason: format!("failed to parse firewall rules: {}", e),
        })?
    };

    for rule in &rules {
        validate_rule(rule)?;
    }

    // Ensure table exists
    run_nft_idempotent(&["add", "table", "inet", table]).await?;

    // Create chains if needed
    for (chain, hook) in [
        ("input", "input"),
        ("forward", "forward"),
        ("output", "output"),
    ] {
        run_nft_idempotent(&[
            "add",
            "chain",
            "inet",
            table,
            chain,
            &format!(
                "{{ type filter hook {} priority 0 ; policy accept ; }}",
                hook
            ),
        ])
        .await?;
    }

    // Flush existing rules in filter chains (atomic replace)
    for chain in ["input", "forward", "output"] {
        let _ = run_nft(&["flush", "chain", "inet", table, chain]).await;
    }

    // Always add conntrack established/related rule first
    run_nft(&[
        "add",
        "rule",
        "inet",
        table,
        "input",
        "ct",
        "state",
        "established,related",
        "accept",
    ])
    .await?;

    // Apply user rules
    for rule in &rules {
        let chain = match rule.direction.as_str() {
            "inbound" => "input",
            "outbound" => "output",
            _ => continue,
        };

        let mut args: Vec<&str> = vec!["add", "rule", "inet", table, chain];

        // Protocol match (skip for "all")
        let protocol_lower = rule.protocol.to_lowercase();
        if protocol_lower != "all" {
            args.push("meta");
            args.push("l4proto");
            args.push(&protocol_lower);
        }

        // Source CIDR match
        let cidr_owned;
        if let Some(ref cidr) = rule.source_cidr {
            args.push("ip");
            args.push("saddr");
            cidr_owned = cidr.clone();
            args.push(&cidr_owned);
        }

        // Destination port match
        let port_owned;
        if let Some(ref port) = rule.dest_port {
            if protocol_lower == "tcp" || protocol_lower == "udp" {
                args.push(&protocol_lower);
                args.push("dport");
                port_owned = port.clone();
                args.push(&port_owned);
            }
        }

        // Action
        let action = rule.action.to_lowercase();
        args.push(&action);

        run_nft(&args).await?;
    }

    info!(table = %table, rule_count = rules.len(), "firewall rules applied");
    Ok(())
}

#[derive(Clone, Debug, Deserialize)]
pub struct NatRule {
    pub source_cidr: String,
    pub dest_cidr: Option<String>,
    pub masquerade: Option<bool>,
}

pub async fn apply_nat_rules(table: &str, policy_json: &[u8]) -> Result<(), ChvError> {
    let rules: Vec<NatRule> = if policy_json.is_empty() {
        Vec::new()
    } else {
        serde_json::from_slice(policy_json).map_err(|e| ChvError::InvalidArgument {
            field: "policy_json".to_string(),
            reason: format!("failed to parse NAT rules: {}", e),
        })?
    };

    for rule in &rules {
        if !is_valid_cidr(&rule.source_cidr) {
            return Err(ChvError::InvalidArgument {
                field: "source_cidr".to_string(),
                reason: format!("invalid CIDR: '{}'", rule.source_cidr),
            });
        }
        if let Some(ref dest) = rule.dest_cidr {
            if !is_valid_cidr(dest) {
                return Err(ChvError::InvalidArgument {
                    field: "dest_cidr".to_string(),
                    reason: format!("invalid CIDR: '{}'", dest),
                });
            }
        }
    }

    // Ensure table and postrouting chain exist
    run_nft_idempotent(&["add", "table", "inet", table]).await?;
    run_nft_idempotent(&[
        "add",
        "chain",
        "inet",
        table,
        "postrouting",
        "{ type nat hook postrouting priority 100 ; policy accept ; }",
    ])
    .await?;

    // Flush existing NAT rules
    let _ = run_nft(&["flush", "chain", "inet", table, "postrouting"]).await;

    if rules.is_empty() {
        // Default: masquerade all non-loopback traffic
        run_nft(&[
            "add",
            "rule",
            "inet",
            table,
            "postrouting",
            "oif",
            "!=",
            "lo",
            "masquerade",
        ])
        .await?;
    } else {
        for rule in &rules {
            let mut args: Vec<&str> = vec!["add", "rule", "inet", table, "postrouting"];

            args.push("ip");
            args.push("saddr");
            args.push(&rule.source_cidr);

            if let Some(ref dest) = rule.dest_cidr {
                args.push("ip");
                args.push("daddr");
                args.push(dest);
            }

            if rule.masquerade.unwrap_or(true) {
                args.push("masquerade");
            }

            run_nft(&args).await?;
        }
    }

    info!(table = %table, rule_count = rules.len(), "NAT rules applied");
    Ok(())
}

async fn run_nft(args: &[&str]) -> Result<(), ChvError> {
    let out = Command::new("nft")
        .args(args)
        .output()
        .await
        .map_err(|e| ChvError::Io {
            path: "nft".to_string(),
            source: e,
        })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(ChvError::NetworkUnavailable {
            resource: "nft".to_string(),
            reason: format!("nft {} failed: {}", args.join(" "), stderr),
        });
    }
    Ok(())
}

async fn run_nft_idempotent(args: &[&str]) -> Result<(), ChvError> {
    match run_nft(args).await {
        Ok(()) => Ok(()),
        Err(ChvError::NetworkUnavailable { reason, .. }) => {
            if reason.contains("File exists") || reason.contains("already exists") {
                Ok(())
            } else {
                Err(ChvError::NetworkUnavailable {
                    resource: "nft".to_string(),
                    reason,
                })
            }
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_rule_valid() {
        let rule = FirewallRule {
            direction: "inbound".to_string(),
            protocol: "tcp".to_string(),
            source_cidr: Some("10.0.0.0/24".to_string()),
            dest_port: Some("443".to_string()),
            action: "accept".to_string(),
        };
        assert!(validate_rule(&rule).is_ok());
    }

    #[test]
    fn test_validate_rule_invalid_direction() {
        let rule = FirewallRule {
            direction: "sideways".to_string(),
            protocol: "tcp".to_string(),
            source_cidr: None,
            dest_port: None,
            action: "accept".to_string(),
        };
        assert!(validate_rule(&rule).is_err());
    }

    #[test]
    fn test_validate_rule_invalid_protocol() {
        let rule = FirewallRule {
            direction: "inbound".to_string(),
            protocol: "gopher".to_string(),
            source_cidr: None,
            dest_port: None,
            action: "accept".to_string(),
        };
        assert!(validate_rule(&rule).is_err());
    }

    #[test]
    fn test_validate_rule_invalid_action() {
        let rule = FirewallRule {
            direction: "inbound".to_string(),
            protocol: "tcp".to_string(),
            source_cidr: None,
            dest_port: None,
            action: "explode".to_string(),
        };
        assert!(validate_rule(&rule).is_err());
    }

    #[test]
    fn test_validate_rule_invalid_cidr() {
        let rule = FirewallRule {
            direction: "inbound".to_string(),
            protocol: "tcp".to_string(),
            source_cidr: Some("not-a-cidr".to_string()),
            dest_port: None,
            action: "accept".to_string(),
        };
        assert!(validate_rule(&rule).is_err());
    }

    #[test]
    fn test_validate_rule_port_range() {
        let rule = FirewallRule {
            direction: "inbound".to_string(),
            protocol: "tcp".to_string(),
            source_cidr: None,
            dest_port: Some("8000-9000".to_string()),
            action: "drop".to_string(),
        };
        assert!(validate_rule(&rule).is_ok());
    }

    #[test]
    fn test_validate_rule_invalid_port() {
        let rule = FirewallRule {
            direction: "inbound".to_string(),
            protocol: "tcp".to_string(),
            source_cidr: None,
            dest_port: Some("abc".to_string()),
            action: "accept".to_string(),
        };
        assert!(validate_rule(&rule).is_err());
    }

    #[test]
    fn test_is_valid_cidr() {
        assert!(is_valid_cidr("10.0.0.0/24"));
        assert!(is_valid_cidr("192.168.1.0/16"));
        assert!(is_valid_cidr("::1/128"));
        assert!(!is_valid_cidr("10.0.0.0"));
        assert!(!is_valid_cidr("not-ip/24"));
        assert!(!is_valid_cidr("10.0.0.0/999"));
    }

    #[test]
    fn test_parse_empty_policy() {
        let rules: Vec<FirewallRule> = serde_json::from_slice(b"[]").unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn test_parse_policy_json() {
        let json = r#"[
            {"direction": "inbound", "protocol": "tcp", "source_cidr": "10.0.0.0/8", "dest_port": "22", "action": "accept"},
            {"direction": "outbound", "protocol": "all", "action": "accept"}
        ]"#;
        let rules: Vec<FirewallRule> = serde_json::from_slice(json.as_bytes()).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].dest_port.as_deref(), Some("22"));
        assert!(rules[1].source_cidr.is_none());
    }
}
