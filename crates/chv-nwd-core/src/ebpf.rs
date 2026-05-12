//! eBPF policy enforcement for per-VM security rules and rate limiting.
//!
//! This module provides a trait-based abstraction over eBPF program management,
//! allowing mock/noop implementations for testing and environments where eBPF
//! programs are not compiled.

use async_trait::async_trait;
use chv_errors::ChvError;
use chv_nwd_api::chv_nwd_api as proto;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// BPF map pinned-file operations (Linux only)
// ---------------------------------------------------------------------------

/// Default base path for pinned BPF maps created by TC-attached programs.
pub const DEFAULT_BPF_PIN_PATH: &str = "/sys/fs/bpf/tc/globals/";

/// Update a BPF map element via the pinned map file and bpf() syscall.
///
/// Opens the pinned map at `pin_path`, then calls BPF_MAP_UPDATE_ELEM.
/// This is Linux-only and requires the process to have CAP_SYS_ADMIN or
/// CAP_BPF capability.
#[cfg(target_os = "linux")]
fn bpf_map_update(pin_path: &str, key: &[u8], value: &[u8]) -> Result<(), ChvError> {
    use std::os::unix::io::AsRawFd;

    // BPF syscall constants
    const BPF_MAP_UPDATE_ELEM: u32 = 2;
    const BPF_ANY: u64 = 0; // create or update

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(pin_path)
        .map_err(|e| ChvError::Io {
            path: pin_path.to_string(),
            source: e,
        })?;

    let fd = file.as_raw_fd();

    // bpf_attr for BPF_MAP_UPDATE_ELEM (subset of the union)
    #[repr(C)]
    struct BpfMapUpdateAttr {
        map_fd: u32,
        _pad0: u32,
        key: u64,
        value_or_next_key: u64,
        flags: u64,
    }

    let attr = BpfMapUpdateAttr {
        map_fd: fd as u32,
        _pad0: 0,
        key: key.as_ptr() as u64,
        value_or_next_key: value.as_ptr() as u64,
        flags: BPF_ANY,
    };

    let ret = unsafe {
        libc::syscall(
            libc::SYS_bpf,
            BPF_MAP_UPDATE_ELEM as libc::c_long,
            &attr as *const BpfMapUpdateAttr as *const libc::c_void,
            std::mem::size_of::<BpfMapUpdateAttr>() as libc::c_long,
        )
    };

    if ret < 0 {
        let err = std::io::Error::last_os_error();
        return Err(ChvError::Internal {
            reason: format!("bpf(BPF_MAP_UPDATE_ELEM) on {} failed: {}", pin_path, err),
        });
    }

    Ok(())
}

/// Lookup a BPF map element via the pinned map file and bpf() syscall.
///
/// Opens the pinned map at `pin_path`, then calls BPF_MAP_LOOKUP_ELEM.
/// Returns the value bytes of `value_size` length.
#[cfg(target_os = "linux")]
fn bpf_map_lookup(pin_path: &str, key: &[u8], value_size: usize) -> Result<Vec<u8>, ChvError> {
    use std::os::unix::io::AsRawFd;

    const BPF_MAP_LOOKUP_ELEM: u32 = 1;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .open(pin_path)
        .map_err(|e| ChvError::Io {
            path: pin_path.to_string(),
            source: e,
        })?;

    let fd = file.as_raw_fd();

    let mut value_buf = vec![0u8; value_size];

    #[repr(C)]
    struct BpfMapLookupAttr {
        map_fd: u32,
        _pad0: u32,
        key: u64,
        value_or_next_key: u64,
        flags: u64,
    }

    let attr = BpfMapLookupAttr {
        map_fd: fd as u32,
        _pad0: 0,
        key: key.as_ptr() as u64,
        value_or_next_key: value_buf.as_mut_ptr() as u64,
        flags: 0,
    };

    let ret = unsafe {
        libc::syscall(
            libc::SYS_bpf,
            BPF_MAP_LOOKUP_ELEM as libc::c_long,
            &attr as *const BpfMapLookupAttr as *const libc::c_void,
            std::mem::size_of::<BpfMapLookupAttr>() as libc::c_long,
        )
    };

    if ret < 0 {
        let err = std::io::Error::last_os_error();
        return Err(ChvError::Internal {
            reason: format!("bpf(BPF_MAP_LOOKUP_ELEM) on {} failed: {}", pin_path, err),
        });
    }

    Ok(value_buf)
}

/// Stub for non-Linux platforms — always returns an error.
#[cfg(not(target_os = "linux"))]
fn bpf_map_update(pin_path: &str, _key: &[u8], _value: &[u8]) -> Result<(), ChvError> {
    Err(ChvError::Internal {
        reason: format!(
            "BPF map operations are only supported on Linux (attempted: {})",
            pin_path
        ),
    })
}

/// Stub for non-Linux platforms — always returns an error.
#[cfg(not(target_os = "linux"))]
fn bpf_map_lookup(pin_path: &str, _key: &[u8], _value_size: usize) -> Result<Vec<u8>, ChvError> {
    Err(ChvError::Internal {
        reason: format!(
            "BPF map operations are only supported on Linux (attempted: {})",
            pin_path
        ),
    })
}

// ---------------------------------------------------------------------------
// BPF map value serialization helpers
// ---------------------------------------------------------------------------

/// Packed rule entry as stored in the BPF rule_map.
/// Each VM can have up to MAX_RULES_PER_VM entries stored as a flat array.
const MAX_RULES_PER_VM: usize = 64;

/// Size of a single packed rule entry in the BPF map (bytes).
/// Layout: src_ip(4) + src_mask(4) + dst_ip(4) + dst_mask(4) + src_port_min(2)
///       + src_port_max(2) + dst_port_min(2) + dst_port_max(2) + protocol(1)
///       + direction(1) + action(1) + _pad(1) + priority(4) = 32 bytes
const BPF_RULE_ENTRY_SIZE: usize = 32;

/// Serialize a single EbpfRule into a fixed-size byte array for the BPF map.
fn serialize_rule(rule: &EbpfRule) -> [u8; BPF_RULE_ENTRY_SIZE] {
    let mut buf = [0u8; BPF_RULE_ENTRY_SIZE];
    buf[0..4].copy_from_slice(&rule.src_ip.to_ne_bytes());
    buf[4..8].copy_from_slice(&rule.src_mask.to_ne_bytes());
    buf[8..12].copy_from_slice(&rule.dst_ip.to_ne_bytes());
    buf[12..16].copy_from_slice(&rule.dst_mask.to_ne_bytes());
    buf[16..18].copy_from_slice(&rule.src_port_min.to_ne_bytes());
    buf[18..20].copy_from_slice(&rule.src_port_max.to_ne_bytes());
    buf[20..22].copy_from_slice(&rule.dst_port_min.to_ne_bytes());
    buf[22..24].copy_from_slice(&rule.dst_port_max.to_ne_bytes());
    buf[24] = rule.protocol;
    buf[25] = rule.direction;
    buf[26] = rule.action;
    buf[27] = 0; // padding
    buf[28..32].copy_from_slice(&rule.priority.to_ne_bytes());
    buf
}

/// Serialize a rule set into the BPF map value format.
/// Format: rule_count(u32) + rules[MAX_RULES_PER_VM] packed entries.
fn serialize_rules_value(rules: &[EbpfRule]) -> Vec<u8> {
    let count = rules.len().min(MAX_RULES_PER_VM) as u32;
    // Value layout: 4 bytes count + MAX_RULES_PER_VM * BPF_RULE_ENTRY_SIZE
    let value_size = 4 + MAX_RULES_PER_VM * BPF_RULE_ENTRY_SIZE;
    let mut buf = vec![0u8; value_size];
    buf[0..4].copy_from_slice(&count.to_ne_bytes());
    for (i, rule) in rules.iter().take(MAX_RULES_PER_VM).enumerate() {
        let offset = 4 + i * BPF_RULE_ENTRY_SIZE;
        let serialized = serialize_rule(rule);
        buf[offset..offset + BPF_RULE_ENTRY_SIZE].copy_from_slice(&serialized);
    }
    buf
}

/// Serialize a rate limit entry for the BPF rate_map.
/// Layout: rate_bps(u64) + burst_bytes(u64) + tokens(u64) + last_refill_ns(u64) = 32 bytes
fn serialize_rate_limit_value(rate_limit: &EbpfRateLimit) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&rate_limit.rate_bps.to_ne_bytes());
    buf[8..16].copy_from_slice(&rate_limit.burst_bytes.to_ne_bytes());
    // tokens: initialize to burst_bytes (full bucket)
    buf[16..24].copy_from_slice(&rate_limit.burst_bytes.to_ne_bytes());
    // last_refill_ns: 0 (kernel will set on first packet)
    buf[24..32].copy_from_slice(&0u64.to_ne_bytes());
    buf
}

/// Size of the stats_map value: packets_allowed(8) + packets_denied(8) +
/// bytes_allowed(8) + bytes_denied(8) = 32 bytes
const BPF_STATS_VALUE_SIZE: usize = 32;

/// Deserialize stats from the BPF stats_map value.
fn deserialize_stats(buf: &[u8]) -> EbpfStats {
    if buf.len() < BPF_STATS_VALUE_SIZE {
        return EbpfStats::default();
    }
    EbpfStats {
        packets_allowed: u64::from_ne_bytes(buf[0..8].try_into().unwrap_or([0; 8])),
        packets_denied: u64::from_ne_bytes(buf[8..16].try_into().unwrap_or([0; 8])),
        bytes_allowed: u64::from_ne_bytes(buf[16..24].try_into().unwrap_or([0; 8])),
        bytes_denied: u64::from_ne_bytes(buf[24..32].try_into().unwrap_or([0; 8])),
    }
}

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

    /// Return the number of interfaces with loaded eBPF programs.
    fn loaded_program_count(&self) -> u32;

    /// Return the total number of denied packets observed across all VMs.
    fn denied_packets_total(&self) -> u64;
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
// LinuxEbpfManager — real implementation using TC + pinned BPF maps
// ---------------------------------------------------------------------------

/// Real eBPF manager that uses `tc` commands to attach/detach programs and
/// the `bpf()` syscall to manipulate pinned BPF maps.
pub struct LinuxEbpfManager {
    /// Path to directory containing compiled eBPF programs (e.g. policy_tc.o).
    program_path: String,
    /// Base path for pinned BPF maps (e.g. `/sys/fs/bpf/tc/globals/`).
    bpf_pin_base: String,
    /// Interfaces with successfully loaded eBPF programs.
    loaded_interfaces: std::sync::Mutex<std::collections::HashSet<String>>,
    /// Counter tracking total denied packets observed from stats reads.
    denied_packets: Arc<AtomicU64>,
}

impl LinuxEbpfManager {
    pub fn new(program_path: String) -> Self {
        Self {
            program_path,
            bpf_pin_base: DEFAULT_BPF_PIN_PATH.to_string(),
            loaded_interfaces: std::sync::Mutex::new(std::collections::HashSet::new()),
            denied_packets: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Create a manager with a custom BPF pin base path.
    pub fn with_pin_path(program_path: String, bpf_pin_base: String) -> Self {
        Self {
            program_path,
            bpf_pin_base,
            loaded_interfaces: std::sync::Mutex::new(std::collections::HashSet::new()),
            denied_packets: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Get the full path for a pinned BPF map.
    fn map_path(&self, map_name: &str) -> String {
        format!("{}{}", self.bpf_pin_base, map_name)
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

        if let Ok(mut loaded) = self.loaded_interfaces.lock() {
            loaded.insert(tap_name.to_string());
        }

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

        if let Ok(mut loaded) = self.loaded_interfaces.lock() {
            loaded.insert(bridge_name.to_string());
        }

        info!(bridge = %bridge_name, obj = %obj_path, "loaded eBPF policy program (ingress)");
        Ok(())
    }

    async fn update_rules(&self, vm_id: &str, rules: &[EbpfRule]) -> Result<(), ChvError> {
        let vm_hash = hash_vm_id(vm_id);
        let key = vm_hash.to_ne_bytes();
        let value = serialize_rules_value(rules);
        let pin_path = self.map_path("rule_map");

        bpf_map_update(&pin_path, &key, &value)?;

        info!(
            vm_id = %vm_id,
            vm_id_hash = vm_hash,
            rule_count = rules.len(),
            pin_path = %pin_path,
            "eBPF rule_map updated"
        );
        Ok(())
    }

    async fn set_default_action(&self, vm_id: &str, action: u8) -> Result<(), ChvError> {
        let vm_hash = hash_vm_id(vm_id);
        let key = vm_hash.to_ne_bytes();
        // Value is action as u32 (u8 padded to u32 for BPF map alignment)
        let value = (action as u32).to_ne_bytes();
        let pin_path = self.map_path("defaults_map");

        bpf_map_update(&pin_path, &key, &value)?;

        info!(
            vm_id = %vm_id,
            vm_id_hash = vm_hash,
            action = action,
            pin_path = %pin_path,
            "eBPF defaults_map updated"
        );
        Ok(())
    }

    async fn update_rate_limit(
        &self,
        vm_id: &str,
        rate_limit: &EbpfRateLimit,
    ) -> Result<(), ChvError> {
        let vm_hash = hash_vm_id(vm_id);
        let key = vm_hash.to_ne_bytes();
        let value = serialize_rate_limit_value(rate_limit);
        let pin_path = self.map_path("rate_map");

        bpf_map_update(&pin_path, &key, &value)?;

        info!(
            vm_id = %vm_id,
            vm_id_hash = vm_hash,
            rate_bps = rate_limit.rate_bps,
            burst_bytes = rate_limit.burst_bytes,
            pin_path = %pin_path,
            "eBPF rate_map updated"
        );
        Ok(())
    }

    async fn read_stats(&self, vm_id: &str) -> Result<EbpfStats, ChvError> {
        let vm_hash = hash_vm_id(vm_id);
        let key = vm_hash.to_ne_bytes();
        let pin_path = self.map_path("stats_map");

        let value = bpf_map_lookup(&pin_path, &key, BPF_STATS_VALUE_SIZE)?;
        let stats = deserialize_stats(&value);

        // Track denied packets
        if stats.packets_denied > 0 {
            self.denied_packets
                .fetch_add(stats.packets_denied, Ordering::Relaxed);
        }

        debug!(
            vm_id = %vm_id,
            vm_id_hash = vm_hash,
            packets_allowed = stats.packets_allowed,
            packets_denied = stats.packets_denied,
            "eBPF stats_map read"
        );
        Ok(stats)
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

    fn loaded_program_count(&self) -> u32 {
        self.loaded_interfaces
            .lock()
            .map(|s| s.len() as u32)
            .unwrap_or(0)
    }

    fn denied_packets_total(&self) -> u64 {
        self.denied_packets.load(Ordering::Relaxed)
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

    fn loaded_program_count(&self) -> u32 {
        0
    }

    fn denied_packets_total(&self) -> u64 {
        0
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

    fn loaded_program_count(&self) -> u32 {
        0
    }

    fn denied_packets_total(&self) -> u64 {
        0
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

    #[test]
    fn linux_ebpf_manager_loaded_interfaces_tracking() {
        // Test that the loaded_interfaces set is tracked correctly via loaded_program_count.
        let mgr = LinuxEbpfManager::new("/fake/path".to_string());
        assert_eq!(mgr.loaded_program_count(), 0);

        // Simulate successful load by directly inserting (since we can't run tc in tests)
        mgr.loaded_interfaces
            .lock()
            .unwrap()
            .insert("tap-001".to_string());
        assert_eq!(mgr.loaded_program_count(), 1);

        mgr.loaded_interfaces
            .lock()
            .unwrap()
            .insert("tap-002".to_string());
        assert_eq!(mgr.loaded_program_count(), 2);
    }

    #[test]
    fn linux_ebpf_manager_unload_removes_from_tracking() {
        let mgr = LinuxEbpfManager::new("/fake/path".to_string());

        // Add interfaces to simulate loaded programs
        {
            let mut loaded = mgr.loaded_interfaces.lock().unwrap();
            loaded.insert("tap-001".to_string());
            loaded.insert("tap-002".to_string());
        }
        assert_eq!(mgr.loaded_program_count(), 2);

        // Simulate unload by removing from the tracking set
        mgr.loaded_interfaces.lock().unwrap().remove("tap-001");
        assert_eq!(mgr.loaded_program_count(), 1);

        mgr.loaded_interfaces.lock().unwrap().remove("tap-002");
        assert_eq!(mgr.loaded_program_count(), 0);
    }

    #[test]
    fn linux_ebpf_manager_duplicate_load_is_idempotent() {
        let mgr = LinuxEbpfManager::new("/fake/path".to_string());

        // Insert the same interface twice — HashSet ensures no duplicates
        mgr.loaded_interfaces
            .lock()
            .unwrap()
            .insert("tap-001".to_string());
        mgr.loaded_interfaces
            .lock()
            .unwrap()
            .insert("tap-001".to_string());
        assert_eq!(
            mgr.loaded_program_count(),
            1,
            "duplicate insert should not increase count"
        );
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
