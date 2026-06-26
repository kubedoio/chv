use crate::cache::{NodeCache, VmNicAttachment};
use crate::daemon_clients::{NwdClient, StordClient};
use crate::migration_registry::MigrationTaskRegistry;
use crate::state_machine::NodeState;
use crate::vm_runtime::VmRuntime;
use chv_agent_runtime_ch::adapter::{VmConfig, VmDiskConfig, VmNicConfig};
use chv_errors::ChvError;
use futures_util::stream::{self, StreamExt};
use std::collections::{BTreeSet, HashSet};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Maximum number of VMs to reconcile concurrently within a single tick.
///
/// Each parallel slot performs full per-VM work — opening volumes via stord,
/// attaching NICs via nwd, and creating/starting/stopping/deleting the VM via
/// the hypervisor adapter. Each slot also opens its own short-lived stord and
/// nwd connections, so this constant doubles as a bound on per-tick socket
/// pressure on those daemons.
const VM_RECONCILE_CONCURRENCY: usize = 8;

/// Construct a bridge name for a network, guaranteed to be <= 15 chars (IFNAMSIZ limit).
///
/// For the "default" network, returns "chvbr0". For other networks, returns
/// "br-{net_id}" if it fits in 15 chars, otherwise truncates net_id and appends
/// a 4-hex-char hash suffix to avoid collisions: "br-{prefix}{hash}".
pub(crate) fn bridge_name_for_network(net_id: &str) -> String {
    if net_id == "default" {
        return "chvbr0".to_string();
    }
    let candidate = format!("br-{}", net_id);
    if candidate.len() <= 15 {
        return candidate;
    }
    // "br-" (3) + up to 8 chars of net_id + 4-char hash = 15 chars total
    let prefix: String = net_id.chars().take(8).collect();
    let hash = {
        let mut h: u32 = 0x811c9dc5;
        for b in net_id.as_bytes() {
            h = h.wrapping_mul(0x01000193) ^ (*b as u32);
        }
        format!("{:04x}", h & 0xffff)
    };
    format!("br-{}{}", prefix, hash)
}

pub struct Reconciler {
    pub cache: Arc<tokio::sync::Mutex<NodeCache>>,
    pub vm_runtime: VmRuntime,
    pub stord_socket: PathBuf,
    pub nwd_socket: PathBuf,
    pub runtime_dir: PathBuf,
    reconcile_tick: u64,
    degraded_ticks: u32,
    /// Tracks VMs that have already been requested for drain migration
    /// to avoid re-requesting on every tick.
    drain_requested_vms: HashSet<String>,
    /// Shared registry of in-flight migration tasks. The drain transition to
    /// Maintenance must not fire while disk migrations are still in progress;
    /// a VM handed off to chv-stord leaves vm_runtime.list() but the transfer
    /// is ongoing.
    migration_registry: Arc<MigrationTaskRegistry>,
}

/// Returns the per-VM runtime directory for the given VM.
/// This directory holds the VM's socket, logs, PID file, and other runtime artifacts.
pub fn vm_runtime_dir(base: &Path, vm_id: &str) -> PathBuf {
    base.join("vms").join(vm_id)
}

/// Backoff predicate: should we skip this VM on this tick?
///
/// Free function so it can be called from per-VM async closures running in
/// parallel without borrowing the Reconciler.
fn should_skip_vm_for_tick(reconcile_tick: u64, failures: u32) -> bool {
    if reconcile_tick <= 1 {
        return false;
    }
    if failures >= 10 {
        !reconcile_tick.is_multiple_of(60)
    } else if failures >= 3 {
        !reconcile_tick.is_multiple_of(6)
    } else {
        false
    }
}

/// Emit the standard "we are skipping this VM due to backoff" warning.
fn log_backoff_skip(vm_id: &str, failures: u32) {
    if failures >= 10 {
        warn!(vm_id = %vm_id, failures = failures, "VM in persistent failure, retrying every ~5min");
    } else {
        warn!(vm_id = %vm_id, failures = failures, "VM failing repeatedly, retrying every ~30s");
    }
}

impl Reconciler {
    pub async fn new(
        cache: Arc<tokio::sync::Mutex<NodeCache>>,
        vm_runtime: VmRuntime,
        stord_socket: PathBuf,
        nwd_socket: PathBuf,
        runtime_dir: PathBuf,
        migration_registry: Arc<MigrationTaskRegistry>,
    ) -> Self {
        Self {
            cache,
            vm_runtime,
            stord_socket,
            nwd_socket,
            runtime_dir,
            reconcile_tick: 0,
            degraded_ticks: 0,
            drain_requested_vms: HashSet::new(),
            migration_registry,
        }
    }

    pub async fn current_state(&self) -> NodeState {
        self.cache.lock().await.current_node_state()
    }

    pub async fn transition_state(&self, to: NodeState) -> Result<NodeState, ChvError> {
        let mut cache = self.cache.lock().await;
        cache.transition_node_state(to)
    }

    pub async fn run_once(&mut self) -> Result<(), ChvError> {
        self.reconcile_tick = self.reconcile_tick.wrapping_add(1);

        let operation_id = uuid::Uuid::new_v4().to_string();
        let span = tracing::info_span!(
            "reconcile_tick",
            operation_id = %operation_id,
            tick = self.reconcile_tick,
        );
        let _guard = span.enter();

        // Read state once under the lock to avoid a TOCTOU race where the
        // state could change between the debug-log read and the match read.
        let state = self.current_state().await;
        debug!(
            state = %state.as_str(),
            "reconcile tick"
        );

        match state {
            NodeState::Discovered => {
                self.transition_state(NodeState::Bootstrapping).await?;
            }
            NodeState::Bootstrapping => {
                // Probe stord: if reachable, advance to HostReady
                match StordClient::connect(&self.stord_socket).await {
                    Ok(_) => {
                        self.transition_state(NodeState::HostReady).await?;
                    }
                    Err(e) => {
                        warn!(error = %e, "stord not reachable, staying in Bootstrapping");
                    }
                }
            }
            NodeState::HostReady => {
                // Verify stord can serve volume sessions, then advance to StorageReady
                match StordClient::connect(&self.stord_socket).await {
                    Ok(mut stord) => match stord.health_probe().await {
                        Ok(_) => {
                            self.transition_state(NodeState::StorageReady).await?;
                        }
                        Err(e) => {
                            warn!(error = %e, "stord health_probe failed, staying in HostReady");
                        }
                    },
                    Err(e) => {
                        warn!(error = %e, "stord not reachable, staying in HostReady");
                    }
                }
            }
            NodeState::StorageReady => {
                // Verify nwd can respond, then advance to NetworkReady
                match NwdClient::connect(&self.nwd_socket).await {
                    Ok(mut nwd) => match nwd.list_namespace_state().await {
                        Ok(_) => {
                            self.transition_state(NodeState::NetworkReady).await?;
                        }
                        Err(e) => {
                            warn!(error = %e, "nwd list_namespace_state failed, staying in StorageReady");
                        }
                    },
                    Err(e) => {
                        warn!(error = %e, "nwd not reachable, staying in StorageReady");
                    }
                }
            }
            NodeState::NetworkReady => {
                // Compound readiness: verify both storage and network daemons are
                // healthy before advancing to TenantReady. This prevents entering
                // the fully-operational state when one subsystem has regressed.
                let stord_ok = match StordClient::connect(&self.stord_socket).await {
                    Ok(mut c) => c.health_probe().await.unwrap_or(false),
                    Err(_) => false,
                };
                let nwd_ok = match NwdClient::connect(&self.nwd_socket).await {
                    Ok(mut c) => c.health_probe().await.unwrap_or(false),
                    Err(_) => false,
                };
                if stord_ok && nwd_ok {
                    self.transition_state(NodeState::TenantReady).await?;
                } else {
                    warn!(
                        stord_ok = stord_ok,
                        nwd_ok = nwd_ok,
                        "compound readiness check failed, staying in NetworkReady"
                    );
                }
            }
            NodeState::TenantReady => {
                let net_ok = self.reconcile_networks().await.is_ok();
                let vol_ok = self.reconcile_volumes().await.is_ok();
                let vm_ok = self.reconcile_vms().await.is_ok();
                if net_ok && vol_ok && vm_ok {
                    self.degraded_ticks = 0;
                } else {
                    self.degraded_ticks += 1;
                    warn!(
                        net_ok = net_ok,
                        vol_ok = vol_ok,
                        vm_ok = vm_ok,
                        degraded_ticks = self.degraded_ticks,
                        "reconcile failure detected"
                    );
                    if self.degraded_ticks >= 3 {
                        self.transition_state(NodeState::Degraded).await?;
                    }
                }
            }
            NodeState::Failed => {
                // Attempt recovery: if both daemons are healthy, restart bootstrap sequence
                let stord_ok = match StordClient::connect(&self.stord_socket).await {
                    Ok(mut c) => c.health_probe().await.unwrap_or(false),
                    Err(_) => false,
                };
                let nwd_ok = match NwdClient::connect(&self.nwd_socket).await {
                    Ok(mut c) => c.health_probe().await.unwrap_or(false),
                    Err(_) => false,
                };
                if stord_ok && nwd_ok {
                    info!("recovered from Failed, transitioning to HostReady");
                    self.transition_state(NodeState::HostReady).await?;
                } else {
                    warn!(stord_ok, nwd_ok, "remaining in Failed, daemons not healthy");
                }
            }
            NodeState::Degraded => {
                self.degraded_ticks += 1;
                // Probe daemons to check if recovery is possible
                let stord_ok = match StordClient::connect(&self.stord_socket).await {
                    Ok(mut c) => c.health_probe().await.unwrap_or(false),
                    Err(_) => false,
                };
                let nwd_ok = match NwdClient::connect(&self.nwd_socket).await {
                    Ok(mut c) => c.health_probe().await.unwrap_or(false),
                    Err(_) => false,
                };
                if stord_ok && nwd_ok {
                    info!("recovered from Degraded, transitioning to TenantReady");
                    self.transition_state(NodeState::TenantReady).await?;
                    self.degraded_ticks = 0;
                } else if self.degraded_ticks >= 30 {
                    // ~5 minutes at 10s tick interval — unrecoverable
                    error!(
                        degraded_ticks = self.degraded_ticks,
                        stord_ok, nwd_ok, "node degraded too long, transitioning to Failed"
                    );
                    self.transition_state(NodeState::Failed).await?;
                    self.degraded_ticks = 0;
                } else {
                    warn!(
                        degraded_ticks = self.degraded_ticks,
                        stord_ok, nwd_ok, "node degraded, waiting for recovery"
                    );
                }
            }
            NodeState::Draining => {
                // Evacuate running VMs by requesting migration to the control plane.
                let running_vms: Vec<String> = self
                    .vm_runtime
                    .list()
                    .await
                    .into_iter()
                    .filter(|r| r.runtime_status == "Running" || r.runtime_status == "Created")
                    .map(|r| r.vm_id)
                    .collect();

                if running_vms.is_empty() && self.migration_registry.is_empty() {
                    // All VMs evacuated and no in-flight disk migrations — safe to Maintenance
                    info!(
                        operation_id = %operation_id,
                        "drain complete, all VMs evacuated and no in-flight migrations — transitioning to Maintenance"
                    );
                    self.drain_requested_vms.clear();
                    self.transition_state(NodeState::Maintenance).await?;
                } else {
                    // Request migration for each running VM via control plane event,
                    // but only if we haven't already requested it.
                    let node_id = {
                        let cache = self.cache.lock().await;
                        cache.node_id.clone()
                    };
                    for vm_id in &running_vms {
                        if self.drain_requested_vms.contains(vm_id) {
                            debug!(vm_id = %vm_id, "drain migration already requested, skipping");
                            continue;
                        }
                        info!(vm_id = %vm_id, operation_id = %operation_id, "requesting evacuation migration for drain");
                        let event =
                            control_plane_node_api::control_plane_node_api::PublishEventRequest {
                                meta: Some(
                                    control_plane_node_api::control_plane_node_api::RequestMeta {
                                        operation_id: format!(
                                            "drain-migrate-{}-{}",
                                            vm_id, self.reconcile_tick
                                        ),
                                        requested_by: "agent".to_string(),
                                        target_node_id: node_id.clone(),
                                        desired_state_version: String::new(),
                                        request_unix_ms: chv_common::now_unix_ms(),
                                    },
                                ),
                                node_id: node_id.clone(),
                                severity: "Warning".to_string(),
                                event_type: "DrainMigrateRequest".to_string(),
                                summary: format!(
                                    "node drain: requesting migration for VM {}",
                                    vm_id
                                ),
                                details_json: serde_json::to_vec(&serde_json::json!({
                                    "vm_id": vm_id,
                                    "reason": "node_drain"
                                }))
                                .unwrap_or_default(),
                            };
                        let mut cache = self.cache.lock().await;
                        cache.enqueue_pending_message(
                            crate::cache::PendingControlPlaneMessage::event(event),
                        );
                        drop(cache);
                        self.drain_requested_vms.insert(vm_id.clone());
                    }
                    debug!(
                        remaining_vms = running_vms.len(),
                        operation_id = %operation_id,
                        "drain in progress — waiting for VM evacuation"
                    );
                }
            }
            NodeState::Maintenance => {}
        }

        Ok(())
    }

    async fn reconcile_networks(&mut self) -> Result<(), ChvError> {
        // Build a map of network_id -> cidr from network fragments (spec_json).
        // Falls back to the hardcoded default if the fragment has no cidr.
        const DEFAULT_CIDR: &str = "10.0.0.0/24";

        let (
            desired_networks,
            network_cidrs,
            network_gateways,
            network_bridges,
            network_firewall_rules,
            network_nat_enabled,
            network_nat_rules,
            network_dhcp_enabled,
            network_dhcp_scopes,
            network_dns_enabled,
            network_dns_scopes,
        ) = {
            let cache = self.cache.lock().await;
            let mut desired_networks: BTreeSet<String> =
                cache.vm_network_ids().into_iter().collect();
            info!(desired_networks = ?desired_networks, "reconcile_networks: desired networks");
            desired_networks.extend(cache.network_fragments.keys().cloned());

            let mut network_cidrs: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            let mut network_gateways: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            let mut network_bridges: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            let mut network_firewall_rules: std::collections::HashMap<String, serde_json::Value> =
                std::collections::HashMap::new();
            let mut network_nat_enabled: std::collections::HashMap<String, bool> =
                std::collections::HashMap::new();
            let mut network_nat_rules: std::collections::HashMap<String, serde_json::Value> =
                std::collections::HashMap::new();
            let mut network_dhcp_enabled: std::collections::HashMap<String, bool> =
                std::collections::HashMap::new();
            let mut network_dhcp_scopes: std::collections::HashMap<String, serde_json::Value> =
                std::collections::HashMap::new();
            let mut network_dns_enabled: std::collections::HashMap<String, bool> =
                std::collections::HashMap::new();
            let mut network_dns_scopes: std::collections::HashMap<String, serde_json::Value> =
                std::collections::HashMap::new();
            for (net_id, frag) in &cache.network_fragments {
                let spec = match serde_json::from_slice::<serde_json::Value>(&frag.spec_json) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warn!(
                            network_id = %net_id,
                            error = %e,
                            fragment = %String::from_utf8_lossy(&frag.spec_json),
                            "failed to parse network spec_json, falling back to defaults (bridge=br-<id>, cidr=10.0.0.0/24)"
                        );
                        None
                    }
                };
                let cidr = spec
                    .as_ref()
                    .and_then(|v| {
                        v.get("cidr")
                            .and_then(|c| c.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| DEFAULT_CIDR.to_string());
                let gateway = spec
                    .as_ref()
                    .and_then(|v| {
                        v.get("gateway")
                            .and_then(|c| c.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_default();
                let bridge = spec
                    .as_ref()
                    .and_then(|v| {
                        v.get("bridge_name")
                            .and_then(|c| c.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| bridge_name_for_network(net_id));
                if let Some(rules) = spec.as_ref().and_then(|v| v.get("firewall_rules").cloned()) {
                    network_firewall_rules.insert(net_id.clone(), rules);
                }
                if let Some(enabled) = spec
                    .as_ref()
                    .and_then(|v| v.get("nat_enabled"))
                    .and_then(|v| v.as_bool())
                {
                    network_nat_enabled.insert(net_id.clone(), enabled);
                }
                if let Some(rules) = spec.as_ref().and_then(|v| v.get("nat_rules").cloned()) {
                    network_nat_rules.insert(net_id.clone(), rules);
                }
                if let Some(enabled) = spec
                    .as_ref()
                    .and_then(|v| v.get("dhcp_enabled"))
                    .and_then(|v| v.as_bool())
                {
                    network_dhcp_enabled.insert(net_id.clone(), enabled);
                }
                if let Some(scope) = spec.as_ref().and_then(|v| v.get("dhcp_scope").cloned()) {
                    network_dhcp_scopes.insert(net_id.clone(), scope);
                }
                if let Some(enabled) = spec
                    .as_ref()
                    .and_then(|v| v.get("dns_enabled"))
                    .and_then(|v| v.as_bool())
                {
                    network_dns_enabled.insert(net_id.clone(), enabled);
                }
                if let Some(scope) = spec.as_ref().and_then(|v| v.get("dns_scope").cloned()) {
                    network_dns_scopes.insert(net_id.clone(), scope);
                }
                network_cidrs.insert(net_id.clone(), cidr);
                network_gateways.insert(net_id.clone(), gateway);
                network_bridges.insert(net_id.clone(), bridge);
            }

            (
                desired_networks,
                network_cidrs,
                network_gateways,
                network_bridges,
                network_firewall_rules,
                network_nat_enabled,
                network_nat_rules,
                network_dhcp_enabled,
                network_dhcp_scopes,
                network_dns_enabled,
                network_dns_scopes,
            )
        };
        // Cache lock is dropped here — all subsequent operations are lock-free async I/O.

        let mut nwd = NwdClient::connect(&self.nwd_socket).await?;
        for net_id in &desired_networks {
            let bridge = network_bridges
                .get(net_id)
                .cloned()
                .unwrap_or_else(|| bridge_name_for_network(net_id));
            let cidr = network_cidrs
                .get(net_id)
                .map(|s| s.as_str())
                .unwrap_or(DEFAULT_CIDR);
            let gateway = network_gateways
                .get(net_id)
                .map(|s| s.as_str())
                .unwrap_or("");
            let op_id = format!("reconcile-network-ensure-{}", net_id);
            info!(network_id = %net_id, bridge = %bridge, "reconcile_networks: calling ensure_network_topology");
            if let Err(e) = nwd
                .ensure_network_topology(net_id, &bridge, cidr, gateway, Some(&op_id))
                .await
            {
                warn!(network_id = %net_id, error = %e, "failed to ensure network topology");
            } else {
                info!(network_id = %net_id, bridge = %bridge, "reconcile_networks: ensure_network_topology succeeded");
                // Network health check (Sprint 11 A1)
                match nwd.get_network_health(net_id).await {
                    Ok(health) => {
                        if health.health_status != "healthy" && health.health_status != "ok" {
                            warn!(network_id = %net_id, health_status = %health.health_status, last_error = %health.last_error, "network health degraded");
                        }
                    }
                    Err(e) => {
                        warn!(network_id = %net_id, error = %e, "failed to get network health");
                    }
                }
                // Firewall policy
                if let Some(rules) = network_firewall_rules.get(net_id) {
                    let fw_op_id = format!("{}-firewall", op_id);
                    match serde_json::to_vec(rules) {
                        Ok(policy_json) => {
                            if let Err(e) = nwd
                                .set_firewall_policy(net_id, "v1", policy_json, Some(&fw_op_id))
                                .await
                            {
                                warn!(network_id = %net_id, error = %e, "failed to set firewall policy");
                            }
                        }
                        Err(e) => {
                            warn!(network_id = %net_id, error = %e, "failed to serialize firewall rules, skipping policy update");
                        }
                    }
                }
                // NAT policy
                if network_nat_enabled.get(net_id).copied().unwrap_or(false) {
                    let nat_op_id = format!("{}-nat", op_id);
                    let policy_json = match network_nat_rules.get(net_id) {
                        Some(v) => match serde_json::to_vec(v) {
                            Ok(json) => Some(json),
                            Err(e) => {
                                warn!(network_id = %net_id, error = %e, "failed to serialize NAT rules, skipping policy update");
                                None
                            }
                        },
                        None => Some(Vec::new()),
                    };
                    if let Some(policy_json) = policy_json {
                        if let Err(e) = nwd
                            .set_nat_policy(net_id, "v1", policy_json, Some(&nat_op_id))
                            .await
                        {
                            warn!(network_id = %net_id, error = %e, "failed to set nat policy");
                        }
                    }
                }
                // DHCP scope
                if network_dhcp_enabled.get(net_id).copied().unwrap_or(false) {
                    if let Some(scope) = network_dhcp_scopes.get(net_id) {
                        let dhcp_op_id = format!("{}-dhcp", op_id);
                        let range_start = scope
                            .get("range_start")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let range_end = scope
                            .get("range_end")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let dns_servers: Vec<String> = scope
                            .get("dns_servers")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        if let Err(e) = nwd
                            .ensure_dhcp_scope(
                                net_id,
                                cidr,
                                range_start,
                                range_end,
                                dns_servers,
                                Some(&dhcp_op_id),
                            )
                            .await
                        {
                            warn!(network_id = %net_id, error = %e, "failed to ensure dhcp scope");
                        }
                    }
                }
                // DNS scope
                if network_dns_enabled.get(net_id).copied().unwrap_or(false) {
                    if let Some(scope) = network_dns_scopes.get(net_id) {
                        let dns_op_id = format!("{}-dns", op_id);
                        let forwarders: Vec<String> = scope
                            .get("forwarders")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let static_records: std::collections::HashMap<String, String> = scope
                            .get("static_records")
                            .and_then(|v| v.as_object())
                            .map(|obj| {
                                obj.iter()
                                    .filter_map(|(k, v)| {
                                        v.as_str().map(|s| (k.clone(), s.to_string()))
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        if let Err(e) = nwd
                            .ensure_dns_scope(net_id, forwarders, static_records, Some(&dns_op_id))
                            .await
                        {
                            warn!(network_id = %net_id, error = %e, "failed to ensure dns scope");
                        }
                    }
                }
            }
        }

        let actual = nwd.list_namespace_state().await?;
        for state in actual.items {
            if !desired_networks.contains(&state.network_id) {
                // TODO(Sprint 11): Wire withdraw_service_exposure for exposures removed from desired state before deleting topology.
                let op_id = format!("reconcile-network-delete-{}", state.network_id);
                if let Err(e) = nwd
                    .delete_network_topology(&state.network_id, Some(&op_id))
                    .await
                {
                    warn!(network_id = %state.network_id, error = %e, "failed to delete orphan network topology");
                }
            }
        }
        Ok(())
    }

    async fn reconcile_volumes(&mut self) -> Result<(), ChvError> {
        let (pairs, cached_handles, volume_fragments) = {
            let cache = self.cache.lock().await;
            let pairs: HashSet<(String, String)> = cache.vm_volume_handles().into_iter().collect();
            let cached_handles = cache.volume_handles.clone();
            let volume_fragments = cache.volume_fragments.clone();
            (pairs, cached_handles, volume_fragments)
        };
        let needs_stord = !pairs.is_empty() || !volume_fragments.is_empty();
        if !needs_stord {
            return Ok(());
        }
        let mut stord = match StordClient::connect(&self.stord_socket).await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "failed to connect to stord, skipping volume reconcile");
                return Ok(());
            }
        };
        for (vm_id, volume_id) in pairs {
            // Only re-attach volumes that were previously opened (cached).
            // Fresh volumes will be opened (with seed_from + size_bytes) by prepare_vm in reconcile_vms.
            // Without this guard, open_volume with no options defaults size_bytes to 10 GiB
            // and creates an empty sparse volume, skipping seeding when prepare_vm runs later.
            if !cached_handles.contains_key(&volume_id) {
                continue;
            }
            let locator = format!("{}.img", volume_id);
            let op_id = format!("reconcile-volume-attach-{}-{}", vm_id, volume_id);
            match stord
                .open_volume(&volume_id, "local", &locator, Some(&op_id))
                .await
            {
                Ok((_, handle, _)) => {
                    if let Err(e) = stord
                        .attach_volume_to_vm(&volume_id, &vm_id, &handle, Some(&op_id))
                        .await
                    {
                        warn!(volume_id = %volume_id, vm_id = %vm_id, error = %e, "failed to attach volume");
                    } else {
                        // Volume health check (Sprint 11 A1)
                        match stord.get_volume_health(&volume_id).await {
                            Ok(health) => {
                                if health.health_status != "healthy" && health.health_status != "ok"
                                {
                                    warn!(volume_id = %volume_id, health_status = %health.health_status, last_error = %health.last_error, "volume health degraded");
                                }
                            }
                            Err(e) => {
                                warn!(volume_id = %volume_id, error = %e, "failed to get volume health");
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(volume_id = %volume_id, error = %e, "failed to open volume during reconcile");
                }
            }
        }

        // Snapshot and clone operations from volume desired state fragments.
        for (volume_id, fragment) in &volume_fragments {
            let spec = match std::str::from_utf8(&fragment.spec_json) {
                Ok(r) => r,
                Err(e) => {
                    warn!(volume_id = %volume_id, error = %e, "failed to decode volume_fragment spec_json as utf-8");
                    continue;
                }
            };
            let spec = match serde_json::from_str::<serde_json::Value>(spec) {
                Ok(s) => s,
                Err(e) => {
                    warn!(volume_id = %volume_id, error = %e, "failed to parse volume_fragment spec_json");
                    continue;
                }
            };

            // Snapshot operations
            if let Some(snapshot_op) = spec.get("snapshot_op").and_then(|v| v.as_str()) {
                let snapshot_name = spec
                    .get("snapshot_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(volume_id);
                let op_id = format!("reconcile-volume-snapshot-{}-{}", volume_id, snapshot_op);
                let result = match snapshot_op {
                    "create" => {
                        stord
                            .prepare_snapshot(volume_id, snapshot_name, Some(&op_id))
                            .await
                    }
                    "restore" => {
                        stord
                            .restore_snapshot(volume_id, snapshot_name, Some(&op_id))
                            .await
                    }
                    "delete" => {
                        stord
                            .delete_snapshot(volume_id, snapshot_name, Some(&op_id))
                            .await
                    }
                    _ => {
                        warn!(volume_id = %volume_id, snapshot_op = %snapshot_op, "unknown snapshot_op");
                        continue;
                    }
                };
                if let Err(e) = result {
                    warn!(volume_id = %volume_id, snapshot_op = %snapshot_op, error = %e, "snapshot operation failed");
                } else {
                    info!(volume_id = %volume_id, snapshot_op = %snapshot_op, "snapshot operation succeeded");
                    let mut cache = self.cache.lock().await;
                    if let Some(frag) = cache.volume_fragments.get_mut(volume_id) {
                        if let Ok(mut spec) =
                            serde_json::from_slice::<serde_json::Value>(&frag.spec_json)
                        {
                            if let Some(obj) = spec.as_object_mut() {
                                obj.remove("snapshot_op");
                                obj.remove("snapshot_name");
                            }
                            if let Ok(bytes) = serde_json::to_vec(&spec) {
                                frag.spec_json = bytes;
                            }
                        }
                    }
                }
            }

            // Clone operations
            if let Some(source_volume_id) =
                spec.get("clone_source_volume_id").and_then(|v| v.as_str())
            {
                let op_id = format!(
                    "reconcile-volume-clone-{}-from-{}",
                    volume_id, source_volume_id
                );
                if let Err(e) = stord
                    .prepare_clone(source_volume_id, volume_id, Some(&op_id))
                    .await
                {
                    warn!(volume_id = %volume_id, source_volume_id = %source_volume_id, error = %e, "clone operation failed");
                } else {
                    info!(volume_id = %volume_id, source_volume_id = %source_volume_id, "clone operation succeeded");
                    let mut cache = self.cache.lock().await;
                    if let Some(frag) = cache.volume_fragments.get_mut(volume_id) {
                        if let Ok(mut spec) =
                            serde_json::from_slice::<serde_json::Value>(&frag.spec_json)
                        {
                            if let Some(obj) = spec.as_object_mut() {
                                obj.remove("clone_source_volume_id");
                            }
                            if let Ok(bytes) = serde_json::to_vec(&spec) {
                                frag.spec_json = bytes;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Per-VM volume-and-NIC preparation, factored out so it can run inside the
/// parallel reconcile fan-out without borrowing `&mut self`.
///
/// Takes the cache as an owned `Arc` so it can be cloned cheaply per parallel
/// slot. The stord/nwd clients are passed by `&mut` because each slot owns its
/// own short-lived clients in the parallel section.
async fn prepare_vm_resources(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    runtime_dir: &Path,
    stord: &mut StordClient,
    nwd: &mut NwdClient,
    vm_id: &str,
    vm_spec: &crate::spec::VmSpec,
    operation_id: &str,
) -> Result<VmConfig, ChvError> {
    let vm_dir = vm_runtime_dir(runtime_dir, vm_id);
    tokio::fs::create_dir_all(&vm_dir)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to create vm dir: {}", e),
        })?;

    let mut disks = Vec::new();
    let mut volume_ids = Vec::new();
    for disk in &vm_spec.disks {
        let open_op_id = format!("{}-open-volume-{}", operation_id, disk.volume_id);
        let mut open_options = std::collections::HashMap::new();
        if let Some(size_bytes) = disk.size_bytes {
            open_options.insert("size_bytes".to_string(), size_bytes.to_string());
        }
        if let Some(seed_from) = vm_spec
            .disk_seed_path
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            open_options.insert("seed_from".to_string(), seed_from.to_string());
        }
        let disk_path = vm_dir.join(format!("{}.img", disk.volume_id));
        tracing::info!(
            vm_id = %vm_id,
            volume_id = %disk.volume_id,
            locator = %disk_path.display(),
            "opening volume via stord"
        );
        let (_volume_id, handle, export_path) = stord
            .open_volume_with_options(
                &disk.volume_id,
                "local",
                &disk_path.to_string_lossy(),
                open_options,
                Some(&open_op_id),
            )
            .await?;
        tracing::info!(
            vm_id = %vm_id,
            volume_id = %disk.volume_id,
            export_path = %export_path,
            "stord returned export path"
        );
        stord
            .attach_volume_to_vm(&disk.volume_id, vm_id, &handle, Some(&open_op_id))
            .await?;
        // TODO(Sprint 11): Wire set_device_policy when DiskSpec includes device policy configuration.
        disks.push(VmDiskConfig {
            path: PathBuf::from(export_path),
            read_only: disk.read_only,
            id: Some(disk.volume_id.clone()),
        });
        volume_ids.push(disk.volume_id.clone());
        let mut cache_guard = cache.lock().await;
        cache_guard
            .volume_handles
            .insert(disk.volume_id.clone(), handle);
    }

    const DEFAULT_NIC_CIDR: &str = "10.0.0.0/24";
    let mut nics = Vec::new();
    let mut nic_attachments = Vec::new();
    for nic in &vm_spec.nics {
        let nic_id = format!("{}-{}", vm_id, nic.network_id);
        let nic_op_id = format!("{}-attach-nic-{}", operation_id, nic_id);
        let nic_cidr = if nic.cidr.is_empty() {
            DEFAULT_NIC_CIDR.to_string()
        } else {
            nic.cidr.clone()
        };
        let nic_gateway = nic.gateway.clone();
        let bridge = if nic.network_id == "default" {
            "chvbr0".to_string()
        } else {
            bridge_name_for_network(&nic.network_id)
        };
        if let Err(e) = nwd
            .ensure_network_topology(
                &nic.network_id,
                &bridge,
                &nic_cidr,
                &nic_gateway,
                Some(&nic_op_id),
            )
            .await
        {
            warn!(network_id = %nic.network_id, error = %e, "failed to ensure network topology");
        }
        let nic_result = nwd
            .attach_vm_nic(
                &nic_id,
                vm_id,
                &nic.network_id,
                &nic.mac_address,
                &nic.ip_address,
                Some(&nic_op_id),
            )
            .await;
        if let Err(e) = nic_result {
            // Clean up already-opened volumes before propagating the error
            let cleanup_op_id = format!("{}-cleanup-nic-fail", operation_id);
            let handles: Vec<(String, Option<String>)> = {
                let cache_guard = cache.lock().await;
                volume_ids
                    .iter()
                    .map(|vid| (vid.clone(), cache_guard.volume_handles.get(vid).cloned()))
                    .collect()
            };
            for (vol_id, handle) in &handles {
                if let Err(cleanup_err) = stord
                    .detach_volume_from_vm(vol_id, vm_id, false, Some(&cleanup_op_id))
                    .await
                {
                    tracing::warn!(
                        volume_id = %vol_id,
                        error = %cleanup_err,
                        "failed to detach volume after NIC attach failure"
                    );
                }
                if let Some(h) = handle {
                    if let Err(cleanup_err) =
                        stord.close_volume(vol_id, h, Some(&cleanup_op_id)).await
                    {
                        tracing::warn!(
                            volume_id = %vol_id,
                            error = %cleanup_err,
                            "failed to close volume after NIC attach failure"
                        );
                    }
                }
            }
            return Err(e);
        }
        let (_namespace_handle, tap_handle) = nic_result.unwrap();
        nics.push(VmNicConfig {
            network_id: nic.network_id.clone(),
            mac_address: nic.mac_address.clone(),
            ip_address: nic.ip_address.clone(),
            tap_name: tap_handle,
            cidr: nic.cidr.clone(),
            gateway: nic.gateway.clone(),
        });
        nic_attachments.push(VmNicAttachment {
            nic_id,
            network_id: nic.network_id.clone(),
        });
    }

    if !volume_ids.is_empty() || !nic_attachments.is_empty() {
        let mut cache_guard = cache.lock().await;
        cache_guard.observe_vm_attachment(vm_id, &volume_ids, &nic_attachments);
    }

    Ok(VmConfig {
        vm_id: vm_id.to_string(),
        cpus: vm_spec.cpus,
        memory_bytes: vm_spec.memory_bytes,
        kernel_path: PathBuf::from(&vm_spec.kernel_path),
        firmware_path: vm_spec.firmware_path.as_ref().map(PathBuf::from),
        disks,
        nics,
        api_socket_path: vm_dir.join("vm.sock"),
        cloud_init_userdata: vm_spec.cloud_init_userdata.clone(),
        hypervisor_overrides: vm_spec.hypervisor_overrides.clone(),
    })
}

impl Reconciler {
    async fn reconcile_vms(&mut self) -> Result<(), ChvError> {
        let (desired, actual) = {
            let cache = self.cache.lock().await;
            let desired: BTreeSet<String> = cache.vm_fragments.keys().cloned().collect();
            let actual: BTreeSet<String> = self
                .vm_runtime
                .list()
                .await
                .into_iter()
                .map(|r| r.vm_id)
                .collect();
            (desired, actual)
        };

        // Probe stord/nwd connectivity once before fanning out: if either
        // daemon is unreachable, every per-VM future would fail to even
        // connect, so we short-circuit the entire tick. The probe connections
        // are dropped immediately; each parallel slot opens its own.
        match StordClient::connect(&self.stord_socket).await {
            Ok(_) => {}
            Err(e) => {
                warn!(error = %e, "failed to connect to stord, skipping vm reconcile");
                return Ok(());
            }
        };
        match NwdClient::connect(&self.nwd_socket).await {
            Ok(_) => {}
            Err(e) => {
                warn!(error = %e, "failed to connect to nwd, skipping vm reconcile");
                return Ok(());
            }
        };

        let reconcile_tick = self.reconcile_tick;

        // ---- CREATE: missing VMs ---------------------------------------
        // Snapshot all per-VM inputs we'll need under a single lock, so the
        // parallel section never re-acquires the cache lock for reads.
        let create_inputs: Vec<CreateInput> = {
            let cache = self.cache.lock().await;
            desired
                .difference(&actual)
                .filter_map(|vm_id| {
                    let fragment = cache.vm_fragments.get(vm_id)?;
                    Some(CreateInput {
                        vm_id: vm_id.clone(),
                        generation: fragment.generation.clone(),
                        spec_raw: fragment.spec_json.clone(),
                    })
                })
                .collect()
        };

        if !create_inputs.is_empty() {
            let cache = self.cache.clone();
            let vm_runtime = self.vm_runtime.clone();
            let stord_socket: Arc<PathBuf> = Arc::new(self.stord_socket.clone());
            let nwd_socket: Arc<PathBuf> = Arc::new(self.nwd_socket.clone());
            let runtime_dir: Arc<PathBuf> = Arc::new(self.runtime_dir.clone());

            let _: Vec<()> = stream::iter(create_inputs)
                .map(|input| {
                    let cache = cache.clone();
                    let vm_runtime = vm_runtime.clone();
                    let stord_socket = stord_socket.clone();
                    let nwd_socket = nwd_socket.clone();
                    let runtime_dir = runtime_dir.clone();
                    async move {
                        create_one_vm(
                            input,
                            cache,
                            vm_runtime,
                            stord_socket,
                            nwd_socket,
                            runtime_dir,
                            reconcile_tick,
                        )
                        .await
                    }
                })
                .buffer_unordered(VM_RECONCILE_CONCURRENCY)
                .collect()
                .await;
        }

        // ---- DELETE: extra VMs (in actual, not in desired) -------------
        let delete_ids: Vec<String> = actual.difference(&desired).cloned().collect();
        let mut delete_results: Vec<DeleteResult> = Vec::new();
        if !delete_ids.is_empty() {
            let cache = self.cache.clone();
            let vm_runtime = self.vm_runtime.clone();
            let stord_socket: Arc<PathBuf> = Arc::new(self.stord_socket.clone());
            let nwd_socket: Arc<PathBuf> = Arc::new(self.nwd_socket.clone());
            let runtime_dir: Arc<PathBuf> = Arc::new(self.runtime_dir.clone());

            delete_results = stream::iter(delete_ids)
                .map(|vm_id| {
                    let cache = cache.clone();
                    let vm_runtime = vm_runtime.clone();
                    let stord_socket = stord_socket.clone();
                    let nwd_socket = nwd_socket.clone();
                    let runtime_dir = runtime_dir.clone();
                    async move {
                        delete_one_vm(
                            vm_id,
                            cache,
                            vm_runtime,
                            stord_socket,
                            nwd_socket,
                            runtime_dir,
                        )
                        .await
                    }
                })
                .buffer_unordered(VM_RECONCILE_CONCURRENCY)
                .collect()
                .await;
        }

        // Apply the deferred cache mutations from delete loop under one lock.
        if !delete_results.is_empty() {
            let mut cache = self.cache.lock().await;
            for r in &delete_results {
                if r.remove_state {
                    cache.remove_vm_state(&r.vm_id);
                }
            }
        }

        // ---- RECONCILE: existing VMs (in both desired and actual) ------
        let reconcile_inputs: Vec<ReconcileInput> = {
            let cache = self.cache.lock().await;
            desired
                .intersection(&actual)
                .filter_map(|vm_id| {
                    let fragment = cache.vm_fragments.get(vm_id)?;
                    Some(ReconcileInput {
                        vm_id: vm_id.clone(),
                        generation: fragment.generation.clone(),
                        spec_raw: fragment.spec_json.clone(),
                    })
                })
                .collect()
        };

        let mut reconcile_results: Vec<ReconcileResult> = Vec::new();
        if !reconcile_inputs.is_empty() {
            let cache = self.cache.clone();
            let vm_runtime = self.vm_runtime.clone();
            let stord_socket: Arc<PathBuf> = Arc::new(self.stord_socket.clone());
            let nwd_socket: Arc<PathBuf> = Arc::new(self.nwd_socket.clone());
            let runtime_dir: Arc<PathBuf> = Arc::new(self.runtime_dir.clone());

            reconcile_results = stream::iter(reconcile_inputs)
                .map(|input| {
                    let cache = cache.clone();
                    let vm_runtime = vm_runtime.clone();
                    let stord_socket = stord_socket.clone();
                    let nwd_socket = nwd_socket.clone();
                    let runtime_dir = runtime_dir.clone();
                    async move {
                        reconcile_one_vm(
                            input,
                            cache,
                            vm_runtime,
                            stord_socket,
                            nwd_socket,
                            runtime_dir,
                            reconcile_tick,
                        )
                        .await
                    }
                })
                .buffer_unordered(VM_RECONCILE_CONCURRENCY)
                .collect()
                .await;
        }

        // Apply the deferred cache mutations from the existing-VM loop's
        // "Deleted" branch under one lock.
        if !reconcile_results.is_empty() {
            let mut cache = self.cache.lock().await;
            for r in &reconcile_results {
                if r.remove_fragment {
                    cache.vm_fragments.remove(&r.vm_id);
                }
                if r.remove_state {
                    cache.remove_vm_state(&r.vm_id);
                }
            }
        }

        Ok(())
    }
}

/// Snapshot of per-VM input for the parallel CREATE pass.
struct CreateInput {
    vm_id: String,
    generation: String,
    spec_raw: Vec<u8>,
}

/// Snapshot of per-VM input for the parallel RECONCILE pass over existing VMs.
struct ReconcileInput {
    vm_id: String,
    generation: String,
    spec_raw: Vec<u8>,
}

/// Per-VM cache mutations to apply serially after the parallel DELETE pass.
struct DeleteResult {
    vm_id: String,
    remove_state: bool,
}

/// Per-VM cache mutations to apply serially after the parallel RECONCILE pass.
struct ReconcileResult {
    vm_id: String,
    remove_fragment: bool,
    remove_state: bool,
}

/// Per-VM CREATE worker. Runs in parallel with up to
/// `VM_RECONCILE_CONCURRENCY-1` peers. Opens its own short-lived stord/nwd
/// clients so it does not contend with sibling slots on a shared connection.
async fn create_one_vm(
    input: CreateInput,
    cache: Arc<tokio::sync::Mutex<NodeCache>>,
    vm_runtime: VmRuntime,
    stord_socket: Arc<PathBuf>,
    nwd_socket: Arc<PathBuf>,
    runtime_dir: Arc<PathBuf>,
    reconcile_tick: u64,
) {
    let CreateInput {
        vm_id,
        generation,
        spec_raw,
    } = input;

    let span = tracing::info_span!("reconcile_create_vm", vm_id = %vm_id);
    let _enter = span.enter();

    let failures = vm_runtime
        .consecutive_failures_for_generation(&vm_id, &generation)
        .await;
    if should_skip_vm_for_tick(reconcile_tick, failures) {
        log_backoff_skip(&vm_id, failures);
        return;
    }

    let op_id = format!("reconcile-vm-create-{}", vm_id);
    let raw = match std::str::from_utf8(&spec_raw) {
        Ok(r) => r,
        Err(e) => {
            warn!(vm_id = %vm_id, error = %e, "failed to decode vm_fragment spec_json as utf-8");
            vm_runtime
                .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                .await;
            return;
        }
    };
    let spec = match crate::spec::VmSpec::from_json(raw) {
        Ok(s) => s,
        Err(e) => {
            warn!(vm_id = %vm_id, error = %e, "failed to parse vm_fragment spec_json");
            vm_runtime
                .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                .await;
            return;
        }
    };

    let mut stord = match StordClient::connect(stord_socket.as_path()).await {
        Ok(c) => c,
        Err(e) => {
            warn!(vm_id = %vm_id, error = %e, "failed to connect to stord for create");
            vm_runtime
                .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                .await;
            return;
        }
    };
    let mut nwd = match NwdClient::connect(nwd_socket.as_path()).await {
        Ok(c) => c,
        Err(e) => {
            warn!(vm_id = %vm_id, error = %e, "failed to connect to nwd for create");
            vm_runtime
                .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                .await;
            return;
        }
    };

    let config = match prepare_vm_resources(
        &cache,
        runtime_dir.as_path(),
        &mut stord,
        &mut nwd,
        &vm_id,
        &spec,
        &op_id,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            warn!(vm_id = %vm_id, error = %e, "failed to prepare vm");
            vm_runtime
                .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                .await;
            return;
        }
    };
    if let Err(e) = vm_runtime
        .create_vm(vm_id.clone(), generation.clone(), &config, Some(&op_id))
        .await
    {
        warn!(vm_id = %vm_id, error = %e, "failed to create vm");
        vm_runtime
            .record_failure(vm_id.clone(), generation.clone(), e.to_string())
            .await;
        return;
    }
    if spec.desired_state == "Running" {
        let start_op_id = format!("{}-start", op_id);
        if let Err(e) = vm_runtime.start_vm(&vm_id, Some(&start_op_id)).await {
            warn!(vm_id = %vm_id, error = %e, "failed to start vm");
            vm_runtime
                .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                .await;
        }
    }
}

/// Per-VM DELETE worker for VMs that are in actual but not in desired.
/// Returns the cache mutations the caller must apply serially.
async fn delete_one_vm(
    vm_id: String,
    cache: Arc<tokio::sync::Mutex<NodeCache>>,
    vm_runtime: VmRuntime,
    stord_socket: Arc<PathBuf>,
    nwd_socket: Arc<PathBuf>,
    runtime_dir: Arc<PathBuf>,
) -> DeleteResult {
    let span = tracing::info_span!("reconcile_delete_vm", vm_id = %vm_id);
    let _enter = span.enter();

    let op_id = format!("reconcile-vm-delete-{}", vm_id);
    if let Err(e) = vm_runtime.stop_vm(&vm_id, false, Some(&op_id)).await {
        warn!(vm_id = %vm_id, error = %e, "failed to stop vm before delete");
    }
    let cleanup_op_id = format!("reconcile-vm-cleanup-{}", vm_id);
    if let Err(e) = cleanup_vm_resources(
        &cache,
        stord_socket.as_path(),
        nwd_socket.as_path(),
        &vm_id,
        Some(&cleanup_op_id),
    )
    .await
    {
        warn!(vm_id = %vm_id, error = %e, "cleanup vm failed");
    }
    if let Err(e) = vm_runtime.delete_vm(&vm_id, Some(&op_id)).await {
        warn!(vm_id = %vm_id, error = %e, "failed to delete vm");
        return DeleteResult {
            vm_id,
            remove_state: false,
        };
    }
    let vm_dir = vm_runtime_dir(runtime_dir.as_path(), &vm_id);
    let _ = tokio::fs::remove_dir_all(&vm_dir).await;
    vm_runtime.clear_failure_count(&vm_id).await;
    DeleteResult {
        vm_id,
        remove_state: true,
    }
}

/// Per-VM RECONCILE worker for VMs that exist in both desired and actual.
/// Drives the start/stop/recover/resize/delete state machine for one VM and
/// returns the cache mutations the caller must apply serially.
async fn reconcile_one_vm(
    input: ReconcileInput,
    cache: Arc<tokio::sync::Mutex<NodeCache>>,
    vm_runtime: VmRuntime,
    stord_socket: Arc<PathBuf>,
    nwd_socket: Arc<PathBuf>,
    runtime_dir: Arc<PathBuf>,
    reconcile_tick: u64,
) -> ReconcileResult {
    let ReconcileInput {
        vm_id,
        generation,
        spec_raw,
    } = input;

    let span = tracing::info_span!("reconcile_existing_vm", vm_id = %vm_id);
    let _enter = span.enter();

    let mut result = ReconcileResult {
        vm_id: vm_id.clone(),
        remove_fragment: false,
        remove_state: false,
    };

    let failures = vm_runtime
        .consecutive_failures_for_generation(&vm_id, &generation)
        .await;
    if should_skip_vm_for_tick(reconcile_tick, failures) {
        log_backoff_skip(&vm_id, failures);
        return result;
    }
    let raw = match std::str::from_utf8(&spec_raw) {
        Ok(r) => r,
        Err(e) => {
            warn!(vm_id = %vm_id, error = %e, "failed to decode vm_fragment spec_json as utf-8");
            vm_runtime
                .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                .await;
            return result;
        }
    };
    let spec = match crate::spec::VmSpec::from_json(raw) {
        Ok(s) => s,
        Err(e) => {
            warn!(vm_id = %vm_id, error = %e, "failed to parse vm_fragment spec_json");
            vm_runtime
                .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                .await;
            return result;
        }
    };
    let Some(record) = vm_runtime.get(&vm_id).await else {
        warn!(vm_id = %vm_id, "vm runtime record missing during reconcile");
        return result;
    };

    // Recovery path for VMs stuck in "Failed" state.
    // After `should_skip_vm` has gated us here (meaning we are within retry
    // window), attempt to recover by re-creating the VM from scratch.
    if record.runtime_status == "Failed" {
        warn!(vm_id = %vm_id, failures = failures, "VM in Failed state, attempting recovery via re-create");
        if let Err(e) = vm_runtime.delete_vm(&vm_id, None).await {
            warn!(vm_id = %vm_id, error = %e, "delete_vm failed during recovery cleanup");
        }
        let vm_dir = vm_runtime_dir(runtime_dir.as_path(), &vm_id);
        let _ = tokio::fs::remove_file(vm_dir.join("vm.sock")).await;
        let _ = tokio::fs::remove_file(vm_dir.join("console.log")).await;
        let recover_op_id = format!("reconcile-vm-recover-{}", vm_id);

        let mut stord = match StordClient::connect(stord_socket.as_path()).await {
            Ok(c) => c,
            Err(e) => {
                warn!(vm_id = %vm_id, error = %e, "failed to connect to stord for recovery");
                vm_runtime
                    .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                    .await;
                return result;
            }
        };
        let mut nwd = match NwdClient::connect(nwd_socket.as_path()).await {
            Ok(c) => c,
            Err(e) => {
                warn!(vm_id = %vm_id, error = %e, "failed to connect to nwd for recovery");
                vm_runtime
                    .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                    .await;
                return result;
            }
        };

        let config = match prepare_vm_resources(
            &cache,
            runtime_dir.as_path(),
            &mut stord,
            &mut nwd,
            &vm_id,
            &spec,
            &recover_op_id,
        )
        .await
        {
            Ok(c) => c,
            Err(e) => {
                warn!(vm_id = %vm_id, error = %e, "failed to prepare vm for recovery");
                vm_runtime
                    .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                    .await;
                return result;
            }
        };
        if let Err(e) = vm_runtime
            .create_vm(
                vm_id.clone(),
                generation.clone(),
                &config,
                Some(&recover_op_id),
            )
            .await
        {
            warn!(vm_id = %vm_id, error = %e, "failed to re-create vm during recovery");
            vm_runtime
                .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                .await;
            return result;
        }
        if spec.desired_state == "Running" {
            let start_op_id = format!("{}-start", recover_op_id);
            if let Err(e) = vm_runtime.start_vm(&vm_id, Some(&start_op_id)).await {
                warn!(vm_id = %vm_id, error = %e, "failed to start vm after recovery");
                vm_runtime
                    .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                    .await;
            }
        }
        return result;
    }

    if spec.desired_state == "Running" && record.runtime_status != "Running" {
        let op_id = format!("reconcile-vm-start-{}", vm_id);
        if let Err(e) = vm_runtime.start_vm(&vm_id, Some(&op_id)).await {
            let err_str = e.to_string();
            if err_str.contains("No such file or directory")
                || err_str.contains("Connection refused")
                || err_str.contains("not found")
            {
                warn!(vm_id = %vm_id, "CH process dead, re-creating VM");
                let _ = vm_runtime.delete_vm(&vm_id, Some(&op_id)).await;
                let vm_dir = vm_runtime_dir(runtime_dir.as_path(), &vm_id);
                let _ = tokio::fs::remove_file(vm_dir.join("vm.sock")).await;
                let _ = tokio::fs::remove_file(vm_dir.join("console.log")).await;
                let recreate_op_id = format!("reconcile-vm-recreate-{}", vm_id);

                let mut stord = match StordClient::connect(stord_socket.as_path()).await {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(vm_id = %vm_id, error = %e, "failed to connect to stord for re-creation");
                        vm_runtime
                            .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                            .await;
                        return result;
                    }
                };
                let mut nwd = match NwdClient::connect(nwd_socket.as_path()).await {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(vm_id = %vm_id, error = %e, "failed to connect to nwd for re-creation");
                        vm_runtime
                            .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                            .await;
                        return result;
                    }
                };

                let config = match prepare_vm_resources(
                    &cache,
                    runtime_dir.as_path(),
                    &mut stord,
                    &mut nwd,
                    &vm_id,
                    &spec,
                    &recreate_op_id,
                )
                .await
                {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(vm_id = %vm_id, error = %e, "failed to prepare vm for re-creation");
                        vm_runtime
                            .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                            .await;
                        return result;
                    }
                };
                if let Err(e) = vm_runtime
                    .create_vm(
                        vm_id.clone(),
                        generation.clone(),
                        &config,
                        Some(&recreate_op_id),
                    )
                    .await
                {
                    warn!(vm_id = %vm_id, error = %e, "failed to re-create vm");
                    vm_runtime
                        .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                        .await;
                    return result;
                }
                let start_op_id = format!("{}-start", recreate_op_id);
                if let Err(e) = vm_runtime.start_vm(&vm_id, Some(&start_op_id)).await {
                    warn!(vm_id = %vm_id, error = %e, "failed to start re-created vm");
                    vm_runtime
                        .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                        .await;
                }
            } else {
                warn!(vm_id = %vm_id, error = %e, "failed to start vm");
                vm_runtime
                    .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                    .await;
            }
            return result;
        }
    } else if spec.desired_state == "Stopped" && record.runtime_status == "Running" {
        let op_id = format!("reconcile-vm-stop-{}", vm_id);
        if let Err(e) = vm_runtime.stop_vm(&vm_id, false, Some(&op_id)).await {
            warn!(vm_id = %vm_id, error = %e, "failed to stop vm");
            vm_runtime
                .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                .await;
            return result;
        }
    } else if spec.desired_state == "Deleted" {
        let op_id = format!("reconcile-vm-delete-{}", vm_id);
        // Stop the VM if it is still running
        if record.runtime_status == "Running" {
            if let Err(e) = vm_runtime.stop_vm(&vm_id, true, Some(&op_id)).await {
                warn!(vm_id = %vm_id, error = %e, "failed to stop vm before delete");
            }
        }
        // Clean up associated resources (volumes, NICs)
        let cleanup_op_id = format!("reconcile-vm-cleanup-{}", vm_id);
        if let Err(e) = cleanup_vm_resources(
            &cache,
            stord_socket.as_path(),
            nwd_socket.as_path(),
            &vm_id,
            Some(&cleanup_op_id),
        )
        .await
        {
            warn!(vm_id = %vm_id, error = %e, "cleanup vm failed during delete");
        }
        // Remove from runtime
        if let Err(e) = vm_runtime.delete_vm(&vm_id, Some(&op_id)).await {
            warn!(vm_id = %vm_id, error = %e, "failed to delete vm");
            vm_runtime
                .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                .await;
            return result;
        }
        let vm_dir = vm_runtime_dir(runtime_dir.as_path(), &vm_id);
        let _ = tokio::fs::remove_dir_all(&vm_dir).await;
        vm_runtime.clear_failure_count(&vm_id).await;
        // Defer cache mutation: orchestrator will remove fragment and state
        // under one lock acquisition after the parallel section returns.
        result.remove_fragment = true;
        result.remove_state = true;
        return result;
    }

    // Resize if cpus or memory changed
    if record.cpus != spec.cpus || record.memory_bytes != spec.memory_bytes {
        let op_id = format!("reconcile-vm-resize-{}", vm_id);
        if let Err(e) = vm_runtime
            .resize_vm(
                &vm_id,
                Some(spec.cpus),
                Some(spec.memory_bytes),
                Some(&op_id),
            )
            .await
        {
            warn!(vm_id = %vm_id, error = %e, "failed to resize vm");
            vm_runtime
                .record_failure(vm_id.clone(), generation.clone(), e.to_string())
                .await;
            return result;
        }
        // Update stored config so we don't resize every tick
        vm_runtime
            .update_vm_config(&vm_id, spec.cpus, spec.memory_bytes)
            .await;
    }

    result
}

pub(crate) async fn cleanup_vm_resources(
    cache: &Arc<tokio::sync::Mutex<NodeCache>>,
    stord_socket: &Path,
    nwd_socket: &Path,
    vm_id: &str,
    operation_id: Option<&str>,
) -> Result<(), ChvError> {
    let (volumes, nics) = {
        let cache = cache.lock().await;
        let derived_attachments = cache
            .vm_fragments
            .get(vm_id)
            .and_then(|fragment| std::str::from_utf8(&fragment.spec_json).ok())
            .and_then(|raw| crate::spec::VmSpec::from_json(raw).ok())
            .map(|spec| {
                let volume_ids = spec
                    .disks
                    .into_iter()
                    .map(|disk| disk.volume_id)
                    .collect::<Vec<_>>();
                let nics = spec
                    .nics
                    .into_iter()
                    .map(|nic| VmNicAttachment {
                        nic_id: format!("{}-{}", vm_id, nic.network_id),
                        network_id: nic.network_id,
                    })
                    .collect::<Vec<_>>();
                (volume_ids, nics)
            })
            .unwrap_or_default();

        let volume_ids = cache
            .vm_attachment_state(vm_id)
            .map(|state| state.volume_ids.clone())
            .unwrap_or(derived_attachments.0);
        let nics = cache
            .vm_attachment_state(vm_id)
            .map(|state| {
                state
                    .nics
                    .iter()
                    .map(|nic| (nic.nic_id.clone(), nic.network_id.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                derived_attachments
                    .1
                    .into_iter()
                    .map(|nic| (nic.nic_id, nic.network_id))
                    .collect::<Vec<_>>()
            });

        let volumes = volume_ids
            .into_iter()
            .map(|volume_id| {
                let handle = cache.volume_handles.get(&volume_id).cloned();
                (volume_id, handle)
            })
            .collect::<Vec<_>>();

        (volumes, nics)
    };

    let mut first_error: Option<ChvError> = None;

    if !volumes.is_empty() {
        let mut stord = StordClient::connect(stord_socket).await?;
        for (volume_id, handle) in &volumes {
            if let Err(e) = stord
                .detach_volume_from_vm(volume_id, vm_id, false, operation_id)
                .await
            {
                tracing::warn!(volume_id = %volume_id, vm_id = %vm_id, error = %e, "cleanup: detach_volume failed, continuing");
                if first_error.is_none() {
                    first_error = Some(e);
                }
                continue;
            }
            if let Some(handle) = handle {
                if let Err(e) = stord.close_volume(volume_id, handle, operation_id).await {
                    tracing::warn!(volume_id = %volume_id, error = %e, "cleanup: close_volume failed, continuing");
                    if first_error.is_none() {
                        first_error = Some(e);
                    }
                }
            }
        }
    }

    if !nics.is_empty() {
        let mut nwd = NwdClient::connect(nwd_socket).await?;
        for (nic_id, network_id) in &nics {
            if let Err(e) = nwd
                .detach_vm_nic(nic_id, vm_id, network_id, operation_id)
                .await
            {
                tracing::warn!(nic_id = %nic_id, vm_id = %vm_id, error = %e, "cleanup: detach_nic failed, continuing");
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
        }
    }

    let mut cache = cache.lock().await;
    for (volume_id, _) in volumes {
        cache.volume_handles.remove(&volume_id);
    }
    cache.vm_attachments.remove(vm_id);

    match first_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;

    fn test_cache() -> NodeCache {
        use crate::cache::DesiredStateFragment;
        NodeCache {
            node_state: "TenantReady".to_string(),
            vm_fragments: {
                let mut m = HashMap::new();
                m.insert("vm-1".to_string(), DesiredStateFragment {
                    id: "vm-1".to_string(),
                    kind: "vm".to_string(),
                    generation: "1".to_string(),
                    spec_json: br#"{"name":"vm-1","cpus":1,"memory_bytes":1024,"kernel_path":"/dev/null","disks":[{"volume_id":"vol-1"}],"nics":[{"network_id":"net-1","mac_address":"00:00:00:00:00:01","ip_address":"10.0.0.2"}]}"#.to_vec(),
                    policy_json: vec![],
                    updated_at: "2024-01-01T00:00:00Z".to_string(),
                    updated_by: "cp".to_string(),
                });
                m
            },
            ..Default::default()
        }
    }

    fn empty_cache() -> NodeCache {
        NodeCache {
            node_state: "TenantReady".to_string(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn reconciler_skips_when_not_tenant_ready() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = test_cache();
        cache.node_state = "Bootstrapping".to_string();
        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(cache)),
            VmRuntime::new(std::sync::Arc::new(
                chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default(),
            )),
            PathBuf::from("/tmp/fake-stord.sock"),
            PathBuf::from("/tmp/fake-nwd.sock"),
            dir.path().to_path_buf(),
            Arc::new(MigrationTaskRegistry::new()),
        )
        .await;
        assert!(rec.run_once().await.is_ok());
    }

    #[tokio::test]
    async fn reconciler_advances_from_discovered_to_bootstrapping() {
        let dir = tempfile::tempdir().unwrap();
        let cache = NodeCache {
            node_state: "Discovered".to_string(),
            ..Default::default()
        };
        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(cache)),
            VmRuntime::new(std::sync::Arc::new(
                chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default(),
            )),
            PathBuf::from("/tmp/fake-stord-discovered.sock"),
            PathBuf::from("/tmp/fake-nwd-discovered.sock"),
            dir.path().to_path_buf(),
            Arc::new(MigrationTaskRegistry::new()),
        )
        .await;
        assert!(rec.run_once().await.is_ok());
        assert_eq!(rec.current_state().await, NodeState::Bootstrapping);
    }

    #[tokio::test]
    async fn reconciler_uses_latest_cached_node_state() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Arc::new(tokio::sync::Mutex::new(test_cache()));
        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let mut rec = Reconciler::new(
            cache.clone(),
            VmRuntime::new(mock.clone()),
            PathBuf::from("/tmp/fake-stord.sock"),
            PathBuf::from("/tmp/fake-nwd.sock"),
            dir.path().to_path_buf(),
            Arc::new(MigrationTaskRegistry::new()),
        )
        .await;

        {
            let mut cache = cache.lock().await;
            cache.transition_node_state(NodeState::Draining).unwrap();
        }

        assert!(rec.run_once().await.is_ok());
        assert!(mock.vms.lock().unwrap().is_empty());
    }

    // ------------------------------------------------------------------
    // Mock gRPC servers for stord/nwd (used by reconciler_creates_missing_vm)
    // ------------------------------------------------------------------
    use chv_nwd_api::chv_nwd_api::network_service_server::NetworkService;
    use chv_stord_api::chv_stord_api::storage_service_server::StorageService;
    use tonic::{Request, Response, Status};

    #[allow(clippy::result_large_err)]
    fn stord_operation_id(
        meta: Option<chv_stord_api::chv_stord_api::Meta>,
    ) -> Result<String, Status> {
        let operation_id = meta.map(|m| m.operation_id).unwrap_or_default();
        if operation_id.is_empty() {
            Err(Status::invalid_argument("missing operation_id"))
        } else {
            Ok(operation_id)
        }
    }

    #[allow(clippy::result_large_err)]
    fn nwd_operation_id(meta: Option<chv_nwd_api::chv_nwd_api::Meta>) -> Result<String, Status> {
        let operation_id = meta.map(|m| m.operation_id).unwrap_or_default();
        if operation_id.is_empty() {
            Err(Status::invalid_argument("missing operation_id"))
        } else {
            Ok(operation_id)
        }
    }

    struct MockStordOk;
    #[tonic::async_trait]
    impl StorageService for MockStordOk {
        async fn list_volume_sessions(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::ListVolumeSessionsRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::ListVolumeSessionsResponse>, Status>
        {
            Ok(Response::new(
                chv_stord_api::chv_stord_api::ListVolumeSessionsResponse { sessions: vec![] },
            ))
        }
        async fn open_volume(
            &self,
            req: Request<chv_stord_api::chv_stord_api::OpenVolumeRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::OpenVolumeResponse>, Status> {
            let inner = req.into_inner();
            stord_operation_id(inner.meta.clone())?;
            Ok(Response::new(
                chv_stord_api::chv_stord_api::OpenVolumeResponse {
                    result: Some(chv_stord_api::chv_stord_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                    volume_id: inner.volume_id.clone(),
                    attachment_handle: format!("handle-{}", inner.volume_id),
                    export_kind: "local".to_string(),
                    export_path: format!("/tmp/{}.img", inner.volume_id),
                },
            ))
        }
        async fn close_volume(
            &self,
            req: Request<chv_stord_api::chv_stord_api::CloseVolumeRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            stord_operation_id(req.into_inner().meta)?;
            Ok(Response::new(chv_stord_api::chv_stord_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn get_volume_health(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::VolumeHealthRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::VolumeHealthResponse>, Status> {
            Ok(Response::new(
                chv_stord_api::chv_stord_api::VolumeHealthResponse {
                    result: Some(chv_stord_api::chv_stord_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                    volume_id: "".to_string(),
                    health_status: "healthy".to_string(),
                    backend_state: "".to_string(),
                    last_error: "".to_string(),
                },
            ))
        }
        async fn attach_volume_to_vm(
            &self,
            req: Request<chv_stord_api::chv_stord_api::AttachVolumeToVmRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::AttachVolumeToVmResponse>, Status>
        {
            let inner = req.into_inner();
            stord_operation_id(inner.meta.clone())?;
            Ok(Response::new(
                chv_stord_api::chv_stord_api::AttachVolumeToVmResponse {
                    result: Some(chv_stord_api::chv_stord_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                    volume_id: inner.volume_id.clone(),
                    vm_id: inner.vm_id.clone(),
                    export_kind: "local".to_string(),
                    export_path: format!("/tmp/{}.img", inner.volume_id),
                },
            ))
        }
        async fn detach_volume_from_vm(
            &self,
            req: Request<chv_stord_api::chv_stord_api::DetachVolumeFromVmRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            stord_operation_id(req.into_inner().meta)?;
            Ok(Response::new(chv_stord_api::chv_stord_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn resize_volume(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::ResizeVolumeRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Ok(Response::new(chv_stord_api::chv_stord_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn prepare_snapshot(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::PrepareSnapshotRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Ok(Response::new(chv_stord_api::chv_stord_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn prepare_clone(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::PrepareCloneRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Ok(Response::new(chv_stord_api::chv_stord_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn restore_snapshot(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::RestoreSnapshotRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Ok(Response::new(chv_stord_api::chv_stord_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn delete_snapshot(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::DeleteSnapshotRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Ok(Response::new(chv_stord_api::chv_stord_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn set_device_policy(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::SetDevicePolicyRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::Result>, Status> {
            Ok(Response::new(chv_stord_api::chv_stord_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn trigger_disk_migration(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::TriggerDiskMigrationRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::TriggerDiskMigrationResponse>, Status>
        {
            Ok(Response::new(
                chv_stord_api::chv_stord_api::TriggerDiskMigrationResponse {
                    result: Some(chv_stord_api::chv_stord_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                    migration_id: "dm-mock-123".to_string(),
                },
            ))
        }
        async fn get_disk_migration_status(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::GetDiskMigrationStatusRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::GetDiskMigrationStatusResponse>, Status>
        {
            Ok(Response::new(
                chv_stord_api::chv_stord_api::GetDiskMigrationStatusResponse {
                    result: Some(chv_stord_api::chv_stord_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                    phase: 4, // Completed
                    convergence_round: 0,
                    dirty_blocks_remaining: 0,
                    bytes_transferred: 0,
                    total_bytes: 0,
                    needs_vm_pause: false,
                    error_message: "".to_string(),
                },
            ))
        }
        async fn resume_disk_migration(
            &self,
            _req: Request<chv_stord_api::chv_stord_api::ResumeDiskMigrationRequest>,
        ) -> Result<Response<chv_stord_api::chv_stord_api::ResumeDiskMigrationResponse>, Status>
        {
            Ok(Response::new(
                chv_stord_api::chv_stord_api::ResumeDiskMigrationResponse {
                    result: Some(chv_stord_api::chv_stord_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                },
            ))
        }
    }

    struct MockNwdOk;
    #[tonic::async_trait]
    impl NetworkService for MockNwdOk {
        async fn list_namespace_state(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::ListNamespaceStateRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::ListNamespaceStateResponse>, Status>
        {
            Ok(Response::new(
                chv_nwd_api::chv_nwd_api::ListNamespaceStateResponse { items: vec![] },
            ))
        }
        async fn ensure_network_topology(
            &self,
            req: Request<chv_nwd_api::chv_nwd_api::EnsureNetworkTopologyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            nwd_operation_id(req.into_inner().meta)?;
            Ok(Response::new(chv_nwd_api::chv_nwd_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn delete_network_topology(
            &self,
            req: Request<chv_nwd_api::chv_nwd_api::DeleteNetworkTopologyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            nwd_operation_id(req.into_inner().meta)?;
            Ok(Response::new(chv_nwd_api::chv_nwd_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn get_network_health(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::NetworkHealthRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::NetworkHealthResponse>, Status> {
            Ok(Response::new(
                chv_nwd_api::chv_nwd_api::NetworkHealthResponse {
                    result: Some(chv_nwd_api::chv_nwd_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                    network_id: "".to_string(),
                    health_status: "healthy".to_string(),
                    last_error: "".to_string(),
                },
            ))
        }
        async fn attach_vm_nic(
            &self,
            req: Request<chv_nwd_api::chv_nwd_api::AttachVmNicRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::AttachVmNicResponse>, Status> {
            let inner = req.into_inner();
            nwd_operation_id(inner.meta.clone())?;
            let nic = inner.nic.unwrap();
            Ok(Response::new(
                chv_nwd_api::chv_nwd_api::AttachVmNicResponse {
                    result: Some(chv_nwd_api::chv_nwd_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                    namespace_handle: format!("ns-{}", nic.network_id),
                    tap_handle: format!("tap-{}", nic.network_id),
                },
            ))
        }
        async fn detach_vm_nic(
            &self,
            req: Request<chv_nwd_api::chv_nwd_api::DetachVmNicRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            nwd_operation_id(req.into_inner().meta)?;
            Ok(Response::new(chv_nwd_api::chv_nwd_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn set_firewall_policy(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::SetFirewallPolicyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Ok(Response::new(chv_nwd_api::chv_nwd_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn set_nat_policy(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::SetNatPolicyRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Ok(Response::new(chv_nwd_api::chv_nwd_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn ensure_dhcp_scope(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::EnsureDhcpScopeRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Ok(Response::new(chv_nwd_api::chv_nwd_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn ensure_dns_scope(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::EnsureDnsScopeRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Ok(Response::new(chv_nwd_api::chv_nwd_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn expose_service(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::ExposeServiceRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Ok(Response::new(chv_nwd_api::chv_nwd_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }
        async fn withdraw_service_exposure(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::WithdrawServiceExposureRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::Result>, Status> {
            Ok(Response::new(chv_nwd_api::chv_nwd_api::Result {
                status: "ok".to_string(),
                error_code: "".to_string(),
                human_summary: "".to_string(),
            }))
        }

        async fn update_overlay(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::UpdateOverlayRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::UpdateOverlayResponse>, Status> {
            Ok(Response::new(
                chv_nwd_api::chv_nwd_api::UpdateOverlayResponse {
                    result: Some(chv_nwd_api::chv_nwd_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                },
            ))
        }

        async fn update_security_policy(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::SecurityPolicy>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::UpdateSecurityPolicyResponse>, Status>
        {
            Ok(Response::new(
                chv_nwd_api::chv_nwd_api::UpdateSecurityPolicyResponse {
                    result: Some(chv_nwd_api::chv_nwd_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                },
            ))
        }

        async fn update_rate_limit(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::RateLimitPolicy>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::UpdateRateLimitResponse>, Status> {
            Ok(Response::new(
                chv_nwd_api::chv_nwd_api::UpdateRateLimitResponse {
                    result: Some(chv_nwd_api::chv_nwd_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                },
            ))
        }

        async fn get_overlay_status(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::GetOverlayStatusRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::OverlayStatus>, Status> {
            Ok(Response::new(chv_nwd_api::chv_nwd_api::OverlayStatus {
                network_id: "".to_string(),
                vni: 0,
                vxlan_interface_up: false,
                fdb_entry_count: 0,
                ebpf_programs_loaded: 0,
            }))
        }

        async fn send_gratuitous_arp(
            &self,
            _req: Request<chv_nwd_api::chv_nwd_api::SendGratuitousArpRequest>,
        ) -> Result<Response<chv_nwd_api::chv_nwd_api::SendGratuitousArpResponse>, Status> {
            Ok(Response::new(
                chv_nwd_api::chv_nwd_api::SendGratuitousArpResponse {
                    result: Some(chv_nwd_api::chv_nwd_api::Result {
                        status: "ok".to_string(),
                        error_code: "".to_string(),
                        human_summary: "".to_string(),
                    }),
                },
            ))
        }
    }

    async fn start_mock_stord(socket: &std::path::Path) {
        let uds = tokio::net::UnixListener::bind(socket).unwrap();
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(
                    chv_stord_api::chv_stord_api::storage_service_server::StorageServiceServer::new(
                        MockStordOk,
                    ),
                )
                .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(uds))
                .await
                .ok();
        });
        for _ in 0..10 {
            if StordClient::connect(socket).await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn start_mock_nwd(socket: &std::path::Path) {
        let uds = tokio::net::UnixListener::bind(socket).unwrap();
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(
                    chv_nwd_api::chv_nwd_api::network_service_server::NetworkServiceServer::new(
                        MockNwdOk,
                    ),
                )
                .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(uds))
                .await
                .ok();
        });
        for _ in 0..10 {
            if NwdClient::connect(socket).await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn reconciler_creates_missing_vm() {
        let dir = tempfile::tempdir().unwrap();
        let stord_socket = dir.path().join("stord.sock");
        let nwd_socket = dir.path().join("nwd.sock");
        start_mock_stord(&stord_socket).await;
        start_mock_nwd(&nwd_socket).await;

        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(test_cache())),
            VmRuntime::new(mock.clone()),
            stord_socket,
            nwd_socket,
            dir.path().to_path_buf(),
            Arc::new(MigrationTaskRegistry::new()),
        )
        .await;
        rec.reconcile_vms().await.unwrap();

        let vms = mock.vms.lock().unwrap();
        assert!(vms.contains_key("vm-1"));
        let config = vms.get("vm-1").unwrap();
        assert_eq!(config.cpus, 1);
        assert_eq!(config.memory_bytes, 1024);
    }

    #[tokio::test]
    async fn reconciler_deletes_orphan_vm() {
        let dir = tempfile::tempdir().unwrap();
        let stord_socket = dir.path().join("stord.sock");
        let nwd_socket = dir.path().join("nwd.sock");
        start_mock_stord(&stord_socket).await;
        start_mock_nwd(&nwd_socket).await;

        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let runtime = VmRuntime::new(mock.clone());
        let config = VmConfig {
            vm_id: "vm-orphan".to_string(),
            cpus: 1,
            memory_bytes: 512,
            kernel_path: PathBuf::from("/dev/null"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: dir.path().join("vms/vm-orphan/vm.sock"),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        };
        runtime
            .create_vm("vm-orphan", "1", &config, None)
            .await
            .unwrap();

        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(empty_cache())),
            runtime,
            stord_socket,
            nwd_socket,
            dir.path().to_path_buf(),
            Arc::new(MigrationTaskRegistry::new()),
        )
        .await;
        rec.reconcile_vms().await.unwrap();

        assert!(mock.vms.lock().unwrap().get("vm-orphan").is_none());
    }

    #[tokio::test]
    async fn draining_with_running_vms_requests_migrations() {
        let dir = tempfile::tempdir().unwrap();
        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let runtime = VmRuntime::new(mock.clone());

        // Create two VMs in "Running" and "Created" states
        let config1 = VmConfig {
            vm_id: "vm-drain-1".to_string(),
            cpus: 1,
            memory_bytes: 512,
            kernel_path: PathBuf::from("/dev/null"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: dir.path().join("vms/vm-drain-1/vm.sock"),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        };
        runtime
            .create_vm("vm-drain-1", "1", &config1, None)
            .await
            .unwrap();
        runtime.start_vm("vm-drain-1", None).await.unwrap();

        let config2 = VmConfig {
            vm_id: "vm-drain-2".to_string(),
            cpus: 1,
            memory_bytes: 512,
            kernel_path: PathBuf::from("/dev/null"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: dir.path().join("vms/vm-drain-2/vm.sock"),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        };
        runtime
            .create_vm("vm-drain-2", "1", &config2, None)
            .await
            .unwrap();
        // vm-drain-2 stays in "Created" status (also eligible for drain)

        let mut cache = NodeCache {
            node_state: "Draining".to_string(),
            node_id: "test-node".to_string(),
            ..Default::default()
        };
        // Transition to Draining via valid path
        cache.node_state = NodeState::Draining.as_str().to_string();

        let cache = Arc::new(tokio::sync::Mutex::new(cache));
        let mut rec = Reconciler::new(
            cache.clone(),
            runtime,
            PathBuf::from("/tmp/fake-stord.sock"),
            PathBuf::from("/tmp/fake-nwd.sock"),
            dir.path().to_path_buf(),
            Arc::new(MigrationTaskRegistry::new()),
        )
        .await;

        rec.run_once().await.unwrap();

        // Both VMs should be in drain_requested_vms
        assert!(rec.drain_requested_vms.contains("vm-drain-1"));
        assert!(rec.drain_requested_vms.contains("vm-drain-2"));

        // Pending messages should have been enqueued for both VMs
        let c = cache.lock().await;
        let pending = c.pending_control_plane_messages();
        assert!(
            pending.len() >= 2,
            "expected at least 2 pending messages, got {}",
            pending.len()
        );
    }

    #[tokio::test]
    async fn draining_with_no_vms_transitions_to_maintenance() {
        let dir = tempfile::tempdir().unwrap();
        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let runtime = VmRuntime::new(mock.clone());

        // No VMs in the runtime

        let cache = NodeCache {
            node_state: NodeState::Draining.as_str().to_string(),
            node_id: "test-node".to_string(),
            ..Default::default()
        };

        let cache = Arc::new(tokio::sync::Mutex::new(cache));
        let mut rec = Reconciler::new(
            cache.clone(),
            runtime,
            PathBuf::from("/tmp/fake-stord.sock"),
            PathBuf::from("/tmp/fake-nwd.sock"),
            dir.path().to_path_buf(),
            Arc::new(MigrationTaskRegistry::new()),
        )
        .await;

        rec.run_once().await.unwrap();

        // Should transition to Maintenance
        assert_eq!(rec.current_state().await, NodeState::Maintenance);
        // drain_requested_vms should be cleared
        assert!(rec.drain_requested_vms.is_empty());
    }

    #[tokio::test]
    async fn draining_skips_already_requested_vms() {
        let dir = tempfile::tempdir().unwrap();
        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let runtime = VmRuntime::new(mock.clone());

        // Create a running VM
        let config = VmConfig {
            vm_id: "vm-already".to_string(),
            cpus: 1,
            memory_bytes: 512,
            kernel_path: PathBuf::from("/dev/null"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: dir.path().join("vms/vm-already/vm.sock"),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        };
        runtime
            .create_vm("vm-already", "1", &config, None)
            .await
            .unwrap();
        runtime.start_vm("vm-already", None).await.unwrap();

        let cache = NodeCache {
            node_state: NodeState::Draining.as_str().to_string(),
            node_id: "test-node".to_string(),
            ..Default::default()
        };

        let cache = Arc::new(tokio::sync::Mutex::new(cache));
        let mut rec = Reconciler::new(
            cache.clone(),
            runtime,
            PathBuf::from("/tmp/fake-stord.sock"),
            PathBuf::from("/tmp/fake-nwd.sock"),
            dir.path().to_path_buf(),
            Arc::new(MigrationTaskRegistry::new()),
        )
        .await;

        // Pre-populate drain_requested_vms — as if we already requested this VM
        rec.drain_requested_vms.insert("vm-already".to_string());

        rec.run_once().await.unwrap();

        // Should still contain the VM (not removed)
        assert!(rec.drain_requested_vms.contains("vm-already"));

        // No NEW pending messages should be enqueued (since it was already requested)
        let c = cache.lock().await;
        let pending = c.pending_control_plane_messages();
        assert_eq!(
            pending.len(),
            0,
            "expected 0 pending messages for already-requested VM, got {}",
            pending.len()
        );
    }

    #[tokio::test]
    async fn reconciler_starts_stopped_vm() {
        let dir = tempfile::tempdir().unwrap();
        let stord_socket = dir.path().join("stord.sock");
        let nwd_socket = dir.path().join("nwd.sock");
        start_mock_stord(&stord_socket).await;
        start_mock_nwd(&nwd_socket).await;

        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let runtime = VmRuntime::new(mock.clone());
        let config = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 1,
            memory_bytes: 1024,
            kernel_path: PathBuf::from("/dev/null"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: dir.path().join("vms/vm-1/vm.sock"),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        };
        runtime.create_vm("vm-1", "1", &config, None).await.unwrap();
        runtime.stop_vm("vm-1", false, None).await.unwrap();
        assert_eq!(runtime.get("vm-1").await.unwrap().runtime_status, "Stopped");

        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(test_cache())),
            runtime,
            stord_socket,
            nwd_socket,
            dir.path().to_path_buf(),
            Arc::new(MigrationTaskRegistry::new()),
        )
        .await;
        rec.reconcile_vms().await.unwrap();

        assert_eq!(
            rec.vm_runtime.get("vm-1").await.unwrap().runtime_status,
            "Running"
        );
    }

    /// Adapter wrapper that forwards every call to an inner mock but injects a
    /// fixed sleep into `create_vm`. Used to make per-VM-create latency
    /// dominate the reconcile-tick wall-clock so parallelism is observable.
    struct DelayingMockAdapter {
        inner: chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter,
        create_delay: Duration,
        create_calls: std::sync::atomic::AtomicUsize,
    }

    impl DelayingMockAdapter {
        fn new(create_delay: Duration) -> Self {
            Self {
                inner: chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default(),
                create_delay,
                create_calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn create_call_count(&self) -> usize {
            self.create_calls.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    #[tonic::async_trait]
    impl chv_agent_runtime_ch::adapter::CloudHypervisorAdapter for DelayingMockAdapter {
        async fn create_vm(
            &self,
            config: &chv_agent_runtime_ch::adapter::VmConfig,
            operation_id: Option<&str>,
        ) -> Result<String, ChvError> {
            // The sleep MUST happen before we record the call so that all N
            // sleeps overlap rather than executing serially.
            tokio::time::sleep(self.create_delay).await;
            self.create_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.inner.create_vm(config, operation_id).await
        }

        async fn start_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
            self.inner.start_vm(vm_id, operation_id).await
        }

        async fn stop_vm(
            &self,
            vm_id: &str,
            force: bool,
            operation_id: Option<&str>,
        ) -> Result<(), ChvError> {
            self.inner.stop_vm(vm_id, force, operation_id).await
        }

        async fn delete_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
            self.inner.delete_vm(vm_id, operation_id).await
        }

        async fn reboot_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
            self.inner.reboot_vm(vm_id, operation_id).await
        }

        async fn pause_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
            self.inner.pause_vm(vm_id, operation_id).await
        }

        async fn resume_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
            self.inner.resume_vm(vm_id, operation_id).await
        }

        async fn power_button(
            &self,
            vm_id: &str,
            operation_id: Option<&str>,
        ) -> Result<(), ChvError> {
            self.inner.power_button(vm_id, operation_id).await
        }

        async fn resize_vm(
            &self,
            vm_id: &str,
            cpus: Option<u32>,
            memory_bytes: Option<u64>,
            operation_id: Option<&str>,
        ) -> Result<(), ChvError> {
            self.inner
                .resize_vm(vm_id, cpus, memory_bytes, operation_id)
                .await
        }

        async fn add_disk(
            &self,
            vm_id: &str,
            params: &chv_agent_runtime_ch::adapter::AddDiskParams,
            operation_id: Option<&str>,
        ) -> Result<String, ChvError> {
            self.inner.add_disk(vm_id, params, operation_id).await
        }

        async fn remove_device(
            &self,
            vm_id: &str,
            device_id: &str,
            operation_id: Option<&str>,
        ) -> Result<(), ChvError> {
            self.inner
                .remove_device(vm_id, device_id, operation_id)
                .await
        }

        async fn add_net(
            &self,
            vm_id: &str,
            params: &chv_agent_runtime_ch::adapter::AddNetParams,
            operation_id: Option<&str>,
        ) -> Result<String, ChvError> {
            self.inner.add_net(vm_id, params, operation_id).await
        }

        async fn resize_disk(
            &self,
            vm_id: &str,
            disk_id: &str,
            new_size_bytes: u64,
            operation_id: Option<&str>,
        ) -> Result<(), ChvError> {
            self.inner
                .resize_disk(vm_id, disk_id, new_size_bytes, operation_id)
                .await
        }

        async fn vm_info(
            &self,
            vm_id: &str,
        ) -> Result<chv_agent_runtime_ch::adapter::VmInfo, ChvError> {
            self.inner.vm_info(vm_id).await
        }

        async fn vm_counters(
            &self,
            vm_id: &str,
        ) -> Result<chv_agent_runtime_ch::adapter::VmCounters, ChvError> {
            self.inner.vm_counters(vm_id).await
        }

        async fn ping(&self, vm_id: &str) -> Result<bool, ChvError> {
            self.inner.ping(vm_id).await
        }

        async fn snapshot_vm(
            &self,
            vm_id: &str,
            destination: &str,
            operation_id: Option<&str>,
        ) -> Result<(), ChvError> {
            self.inner
                .snapshot_vm(vm_id, destination, operation_id)
                .await
        }

        async fn restore_snapshot(
            &self,
            vm_id: &str,
            source: &str,
            operation_id: Option<&str>,
        ) -> Result<(), ChvError> {
            self.inner
                .restore_snapshot(vm_id, source, operation_id)
                .await
        }

        async fn send_migration(
            &self,
            vm_id: &str,
            destination_url: &str,
            operation_id: Option<&str>,
        ) -> Result<(), ChvError> {
            self.inner
                .send_migration(vm_id, destination_url, operation_id)
                .await
        }

        async fn receive_migration(
            &self,
            vm_id: &str,
            receiver_url: &str,
            operation_id: Option<&str>,
        ) -> Result<(), ChvError> {
            self.inner
                .receive_migration(vm_id, receiver_url, operation_id)
                .await
        }

        async fn get_vm_state(&self, vm_id: &str) -> Result<String, ChvError> {
            self.inner.get_vm_state(vm_id).await
        }

        async fn coredump(
            &self,
            vm_id: &str,
            destination: &str,
            operation_id: Option<&str>,
        ) -> Result<(), ChvError> {
            self.inner.coredump(vm_id, destination, operation_id).await
        }
    }

    /// Verifies that `reconcile_vms` runs per-VM CREATE work in parallel.
    ///
    /// We populate the cache with N=20 desired VMs, each having no disks and
    /// no nics so `prepare_vm_resources` is fast. The adapter's `create_vm`
    /// sleeps for `delay` per call. With `VM_RECONCILE_CONCURRENCY = 8`,
    /// fully sequential execution would take `N * delay`, fully parallel
    /// would take `ceil(N / 8) * delay`. We assert the elapsed wall-clock
    /// is comfortably below the sequential bound.
    #[tokio::test]
    async fn reconcile_vms_creates_in_parallel() {
        const N: usize = 20;
        const DELAY: Duration = Duration::from_millis(100);
        // Sequential lower bound: N * DELAY = 2000ms.
        // Parallel upper bound (8 slots, 20 items, 3 batches): ~3 * DELAY = 300ms.
        // We allow generous slack for connection probes and runtime overhead.
        const PARALLEL_BUDGET: Duration = Duration::from_millis(1200);

        let dir = tempfile::tempdir().unwrap();
        let stord_socket = dir.path().join("stord.sock");
        let nwd_socket = dir.path().join("nwd.sock");
        start_mock_stord(&stord_socket).await;
        start_mock_nwd(&nwd_socket).await;

        let adapter = std::sync::Arc::new(DelayingMockAdapter::new(DELAY));

        let mut cache = empty_cache();
        for i in 0..N {
            let vm_id = format!("vm-par-{:02}", i);
            let spec_json = format!(
                r#"{{"name":"{}","cpus":1,"memory_bytes":1024,"kernel_path":"/dev/null","disks":[],"nics":[],"desired_state":"Stopped"}}"#,
                vm_id
            )
            .into_bytes();
            cache.vm_fragments.insert(
                vm_id.clone(),
                crate::cache::DesiredStateFragment {
                    id: vm_id,
                    kind: "vm".to_string(),
                    generation: "1".to_string(),
                    spec_json,
                    policy_json: vec![],
                    updated_at: "2024-01-01T00:00:00Z".to_string(),
                    updated_by: "cp".to_string(),
                },
            );
        }

        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(cache)),
            VmRuntime::new(adapter.clone()),
            stord_socket,
            nwd_socket,
            dir.path().to_path_buf(),
            Arc::new(MigrationTaskRegistry::new()),
        )
        .await;

        let started = std::time::Instant::now();
        rec.reconcile_vms().await.unwrap();
        let elapsed = started.elapsed();

        assert_eq!(
            adapter.create_call_count(),
            N,
            "expected create_vm called N={} times, got {}",
            N,
            adapter.create_call_count()
        );
        assert!(
            elapsed < PARALLEL_BUDGET,
            "reconcile_vms took {:?} for {} VMs at {:?}/each; expected < {:?} (sequential would be {:?})",
            elapsed,
            N,
            DELAY,
            PARALLEL_BUDGET,
            DELAY * N as u32
        );
    }

    // ------------------------------------------------------------------
    // Backoff predicate (pure unit tests, no I/O)
    // ------------------------------------------------------------------

    // Tick 0 and 1 must always run regardless of failure count: this is the
    // "boot window" where we have not yet observed enough failures to back
    // off, and missing the first tick would delay every freshly-scheduled VM.
    #[test]
    fn should_skip_vm_for_tick_runs_first_two_ticks() {
        assert!(!should_skip_vm_for_tick(0, 0));
        assert!(!should_skip_vm_for_tick(0, 100));
        assert!(!should_skip_vm_for_tick(1, 0));
        assert!(!should_skip_vm_for_tick(1, 100));
    }

    // Below 3 failures the predicate must never gate. This guards the "no
    // backoff for healthy VMs" invariant that lives in the same body as the
    // tier thresholds and is easy to break.
    #[test]
    fn should_skip_vm_for_tick_no_skip_below_three_failures() {
        for tick in 2u64..=120 {
            for failures in 0u32..3 {
                assert!(
                    !should_skip_vm_for_tick(tick, failures),
                    "tick={} failures={} must not be skipped",
                    tick,
                    failures
                );
            }
        }
    }

    // Mid-tier (3..10 failures): retry every 6th tick.
    #[test]
    fn should_skip_vm_for_tick_mid_tier_runs_every_six_ticks() {
        for tick in 2u64..=120 {
            let expected_skip = !tick.is_multiple_of(6);
            assert_eq!(
                should_skip_vm_for_tick(tick, 5),
                expected_skip,
                "tick={} mid-tier should skip={}",
                tick,
                expected_skip
            );
        }
    }

    // Persistent-failure tier (>=10 failures): retry every 60th tick.
    #[test]
    fn should_skip_vm_for_tick_persistent_tier_runs_every_sixty_ticks() {
        for tick in 2u64..=240 {
            let expected_skip = !tick.is_multiple_of(60);
            assert_eq!(
                should_skip_vm_for_tick(tick, 25),
                expected_skip,
                "tick={} persistent-tier should skip={}",
                tick,
                expected_skip
            );
        }
    }

    // ------------------------------------------------------------------
    // VmRuntime failure-tracking API (used by per-VM reconcile workers)
    // ------------------------------------------------------------------

    // record_failure on a VM that was never created must still bump the
    // failure_counts map so future ticks can back off. Crucially it must NOT
    // create a phantom VmRecord — that regression caused the reconciler to
    // skip create and try start/stop on a non-existent VM.
    #[tokio::test]
    async fn record_failure_without_record_increments_counter_only() {
        let runtime = VmRuntime::new(std::sync::Arc::new(
            chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default(),
        ));
        runtime.record_failure("vm-ghost", "1", "boom").await;
        runtime.record_failure("vm-ghost", "1", "boom again").await;
        assert_eq!(runtime.consecutive_failures("vm-ghost").await, 2);
        assert_eq!(
            runtime
                .consecutive_failures_for_generation("vm-ghost", "1")
                .await,
            2
        );
        assert!(
            runtime.get("vm-ghost").await.is_none(),
            "record_failure must not synthesise a VmRecord for a never-created VM"
        );
    }

    // clear_failure_count zeroes the counter so the per-generation lookup
    // also returns 0. This is the success path called after a successful
    // create/start/delete and is what stops the backoff once a VM recovers.
    #[tokio::test]
    async fn clear_failure_count_resets_counter() {
        let runtime = VmRuntime::new(std::sync::Arc::new(
            chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default(),
        ));
        runtime.record_failure("vm-x", "1", "err").await;
        runtime.record_failure("vm-x", "1", "err").await;
        runtime.record_failure("vm-x", "1", "err").await;
        assert_eq!(runtime.consecutive_failures("vm-x").await, 3);
        runtime.clear_failure_count("vm-x").await;
        assert_eq!(runtime.consecutive_failures("vm-x").await, 0);
        assert_eq!(
            runtime
                .consecutive_failures_for_generation("vm-x", "1")
                .await,
            0
        );
    }

    // Failures are tracked per-generation: a stale failure from generation
    // "1" must NOT cause generation "2" to back off when looked up by gen.
    // The implementation keys the counter map by vm_id and stores a single
    // (count, latest_generation) pair: the per-generation lookup matches
    // strictly, so once gen "2" overwrites the stored gen, gen "1" reads 0.
    // This is what lets a CP operator unstick a VM by bumping its generation.
    #[tokio::test]
    async fn record_failure_isolates_generations() {
        let runtime = VmRuntime::new(std::sync::Arc::new(
            chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default(),
        ));
        runtime.record_failure("vm-y", "1", "old gen failed").await;
        runtime.record_failure("vm-y", "1", "old gen failed").await;
        // Generation "1" has 2 failures; gen "2" lookup must return 0.
        assert_eq!(
            runtime
                .consecutive_failures_for_generation("vm-y", "1")
                .await,
            2
        );
        assert_eq!(
            runtime
                .consecutive_failures_for_generation("vm-y", "2")
                .await,
            0,
            "stale gen-1 failures must not gate gen-2"
        );
        // After recording under "2", the stored generation flips to "2", so
        // gen "1" lookups return 0 — this is what lets the CP unstick a VM
        // by bumping its generation.
        runtime.record_failure("vm-y", "2", "new gen failed").await;
        assert_eq!(
            runtime
                .consecutive_failures_for_generation("vm-y", "1")
                .await,
            0,
            "after gen rollover, gen-1 lookups must read zero"
        );
        assert!(
            runtime
                .consecutive_failures_for_generation("vm-y", "2")
                .await
                >= 1,
            "gen-2 must have at least one tracked failure"
        );
    }

    // record_failure on an existing VmRecord must flip status to Failed and
    // populate last_error, so the reconciler's recovery path can detect it.
    #[tokio::test]
    async fn record_failure_marks_existing_record_failed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let runtime = VmRuntime::new(mock);
        let config = VmConfig {
            vm_id: "vm-flip".to_string(),
            cpus: 1,
            memory_bytes: 512,
            kernel_path: PathBuf::from("/dev/null"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: dir.path().join("vms/vm-flip/vm.sock"),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        };
        runtime
            .create_vm("vm-flip", "1", &config, None)
            .await
            .expect("create_vm");
        runtime.record_failure("vm-flip", "1", "explosion").await;

        let rec = runtime.get("vm-flip").await.expect("record present");
        assert_eq!(rec.runtime_status, "Failed");
        assert_eq!(rec.last_error.as_deref(), Some("explosion"));
        assert_eq!(rec.consecutive_failures, 1);
        assert_eq!(
            runtime
                .consecutive_failures_for_generation("vm-flip", "1")
                .await,
            1
        );
    }

    // ------------------------------------------------------------------
    // Reconcile invariants
    // ------------------------------------------------------------------

    // When desired matches observed (Running == Running, same cpus/mem),
    // reconcile_vms must not delete the VM nor toggle its state. This guards
    // the "steady state is idempotent" property that the FSM depends on.
    #[tokio::test]
    async fn reconcile_vms_is_noop_when_running_vm_matches_desired() {
        let dir = tempfile::tempdir().expect("tempdir");
        let stord_socket = dir.path().join("stord.sock");
        let nwd_socket = dir.path().join("nwd.sock");
        start_mock_stord(&stord_socket).await;
        start_mock_nwd(&nwd_socket).await;

        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let runtime = VmRuntime::new(mock.clone());
        // Pre-create and start a VM whose cpus/memory match the test_cache spec.
        let config = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 1,
            memory_bytes: 1024,
            kernel_path: PathBuf::from("/dev/null"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: dir.path().join("vms/vm-1/vm.sock"),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        };
        runtime
            .create_vm("vm-1", "1", &config, None)
            .await
            .expect("create_vm");
        runtime.start_vm("vm-1", None).await.expect("start_vm");
        assert_eq!(
            runtime.get("vm-1").await.expect("record").runtime_status,
            "Running"
        );

        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(test_cache())),
            runtime,
            stord_socket,
            nwd_socket,
            dir.path().to_path_buf(),
            Arc::new(MigrationTaskRegistry::new()),
        )
        .await;
        rec.reconcile_vms().await.expect("reconcile_vms");

        // VM must still be present in the runtime map (not deleted).
        assert!(mock.vms.lock().expect("mock lock").contains_key("vm-1"));
        // VmRecord must still be Running with the same cpus/mem.
        let final_rec = rec
            .vm_runtime
            .get("vm-1")
            .await
            .expect("record present after reconcile");
        assert_eq!(final_rec.runtime_status, "Running");
        assert_eq!(final_rec.cpus, 1);
        assert_eq!(final_rec.memory_bytes, 1024);
        assert_eq!(final_rec.consecutive_failures, 0);
    }

    // Bootstrapping is a node-level state that gates all VM operations. Even
    // if the cache somehow contains a VM fragment, run_once must NOT call
    // reconcile_vms at all — the agent has not yet been told it can host
    // tenant workloads. Regression class: a refactor that pulls VM ops above
    // the node-state guard.
    #[tokio::test]
    async fn run_once_in_bootstrapping_does_not_create_vms() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());

        // Cache has a VM fragment AND a non-tenant-ready node_state.
        let mut cache = test_cache();
        cache.node_state = NodeState::Bootstrapping.as_str().to_string();

        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(cache)),
            VmRuntime::new(mock.clone()),
            // Use bogus sockets — we expect reconcile_vms NOT to be called,
            // so no probe should ever happen. If the guard is broken, the
            // probe would fail-soft (warn + return), not panic, but the
            // post-condition below would still catch it: no VM created.
            PathBuf::from("/tmp/chv-test-fake-stord-bootstrap.sock"),
            PathBuf::from("/tmp/chv-test-fake-nwd-bootstrap.sock"),
            dir.path().to_path_buf(),
            Arc::new(MigrationTaskRegistry::new()),
        )
        .await;

        rec.run_once().await.expect("run_once");

        // No VM was created — the bootstrapping guard held.
        assert!(
            mock.vms.lock().expect("mock lock").is_empty(),
            "Bootstrapping node must not run VM reconcile"
        );
    }

    // Malformed spec_json (invalid UTF-8) on an existing VM must record a
    // failure and never panic. Guards against `from_utf8` being upgraded to
    // a panicking variant during a refactor.
    #[tokio::test]
    async fn reconcile_vms_records_failure_on_malformed_spec_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        let stord_socket = dir.path().join("stord.sock");
        let nwd_socket = dir.path().join("nwd.sock");
        start_mock_stord(&stord_socket).await;
        start_mock_nwd(&nwd_socket).await;

        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let runtime = VmRuntime::new(mock.clone());
        // Pre-create a VM in the runtime so the existing-VM reconcile path is
        // hit (rather than the create path, which uses a different decoder).
        let config = VmConfig {
            vm_id: "vm-bad".to_string(),
            cpus: 1,
            memory_bytes: 512,
            kernel_path: PathBuf::from("/dev/null"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: dir.path().join("vms/vm-bad/vm.sock"),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        };
        runtime
            .create_vm("vm-bad", "1", &config, None)
            .await
            .expect("create_vm");

        // Cache fragment with invalid UTF-8 bytes (0xFF 0xFE is not valid UTF-8).
        let mut cache = empty_cache();
        cache.vm_fragments.insert(
            "vm-bad".to_string(),
            crate::cache::DesiredStateFragment {
                id: "vm-bad".to_string(),
                kind: "vm".to_string(),
                generation: "1".to_string(),
                spec_json: vec![0xFF, 0xFE, 0xFD],
                policy_json: vec![],
                updated_at: "2024-01-01T00:00:00Z".to_string(),
                updated_by: "cp".to_string(),
            },
        );

        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(cache)),
            runtime,
            stord_socket,
            nwd_socket,
            dir.path().to_path_buf(),
            Arc::new(MigrationTaskRegistry::new()),
        )
        .await;

        // Must not panic — that is the regression we are guarding against.
        rec.reconcile_vms().await.expect("reconcile_vms");

        // A failure should have been recorded for this generation.
        let count = rec
            .vm_runtime
            .consecutive_failures_for_generation("vm-bad", "1")
            .await;
        assert!(
            count >= 1,
            "expected at least one recorded failure, got {}",
            count
        );
    }

    // Same as above but with structurally-valid UTF-8 that fails JSON parse.
    // Exercises the second decoder branch (VmSpec::from_json) which is a
    // separate code path from the UTF-8 check.
    #[tokio::test]
    async fn reconcile_vms_records_failure_on_invalid_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        let stord_socket = dir.path().join("stord.sock");
        let nwd_socket = dir.path().join("nwd.sock");
        start_mock_stord(&stord_socket).await;
        start_mock_nwd(&nwd_socket).await;

        let mock =
            std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());

        let mut cache = empty_cache();
        cache.vm_fragments.insert(
            "vm-junk".to_string(),
            crate::cache::DesiredStateFragment {
                id: "vm-junk".to_string(),
                kind: "vm".to_string(),
                generation: "1".to_string(),
                // Valid UTF-8 but not valid JSON for VmSpec.
                spec_json: b"not-a-json-document".to_vec(),
                policy_json: vec![],
                updated_at: "2024-01-01T00:00:00Z".to_string(),
                updated_by: "cp".to_string(),
            },
        );

        let runtime = VmRuntime::new(mock.clone());
        let mut rec = Reconciler::new(
            Arc::new(tokio::sync::Mutex::new(cache)),
            runtime,
            stord_socket,
            nwd_socket,
            dir.path().to_path_buf(),
            Arc::new(MigrationTaskRegistry::new()),
        )
        .await;

        rec.reconcile_vms().await.expect("reconcile_vms");

        // No phantom record should have been created — record_failure on a
        // non-existent VM must only bump the counter map.
        assert!(rec.vm_runtime.get("vm-junk").await.is_none());
        let count = rec
            .vm_runtime
            .consecutive_failures_for_generation("vm-junk", "1")
            .await;
        assert!(
            count >= 1,
            "expected at least one recorded failure for junk JSON, got {}",
            count
        );
        // And no real VM landed in the adapter.
        assert!(!mock.vms.lock().expect("mock lock").contains_key("vm-junk"));
    }

    /// Contract test: the Draining → Maintenance gate must also wait for
    /// `migration_registry` to be empty. A VM handed off to chv-stord for disk
    /// migration leaves `vm_runtime.list()` but the transfer is still in
    /// progress. This test documents that invariant independently of the full
    /// reconcile loop.
    #[tokio::test]
    async fn drain_gate_requires_empty_migration_registry() {
        use crate::migration_registry::MigrationTaskRegistry;
        use tokio_util::sync::CancellationToken;

        let registry = Arc::new(MigrationTaskRegistry::new());
        // Simulate an in-flight migration.
        let handle = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await
        });
        registry.insert(
            "drain-test-op".to_string(),
            handle.abort_handle(),
            CancellationToken::new(),
        );

        assert!(
            !registry.is_empty(),
            "drain must not complete with in-flight migrations"
        );

        handle.abort();
        registry.remove("drain-test-op");
        assert!(
            registry.is_empty(),
            "drain may complete when registry is empty"
        );
    }
}
