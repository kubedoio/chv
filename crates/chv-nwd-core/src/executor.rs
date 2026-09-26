use async_trait::async_trait;
use chv_errors::ChvError;
use chv_nwd_api::chv_nwd_api::{OverlayType, TopologySpec};
use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{info, warn};

// Metric names for network daemon operations.
const NWD_FDB_ERRORS_TOTAL: &str = "chv_nwd_fdb_errors_total";
const NWD_NFT_ERRORS_TOTAL: &str = "chv_nwd_nft_errors_total";
const NWD_DHCP_ERRORS_TOTAL: &str = "chv_nwd_dhcp_errors_total";

#[derive(Debug, Clone)]
pub struct TopologyApplyResult {
    pub namespace_handle: String,
    pub bridge_handle: String,
}

#[derive(Debug, Clone)]
pub struct OverlayStatusInfo {
    pub vxlan_interface_up: bool,
    pub fdb_entry_count: u32,
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

    async fn detach_vm_nic(
        &self,
        nic_id: &str,
        ownership: chv_common::AttachmentOwnership,
    ) -> Result<(), ChvError>;

    async fn set_firewall_policy(
        &self,
        network_id: &str,
        policy_version: &str,
        policy_json: &[u8],
        bridge_name: &str,
    ) -> Result<(), ChvError>;

    async fn set_nat_policy(
        &self,
        network_id: &str,
        policy_version: &str,
        policy_json: &[u8],
        bridge_name: &str,
    ) -> Result<(), ChvError>;

    async fn ensure_dhcp_scope(
        &self,
        network_id: &str,
        cidr: &str,
        range_start: &str,
        range_end: &str,
        dns_servers: &[String],
    ) -> Result<(), ChvError>;

    async fn ensure_dns_scope(
        &self,
        network_id: &str,
        forwarders: &[&str],
        static_records: &std::collections::HashMap<String, String>,
    ) -> Result<(), ChvError>;

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

    // --- VXLAN overlay methods ---

    async fn create_vxlan_interface(
        &self,
        namespace: &str,
        bridge_name: &str,
        vni: u32,
        vtep_ip: &str,
        vtep_port: u32,
    ) -> Result<(), ChvError>;

    async fn delete_vxlan_interface(&self, namespace: &str, vni: u32) -> Result<(), ChvError>;

    async fn add_fdb_entry(
        &self,
        namespace: &str,
        vni: u32,
        mac_address: &str,
        vtep_ip: &str,
    ) -> Result<(), ChvError>;

    async fn delete_fdb_entry(
        &self,
        namespace: &str,
        vni: u32,
        mac_address: &str,
        vtep_ip: &str,
    ) -> Result<(), ChvError>;

    async fn replace_fdb_entry(
        &self,
        namespace: &str,
        vni: u32,
        mac_address: &str,
        new_vtep_ip: &str,
    ) -> Result<(), ChvError>;

    async fn send_gratuitous_arp(
        &self,
        namespace: &str,
        bridge_name: &str,
        vm_ip: &str,
    ) -> Result<(), ChvError>;

    async fn set_arp_suppression(
        &self,
        namespace: &str,
        vni: u32,
        enabled: bool,
    ) -> Result<(), ChvError>;

    async fn get_overlay_status(
        &self,
        namespace: &str,
        vni: u32,
    ) -> Result<OverlayStatusInfo, ChvError>;
}

/// A service exposure tracked by the executor so the DNAT forward-accept rule
/// can be re-asserted after every firewall apply (which rebuilds the `forward`
/// base chain and would otherwise silently drop exposed flows into default-deny).
#[derive(Clone)]
struct ExposureSpec {
    safe_exposure_id: String,
    protocol: String,
    external_port: u32,
    target_ip: String,
    target_port: u32,
}

pub struct LinuxExecutor {
    _runtime_dir: PathBuf,
    vtep_ip: Option<String>,
    /// Serializes all nft table mutations for this executor (firewall/NAT
    /// apply, service exposure, topology create/delete). The filter and NAT
    /// paths flush+rebuild chains on the per-network table, so concurrent
    /// writers could interleave and leave the table without its terminal
    /// default-deny rules (fail-open for CHV guests). A single writer per
    /// executor closes that race (#227).
    ///
    /// Note: `ensure_topology`/`delete_topology` hold the lock across slow
    /// topology work (dnsmasq spawn, VXLAN/FDB teardown), so a firewall/NAT
    /// apply can briefly queue behind topology operations. This is acceptable
    /// for a control-plane daemon and avoids a finer-grained per-network lock.
    nft_lock: Arc<Mutex<()>>,
    /// Per-network service exposures (keyed by network_id) for re-assertion
    /// after firewall applies.
    exposures: Arc<DashMap<String, Vec<ExposureSpec>>>,
}

impl LinuxExecutor {
    pub fn new(runtime_dir: PathBuf) -> Self {
        Self {
            _runtime_dir: runtime_dir,
            vtep_ip: None,
            nft_lock: Arc::new(Mutex::new(())),
            exposures: Arc::new(DashMap::new()),
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

    async fn run_ip_netns(namespace: &str, args: &[&str]) -> Result<(), ChvError> {
        let mut full_args = vec!["netns", "exec", namespace, "ip"];
        full_args.extend_from_slice(args);
        Self::run_ip(&full_args).await
    }

    async fn run_bridge_netns(namespace: &str, args: &[&str]) -> Result<(), ChvError> {
        let out = Command::new("ip")
            .args(["netns", "exec", namespace, "bridge"])
            .args(args)
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "bridge".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("File exists") || stderr.contains("already exists") {
                return Ok(());
            }
            return Err(ChvError::NetworkUnavailable {
                resource: "bridge".to_string(),
                reason: format!("bridge {} failed: {}", args.join(" "), stderr),
            });
        }
        Ok(())
    }

    async fn run_cmd_netns_output(
        namespace: &str,
        cmd: &str,
        args: &[&str],
    ) -> Result<std::process::Output, ChvError> {
        Command::new("ip")
            .args(["netns", "exec", namespace, cmd])
            .args(args)
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: cmd.to_string(),
                source: e,
            })
    }

    fn vxlan_interface_name(vni: u32) -> String {
        format!("vxlan{}", vni)
    }

    /// Detect the correct inner MTU for VXLAN tunnels.
    /// VXLAN overhead is 50 bytes (14 outer Ethernet + 20 IP + 8 UDP + 8 VXLAN).
    /// Reads the default route interface MTU and subtracts overhead.
    async fn detect_inner_mtu() -> u32 {
        const VXLAN_OVERHEAD: u32 = 50;
        const DEFAULT_MTU: u32 = 1450;

        let output = match Command::new("ip")
            .args(["route", "show", "default"])
            .output()
            .await
        {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => return DEFAULT_MTU,
        };

        // Parse "default via X.X.X.X dev eth0" to get the device name
        let dev = output.split_whitespace().skip_while(|w| *w != "dev").nth(1);

        let dev = match dev {
            Some(d) => d.to_string(),
            None => return DEFAULT_MTU,
        };

        // Get the outer interface MTU
        let mtu_output = match Command::new("ip")
            .args(["link", "show", "dev", &dev])
            .output()
            .await
        {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => return DEFAULT_MTU,
        };

        // Parse "mtu NNNN" from output
        let outer_mtu = mtu_output
            .split_whitespace()
            .skip_while(|w| *w != "mtu")
            .nth(1)
            .and_then(|m| m.parse::<u32>().ok())
            .unwrap_or(1500);

        outer_mtu.saturating_sub(VXLAN_OVERHEAD)
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

    /// Extract the device name from one `ip link show master <bridge>` output line.
    ///
    /// Real-world line shapes (verified on iproute2 6.x):
    ///   355: vA@vB: <BROADCAST,MULTICAST,M-DOWN> ... master brsmp ...
    ///   388: tap-12ab: <BROADCAST,...> ... master brsmp ...
    ///
    /// Field 0 is the ifindex (`355:`), field 1 is `NAME@PEER:` — the actual
    /// interface name with an optional `@peer` suffix and a trailing colon.
    /// Parsing field 0 (the ifindex) instead would silently exclude every real
    /// member from the CHV-owned guard set (fail-open under br_netfilter).
    fn parse_ip_link_master_line(line: &str) -> Option<String> {
        let name = line.split_whitespace().nth(1)?;
        let name = name.split(':').next()?; // drop trailing ':'
        let name = name.split('@').next()?; // drop veth @peer suffix
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }

    /// Resolve the authoritative CHV-owned interface set for a topology bridge:
    /// the bridge itself plus any enslaved member devices (VM TAPs/veths).
    ///
    /// Fails closed when the topology-owned bridge does not exist, so CHV never
    /// guesses a host interface to scope firewall/NAT policy against (#227).
    async fn owned_ifaces_for_bridge(bridge_name: &str) -> Result<Vec<String>, ChvError> {
        if !Self::bridge_exists(bridge_name).await {
            return Err(ChvError::NotFound {
                resource: "bridge".to_string(),
                id: bridge_name.to_string(),
            });
        }
        let out = Command::new("ip")
            .args(["link", "show", "master", bridge_name])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "ip".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            return Err(ChvError::NetworkUnavailable {
                resource: "ip".to_string(),
                reason: format!("ip link show master {} failed", bridge_name),
            });
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        let mut owned: Vec<String> = vec![bridge_name.to_string()];
        for line in stdout.lines() {
            if let Some(dev) = Self::parse_ip_link_master_line(line) {
                if !dev.is_empty() && dev != bridge_name {
                    owned.push(dev);
                }
            }
        }
        owned.sort();
        owned.dedup();
        Ok(owned)
    }

    fn tap_name_for_nic(nic_id: &str) -> String {
        // Linux interface names are limited to 15 bytes (IFNAMSIZ - 1).
        // Derive a stable compact tap name from the nic_id so very long IDs
        // (e.g. UUID-derived values) do not break `ip tuntap add`.
        let hash = chv_common::fnv1a_hash(nic_id);
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

    async fn run_nft_quiet(args: &[&str]) -> Result<(), ()> {
        match Command::new("nft").args(args).output().await {
            Ok(output) if !output.status.success() => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::debug!(args = ?args, stderr = %stderr, "nft command failed (non-fatal)");
                Ok(())
            }
            Err(e) => {
                tracing::debug!(args = ?args, error = %e, "nft command execution failed (non-fatal)");
                Ok(())
            }
            Ok(_) => Ok(()),
        }
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

    fn derive_dhcp_range(cidr: &str) -> Result<(String, String, String), ChvError> {
        let (ip, prefix_str) = cidr
            .split_once('/')
            .ok_or_else(|| ChvError::InvalidArgument {
                field: "cidr".to_string(),
                reason: format!("invalid CIDR: {}", cidr),
            })?;
        let prefix: u8 = prefix_str.parse().map_err(|_| ChvError::InvalidArgument {
            field: "cidr".to_string(),
            reason: format!("invalid prefix in CIDR: {}", cidr),
        })?;

        if prefix == 0 || prefix > 30 {
            return Err(ChvError::InvalidArgument {
                field: "cidr".to_string(),
                reason: format!(
                    "prefix length /{} is not suitable for DHCP (must be 1-30)",
                    prefix
                ),
            });
        }

        let octets: Vec<&str> = ip.split('.').collect();
        if octets.len() != 4 {
            return Err(ChvError::InvalidArgument {
                field: "cidr".to_string(),
                reason: format!("invalid IP in CIDR: {}", cidr),
            });
        }

        let o0: u8 = octets[0].parse().map_err(|_| ChvError::InvalidArgument {
            field: "cidr".to_string(),
            reason: format!("invalid octet in IP: {}", cidr),
        })?;
        let o1: u8 = octets[1].parse().map_err(|_| ChvError::InvalidArgument {
            field: "cidr".to_string(),
            reason: format!("invalid octet in IP: {}", cidr),
        })?;
        let o2: u8 = octets[2].parse().map_err(|_| ChvError::InvalidArgument {
            field: "cidr".to_string(),
            reason: format!("invalid octet in IP: {}", cidr),
        })?;
        let o3: u8 = octets[3].parse().map_err(|_| ChvError::InvalidArgument {
            field: "cidr".to_string(),
            reason: format!("invalid octet in IP: {}", cidr),
        })?;

        let ip_u32 = u32::from_be_bytes([o0, o1, o2, o3]);
        let mask: u32 = !0u32 << (32 - prefix);
        let network = ip_u32 & mask;
        let broadcast = network | !mask;

        // Compute netmask string from prefix
        let netmask_bytes = mask.to_be_bytes();
        let netmask = format!(
            "{}.{}.{}.{}",
            netmask_bytes[0], netmask_bytes[1], netmask_bytes[2], netmask_bytes[3]
        );

        // DHCP range: skip the first few addresses (network + gateway) and last few (broadcast).
        // For large subnets (>100 hosts), use offset of 50 from each end.
        // For small subnets, start at network+2 (skip network and gateway) and end at broadcast-1.
        let host_count = broadcast - network;
        let offset_start = if host_count > 100 { 50 } else { 2 };
        let offset_end = if host_count > 100 { 50 } else { 1 };

        let range_start_u32 = network + offset_start;
        let range_end_u32 = broadcast - offset_end;

        if range_start_u32 >= range_end_u32 {
            return Err(ChvError::InvalidArgument {
                field: "cidr".to_string(),
                reason: format!(
                    "prefix length /{} results in too few addresses for a DHCP range",
                    prefix
                ),
            });
        }

        let start_bytes = range_start_u32.to_be_bytes();
        let end_bytes = range_end_u32.to_be_bytes();

        let range_start = format!(
            "{}.{}.{}.{}",
            start_bytes[0], start_bytes[1], start_bytes[2], start_bytes[3]
        );
        let range_end = format!(
            "{}.{}.{}.{}",
            end_bytes[0], end_bytes[1], end_bytes[2], end_bytes[3]
        );

        Ok((range_start, range_end, netmask))
    }

    async fn is_dnsmasq_running(pid_path: &std::path::Path) -> bool {
        let Ok(pid_str) = tokio::fs::read_to_string(pid_path).await else {
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

    async fn start_dnsmasq(
        network_id: &str,
        bridge_name: &str,
        cidr: &str,
        gateway_ip: &str,
    ) -> Result<(), ChvError> {
        let runtime_dir = PathBuf::from("/run/chv/nwd");
        let _ = tokio::fs::create_dir_all(&runtime_dir).await;

        let conf_path = runtime_dir.join(format!("dnsmasq-{}.conf", network_id));
        let hosts_path = runtime_dir.join(format!("dnsmasq-{}.hosts", network_id));
        let pid_path = runtime_dir.join(format!("dnsmasq-{}.pid", network_id));

        if Self::is_dnsmasq_running(&pid_path).await {
            return Ok(());
        }

        // Create empty hostsfile if not exists
        let _ = tokio::fs::write(&hosts_path, "").await;

        let (range_start, range_end, netmask) = Self::derive_dhcp_range(cidr)?;

        let config = format!(
            "interface={}\nbind-interfaces\nport=0\ndhcp-range={},{},{},12h\ndhcp-option=3,{}\ndhcp-option=6,1.1.1.1\ndhcp-hostsfile={}\nexcept-interface=lo\nno-resolv\n",
            bridge_name,
            range_start,
            range_end,
            netmask,
            gateway_ip,
            hosts_path.display()
        );
        tokio::fs::write(&conf_path, config)
            .await
            .map_err(|e| ChvError::Io {
                path: conf_path.to_string_lossy().to_string(),
                source: e,
            })?;

        let dnsmasq_args = Self::dnsmasq_args(&conf_path, &pid_path);
        let out = Command::new("dnsmasq")
            .args(dnsmasq_args)
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
                reason: format!("dnsmasq failed: {}", stderr),
            });
        }

        Ok(())
    }

    fn dnsmasq_args(conf_path: &std::path::Path, pid_path: &std::path::Path) -> [String; 2] {
        [
            format!("--conf-file={}", conf_path.display()),
            format!("--pid-file={}", pid_path.display()),
        ]
    }

    async fn signal_by_pid_file(pid_path: &std::path::Path, signal: &str) {
        let Ok(pid_str) = tokio::fs::read_to_string(pid_path).await else {
            return;
        };
        let _ = Command::new("kill")
            .args([signal, pid_str.trim()])
            .output()
            .await;
    }

    async fn stop_dnsmasq(network_id: &str) {
        let runtime_dir = PathBuf::from("/run/chv/nwd");
        let pid_path = runtime_dir.join(format!("dnsmasq-{}.pid", network_id));
        let conf_path = runtime_dir.join(format!("dnsmasq-{}.conf", network_id));
        let hosts_path = runtime_dir.join(format!("dnsmasq-{}.hosts", network_id));

        Self::signal_by_pid_file(&pid_path, "-TERM").await;

        let _ = tokio::fs::remove_file(&pid_path).await;
        let _ = tokio::fs::remove_file(&conf_path).await;
        let _ = tokio::fs::remove_file(&hosts_path).await;
    }

    async fn add_dhcp_host(network_id: &str, mac_address: &str, ip_address: &str) {
        let hosts_path = format!("/run/chv/nwd/dnsmasq-{}.hosts", network_id);
        let pid_path = std::path::PathBuf::from(format!("/run/chv/nwd/dnsmasq-{}.pid", network_id));

        let content = tokio::fs::read_to_string(&hosts_path)
            .await
            .unwrap_or_default();
        let entry = format!("{},{}\n", mac_address, ip_address);

        if !content.contains(mac_address) {
            if let Ok(mut file) = tokio::fs::OpenOptions::new()
                .append(true)
                .open(&hosts_path)
                .await
            {
                let _ = file.write_all(entry.as_bytes()).await;
            }
        }

        Self::signal_by_pid_file(&pid_path, "-HUP").await;
    }

    /// Install (idempotently replace) the DNAT rules for one service exposure.
    ///
    /// Deletes any prior rules carrying this exposure's comment marker, then
    /// re-adds the prerouting DNAT and the forward accept. The forward accept
    /// must be evaluated BEFORE the CHV guarded dispatch jumps (otherwise the
    /// flow enters `chv-policy-fwd` default-deny and is dropped), so it is
    /// INSERTED at the head of the forward chain.
    async fn install_exposure_rules(
        &self,
        network_id: &str,
        safe_exposure_id: &str,
        protocol: &str,
        external_port: u32,
        target_ip: &str,
        target_port: u32,
    ) -> Result<(), ChvError> {
        let table = Self::sanitized_nft_table(network_id)?;
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
        Self::run_nft_idempotent(&[
            "add",
            "chain",
            "inet",
            &table,
            "forward",
            "{ type filter hook forward priority filter ; policy accept ; }",
        ])
        .await?;

        // Idempotent replace: drop any prior rules carrying this marker.
        Self::delete_rules_by_comment(&table, "prerouting", safe_exposure_id).await?;
        Self::delete_rules_by_comment(&table, "forward", safe_exposure_id).await?;

        // In an `inet` (dual-stack) table `dnat to` is ambiguous; nft requires
        // `dnat ip to` / `dnat ip6 to` and bracketed IPv6-address:port.
        let (nft_family, dnat_target) = match target_ip.parse::<std::net::IpAddr>() {
            Ok(std::net::IpAddr::V6(_)) => (
                "ip6".to_string(),
                format!("[{}]:{}", target_ip, target_port),
            ),
            _ => ("ip".to_string(), format!("{}:{}", target_ip, target_port)),
        };
        Self::run_nft(&[
            "add",
            "rule",
            "inet",
            &table,
            "prerouting",
            // Never DNAT loopback. A full bind-interface source guard still
            // requires a declared uplink in the exposure API (tracked); the
            // filter hooks use the CHV-owned interface guards from the firewall
            // path, and any unrelated host port collision on the same host IP is
            // a documented residual limitation of the exposure feature.
            "iifname",
            "!=",
            "lo",
            protocol,
            "dport",
            &external_port.to_string(),
            "dnat",
            &nft_family,
            "to",
            &dnat_target,
            "comment",
            &format!("\"{}\"", safe_exposure_id),
        ])
        .await?;
        Self::run_nft(&[
            "insert",
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
        Ok(())
    }

    /// Re-assert all stored service exposures after a firewall apply that
    /// rebuilt the `forward` base chain (exposure forward-accept rules were
    /// destroyed by that rebuild). Idempotent per exposure.
    async fn reassert_exposures(&self, network_id: &str) -> Result<(), ChvError> {
        let recs: Vec<ExposureSpec> = self
            .exposures
            .get(network_id)
            .map(|e| e.iter().cloned().collect())
            .unwrap_or_default();
        for rec in &recs {
            self.install_exposure_rules(
                network_id,
                &rec.safe_exposure_id,
                &rec.protocol,
                rec.external_port,
                &rec.target_ip,
                rec.target_port,
            )
            .await?;
        }
        Ok(())
    }
}

#[async_trait]
impl NetworkExecutor for LinuxExecutor {
    async fn ensure_topology(&self, spec: &TopologySpec) -> Result<TopologyApplyResult, ChvError> {
        let _guard = self.nft_lock.lock().await;
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

        // Assign gateway IP to bridge
        if !spec.gateway_ip.is_empty() && !spec.subnet_cidr.is_empty() {
            let prefix = spec.subnet_cidr.split('/').nth(1).unwrap_or("24");
            if let Err(e) = Self::run_ip(&[
                "addr",
                "add",
                &format!("{}/{}", spec.gateway_ip, prefix),
                "dev",
                &spec.bridge_name,
            ])
            .await
            {
                let reason = e.to_string();
                if !reason.contains("File exists")
                    && !reason.contains("RTNETLINK answers: File exists")
                {
                    tracing::warn!(
                        bridge = %spec.bridge_name,
                        gateway = %spec.gateway_ip,
                        error = %e,
                        "failed to assign gateway IP to bridge"
                    );
                }
            }
        }

        // Start dnsmasq for DHCP
        if !spec.subnet_cidr.is_empty() && !spec.gateway_ip.is_empty() {
            if let Err(e) = Self::start_dnsmasq(
                &spec.network_id,
                &spec.bridge_name,
                &spec.subnet_cidr,
                &spec.gateway_ip,
            )
            .await
            {
                warn!(error = %e, "failed to start dnsmasq");
            }
        }

        // Namespace
        if !Self::namespace_exists(&spec.namespace_name).await {
            Self::run_ip(&["netns", "add", &spec.namespace_name]).await?;
        }

        let _ = Self::run_nft_quiet(&["add", "table", "inet", &format!("chv-{}", spec.network_id)])
            .await;

        // VXLAN overlay: create VXLAN interface if overlay_type is VXLAN and vni > 0
        if spec.vni > 0 && spec.overlay_type == OverlayType::OverlayVxlan as i32 {
            let vtep_ip = self
                .vtep_ip
                .as_deref()
                .ok_or_else(|| ChvError::InvalidArgument {
                    field: "vtep_ip".to_string(),
                    reason: "VXLAN overlay requested but no local VTEP IP configured on executor"
                        .to_string(),
                })?;
            let vtep_port = spec
                .vtep_endpoints
                .first()
                .map(|e| if e.vtep_port == 0 { 4789 } else { e.vtep_port })
                .unwrap_or(4789);

            self.create_vxlan_interface(
                &spec.namespace_name,
                &spec.bridge_name,
                spec.vni,
                vtep_ip,
                vtep_port,
            )
            .await?;

            // Add FDB entries for peer VTEPs (use broadcast MAC for BUM traffic)
            for vtep in &spec.vtep_endpoints {
                self.add_fdb_entry(
                    &spec.namespace_name,
                    spec.vni,
                    "00:00:00:00:00:00",
                    &vtep.vtep_ip,
                )
                .await?;
            }

            info!(
                network_id = %spec.network_id,
                vni = spec.vni,
                vtep_ip = %vtep_ip,
                peer_count = spec.vtep_endpoints.len(),
                "VXLAN overlay configured"
            );
        }

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
        let _guard = self.nft_lock.lock().await;
        info!(
            network_id = %network_id,
            bridge = %state.bridge_name,
            namespace = %state.namespace_name,
            "deleting topology"
        );

        Self::stop_dnsmasq(network_id).await;

        // Tear down VXLAN interface and FDB entries before removing namespace
        if let Some(vni) = state.vni {
            // Delete FDB entries for all peer VTEPs
            for vtep_ip in &state.peer_vteps {
                if let Err(e) = self
                    .delete_fdb_entry(&state.namespace_name, vni, "00:00:00:00:00:00", vtep_ip)
                    .await
                {
                    warn!(vtep_ip = %vtep_ip, error = %e, "failed to delete FDB entry during topology teardown");
                }
            }

            // Delete the VXLAN interface
            if let Err(e) = self
                .delete_vxlan_interface(&state.namespace_name, vni)
                .await
            {
                warn!(vni = vni, error = %e, "failed to delete VXLAN interface during topology teardown");
            }
        }

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
            let _ = Self::run_nft_quiet(&["delete", "table", "inet", &table]).await;
        }
        // Drop remembered service exposures for this network.
        self.exposures.remove(network_id);

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
            return Ok("healthy".to_string());
        }

        let mut missing = Vec::new();
        if !bridge_ok {
            missing.push("bridge");
        }
        if !ns_ok {
            missing.push("namespace");
        }
        Ok(format!("degraded: missing {}", missing.join(", ")))
    }

    async fn attach_vm_nic(
        &self,
        network_id: &str,
        nic_id: &str,
        _vm_id: &str,
        bridge_name: &str,
        mac_address: &str,
        ip_address: &str,
    ) -> Result<(String, String), ChvError> {
        let tap_name = Self::tap_name_for_nic(nic_id);

        // Check if tap already exists; if so, just ensure it's on the right bridge and up.
        let tap_exists = Command::new("ip")
            .args(["link", "show", "dev", &tap_name])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !tap_exists {
            Self::run_ip(&["tuntap", "add", "dev", &tap_name, "mode", "tap"]).await?;
        }
        Self::run_ip(&["link", "set", "dev", &tap_name, "master", bridge_name]).await?;
        Self::run_ip(&["link", "set", "dev", &tap_name, "up"]).await?;

        Self::add_dhcp_host(network_id, mac_address, ip_address).await;

        info!(network_id = %network_id, nic_id = %nic_id, tap = %tap_name, "attached VM NIC");

        Ok((format!("ns-{}", network_id), tap_name))
    }

    async fn detach_vm_nic(
        &self,
        nic_id: &str,
        ownership: chv_common::AttachmentOwnership,
    ) -> Result<(), ChvError> {
        if ownership.vm_id.is_empty() {
            return Err(ChvError::InvalidArgument {
                field: "vm_id".to_string(),
                reason: "missing vm_id for detach".to_string(),
            });
        }
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
        policy_json: &[u8],
        bridge_name: &str,
    ) -> Result<(), ChvError> {
        let _guard = self.nft_lock.lock().await;
        let table = Self::sanitized_nft_table(network_id)?;
        let owned = Self::owned_ifaces_for_bridge(bridge_name).await?;
        crate::firewall::apply_firewall_rules(&table, &owned, policy_json)
            .await
            .inspect_err(|_e| {
                metrics::counter!(NWD_NFT_ERRORS_TOTAL, "operation" => "apply_firewall")
                    .increment(1);
            })?;
        // The firewall apply rebuilds the `forward` base chain, destroying any
        // service-exposure forward-accept rules; re-assert them inside the
        // CHV boundary so exposed flows are not silently dropped (#227 S3).
        self.reassert_exposures(network_id).await
    }

    async fn set_nat_policy(
        &self,
        network_id: &str,
        _policy_version: &str,
        policy_json: &[u8],
        bridge_name: &str,
    ) -> Result<(), ChvError> {
        let _guard = self.nft_lock.lock().await;
        let table = Self::sanitized_nft_table(network_id)?;
        let owned = Self::owned_ifaces_for_bridge(bridge_name).await?;
        crate::firewall::apply_nat_rules(&table, &owned, policy_json)
            .await
            .inspect_err(|_e| {
                metrics::counter!(NWD_NFT_ERRORS_TOTAL, "operation" => "apply_nat").increment(1);
            })
    }

    async fn ensure_dhcp_scope(
        &self,
        network_id: &str,
        cidr: &str,
        range_start: &str,
        range_end: &str,
        dns_servers: &[String],
    ) -> Result<(), ChvError> {
        crate::dhcp::ensure_dhcp_scope(network_id, cidr, range_start, range_end, dns_servers)
            .await
            .inspect_err(|_e| {
                metrics::counter!(NWD_DHCP_ERRORS_TOTAL, "operation" => "ensure_scope")
                    .increment(1);
            })
    }

    async fn ensure_dns_scope(
        &self,
        network_id: &str,
        forwarders: &[&str],
        static_records: &std::collections::HashMap<String, String>,
    ) -> Result<(), ChvError> {
        crate::dns::ensure_dns_scope(network_id, forwarders, static_records).await
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
        // Serialize with firewall/NAT applies and topology teardown so a
        // concurrent policy apply cannot flush this exposure's accept rule
        // mid-install.
        let _guard = self.nft_lock.lock().await;
        // Validate protocol to prevent command injection
        const ALLOWED_PROTOCOLS: &[&str] = &["tcp", "udp", "icmp", "sctp"];
        if !ALLOWED_PROTOCOLS.contains(&protocol) {
            return Err(ChvError::InvalidArgument {
                field: "protocol".to_string(),
                reason: format!(
                    "invalid protocol '{}': must be one of tcp, udp, icmp, sctp",
                    protocol
                ),
            });
        }

        // Validate target_ip to prevent command injection
        if target_ip.parse::<std::net::IpAddr>().is_err() {
            return Err(ChvError::InvalidArgument {
                field: "target_ip".to_string(),
                reason: format!("invalid IP address: '{}'", target_ip),
            });
        }

        let safe_exposure_id = Self::sanitize_id(exposure_id)?;
        self.install_exposure_rules(
            network_id,
            &safe_exposure_id,
            protocol,
            external_port,
            target_ip,
            target_port,
        )
        .await?;
        // Record so the exposure can be re-asserted after a firewall apply
        // rebuilds the forward base chain. Replacing an existing exposure_id
        // replaces its record (no unbounded growth).
        self.exposures
            .entry(network_id.to_string())
            .or_default()
            .retain(|r| r.safe_exposure_id != safe_exposure_id);
        self.exposures
            .entry(network_id.to_string())
            .or_default()
            .push(ExposureSpec {
                safe_exposure_id: safe_exposure_id.clone(),
                protocol: protocol.to_string(),
                external_port,
                target_ip: target_ip.to_string(),
                target_port,
            });
        info!(network_id = %network_id, exposure_id = %exposure_id, "service exposed via DNAT");
        Ok(())
    }

    async fn withdraw_service_exposure(
        &self,
        network_id: &str,
        exposure_id: &str,
    ) -> Result<(), ChvError> {
        let _guard = self.nft_lock.lock().await;
        let table = Self::sanitized_nft_table(network_id)?;
        let safe_exposure_id = Self::sanitize_id(exposure_id)?;
        Self::delete_rules_by_comment(&table, "prerouting", &safe_exposure_id).await?;
        Self::delete_rules_by_comment(&table, "forward", &safe_exposure_id).await?;
        if let Some(mut entry) = self.exposures.get_mut(network_id) {
            entry.retain(|r| r.safe_exposure_id != safe_exposure_id);
        }
        info!(network_id = %network_id, exposure_id = %exposure_id, "service exposure withdrawn");
        Ok(())
    }

    // --- VXLAN overlay implementations ---

    async fn create_vxlan_interface(
        &self,
        namespace: &str,
        bridge_name: &str,
        vni: u32,
        vtep_ip: &str,
        vtep_port: u32,
    ) -> Result<(), ChvError> {
        // Defense-in-depth: VNI is a 24-bit field
        if vni > 16_777_215 {
            return Err(ChvError::InvalidArgument {
                field: "vni".to_string(),
                reason: format!("VNI {} exceeds maximum 16777215", vni),
            });
        }

        let iface = Self::vxlan_interface_name(vni);
        let vni_str = vni.to_string();
        let port_str = vtep_port.to_string();

        // Create VXLAN interface in the default namespace first
        Self::run_ip(&[
            "link",
            "add",
            &iface,
            "type",
            "vxlan",
            "id",
            &vni_str,
            "local",
            vtep_ip,
            "dstport",
            &port_str,
            "nolearning",
        ])
        .await?;

        // Move interface to the namespace
        Self::run_ip(&["link", "set", &iface, "netns", namespace]).await?;

        // Set MTU (VXLAN overhead = 50 bytes: 8 VXLAN + 8 UDP + 20 IP + 14 Ethernet)
        let mtu = Self::detect_inner_mtu().await;
        let mtu_str = mtu.to_string();
        Self::run_ip_netns(namespace, &["link", "set", &iface, "mtu", &mtu_str]).await?;

        // Attach to bridge inside the namespace
        Self::run_ip_netns(namespace, &["link", "set", &iface, "master", bridge_name]).await?;

        // Set bridge MTU to match
        Self::run_ip_netns(namespace, &["link", "set", bridge_name, "mtu", &mtu_str]).await?;

        // Bring up the interface
        Self::run_ip_netns(namespace, &["link", "set", &iface, "up"]).await?;

        info!(namespace = %namespace, vni = vni, vtep_ip = %vtep_ip, mtu = mtu, "VXLAN interface created");
        Ok(())
    }

    async fn delete_vxlan_interface(&self, namespace: &str, vni: u32) -> Result<(), ChvError> {
        let iface = Self::vxlan_interface_name(vni);
        Self::run_ip_netns(namespace, &["link", "del", &iface]).await?;
        info!(namespace = %namespace, vni = vni, "VXLAN interface deleted");
        Ok(())
    }

    async fn add_fdb_entry(
        &self,
        namespace: &str,
        vni: u32,
        mac_address: &str,
        vtep_ip: &str,
    ) -> Result<(), ChvError> {
        let iface = Self::vxlan_interface_name(vni);
        Self::run_bridge_netns(
            namespace,
            &["fdb", "append", mac_address, "dev", &iface, "dst", vtep_ip],
        )
        .await
        .inspect_err(|_e| {
            metrics::counter!(NWD_FDB_ERRORS_TOTAL, "operation" => "add").increment(1);
        })
    }

    async fn delete_fdb_entry(
        &self,
        namespace: &str,
        vni: u32,
        mac_address: &str,
        vtep_ip: &str,
    ) -> Result<(), ChvError> {
        let iface = Self::vxlan_interface_name(vni);
        Self::run_bridge_netns(
            namespace,
            &["fdb", "del", mac_address, "dev", &iface, "dst", vtep_ip],
        )
        .await
        .inspect_err(|_e| {
            metrics::counter!(NWD_FDB_ERRORS_TOTAL, "operation" => "delete").increment(1);
        })
    }

    async fn replace_fdb_entry(
        &self,
        namespace: &str,
        vni: u32,
        mac_address: &str,
        new_vtep_ip: &str,
    ) -> Result<(), ChvError> {
        let iface = Self::vxlan_interface_name(vni);
        Self::run_bridge_netns(
            namespace,
            &[
                "fdb",
                "replace",
                mac_address,
                "dev",
                &iface,
                "dst",
                new_vtep_ip,
            ],
        )
        .await
        .inspect_err(|_e| {
            metrics::counter!(NWD_FDB_ERRORS_TOTAL, "operation" => "replace").increment(1);
        })
    }

    async fn send_gratuitous_arp(
        &self,
        namespace: &str,
        bridge_name: &str,
        vm_ip: &str,
    ) -> Result<(), ChvError> {
        let out = Self::run_cmd_netns_output(
            namespace,
            "arping",
            &["-U", "-c", "3", "-I", bridge_name, vm_ip],
        )
        .await?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            warn!(namespace = %namespace, vm_ip = %vm_ip, error = %stderr, "gratuitous ARP failed");
        }
        Ok(())
    }

    async fn set_arp_suppression(
        &self,
        namespace: &str,
        vni: u32,
        enabled: bool,
    ) -> Result<(), ChvError> {
        let iface = Self::vxlan_interface_name(vni);
        let value = if enabled { "on" } else { "off" };
        Self::run_bridge_netns(
            namespace,
            &["link", "set", "dev", &iface, "neigh_suppress", value],
        )
        .await?;
        info!(namespace = %namespace, vni = vni, enabled = enabled, "ARP suppression set");
        Ok(())
    }

    async fn get_overlay_status(
        &self,
        namespace: &str,
        vni: u32,
    ) -> Result<OverlayStatusInfo, ChvError> {
        let iface = Self::vxlan_interface_name(vni);

        // Check if VXLAN interface exists and is up
        let link_out =
            Self::run_cmd_netns_output(namespace, "ip", &["link", "show", &iface]).await?;
        let link_stdout = String::from_utf8_lossy(&link_out.stdout);
        let vxlan_interface_up = link_out.status.success() && link_stdout.contains("UP");

        // Count FDB entries
        let fdb_out =
            Self::run_cmd_netns_output(namespace, "bridge", &["fdb", "show", "dev", &iface])
                .await?;
        let fdb_entry_count = if fdb_out.status.success() {
            let stdout = String::from_utf8_lossy(&fdb_out.stdout);
            stdout.lines().count() as u32
        } else {
            0
        };

        Ok(OverlayStatusInfo {
            vxlan_interface_up,
            fdb_entry_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

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

    #[test]
    fn dnsmasq_args_use_equals_form_required_by_dnsmasq() {
        let args = LinuxExecutor::dnsmasq_args(
            std::path::Path::new("/run/chv/nwd/dnsmasq-net.conf"),
            std::path::Path::new("/run/chv/nwd/dnsmasq-net.pid"),
        );

        assert_eq!(
            args,
            [
                "--conf-file=/run/chv/nwd/dnsmasq-net.conf".to_string(),
                "--pid-file=/run/chv/nwd/dnsmasq-net.pid".to_string(),
            ]
        );
    }

    /// Mock executor that tracks VXLAN-related calls for verifying delete_topology behavior.
    struct VxlanTrackingExecutor {
        delete_vxlan_calls: Mutex<Vec<(String, u32)>>,
        delete_fdb_calls: Mutex<Vec<(String, u32, String, String)>>,
    }

    impl VxlanTrackingExecutor {
        fn new() -> Self {
            Self {
                delete_vxlan_calls: Mutex::new(Vec::new()),
                delete_fdb_calls: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl NetworkExecutor for VxlanTrackingExecutor {
        async fn ensure_topology(
            &self,
            _spec: &TopologySpec,
        ) -> Result<TopologyApplyResult, ChvError> {
            unimplemented!()
        }

        async fn delete_topology(
            &self,
            _network_id: &str,
            state: &crate::state::TopologyState,
        ) -> Result<(), ChvError> {
            // Replicate the VXLAN teardown logic from LinuxExecutor
            if let Some(vni) = state.vni {
                for vtep_ip in &state.peer_vteps {
                    self.delete_fdb_entry(&state.namespace_name, vni, "00:00:00:00:00:00", vtep_ip)
                        .await?;
                }
                self.delete_vxlan_interface(&state.namespace_name, vni)
                    .await?;
            }
            Ok(())
        }

        async fn health(
            &self,
            _network_id: &str,
            _state: &crate::state::TopologyState,
        ) -> Result<String, ChvError> {
            unimplemented!()
        }

        async fn attach_vm_nic(
            &self,
            _network_id: &str,
            _nic_id: &str,
            _vm_id: &str,
            _bridge_name: &str,
            _mac_address: &str,
            _ip_address: &str,
        ) -> Result<(String, String), ChvError> {
            unimplemented!()
        }

        async fn detach_vm_nic(
            &self,
            _nic_id: &str,
            _ownership: chv_common::AttachmentOwnership,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn set_firewall_policy(
            &self,
            _network_id: &str,
            _policy_version: &str,
            _policy_json: &[u8],
            _bridge_name: &str,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn set_nat_policy(
            &self,
            _network_id: &str,
            _policy_version: &str,
            _policy_json: &[u8],
            _bridge_name: &str,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn ensure_dhcp_scope(
            &self,
            _network_id: &str,
            _cidr: &str,
            _range_start: &str,
            _range_end: &str,
            _dns_servers: &[String],
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn ensure_dns_scope(
            &self,
            _network_id: &str,
            _forwarders: &[&str],
            _static_records: &std::collections::HashMap<String, String>,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn expose_service(
            &self,
            _network_id: &str,
            _exposure_id: &str,
            _protocol: &str,
            _external_port: u32,
            _target_ip: &str,
            _target_port: u32,
            _mode: &str,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn withdraw_service_exposure(
            &self,
            _network_id: &str,
            _exposure_id: &str,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn create_vxlan_interface(
            &self,
            _namespace: &str,
            _bridge_name: &str,
            _vni: u32,
            _vtep_ip: &str,
            _vtep_port: u32,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn delete_vxlan_interface(&self, namespace: &str, vni: u32) -> Result<(), ChvError> {
            self.delete_vxlan_calls
                .lock()
                .unwrap()
                .push((namespace.to_string(), vni));
            Ok(())
        }

        async fn add_fdb_entry(
            &self,
            _namespace: &str,
            _vni: u32,
            _mac_address: &str,
            _vtep_ip: &str,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn delete_fdb_entry(
            &self,
            namespace: &str,
            vni: u32,
            mac_address: &str,
            vtep_ip: &str,
        ) -> Result<(), ChvError> {
            self.delete_fdb_calls.lock().unwrap().push((
                namespace.to_string(),
                vni,
                mac_address.to_string(),
                vtep_ip.to_string(),
            ));
            Ok(())
        }

        async fn replace_fdb_entry(
            &self,
            _namespace: &str,
            _vni: u32,
            _mac_address: &str,
            _new_vtep_ip: &str,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn send_gratuitous_arp(
            &self,
            _namespace: &str,
            _bridge_name: &str,
            _vm_ip: &str,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn set_arp_suppression(
            &self,
            _namespace: &str,
            _vni: u32,
            _enabled: bool,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn get_overlay_status(
            &self,
            _namespace: &str,
            _vni: u32,
        ) -> Result<OverlayStatusInfo, ChvError> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn delete_topology_with_vni_cleans_up_vxlan() {
        let executor = VxlanTrackingExecutor::new();
        let state = crate::state::TopologyState {
            network_id: "net-vxlan".to_string(),
            tenant_id: "t1".to_string(),
            bridge_name: "br-net-vxlan".to_string(),
            namespace_name: "ns-net-vxlan".to_string(),
            subnet_cidr: "10.0.0.0/24".to_string(),
            gateway_ip: "10.0.0.1".to_string(),
            runtime_status: "ensured".to_string(),
            vni: Some(100),
            peer_vteps: vec!["192.168.1.10".to_string(), "192.168.1.11".to_string()],
        };

        executor.delete_topology("net-vxlan", &state).await.unwrap();

        // Should have deleted FDB entries for each peer VTEP
        let fdb_deletes = executor.delete_fdb_calls.lock().unwrap();
        assert_eq!(fdb_deletes.len(), 2);
        assert_eq!(
            fdb_deletes[0],
            (
                "ns-net-vxlan".to_string(),
                100,
                "00:00:00:00:00:00".to_string(),
                "192.168.1.10".to_string()
            )
        );
        assert_eq!(
            fdb_deletes[1],
            (
                "ns-net-vxlan".to_string(),
                100,
                "00:00:00:00:00:00".to_string(),
                "192.168.1.11".to_string()
            )
        );

        // Should have deleted the VXLAN interface
        let vxlan_deletes = executor.delete_vxlan_calls.lock().unwrap();
        assert_eq!(vxlan_deletes.len(), 1);
        assert_eq!(vxlan_deletes[0], ("ns-net-vxlan".to_string(), 100));
    }

    #[tokio::test]
    async fn delete_topology_without_vni_skips_vxlan() {
        let executor = VxlanTrackingExecutor::new();
        let state = crate::state::TopologyState {
            network_id: "net-plain".to_string(),
            tenant_id: "t1".to_string(),
            bridge_name: "br-net-plain".to_string(),
            namespace_name: "ns-net-plain".to_string(),
            subnet_cidr: "10.0.0.0/24".to_string(),
            gateway_ip: "10.0.0.1".to_string(),
            runtime_status: "ensured".to_string(),
            vni: None,
            peer_vteps: Vec::new(),
        };

        executor.delete_topology("net-plain", &state).await.unwrap();

        // No VXLAN cleanup should occur
        let fdb_deletes = executor.delete_fdb_calls.lock().unwrap();
        assert_eq!(
            fdb_deletes.len(),
            0,
            "no FDB deletes expected when vni is None"
        );

        let vxlan_deletes = executor.delete_vxlan_calls.lock().unwrap();
        assert_eq!(
            vxlan_deletes.len(),
            0,
            "no VXLAN delete expected when vni is None"
        );
    }

    #[test]
    fn parser_extracts_enslaved_member_names_not_ifindexes() {
        // Real `ip link show master <br>` output shapes (iproute2 6.x).
        // Field 0 is the ifindex (`355:`); the member name is field 1,
        // with an optional veth `@peer` suffix and a trailing colon.
        let veth_line =
            "355: vA@vB: <BROADCAST,MULTICAST,M-DOWN> mtu 1500 qdisc noop master brnet state DOWN mode DEFAULT group default qlen 1000";
        assert_eq!(
            LinuxExecutor::parse_ip_link_master_line(veth_line),
            Some("vA".to_string())
        );

        let tap_line =
            "388: tap-12ab: <BROADCAST,MULTICAST> mtu 1500 qdisc noop master brnet state UP mode DEFAULT group default qlen 1000";
        assert_eq!(
            LinuxExecutor::parse_ip_link_master_line(tap_line),
            Some("tap-12ab".to_string())
        );

        // Blank / non-member lines must not yield a member name.
        assert_eq!(LinuxExecutor::parse_ip_link_master_line(""), None);
        assert_eq!(LinuxExecutor::parse_ip_link_master_line("    "), None);
    }

    /// End-to-end proof that `owned_ifaces_for_bridge` returns the bridge plus
    /// its real enslaved members (guards the B1 parser regression). Requires
    /// root + `ip`; skipped on CI, run against a real host via:
    ///   sudo the built lib test binary -- --ignored --exact \
    ///     executor::tests::owned_ifaces_resolves_bridge_and_enslaved_members
    #[tokio::test]
    #[ignore = "requires root + iproute2"]
    async fn owned_ifaces_resolves_bridge_and_enslaved_members() {
        use std::process::Command as StdCommand;

        fn sh(args: &[&str]) {
            let status = StdCommand::new("ip")
                .args(args)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .expect("spawn ip");
            assert!(status.success(), "ip {:?} failed with {:?}", args, status);
        }

        let br = format!("brtest{:06x}", std::process::id());
        let veth_name = format!("vt{:06x}", std::process::id());
        let peer_name = format!("vtx{:06x}", std::process::id());

        // Best-effort cleanup on panic/success.
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = StdCommand::new("ip")
                    .args(["link", "del", "dev", &self.0])
                    .status();
            }
        }
        let cleanup = Cleanup(br.clone());

        sh(&["link", "add", &br, "type", "bridge"]);
        sh(&[
            "link", "add", &veth_name, "type", "veth", "peer", "name", &peer_name,
        ]);
        sh(&["link", "set", &veth_name, "master", &br]);

        let owned = LinuxExecutor::owned_ifaces_for_bridge(&br).await.unwrap();
        assert!(
            owned.contains(&br),
            "owned set must contain the bridge, got {:?}",
            owned
        );
        assert!(
            owned.contains(&veth_name),
            "owned set must contain the enslaved veth {}, got {:?}",
            veth_name,
            owned
        );
        assert!(
            !owned.contains(&peer_name),
            "the veth peer (not enslaved) must NOT be in the owned set, got {:?}",
            owned
        );
        drop(cleanup);
    }

    #[tokio::test]
    async fn owned_ifaces_fails_closed_when_bridge_missing() {
        let err = LinuxExecutor::owned_ifaces_for_bridge("definitely-not-a-bridge-xyz")
            .await
            .unwrap_err();
        match err {
            ChvError::NotFound { resource, .. } => {
                assert_eq!(resource, "bridge");
            }
            other => panic!("expected NotFound, got {:?}", other),
        }
    }
}
