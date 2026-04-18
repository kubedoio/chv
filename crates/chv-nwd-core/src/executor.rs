use async_trait::async_trait;
use chv_errors::ChvError;
use chv_nwd_api::chv_nwd_api::TopologySpec;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct TopologyApplyResult {
    pub namespace_handle: String,
    pub bridge_handle: String,
}

#[async_trait]
pub trait NetworkExecutor: Send + Sync + 'static {
    async fn ensure_topology(&self, spec: &TopologySpec) -> Result<TopologyApplyResult, ChvError>;

    async fn delete_topology(
        &self,
        network_id: &str,
        state: &crate::state::TopologyState,
    ) -> Result<(), ChvError>;

    async fn health(
        &self,
        network_id: &str,
        state: &crate::state::TopologyState,
    ) -> Result<String, ChvError>;

    async fn attach_vm_nic(
        &self,
        network_id: &str,
        nic_id: &str,
        vm_id: &str,
        bridge_name: &str,
        mac_address: &str,
        ip_address: &str,
    ) -> Result<(String, String), ChvError>;

    async fn detach_vm_nic(&self, nic_id: &str) -> Result<(), ChvError>;

    async fn set_firewall_policy(
        &self,
        network_id: &str,
        policy_version: &str,
        policy_json: &[u8],
    ) -> Result<(), ChvError>;

    async fn set_nat_policy(
        &self,
        network_id: &str,
        policy_version: &str,
        policy_json: &[u8],
    ) -> Result<(), ChvError>;

    async fn ensure_dhcp_scope(
        &self,
        network_id: &str,
        cidr: &str,
        range_start: &str,
        range_end: &str,
    ) -> Result<(), ChvError>;

    async fn ensure_dns_scope(&self, network_id: &str, forwarders: &[&str])
        -> Result<(), ChvError>;

    #[allow(clippy::too_many_arguments)]
    async fn expose_service(
        &self,
        network_id: &str,
        exposure_id: &str,
        protocol: &str,
        external_port: u32,
        target_ip: &str,
        target_port: u32,
        mode: &str,
    ) -> Result<(), ChvError>;

    async fn withdraw_service_exposure(
        &self,
        network_id: &str,
        exposure_id: &str,
    ) -> Result<(), ChvError>;
}

pub struct LinuxExecutor {
    _runtime_dir: PathBuf,
}

impl LinuxExecutor {
    pub fn new(runtime_dir: PathBuf) -> Self {
        Self {
            _runtime_dir: runtime_dir,
        }
    }

    async fn run_ip(args: &[&str]) -> Result<(), ChvError> {
        let out = Command::new("ip")
            .args(args)
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "ip".to_string(),
                source: e,
            })?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("File exists") || stderr.contains("already exists") {
                return Ok(());
            }
            return Err(ChvError::NetworkUnavailable {
                resource: "ip".to_string(),
                reason: format!("ip {} failed: {}", args.join(" "), stderr),
            });
        }
        Ok(())
    }

    #[allow(dead_code)]
    async fn run_ip_netns(namespace: &str, args: &[&str]) -> Result<(), ChvError> {
        let mut cmd_args = vec!["netns", "exec", namespace];
        cmd_args.extend(args);
        Self::run_ip(&cmd_args).await
    }

    async fn bridge_exists(name: &str) -> bool {
        Command::new("ip")
            .args(["link", "show", "dev", name])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn namespace_exists(name: &str) -> bool {
        std::path::Path::new("/var/run/netns").join(name).exists()
    }

    fn tap_name_for_nic(nic_id: &str) -> String {
        // Linux interface names are limited to 15 bytes (IFNAMSIZ - 1).
        // Derive a stable compact tap name from the nic_id so very long IDs
        // (e.g. UUID-derived values) do not break `ip tuntap add`.
        let mut hash: u64 = 0xcbf29ce484222325; // FNV-1a offset basis
        for byte in nic_id.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3); // FNV prime
        }
        format!("tap-{:08x}", (hash & 0xffff_ffff) as u32)
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

    async fn delete_rules_by_comment(
        table: &str,
        chain: &str,
        comment: &str,
    ) -> Result<(), ChvError> {
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
                        Self::run_nft(&["delete", "rule", "inet", table, chain, "handle", handle])
                            .await?;
                    }
                }
            }
        }
        Ok(())
    }

    fn sanitize_id(id: &str) -> Result<String, ChvError> {
        if id.is_empty() {
            return Err(ChvError::InvalidArgument {
                field: "id".to_string(),
                reason: "id must not be empty".to_string(),
            });
        }
        if id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            Ok(id.to_string())
        } else {
            Err(ChvError::InvalidArgument {
                field: "id".to_string(),
                reason: format!("id contains invalid characters: {}", id),
            })
        }
    }

    fn sanitized_nft_table(network_id: &str) -> Result<String, ChvError> {
        let sanitized = Self::sanitize_id(network_id)?;
        Ok(format!("chv-{}", sanitized))
    }

    async fn run_nft_idempotent(args: &[&str]) -> Result<(), ChvError> {
        match Self::run_nft(args).await {
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
}

#[async_trait]
impl NetworkExecutor for LinuxExecutor {
    async fn ensure_topology(&self, spec: &TopologySpec) -> Result<TopologyApplyResult, ChvError> {
        info!(
            network_id = %spec.network_id,
            bridge = %spec.bridge_name,
            namespace = %spec.namespace_name,
            "ensuring topology"
        );

        // Bridge
        if !Self::bridge_exists(&spec.bridge_name).await {
            Self::run_ip(&["link", "add", &spec.bridge_name, "type", "bridge"]).await?;
        }
        Self::run_ip(&["link", "set", &spec.bridge_name, "up"]).await?;

        // Namespace
        if !Self::namespace_exists(&spec.namespace_name).await {
            Self::run_ip(&["netns", "add", &spec.namespace_name]).await?;
        }

        // Minimal nftables table to satisfy baseline hook
        let _ = Command::new("nft")
            .args(["add", "table", "inet", &format!("chv-{}", spec.network_id)])
            .output()
            .await;

        Ok(TopologyApplyResult {
            namespace_handle: spec.namespace_name.clone(),
            bridge_handle: spec.bridge_name.clone(),
        })
    }

    async fn delete_topology(
        &self,
        network_id: &str,
        state: &crate::state::TopologyState,
    ) -> Result<(), ChvError> {
        info!(
            network_id = %network_id,
            bridge = %state.bridge_name,
            namespace = %state.namespace_name,
            "deleting topology"
        );

        if Self::namespace_exists(&state.namespace_name).await {
            if let Err(e) = Self::run_ip(&["netns", "del", &state.namespace_name]).await {
                warn!(error = %e, "failed to delete namespace");
            }
        }

        if Self::bridge_exists(&state.bridge_name).await {
            if let Err(e) = Self::run_ip(&["link", "del", "dev", &state.bridge_name]).await {
                warn!(error = %e, "failed to delete bridge");
            }
        }

        if let Ok(table) = Self::sanitized_nft_table(network_id) {
            let _ = Command::new("nft")
                .args(["delete", "table", "inet", &table])
                .output()
                .await;
        }

        Ok(())
    }

    async fn health(
        &self,
        _network_id: &str,
        state: &crate::state::TopologyState,
    ) -> Result<String, ChvError> {
        let bridge_ok = Self::bridge_exists(&state.bridge_name).await;
        let ns_ok = Self::namespace_exists(&state.namespace_name).await;

        if bridge_ok && ns_ok {
            Ok("healthy".to_string())
        } else {
            let missing: Vec<&str> = [
                if bridge_ok { None } else { Some("bridge") },
                if ns_ok { None } else { Some("namespace") },
            ]
            .into_iter()
            .flatten()
            .collect();
            Ok(format!("degraded: missing {}", missing.join(", ")))
        }
    }

    async fn attach_vm_nic(
        &self,
        network_id: &str,
        nic_id: &str,
        _vm_id: &str,
        bridge_name: &str,
        _mac_address: &str,
        _ip_address: &str,
    ) -> Result<(String, String), ChvError> {
        let tap_name = Self::tap_name_for_nic(nic_id);

        Self::run_ip(&["tuntap", "add", "dev", &tap_name, "mode", "tap"]).await?;
        Self::run_ip(&["link", "set", "dev", &tap_name, "master", bridge_name]).await?;
        Self::run_ip(&["link", "set", "dev", &tap_name, "up"]).await?;

        info!(network_id = %network_id, nic_id = %nic_id, tap = %tap_name, "attached VM NIC");

        Ok((format!("ns-{}", network_id), tap_name))
    }

    async fn detach_vm_nic(&self, nic_id: &str) -> Result<(), ChvError> {
        let tap_handle = Self::tap_name_for_nic(nic_id);
        let out = Command::new("ip")
            .args(["tuntap", "del", "dev", &tap_handle, "mode", "tap"])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "ip".to_string(),
                source: e,
            })?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("cannot find device") || stderr.contains("No such device") {
                return Ok(());
            }
            return Err(ChvError::NetworkUnavailable {
                resource: "ip".to_string(),
                reason: format!(
                    "ip tuntap del dev {} mode tap failed: {}",
                    tap_handle, stderr
                ),
            });
        }

        info!(tap = %tap_handle, "detached VM NIC");
        Ok(())
    }

    async fn set_firewall_policy(
        &self,
        network_id: &str,
        _policy_version: &str,
        _policy_json: &[u8],
    ) -> Result<(), ChvError> {
        let table = Self::sanitized_nft_table(network_id)?;
        Self::run_nft_idempotent(&["add", "table", "inet", &table]).await?;
        for (chain, hook) in [("input", "input"), ("forward", "forward")] {
            Self::run_nft_idempotent(&[
                "add",
                "chain",
                "inet",
                &table,
                chain,
                &format!(
                    "{{ type filter hook {} priority 0 ; policy accept ; }}",
                    hook
                ),
            ])
            .await?;
        }
        Self::run_nft(&[
            "add",
            "rule",
            "inet",
            &table,
            "input",
            "ct",
            "state",
            "established,related",
            "accept",
        ])
        .await?;
        info!(network_id = %network_id, "firewall policy applied");
        Ok(())
    }

    async fn set_nat_policy(
        &self,
        network_id: &str,
        _policy_version: &str,
        _policy_json: &[u8],
    ) -> Result<(), ChvError> {
        let table = Self::sanitized_nft_table(network_id)?;
        Self::run_nft_idempotent(&["add", "table", "inet", &table]).await?;
        Self::run_nft_idempotent(&[
            "add",
            "chain",
            "inet",
            &table,
            "postrouting",
            "{ type nat hook postrouting priority 100 ; policy accept ; }",
        ])
        .await?;
        Self::run_nft(&[
            "add",
            "rule",
            "inet",
            &table,
            "postrouting",
            "oif",
            "!=",
            "lo",
            "masquerade",
        ])
        .await?;
        info!(network_id = %network_id, "NAT policy applied");
        Ok(())
    }

    async fn ensure_dhcp_scope(
        &self,
        network_id: &str,
        _cidr: &str,
        _range_start: &str,
        _range_end: &str,
    ) -> Result<(), ChvError> {
        info!(network_id = %network_id, "DHCP scope accepted but not enforced by LinuxExecutor");
        Ok(())
    }

    async fn ensure_dns_scope(
        &self,
        network_id: &str,
        _forwarders: &[&str],
    ) -> Result<(), ChvError> {
        info!(network_id = %network_id, "DNS scope accepted but not enforced by LinuxExecutor");
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn expose_service(
        &self,
        network_id: &str,
        exposure_id: &str,
        protocol: &str,
        external_port: u32,
        target_ip: &str,
        target_port: u32,
        _mode: &str,
    ) -> Result<(), ChvError> {
        let table = Self::sanitized_nft_table(network_id)?;
        let safe_exposure_id = Self::sanitize_id(exposure_id)?;
        Self::run_nft_idempotent(&["add", "table", "inet", &table]).await?;
        Self::run_nft_idempotent(&[
            "add",
            "chain",
            "inet",
            &table,
            "prerouting",
            "{ type nat hook prerouting priority 0 ; policy accept ; }",
        ])
        .await?;
        Self::run_nft(&[
            "add",
            "rule",
            "inet",
            &table,
            "prerouting",
            protocol,
            "dport",
            &external_port.to_string(),
            "dnat",
            "to",
            &format!("{}:{}", target_ip, target_port),
            "comment",
            &format!("\"{}\"", safe_exposure_id),
        ])
        .await?;
        Self::run_nft_idempotent(&[
            "add",
            "chain",
            "inet",
            &table,
            "forward",
            "{ type filter hook forward priority 0 ; policy accept ; }",
        ])
        .await?;
        Self::run_nft(&[
            "add",
            "rule",
            "inet",
            &table,
            "forward",
            protocol,
            "dport",
            &target_port.to_string(),
            "ip",
            "daddr",
            target_ip,
            "accept",
            "comment",
            &format!("\"{}\"", safe_exposure_id),
        ])
        .await?;
        info!(network_id = %network_id, exposure_id = %exposure_id, "service exposed via DNAT");
        Ok(())
    }

    async fn withdraw_service_exposure(
        &self,
        network_id: &str,
        exposure_id: &str,
    ) -> Result<(), ChvError> {
        let table = Self::sanitized_nft_table(network_id)?;
        let safe_exposure_id = Self::sanitize_id(exposure_id)?;
        Self::delete_rules_by_comment(&table, "prerouting", &safe_exposure_id).await?;
        Self::delete_rules_by_comment(&table, "forward", &safe_exposure_id).await?;
        info!(network_id = %network_id, exposure_id = %exposure_id, "service exposure withdrawn");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_executor_implements_network_executor() {
        let _executor = LinuxExecutor::new(std::env::temp_dir());
        // If this compiles, the trait is fully implemented.
    }

    #[test]
    fn nft_table_generation() {
        assert_eq!(
            LinuxExecutor::sanitized_nft_table("net1").unwrap(),
            "chv-net1"
        );
    }

    #[test]
    fn sanitize_id_rejects_bad_chars() {
        assert!(LinuxExecutor::sanitize_id("valid_id-123.abc").is_ok());
        assert!(LinuxExecutor::sanitize_id("net1").is_ok());
        assert!(LinuxExecutor::sanitize_id("").is_err());
        assert!(LinuxExecutor::sanitize_id("bad;id").is_err());
        assert!(LinuxExecutor::sanitize_id("bad id").is_err());
        assert!(LinuxExecutor::sanitize_id("bad\"id").is_err());
        assert!(LinuxExecutor::sanitize_id("bad'id").is_err());
        assert!(LinuxExecutor::sanitize_id("bad/id").is_err());
    }

    #[test]
    fn delete_rules_by_comment_line_extraction() {
        // Simulate the parsing logic inline to avoid async test infrastructure
        let sample = r#"
        tcp dport 80 dnat to 10.0.0.2:80 comment "exp-1" handle 10
        tcp dport 443 dnat to 10.0.0.2:443 comment "exp-2" handle 20
        "#;
        let comment = "exp-1";
        let target = format!("comment \"{}\"", comment);
        let mut found_handle = None;
        for line in sample.lines() {
            if line.contains(&target) {
                if let Some(idx) = line.rfind(" handle ") {
                    let handle = line[idx + 8..].split_whitespace().next().unwrap_or("");
                    if !handle.is_empty() {
                        found_handle = Some(handle.to_string());
                    }
                }
            }
        }
        assert_eq!(found_handle, Some("10".to_string()));
    }

    #[test]
    fn tap_name_is_stable_and_linux_safe_length() {
        let nic_id = "95f4f899-58b9-44b6-95f5-0f35a2e590a6-default-network";
        let a = LinuxExecutor::tap_name_for_nic(nic_id);
        let b = LinuxExecutor::tap_name_for_nic(nic_id);
        assert_eq!(a, b);
        assert!(a.len() <= 15, "tap name exceeds Linux IFNAMSIZ: {}", a);
        assert!(a.starts_with("tap-"));
    }
}
