use chv_errors::ChvError;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::info;

const RUNTIME_DIR: &str = "/run/chv/nwd";

fn conf_path(network_id: &str) -> PathBuf {
    PathBuf::from(RUNTIME_DIR).join(format!("dnsmasq-{}.conf", network_id))
}

fn pid_path(network_id: &str) -> PathBuf {
    PathBuf::from(RUNTIME_DIR).join(format!("dnsmasq-{}.pid", network_id))
}

fn hosts_path(network_id: &str) -> PathBuf {
    PathBuf::from(RUNTIME_DIR).join(format!("dnsmasq-{}.hosts", network_id))
}

async fn is_dnsmasq_running(network_id: &str) -> bool {
    let pid_file = pid_path(network_id);
    let Ok(pid_str) = tokio::fs::read_to_string(&pid_file).await else {
        return false;
    };
    let Ok(pid) = pid_str.trim().parse::<i32>() else {
        return false;
    };
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

async fn reload_dnsmasq(network_id: &str) {
    let pid_file = pid_path(network_id);
    let Ok(pid_str) = tokio::fs::read_to_string(&pid_file).await else {
        return;
    };
    let _ = Command::new("kill")
        .args(["-HUP", pid_str.trim()])
        .output()
        .await;
}

pub async fn ensure_dhcp_scope(
    network_id: &str,
    cidr: &str,
    range_start: &str,
    range_end: &str,
    dns_servers: &[String],
) -> Result<(), ChvError> {
    if cidr.is_empty() || range_start.is_empty() || range_end.is_empty() {
        return Err(ChvError::InvalidArgument {
            field: "dhcp_scope".to_string(),
            reason: "cidr, range_start, and range_end are required".to_string(),
        });
    }

    // Validate IPs
    if range_start.parse::<std::net::Ipv4Addr>().is_err() {
        return Err(ChvError::InvalidArgument {
            field: "range_start".to_string(),
            reason: format!("invalid IPv4 address: '{}'", range_start),
        });
    }
    if range_end.parse::<std::net::Ipv4Addr>().is_err() {
        return Err(ChvError::InvalidArgument {
            field: "range_end".to_string(),
            reason: format!("invalid IPv4 address: '{}'", range_end),
        });
    }

    let netmask = cidr_to_netmask(cidr)?;

    let conf = conf_path(network_id);
    let hosts = hosts_path(network_id);
    let pid = pid_path(network_id);

    let _ = tokio::fs::create_dir_all(RUNTIME_DIR).await;

    // Read existing config to detect if scope changed
    let existing = tokio::fs::read_to_string(&conf).await.unwrap_or_default();
    let dhcp_range_line = format!("dhcp-range={},{},{},12h", range_start, range_end, netmask);

    if existing.contains(&dhcp_range_line) && is_dnsmasq_running(network_id).await {
        info!(network_id = %network_id, "DHCP scope unchanged, skipping reload");
        return Ok(());
    }

    // Determine bridge name from existing config or use convention
    let bridge_name = existing
        .lines()
        .find(|l| l.starts_with("interface="))
        .and_then(|l| l.strip_prefix("interface="))
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("br-{}", network_id));

    // Derive gateway from CIDR (first usable IP)
    let gateway = derive_gateway(cidr).unwrap_or_default();

    // Use provided DNS servers or default
    let dns_option = if dns_servers.is_empty() {
        "1.1.1.1".to_string()
    } else {
        dns_servers.join(",")
    };

    let config = format!(
        "interface={}\nbind-interfaces\nport=0\ndhcp-range={},{},{},12h\ndhcp-option=3,{}\ndhcp-option=6,{}\ndhcp-hostsfile={}\nexcept-interface=lo\nno-resolv\n",
        bridge_name, range_start, range_end, netmask, gateway, dns_option, hosts.display()
    );

    tokio::fs::write(&conf, &config)
        .await
        .map_err(|e| ChvError::Io {
            path: conf.to_string_lossy().to_string(),
            source: e,
        })?;

    // Ensure hosts file exists
    if tokio::fs::metadata(&hosts).await.is_err() {
        let _ = tokio::fs::write(&hosts, "").await;
    }

    if is_dnsmasq_running(network_id).await {
        // Reload running instance
        reload_dnsmasq(network_id).await;
        info!(network_id = %network_id, "DHCP scope updated, dnsmasq reloaded");
    } else {
        // Start new dnsmasq
        let out = Command::new("dnsmasq")
            .args([
                &format!("--conf-file={}", conf.display()),
                &format!("--pid-file={}", pid.display()),
            ])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "dnsmasq".to_string(),
                source: e,
            })?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(ChvError::NetworkUnavailable {
                resource: "dnsmasq".to_string(),
                reason: format!("dnsmasq start failed: {}", stderr),
            });
        }
        info!(network_id = %network_id, "DHCP scope applied, dnsmasq started");
    }

    Ok(())
}

fn cidr_to_netmask(cidr: &str) -> Result<String, ChvError> {
    let prefix: u8 = cidr
        .split('/')
        .nth(1)
        .and_then(|p| p.parse().ok())
        .ok_or_else(|| ChvError::InvalidArgument {
            field: "cidr".to_string(),
            reason: format!("invalid CIDR notation: '{}'", cidr),
        })?;

    if prefix > 32 {
        return Err(ChvError::InvalidArgument {
            field: "cidr".to_string(),
            reason: format!("prefix {} exceeds 32", prefix),
        });
    }

    let mask: u32 = if prefix == 0 {
        0
    } else {
        !0u32 << (32 - prefix)
    };
    Ok(format!(
        "{}.{}.{}.{}",
        (mask >> 24) & 0xFF,
        (mask >> 16) & 0xFF,
        (mask >> 8) & 0xFF,
        mask & 0xFF,
    ))
}

fn derive_gateway(cidr: &str) -> Option<String> {
    let ip_str = cidr.split('/').next()?;
    let ip: std::net::Ipv4Addr = ip_str.parse().ok()?;
    let octets = ip.octets();
    Some(format!("{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3].wrapping_add(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cidr_to_netmask() {
        assert_eq!(cidr_to_netmask("10.0.0.0/24").unwrap(), "255.255.255.0");
        assert_eq!(cidr_to_netmask("10.0.0.0/16").unwrap(), "255.255.0.0");
        assert_eq!(cidr_to_netmask("10.0.0.0/8").unwrap(), "255.0.0.0");
        assert_eq!(cidr_to_netmask("10.0.0.0/32").unwrap(), "255.255.255.255");
        assert!(cidr_to_netmask("10.0.0.0").is_err());
    }

    #[test]
    fn test_derive_gateway() {
        assert_eq!(derive_gateway("10.0.0.0/24"), Some("10.0.0.1".to_string()));
        assert_eq!(derive_gateway("192.168.1.0/24"), Some("192.168.1.1".to_string()));
    }
}
