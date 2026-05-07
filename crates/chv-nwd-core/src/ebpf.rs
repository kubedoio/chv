//! eBPF policy enforcement for per-VM security rules and rate limiting.
//!
//! This module provides a trait-based abstraction over eBPF program management,
//! allowing mock/noop implementations for testing and environments where eBPF
//! programs are not compiled.

use async_trait::async_trait;
use chv_errors::ChvError;
use chv_nwd_api::chv_nwd_api as proto;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Data structures for eBPF map entries
// ---------------------------------------------------------------------------

/// Represents a security rule entry for the eBPF rule_map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EbpfRule {
    pub vm_id_hash: u32,
    /// 0=both, 1=ingress, 2=egress
    pub direction: u8,
    pub priority: u32,
    pub src_ip: u32,
    pub src_mask: u32,
    pub dst_ip: u32,
    pub dst_mask: u32,
    pub src_port_min: u16,
    pub src_port_max: u16,
    pub dst_port_min: u16,
    pub dst_port_max: u16,
    /// 0=any, 6=tcp, 17=udp, 1=icmp
    pub protocol: u8,
    /// 0=deny, 1=allow
    pub action: u8,
}

/// Rate limit entry for the eBPF rate_map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EbpfRateLimit {
    pub vm_id_hash: u32,
    pub rate_bps: u64,
    pub burst_bytes: u64,
}

/// Per-VM traffic stats from eBPF stats_map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EbpfStats {
    pub packets_allowed: u64,
    pub packets_denied: u64,
    pub bytes_allowed: u64,
    pub bytes_denied: u64,
}

// ---------------------------------------------------------------------------
// EbpfManager trait
// ---------------------------------------------------------------------------

/// Trait for eBPF program management. Allows mock implementation for testing.
#[async_trait]
pub trait EbpfManager: Send + Sync + 'static {
    /// Load TC classifier program on a TAP interface (egress).
    async fn load_policy_program(&self, tap_name: &str) -> Result<(), ChvError>;

    /// Load TC classifier program on bridge interface (ingress).
    async fn load_ingress_program(&self, bridge_name: &str) -> Result<(), ChvError>;

    /// Update security rules for a VM in the rule_map.
    async fn update_rules(&self, vm_id: &str, rules: &[EbpfRule]) -> Result<(), ChvError>;

    /// Set default action for a VM (applied when no rule matches).
    async fn set_default_action(&self, vm_id: &str, action: u8) -> Result<(), ChvError>;

    /// Update rate limit for a VM in the rate_map.
    async fn update_rate_limit(
        &self,
        vm_id: &str,
        rate_limit: &EbpfRateLimit,
    ) -> Result<(), ChvError>;

    /// Read traffic stats for a VM from stats_map.
    async fn read_stats(&self, vm_id: &str) -> Result<EbpfStats, ChvError>;

    /// Detach TC program from an interface.
    async fn detach_program(&self, interface: &str) -> Result<(), ChvError>;

    /// Check if eBPF programs are available (compiled .o files exist).
    fn is_available(&self) -> bool;
}

// ---------------------------------------------------------------------------
// Hash helper
// ---------------------------------------------------------------------------

/// Hash a vm_id to a u32 for use as eBPF map key.
/// Uses FNV-1a hash truncated to 32 bits.
pub fn hash_vm_id(vm_id: &str) -> u32 {
    let full = chv_common::fnv1a_hash(vm_id);
    (full & 0xffff_ffff) as u32
}

// ---------------------------------------------------------------------------
// CIDR parsing helper
// ---------------------------------------------------------------------------

/// Parse "10.0.0.0/24" into (ip_u32, mask_u32).
/// Returns (0, 0) for empty or unparseable input.
pub fn parse_cidr(cidr: &str) -> (u32, u32) {
    if cidr.is_empty() {
        return (0, 0);
    }

    let Some((ip_str, prefix_str)) = cidr.split_once('/') else {
        // Try bare IP without prefix — treat as /32
        return match parse_ipv4(ip_str_no_prefix(cidr)) {
            Some(ip) => (ip, 0xffff_ffff),
            None => (0, 0),
        };
    };

    let Some(ip) = parse_ipv4(ip_str) else {
        return (0, 0);
    };

    let Ok(prefix) = prefix_str.parse::<u8>() else {
        return (0, 0);
    };

    if prefix > 32 {
        return (0, 0);
    }

    let mask = if prefix == 0 {
        0u32
    } else {
        !0u32 << (32 - prefix)
    };

    (ip, mask)
}

fn ip_str_no_prefix(s: &str) -> &str {
    s
}

fn parse_ipv4(s: &str) -> Option<u32> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let o0: u8 = parts[0].parse().ok()?;
    let o1: u8 = parts[1].parse().ok()?;
    let o2: u8 = parts[2].parse().ok()?;
    let o3: u8 = parts[3].parse().ok()?;
    Some(u32::from_be_bytes([o0, o1, o2, o3]))
}

// ---------------------------------------------------------------------------
// Proto-to-EbpfRule conversion
// ---------------------------------------------------------------------------

/// Convert a proto SecurityPolicy into a Vec<EbpfRule>.
pub fn proto_to_ebpf_rules(vm_id: &str, policy: &proto::SecurityPolicy) -> Vec<EbpfRule> {
    let vm_hash = hash_vm_id(vm_id);
    policy
        .rules
        .iter()
        .map(|rule| {
            let (src_ip, src_mask) = parse_cidr(&rule.src_cidr);
            let (dst_ip, dst_mask) = parse_cidr(&rule.dst_cidr);

            let direction = match rule.direction {
                x if x == proto::Direction::Ingress as i32 => 1,
                x if x == proto::Direction::Egress as i32 => 2,
                _ => 0, // BOTH
            };

            let protocol = match rule.protocol {
                x if x == proto::Protocol::Tcp as i32 => 6,
                x if x == proto::Protocol::Udp as i32 => 17,
                x if x == proto::Protocol::Icmp as i32 => 1,
                _ => 0, // ANY
            };

            let action = if rule.action == proto::PolicyAction::PolicyAllow as i32 {
                1
            } else {
                0
            };

            let (src_port_min, src_port_max) = rule
                .src_port
                .as_ref()
                .map(|p| {
                    let max = if p.max == 0 { p.min } else { p.max };
                    (p.min as u16, max as u16)
                })
                .unwrap_or((0, 0));

            let (dst_port_min, dst_port_max) = rule
                .dst_port
                .as_ref()
                .map(|p| {
                    let max = if p.max == 0 { p.min } else { p.max };
                    (p.min as u16, max as u16)
                })
                .unwrap_or((0, 0));

            EbpfRule {
                vm_id_hash: vm_hash,
                direction,
                priority: rule.priority,
                src_ip,
                src_mask,
                dst_ip,
                dst_mask,
                src_port_min,
                src_port_max,
                dst_port_min,
                dst_port_max,
                protocol,
                action,
            }
        })
        .collect()
}

/// Convert a proto RateLimitPolicy into an EbpfRateLimit.
pub fn proto_to_ebpf_rate_limit(policy: &proto::RateLimitPolicy) -> EbpfRateLimit {
    EbpfRateLimit {
        vm_id_hash: hash_vm_id(&policy.vm_id),
        rate_bps: policy.rate_bps,
        burst_bytes: policy.burst_bytes,
    }
}

// ---------------------------------------------------------------------------
// LinuxEbpfManager — real implementation (stubs BPF map operations)
// ---------------------------------------------------------------------------

/// Real eBPF manager that uses `tc` commands to attach/detach programs.
/// BPF map operations are logged but not executed (requires libbpf-rs and
/// compiled .o programs that are not available in CI).
pub struct LinuxEbpfManager {
    /// Path to directory containing compiled eBPF programs (e.g. policy_tc.o).
    program_path: String,
}

impl LinuxEbpfManager {
    pub fn new(program_path: String) -> Self {
        Self { program_path }
    }

    async fn run_tc(args: &[&str]) -> Result<(), ChvError> {
        let out = tokio::process::Command::new("tc")
            .args(args)
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "tc".to_string(),
                source: e,
            })?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("File exists") || stderr.contains("already exists") {
                return Ok(());
            }
            return Err(ChvError::NetworkUnavailable {
                resource: "tc".to_string(),
                reason: format!("tc {} failed: {}", args.join(" "), stderr),
            });
        }
        Ok(())
    }
}

#[async_trait]
impl EbpfManager for LinuxEbpfManager {
    async fn load_policy_program(&self, tap_name: &str) -> Result<(), ChvError> {
        let obj_path = format!("{}/policy_tc.o", self.program_path);

        // Add clsact qdisc (required for TC eBPF)
        Self::run_tc(&["qdisc", "add", "dev", tap_name, "clsact"]).await?;

        // Attach TC filter on egress
        Self::run_tc(&[
            "filter", "add", "dev", tap_name, "egress", "bpf", "da", "obj", &obj_path, "sec", "tc",
        ])
        .await?;

        info!(tap = %tap_name, obj = %obj_path, "loaded eBPF policy program (egress)");
        Ok(())
    }

    async fn load_ingress_program(&self, bridge_name: &str) -> Result<(), ChvError> {
        let obj_path = format!("{}/policy_tc.o", self.program_path);

        Self::run_tc(&["qdisc", "add", "dev", bridge_name, "clsact"]).await?;

        Self::run_tc(&[
            "filter",
            "add",
            "dev",
            bridge_name,
            "ingress",
            "bpf",
            "da",
            "obj",
            &obj_path,
            "sec",
            "tc",
        ])
        .await?;

        info!(bridge = %bridge_name, obj = %obj_path, "loaded eBPF policy program (ingress)");
        Ok(())
    }

    async fn update_rules(&self, vm_id: &str, rules: &[EbpfRule]) -> Result<(), ChvError> {
        // In a full implementation, this would write to BPF maps via libbpf-rs.
        // For now, log the operation.
        info!(
            vm_id = %vm_id,
            vm_id_hash = hash_vm_id(vm_id),
            rule_count = rules.len(),
            "eBPF rule_map update (libbpf-rs stub)"
        );
        Ok(())
    }

    async fn set_default_action(&self, vm_id: &str, action: u8) -> Result<(), ChvError> {
        info!(
            vm_id = %vm_id,
            vm_id_hash = hash_vm_id(vm_id),
            action = action,
            "eBPF defaults_map update (libbpf-rs stub)"
        );
        Ok(())
    }

    async fn update_rate_limit(
        &self,
        vm_id: &str,
        rate_limit: &EbpfRateLimit,
    ) -> Result<(), ChvError> {
        info!(
            vm_id = %vm_id,
            vm_id_hash = rate_limit.vm_id_hash,
            rate_bps = rate_limit.rate_bps,
            burst_bytes = rate_limit.burst_bytes,
            "eBPF rate_map update (libbpf-rs stub)"
        );
        Ok(())
    }

    async fn read_stats(&self, vm_id: &str) -> Result<EbpfStats, ChvError> {
        debug!(vm_id = %vm_id, "eBPF stats_map read (libbpf-rs stub — returning zeros)");
        Ok(EbpfStats::default())
    }

    async fn detach_program(&self, interface: &str) -> Result<(), ChvError> {
        // Remove clsact qdisc (removes all attached TC eBPF programs)
        Self::run_tc(&["qdisc", "del", "dev", interface, "clsact"]).await?;
        info!(interface = %interface, "detached eBPF TC programs");
        Ok(())
    }

    fn is_available(&self) -> bool {
        let obj_path = format!("{}/policy_tc.o", self.program_path);
        std::path::Path::new(&obj_path).exists()
    }
}

// ---------------------------------------------------------------------------
// NoopEbpfManager — used when eBPF programs are not compiled
// ---------------------------------------------------------------------------

/// No-op eBPF manager that returns Ok for all operations.
/// Used when eBPF programs are not compiled/available.
pub struct NoopEbpfManager;

#[async_trait]
impl EbpfManager for NoopEbpfManager {
    async fn load_policy_program(&self, tap_name: &str) -> Result<(), ChvError> {
        debug!(tap = %tap_name, "noop: eBPF program load skipped (not available)");
        Ok(())
    }

    async fn load_ingress_program(&self, bridge_name: &str) -> Result<(), ChvError> {
        debug!(bridge = %bridge_name, "noop: eBPF ingress program load skipped (not available)");
        Ok(())
    }

    async fn update_rules(&self, vm_id: &str, rules: &[EbpfRule]) -> Result<(), ChvError> {
        debug!(vm_id = %vm_id, rule_count = rules.len(), "noop: eBPF rule update skipped");
        Ok(())
    }

    async fn set_default_action(&self, vm_id: &str, action: u8) -> Result<(), ChvError> {
        debug!(vm_id = %vm_id, action = action, "noop: eBPF default action skipped");
        Ok(())
    }

    async fn update_rate_limit(
        &self,
        vm_id: &str,
        _rate_limit: &EbpfRateLimit,
    ) -> Result<(), ChvError> {
        debug!(vm_id = %vm_id, "noop: eBPF rate limit update skipped");
        Ok(())
    }

    async fn read_stats(&self, _vm_id: &str) -> Result<EbpfStats, ChvError> {
        Ok(EbpfStats::default())
    }

    async fn detach_program(&self, interface: &str) -> Result<(), ChvError> {
        debug!(interface = %interface, "noop: eBPF detach skipped");
        Ok(())
    }

    fn is_available(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// MockEbpfManager — tracks calls for testing
// ---------------------------------------------------------------------------

/// Mock eBPF manager that tracks all calls in memory for testing.
pub struct MockEbpfManager {
    pub loaded_programs: std::sync::Mutex<Vec<String>>,
    pub loaded_ingress: std::sync::Mutex<Vec<String>>,
    pub updated_rules: std::sync::Mutex<Vec<(String, Vec<EbpfRule>)>>,
    pub default_actions: std::sync::Mutex<Vec<(String, u8)>>,
    pub updated_rate_limits: std::sync::Mutex<Vec<(String, EbpfRateLimit)>>,
    pub detached: std::sync::Mutex<Vec<String>>,
    pub stats_reads: std::sync::Mutex<Vec<String>>,
    pub available: bool,
}

impl MockEbpfManager {
    pub fn new(available: bool) -> Self {
        Self {
            loaded_programs: std::sync::Mutex::new(Vec::new()),
            loaded_ingress: std::sync::Mutex::new(Vec::new()),
            updated_rules: std::sync::Mutex::new(Vec::new()),
            default_actions: std::sync::Mutex::new(Vec::new()),
            updated_rate_limits: std::sync::Mutex::new(Vec::new()),
            detached: std::sync::Mutex::new(Vec::new()),
            stats_reads: std::sync::Mutex::new(Vec::new()),
            available,
        }
    }
}

#[async_trait]
impl EbpfManager for MockEbpfManager {
    async fn load_policy_program(&self, tap_name: &str) -> Result<(), ChvError> {
        self.loaded_programs
            .lock()
            .unwrap()
            .push(tap_name.to_string());
        Ok(())
    }

    async fn load_ingress_program(&self, bridge_name: &str) -> Result<(), ChvError> {
        self.loaded_ingress
            .lock()
            .unwrap()
            .push(bridge_name.to_string());
        Ok(())
    }

    async fn update_rules(&self, vm_id: &str, rules: &[EbpfRule]) -> Result<(), ChvError> {
        self.updated_rules
            .lock()
            .unwrap()
            .push((vm_id.to_string(), rules.to_vec()));
        Ok(())
    }

    async fn set_default_action(&self, vm_id: &str, action: u8) -> Result<(), ChvError> {
        self.default_actions
            .lock()
            .unwrap()
            .push((vm_id.to_string(), action));
        Ok(())
    }

    async fn update_rate_limit(
        &self,
        vm_id: &str,
        rate_limit: &EbpfRateLimit,
    ) -> Result<(), ChvError> {
        self.updated_rate_limits
            .lock()
            .unwrap()
            .push((vm_id.to_string(), rate_limit.clone()));
        Ok(())
    }

    async fn read_stats(&self, vm_id: &str) -> Result<EbpfStats, ChvError> {
        self.stats_reads.lock().unwrap().push(vm_id.to_string());
        Ok(EbpfStats {
            packets_allowed: 100,
            packets_denied: 5,
            bytes_allowed: 50000,
            bytes_denied: 1000,
        })
    }

    async fn detach_program(&self, interface: &str) -> Result<(), ChvError> {
        self.detached.lock().unwrap().push(interface.to_string());
        Ok(())
    }

    fn is_available(&self) -> bool {
        self.available
    }
}

// ---------------------------------------------------------------------------
// Stats collection background task
// ---------------------------------------------------------------------------

/// Background task that periodically reads eBPF stats for known VMs and emits metrics.
pub async fn stats_collection_loop(
    ebpf: Arc<dyn EbpfManager>,
    vm_ids: Arc<DashMap<String, ()>>,
    interval_secs: u64,
    mut shutdown: tokio::sync::watch::Receiver<()>,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                for entry in vm_ids.iter() {
                    let vm_id = entry.key();
                    match ebpf.read_stats(vm_id).await {
                        Ok(stats) => {
                            debug!(
                                vm_id = %vm_id,
                                packets_allowed = stats.packets_allowed,
                                packets_denied = stats.packets_denied,
                                bytes_allowed = stats.bytes_allowed,
                                bytes_denied = stats.bytes_denied,
                                "eBPF stats"
                            );
                        }
                        Err(e) => {
                            warn!(vm_id = %vm_id, error = %e, "failed to read eBPF stats");
                        }
                    }
                }
            }
            _ = shutdown.changed() => {
                info!("eBPF stats collection loop shutting down");
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_vm_id_is_deterministic() {
        let h1 = hash_vm_id("vm-abc-123");
        let h2 = hash_vm_id("vm-abc-123");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_vm_id_different_inputs_differ() {
        let h1 = hash_vm_id("vm-1");
        let h2 = hash_vm_id("vm-2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn parse_cidr_24() {
        let (ip, mask) = parse_cidr("10.0.0.0/24");
        assert_eq!(ip, 0x0a000000);
        assert_eq!(mask, 0xffffff00);
    }

    #[test]
    fn parse_cidr_32() {
        let (ip, mask) = parse_cidr("192.168.1.1/32");
        assert_eq!(ip, 0xc0a80101);
        assert_eq!(mask, 0xffffffff);
    }

    #[test]
    fn parse_cidr_0() {
        let (ip, mask) = parse_cidr("0.0.0.0/0");
        assert_eq!(ip, 0);
        assert_eq!(mask, 0);
    }

    #[test]
    fn parse_cidr_16() {
        let (ip, mask) = parse_cidr("172.16.0.0/16");
        assert_eq!(ip, 0xac100000);
        assert_eq!(mask, 0xffff0000);
    }

    #[test]
    fn parse_cidr_empty() {
        let (ip, mask) = parse_cidr("");
        assert_eq!(ip, 0);
        assert_eq!(mask, 0);
    }

    #[test]
    fn parse_cidr_invalid() {
        let (ip, mask) = parse_cidr("not-a-cidr");
        assert_eq!(ip, 0);
        assert_eq!(mask, 0);
    }

    #[test]
    fn parse_cidr_invalid_prefix() {
        let (ip, mask) = parse_cidr("10.0.0.0/33");
        assert_eq!(ip, 0);
        assert_eq!(mask, 0);
    }

    #[test]
    fn proto_to_ebpf_rules_converts_correctly() {
        let policy = proto::SecurityPolicy {
            vm_id: "vm-test".to_string(),
            network_id: "net-1".to_string(),
            default_action: proto::PolicyAction::PolicyDeny as i32,
            rules: vec![
                proto::SecurityRule {
                    direction: proto::Direction::Ingress as i32,
                    protocol: proto::Protocol::Tcp as i32,
                    src_cidr: "10.0.0.0/24".to_string(),
                    dst_cidr: "192.168.1.0/24".to_string(),
                    src_port: Some(proto::PortRange { min: 0, max: 0 }),
                    dst_port: Some(proto::PortRange { min: 80, max: 443 }),
                    action: proto::PolicyAction::PolicyAllow as i32,
                    priority: 100,
                },
                proto::SecurityRule {
                    direction: proto::Direction::Both as i32,
                    protocol: proto::Protocol::Udp as i32,
                    src_cidr: String::new(),
                    dst_cidr: String::new(),
                    src_port: None,
                    dst_port: Some(proto::PortRange { min: 53, max: 0 }),
                    action: proto::PolicyAction::PolicyAllow as i32,
                    priority: 200,
                },
            ],
        };

        let rules = proto_to_ebpf_rules("vm-test", &policy);
        assert_eq!(rules.len(), 2);

        // First rule: TCP ingress 10.0.0.0/24 -> 192.168.1.0/24 port 80-443 allow
        assert_eq!(rules[0].direction, 1); // ingress
        assert_eq!(rules[0].protocol, 6); // TCP
        assert_eq!(rules[0].src_ip, 0x0a000000);
        assert_eq!(rules[0].src_mask, 0xffffff00);
        assert_eq!(rules[0].dst_ip, 0xc0a80100);
        assert_eq!(rules[0].dst_mask, 0xffffff00);
        assert_eq!(rules[0].dst_port_min, 80);
        assert_eq!(rules[0].dst_port_max, 443);
        assert_eq!(rules[0].action, 1); // allow
        assert_eq!(rules[0].priority, 100);

        // Second rule: UDP both any->any port 53 allow
        assert_eq!(rules[1].direction, 0); // both
        assert_eq!(rules[1].protocol, 17); // UDP
        assert_eq!(rules[1].src_ip, 0);
        assert_eq!(rules[1].src_mask, 0);
        assert_eq!(rules[1].dst_port_min, 53);
        assert_eq!(rules[1].dst_port_max, 53); // max=0 means same as min
        assert_eq!(rules[1].action, 1); // allow
        assert_eq!(rules[1].priority, 200);
    }

    #[test]
    fn proto_to_ebpf_rate_limit_converts() {
        let policy = proto::RateLimitPolicy {
            vm_id: "vm-rl".to_string(),
            rate_bps: 1_000_000,
            burst_bytes: 65536,
        };

        let rl = proto_to_ebpf_rate_limit(&policy);
        assert_eq!(rl.vm_id_hash, hash_vm_id("vm-rl"));
        assert_eq!(rl.rate_bps, 1_000_000);
        assert_eq!(rl.burst_bytes, 65536);
    }

    #[tokio::test]
    async fn mock_ebpf_manager_tracks_calls() {
        let mock = MockEbpfManager::new(true);
        assert!(mock.is_available());

        mock.load_policy_program("tap-001").await.unwrap();
        mock.load_ingress_program("br0").await.unwrap();

        let rules = vec![EbpfRule {
            vm_id_hash: 42,
            direction: 1,
            priority: 100,
            src_ip: 0x0a000000,
            src_mask: 0xffffff00,
            dst_ip: 0,
            dst_mask: 0,
            src_port_min: 0,
            src_port_max: 0,
            dst_port_min: 80,
            dst_port_max: 80,
            protocol: 6,
            action: 1,
        }];
        mock.update_rules("vm-1", &rules).await.unwrap();
        mock.set_default_action("vm-1", 0).await.unwrap();

        let rl = EbpfRateLimit {
            vm_id_hash: 42,
            rate_bps: 1_000_000,
            burst_bytes: 65536,
        };
        mock.update_rate_limit("vm-1", &rl).await.unwrap();

        let stats = mock.read_stats("vm-1").await.unwrap();
        assert_eq!(stats.packets_allowed, 100);

        mock.detach_program("tap-001").await.unwrap();

        // Verify tracked calls
        assert_eq!(
            mock.loaded_programs.lock().unwrap().as_slice(),
            &["tap-001"]
        );
        assert_eq!(mock.loaded_ingress.lock().unwrap().as_slice(), &["br0"]);
        assert_eq!(mock.updated_rules.lock().unwrap().len(), 1);
        assert_eq!(
            mock.default_actions.lock().unwrap().as_slice(),
            &[("vm-1".to_string(), 0)]
        );
        assert_eq!(mock.updated_rate_limits.lock().unwrap().len(), 1);
        assert_eq!(
            mock.stats_reads.lock().unwrap().as_slice(),
            &["vm-1".to_string()]
        );
        assert_eq!(
            mock.detached.lock().unwrap().as_slice(),
            &["tap-001".to_string()]
        );
    }

    #[tokio::test]
    async fn noop_ebpf_manager_returns_ok() {
        let noop = NoopEbpfManager;
        assert!(!noop.is_available());

        assert!(noop.load_policy_program("tap-x").await.is_ok());
        assert!(noop.load_ingress_program("br-x").await.is_ok());
        assert!(noop.update_rules("vm-x", &[]).await.is_ok());
        assert!(noop.set_default_action("vm-x", 1).await.is_ok());
        let rl = EbpfRateLimit {
            vm_id_hash: 0,
            rate_bps: 0,
            burst_bytes: 0,
        };
        assert!(noop.update_rate_limit("vm-x", &rl).await.is_ok());
        let stats = noop.read_stats("vm-x").await.unwrap();
        assert_eq!(stats, EbpfStats::default());
        assert!(noop.detach_program("tap-x").await.is_ok());
    }

    #[tokio::test]
    async fn stats_collection_loop_reads_and_shuts_down() {
        let mock = Arc::new(MockEbpfManager::new(true));
        let vm_ids: Arc<DashMap<String, ()>> = Arc::new(DashMap::new());
        vm_ids.insert("vm-loop-1".to_string(), ());
        vm_ids.insert("vm-loop-2".to_string(), ());

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());

        let ebpf: Arc<dyn EbpfManager> = mock.clone();
        let vm_ids_clone = vm_ids.clone();

        let handle = tokio::spawn(async move {
            stats_collection_loop(ebpf, vm_ids_clone, 1, shutdown_rx).await;
        });

        // Let it tick once
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;

        // Signal shutdown
        drop(shutdown_tx);
        handle.await.unwrap();

        // Verify stats were read for both VMs
        let reads = mock.stats_reads.lock().unwrap();
        assert!(
            reads.len() >= 2,
            "expected at least 2 stats reads, got {}",
            reads.len()
        );
    }
}
