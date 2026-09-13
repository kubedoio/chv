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
    match parts[0].parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(_)) => parts[1].parse::<u8>().map(|p| p <= 32).unwrap_or(false),
        Ok(std::net::IpAddr::V6(_)) => parts[1].parse::<u8>().map(|p| p <= 128).unwrap_or(false),
        Err(_) => false,
    }
}

fn is_valid_port_spec(port: &str) -> bool {
    if port.contains('-') {
        let parts: Vec<&str> = port.splitn(2, '-').collect();
        parts.len() == 2 && parts[0].parse::<u16>().is_ok() && parts[1].parse::<u16>().is_ok()
    } else {
        port.parse::<u16>().is_ok()
    }
}

pub fn validate_interface_name(name: &str) -> Result<(), ChvError> {
    if name.is_empty() {
        return Err(ChvError::InvalidArgument {
            field: "interface_name".to_string(),
            reason: "interface name must not be empty".to_string(),
        });
    }
    if name.len() > 15 {
        return Err(ChvError::InvalidArgument {
            field: "interface_name".to_string(),
            reason: format!(
                "interface name '{}' exceeds 15 bytes (IFNAMSIZ-1, got {})",
                name,
                name.len()
            ),
        });
    }
    if name.starts_with('-')
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(ChvError::InvalidArgument {
            field: "interface_name".to_string(),
            reason: format!(
                "interface name '{}' contains invalid characters: must be alphanumeric, '_', '-', or '.' and not start with '-'",
                name
            ),
        });
    }
    Ok(())
}

pub fn validate_table_name(name: &str) -> Result<(), ChvError> {
    if name.is_empty() {
        return Err(ChvError::InvalidArgument {
            field: "table".to_string(),
            reason: "table name must not be empty".to_string(),
        });
    }
    // nftables names are limited to 255 bytes (NFT_NAME_MAXLEN excludes the
    // terminating NUL). Network IDs are commonly UUIDs, so do not impose a
    // smaller application-specific limit here.
    if name.len() > 255 {
        return Err(ChvError::InvalidArgument {
            field: "table".to_string(),
            reason: format!(
                "table name '{}' exceeds 255 bytes (got {})",
                name,
                name.len()
            ),
        });
    }
    if name.starts_with('-')
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(ChvError::InvalidArgument {
            field: "table".to_string(),
            reason: format!(
                "table name '{}' contains invalid characters: must be alphanumeric, '_', '-', or '.' and not start with '-'",
                name
            ),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NftAction {
    AddIdempotent(Vec<String>),
    FlushChain(String),
    DeleteByComment { chain: String, comment: String },
    AddRule { chain: String, args: Vec<String> },
}

pub fn plan_firewall_rules(
    table: &str,
    bridge_name: &str,
    rules: &[FirewallRule],
) -> Result<Vec<NftAction>, ChvError> {
    validate_table_name(table)?;
    validate_interface_name(bridge_name)?;

    for rule in rules {
        validate_rule(rule)?;
    }

    let mut actions = Vec::new();

    // Ensure table exists
    actions.push(NftAction::AddIdempotent(vec![
        "add".into(),
        "table".into(),
        "inet".into(),
        table.into(),
    ]));

    // Base chains with policy accept - ensures host-wide, Docker, and K8s/CNI traffic is NOT blocked.
    for (chain, hook) in [
        ("input", "input"),
        ("forward", "forward"),
        ("output", "output"),
    ] {
        actions.push(NftAction::AddIdempotent(vec![
            "add".into(),
            "chain".into(),
            "inet".into(),
            table.into(),
            chain.into(),
            format!(
                "{{ type filter hook {} priority filter ; policy accept ; }}",
                hook
            ),
        ]));
        // In case chain already existed with policy drop from previous runs, update policy to accept
        actions.push(NftAction::AddIdempotent(vec![
            "add".into(),
            "chain".into(),
            "inet".into(),
            table.into(),
            chain.into(),
            "{ policy accept ; }".into(),
        ]));
    }

    // Create subchains for CHV-scoped traffic filtering
    for subchain in ["chv-input", "chv-forward", "chv-output"] {
        actions.push(NftAction::AddIdempotent(vec![
            "add".into(),
            "chain".into(),
            "inet".into(),
            table.into(),
            subchain.into(),
        ]));
        actions.push(NftAction::FlushChain(subchain.into()));
    }

    // Flush base input and output chains, and configure interface-scoped jumps to subchains
    actions.push(NftAction::FlushChain("input".into()));
    actions.push(NftAction::AddRule {
        chain: "input".into(),
        args: vec![
            "iifname".into(),
            bridge_name.into(),
            "jump".into(),
            "chv-input".into(),
        ],
    });

    actions.push(NftAction::FlushChain("output".into()));
    actions.push(NftAction::AddRule {
        chain: "output".into(),
        args: vec![
            "oifname".into(),
            bridge_name.into(),
            "jump".into(),
            "chv-output".into(),
        ],
    });

    // For forward chain, remove existing jumps by comment to preserve any exposed service rules, then add jumps
    actions.push(NftAction::DeleteByComment {
        chain: "forward".into(),
        comment: "chv-fwd-in".into(),
    });
    actions.push(NftAction::DeleteByComment {
        chain: "forward".into(),
        comment: "chv-fwd-out".into(),
    });
    actions.push(NftAction::AddRule {
        chain: "forward".into(),
        args: vec![
            "iifname".into(),
            bridge_name.into(),
            "jump".into(),
            "chv-forward".into(),
            "comment".into(),
            "\"chv-fwd-in\"".into(),
        ],
    });
    actions.push(NftAction::AddRule {
        chain: "forward".into(),
        args: vec![
            "oifname".into(),
            bridge_name.into(),
            "jump".into(),
            "chv-forward".into(),
            "comment".into(),
            "\"chv-fwd-out\"".into(),
        ],
    });

    // Populate chv-input:
    // 1) Conntrack established/related accept
    actions.push(NftAction::AddRule {
        chain: "chv-input".into(),
        args: vec![
            "ct".into(),
            "state".into(),
            "established,related".into(),
            "accept".into(),
        ],
    });

    // Populate chv-output:
    // 1) Conntrack established/related accept
    actions.push(NftAction::AddRule {
        chain: "chv-output".into(),
        args: vec![
            "ct".into(),
            "state".into(),
            "established,related".into(),
            "accept".into(),
        ],
    });

    // Populate chv-forward:
    // 1) Conntrack established/related accept
    actions.push(NftAction::AddRule {
        chain: "chv-forward".into(),
        args: vec![
            "ct".into(),
            "state".into(),
            "established,related".into(),
            "accept".into(),
        ],
    });

    // Order user rules: deny/reject first, then accept
    let ordered_rules: Vec<&FirewallRule> = rules
        .iter()
        .filter(|r| r.action == "drop" || r.action == "reject")
        .chain(rules.iter().filter(|r| r.action == "accept"))
        .collect();

    for rule in &ordered_rules {
        let protocol_lower = rule.protocol.to_lowercase();
        let action = rule.action.to_lowercase();

        // 1) Input / Output subchain rule
        let target_subchain = match rule.direction.as_str() {
            "inbound" => "chv-input",
            "outbound" => "chv-output",
            _ => continue,
        };

        let mut args: Vec<String> = Vec::new();
        if protocol_lower != "all" {
            args.push("meta".into());
            args.push("l4proto".into());
            args.push(protocol_lower.clone());
        }
        if let Some(ref cidr) = rule.source_cidr {
            args.push("ip".into());
            args.push("saddr".into());
            args.push(cidr.clone());
        }
        if let Some(ref port) = rule.dest_port {
            if protocol_lower == "tcp" || protocol_lower == "udp" {
                args.push(protocol_lower.clone());
                args.push("dport".into());
                args.push(port.clone());
            }
        }
        args.push(action.clone());

        actions.push(NftAction::AddRule {
            chain: target_subchain.into(),
            args: args.clone(),
        });

        // 2) Forward subchain rule
        // Inbound forwarded traffic enters VM via bridge (oifname bridge_name)
        // Outbound forwarded traffic leaves VM via bridge (iifname bridge_name)
        let mut fwd_args: Vec<String> = Vec::new();
        match rule.direction.as_str() {
            "inbound" => {
                fwd_args.push("oifname".into());
                fwd_args.push(bridge_name.into());
            }
            "outbound" => {
                fwd_args.push("iifname".into());
                fwd_args.push(bridge_name.into());
            }
            _ => continue,
        }
        if protocol_lower != "all" {
            fwd_args.push("meta".into());
            fwd_args.push("l4proto".into());
            fwd_args.push(protocol_lower.clone());
        }
        if let Some(ref cidr) = rule.source_cidr {
            fwd_args.push("ip".into());
            fwd_args.push("saddr".into());
            fwd_args.push(cidr.clone());
        }
        if let Some(ref port) = rule.dest_port {
            if protocol_lower == "tcp" || protocol_lower == "udp" {
                fwd_args.push(protocol_lower);
                fwd_args.push("dport".into());
                fwd_args.push(port.clone());
            }
        }
        fwd_args.push(action);

        actions.push(NftAction::AddRule {
            chain: "chv-forward".into(),
            args: fwd_args,
        });
    }

    // Terminal default-drop rules in each CHV subchain to preserve default-deny semantics for CHV traffic
    actions.push(NftAction::AddRule {
        chain: "chv-input".into(),
        args: vec!["drop".into()],
    });
    actions.push(NftAction::AddRule {
        chain: "chv-output".into(),
        args: vec!["drop".into()],
    });
    actions.push(NftAction::AddRule {
        chain: "chv-forward".into(),
        args: vec!["drop".into()],
    });

    Ok(actions)
}

pub async fn apply_firewall_rules(
    table: &str,
    bridge_name: &str,
    policy_json: &[u8],
) -> Result<(), ChvError> {
    let rules: Vec<FirewallRule> = if policy_json.is_empty() {
        Vec::new()
    } else {
        serde_json::from_slice(policy_json).map_err(|e| ChvError::InvalidArgument {
            field: "policy_json".to_string(),
            reason: format!("failed to parse firewall rules: {}", e),
        })?
    };

    let plan = plan_firewall_rules(table, bridge_name, &rules)?;

    for action in plan {
        match action {
            NftAction::AddIdempotent(args) => {
                let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                run_nft_idempotent(&str_args).await?;
            }
            NftAction::FlushChain(chain) => {
                if let Err(e) = run_nft(&["flush", "chain", "inet", table, &chain]).await {
                    tracing::warn!(table, chain = %chain, error = %e, "failed to flush nftables chain");
                }
            }
            NftAction::DeleteByComment { chain, comment } => {
                delete_rules_by_comment(table, &chain, &comment).await?;
            }
            NftAction::AddRule { chain, args } => {
                let mut full_args = vec!["add", "rule", "inet", table, &chain];
                for a in &args {
                    full_args.push(a.as_str());
                }
                run_nft(&full_args).await?;
            }
        }
    }

    info!(
        table = %table,
        bridge = %bridge_name,
        rule_count = rules.len(),
        "firewall rules applied (scoped)"
    );
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
    if let Err(e) = run_nft(&["flush", "chain", "inet", table, "postrouting"]).await {
        tracing::warn!(table, error = %e, "failed to flush postrouting chain");
    }

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

async fn delete_rules_by_comment(table: &str, chain: &str, comment: &str) -> Result<(), ChvError> {
    let out = Command::new("nft")
        .args(["-a", "list", "chain", "inet", table, chain])
        .output()
        .await
        .map_err(|e| ChvError::Io {
            path: "nft".to_string(),
            source: e,
        })?;
    if !out.status.success() {
        return Ok(()); // chain may not exist
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let target = format!("comment \"{}\"", comment);
    for line in stdout.lines() {
        if line.contains(&target) {
            if let Some(idx) = line.rfind(" handle ") {
                let handle = line[idx + 8..].split_whitespace().next().unwrap_or("");
                if !handle.is_empty() {
                    let _ =
                        run_nft(&["delete", "rule", "inet", table, chain, "handle", handle]).await;
                }
            }
        }
    }
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

    #[test]
    fn test_validate_interface_name() {
        assert!(validate_interface_name("br-net1").is_ok());
        assert!(validate_interface_name("chvbr0").is_ok());
        assert!(validate_interface_name("tap-1234abcd").is_ok());
        assert!(validate_interface_name("eth0.100").is_ok());
        assert!(validate_interface_name("br_test").is_ok());

        assert!(validate_interface_name("").is_err());
        assert!(validate_interface_name("toolonginterfacename").is_err());
        assert!(validate_interface_name("-invalid").is_err());
        assert!(validate_interface_name("br 1").is_err());
        assert!(validate_interface_name("br;rm").is_err());
        assert!(validate_interface_name("br/0").is_err());
    }

    #[test]
    fn test_validate_table_name() {
        assert!(validate_table_name("chv-net1").is_ok());
        assert!(validate_table_name("chv-123_456").is_ok());
        assert!(validate_table_name("chv-550e8400-e29b-41d4-a716-446655440000").is_ok());

        assert!(validate_table_name("").is_err());
        assert!(validate_table_name("-chv").is_err());
        assert!(validate_table_name("chv net").is_err());
        assert!(validate_table_name("chv;drop").is_err());
        assert!(validate_table_name(&format!("chv-{}", "a".repeat(252))).is_err());
    }

    #[test]
    fn test_plan_preserves_forward_chain_and_non_firewall_rules() {
        let table = "chv-test";
        let bridge = "br-test";
        let plan = plan_firewall_rules(table, bridge, &[]).expect("plan rules");

        // Forward chain must NEVER be flushed so existing service exposures are retained
        assert!(
            !plan.contains(&NftAction::FlushChain("forward".to_string())),
            "Forward chain must not be flushed"
        );

        // Input and output chains ARE flushed because they are exclusively firewall-owned
        assert!(
            plan.contains(&NftAction::FlushChain("input".to_string())),
            "Input chain must be flushed"
        );
        assert!(
            plan.contains(&NftAction::FlushChain("output".to_string())),
            "Output chain must be flushed"
        );

        // In the forward base chain, the only deletions are targeted by comment for CHV jump rules
        let forward_deletions: Vec<_> = plan
            .iter()
            .filter_map(|a| match a {
                NftAction::DeleteByComment { chain, comment } if chain == "forward" => {
                    Some(comment.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(forward_deletions, vec!["chv-fwd-in", "chv-fwd-out"]);

        // Jump rules added to the forward base chain are commented to match the deletion targets
        let forward_jumps: Vec<_> = plan
            .iter()
            .filter_map(|a| match a {
                NftAction::AddRule { chain, args } if chain == "forward" => Some(args),
                _ => None,
            })
            .collect();
        assert_eq!(forward_jumps.len(), 2);
        assert!(forward_jumps[0].contains(&"\"chv-fwd-in\"".to_string()));
        assert!(forward_jumps[1].contains(&"\"chv-fwd-out\"".to_string()));
    }

    #[test]
    fn test_plan_empty_policy_no_hostwide_drop() {
        let table = "chv-test";
        let bridge = "br-test";
        let plan = plan_firewall_rules(table, bridge, &[]).expect("plan empty policy");

        // Verify base chains have policy accept and NEVER policy drop
        for action in &plan {
            if let NftAction::AddIdempotent(args) = action {
                let text = args.join(" ");
                if text.contains("chain")
                    && (text.contains("input")
                        || text.contains("forward")
                        || text.contains("output"))
                {
                    assert!(
                        !text.contains("policy drop"),
                        "Base chain must never have policy drop: {}",
                        text
                    );
                }
            }
            if let NftAction::AddRule { chain, args } = action {
                if chain == "input" || chain == "forward" || chain == "output" {
                    assert_ne!(
                        args.as_slice(),
                        &["drop"],
                        "Base chain {} must never have an unscoped drop rule",
                        chain
                    );
                }
            }
        }

        // Verify that base chains only jump to subchains scoped to the bridge interface
        let input_jumps: Vec<_> = plan
            .iter()
            .filter(|a| {
                matches!(a, NftAction::AddRule { chain, args } if chain == "input" && args.contains(&"jump".to_string()))
            })
            .collect();
        assert_eq!(input_jumps.len(), 1);
        if let NftAction::AddRule { args, .. } = input_jumps[0] {
            assert_eq!(args, &["iifname", bridge, "jump", "chv-input"]);
        }

        let output_jumps: Vec<_> = plan
            .iter()
            .filter(|a| {
                matches!(a, NftAction::AddRule { chain, args } if chain == "output" && args.contains(&"jump".to_string()))
            })
            .collect();
        assert_eq!(output_jumps.len(), 1);
        if let NftAction::AddRule { args, .. } = output_jumps[0] {
            assert_eq!(args, &["oifname", bridge, "jump", "chv-output"]);
        }

        let forward_jumps: Vec<_> = plan
            .iter()
            .filter(|a| {
                matches!(a, NftAction::AddRule { chain, args } if chain == "forward" && args.contains(&"jump".to_string()))
            })
            .collect();
        assert_eq!(forward_jumps.len(), 2);

        // Verify that CHV subchains terminate with drop, preserving default-deny for CHV traffic
        for subchain in ["chv-input", "chv-forward", "chv-output"] {
            let last_rule = plan
                .iter()
                .filter_map(|a| match a {
                    NftAction::AddRule { chain, args } if chain == subchain => Some(args),
                    _ => None,
                })
                .last()
                .expect("subchain must have rules");
            assert_eq!(
                last_rule,
                &["drop"],
                "Subchain {} must terminate with default drop for CHV traffic",
                subchain
            );
        }
    }

    #[test]
    fn test_plan_rules_with_inbound_and_outbound() {
        let table = "chv-test";
        let bridge = "br-test";
        let rules = vec![
            FirewallRule {
                direction: "inbound".to_string(),
                protocol: "tcp".to_string(),
                source_cidr: Some("10.0.0.0/8".to_string()),
                dest_port: Some("80".to_string()),
                action: "accept".to_string(),
            },
            FirewallRule {
                direction: "inbound".to_string(),
                protocol: "tcp".to_string(),
                source_cidr: Some("10.1.2.3/32".to_string()),
                dest_port: Some("80".to_string()),
                action: "drop".to_string(),
            },
            FirewallRule {
                direction: "outbound".to_string(),
                protocol: "udp".to_string(),
                source_cidr: None,
                dest_port: Some("53".to_string()),
                action: "accept".to_string(),
            },
        ];

        let plan = plan_firewall_rules(table, bridge, &rules).expect("plan rules");

        // Verify drop rule is evaluated before accept rule in chv-input
        let input_rules: Vec<_> = plan
            .iter()
            .filter_map(|a| match a {
                NftAction::AddRule { chain, args } if chain == "chv-input" => Some(args),
                _ => None,
            })
            .collect();

        // [0] ct established, [1] drop rule (10.1.2.3), [2] accept rule (10.0.0.0/8), [3] default drop
        assert_eq!(input_rules.len(), 4);
        assert_eq!(
            input_rules[0],
            &["ct", "state", "established,related", "accept"]
        );
        assert!(input_rules[1].contains(&"drop".to_string()));
        assert!(input_rules[1].contains(&"10.1.2.3/32".to_string()));
        assert!(input_rules[2].contains(&"accept".to_string()));
        assert!(input_rules[2].contains(&"10.0.0.0/8".to_string()));
        assert_eq!(input_rules[3], &["drop"]);

        // Verify forwarding rules are scoped to bridge interface
        let forward_rules: Vec<_> = plan
            .iter()
            .filter_map(|a| match a {
                NftAction::AddRule { chain, args } if chain == "chv-forward" => Some(args),
                _ => None,
            })
            .collect();

        // Inbound forward rules must specify `oifname br-test`
        let fwd_inbound: Vec<_> = forward_rules
            .iter()
            .filter(|r| r.contains(&"10.0.0.0/8".to_string()))
            .collect();
        assert_eq!(fwd_inbound.len(), 1);
        assert_eq!(fwd_inbound[0][0], "oifname");
        assert_eq!(fwd_inbound[0][1], bridge);

        // Outbound forward rules must specify `iifname br-test`
        let fwd_outbound: Vec<_> = forward_rules
            .iter()
            .filter(|r| r.contains(&"53".to_string()))
            .collect();
        assert_eq!(fwd_outbound.len(), 1);
        assert_eq!(fwd_outbound[0][0], "iifname");
        assert_eq!(fwd_outbound[0][1], bridge);
    }

    #[test]
    fn test_plan_rejects_invalid_names() {
        assert!(plan_firewall_rules("chv-invalid;table", "br-valid", &[]).is_err());
        assert!(plan_firewall_rules("chv-valid", "br-too-long-interface-name", &[]).is_err());
        assert!(plan_firewall_rules("chv-valid", "br;bad", &[]).is_err());
    }
}
