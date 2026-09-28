//! Stretched-L2 fabric (ADR-021) seam for the network daemon.
//!
//! This module adapts the shared Kubedo fabric provider
//! (`fabric-linux`, synchronous) to the async `chv-nwd` service:
//!
//! - [`FabricHandle`] is the async seam consumed by the executor. All
//!   provider calls run inside `tokio::task::spawn_blocking` behind a
//!   `std::sync::Mutex`, because `LinuxFabricProvider` mutates in place.
//! - [`NwdFabricProvider`] owns one provider instance for the daemon
//!   lifetime, lazily opened on first use against the persistent state root
//!   (`/var/lib/chv/nwd/fabric` — never `/run`, which is tmpfs and would
//!   lose the WireGuard key on reboot).
//! - [`to_plan`] converts the proto `FabricPlan` into the provider's
//!   validated [`StretchedL2Plan`]; MTU `0` means "daemon default".
//! - Generation fencing (rejecting plans older than the last applied
//!   generation for a network) is the caller's job: the handlers own the
//!   topology table and check `plan_generation` before invoking apply.
//!
//! Key hygiene (ADR-021 guardrail): the WireGuard private key is generated
//! or adopted by [`fabric_linux::ensure_private_key`] (0600, atomic create,
//! never overwritten), referenced by path, piped to `wg pubkey` via stdin
//! only, and never logged, serialized, or placed in argv. Error text from
//! the provider is guaranteed key-free by the same contract.

use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use chv_config::FabricNwdConfig;
use chv_errors::ChvError;
use chv_nwd_api::chv_nwd_api as proto;
pub use fabric_linux::ApplyReport;
use fabric_linux::{
    derive_public_key, ensure_private_key, CommandOutput, FabricCommand, FabricError,
    FabricLinuxConfig, LinuxFabricProvider, Names, RealCommandRunner,
};
use fabric_plan::{FabricPeer, PublicKey, StretchedL2Plan, UnderlayEndpoint, Vni};

use crate::executor::OverlayStatusInfo;

/// The node's public fabric identity (never the private key).
#[derive(Debug, Clone)]
pub struct FabricIdentity {
    /// Base64 WireGuard public key (44 chars).
    pub public_key: String,
    /// Measured default-route (underlay) MTU.
    pub underlay_mtu: u32,
}

/// Outcome of a successful fabric apply, consumed by the executor to graft
/// the consumer veth into the tenant bridge and by the handlers to persist
/// topology state.
#[derive(Debug, Clone)]
pub struct AppliedFabric {
    /// Provider outcome flags (what this call created).
    pub report: ApplyReport,
    /// The plan generation that was applied.
    pub plan_generation: u64,
    /// The tenant MTU in effect after the apply.
    pub tenant_mtu: u32,
    /// Host-namespace consumer veth to enslave to the tenant bridge.
    pub consumer_veth: String,
}

/// MTU defaults applied when a fabric plan omits an MTU (proto `0`).
#[derive(Debug, Clone, Copy)]
pub struct MtuDefaults {
    pub tenant: u32,
    pub fabric: u32,
}

/// Async seam over the synchronous shared fabric provider.
#[async_trait]
pub trait FabricHandle: Send + Sync + 'static {
    /// Apply one network's fabric plan (idempotent, journal-before-mutate,
    /// fail-closed on foreign state). The caller is responsible for
    /// generation fencing before invoking this.
    async fn apply(
        &self,
        network_id: &str,
        vni: u32,
        plan: &proto::FabricPlan,
    ) -> Result<AppliedFabric, ChvError>;

    /// Remove one network's fabric state (reverse dependency order; the
    /// shared fabric and WireGuard key survive). Idempotent.
    async fn remove_network(&self, network_id: &str) -> Result<(), ChvError>;

    /// Ensure the host keypair exists and return the public identity plus
    /// the measured underlay MTU.
    async fn identity(&self) -> Result<FabricIdentity, ChvError>;

    /// The deterministic host-namespace consumer veth name for a network.
    async fn consumer_veth(&self, network_id: &str) -> Result<String, ChvError>;

    /// Observed fabric overlay status for a network.
    async fn overlay_status(&self, network_id: &str) -> Result<OverlayStatusInfo, ChvError>;
}

/// Convert a proto fabric plan into the provider's validated plan.
///
/// MTU fields of `0` select the daemon defaults. Conversion errors are
/// reported as [`ChvError::InvalidArgument`] with a field path; the final
/// `validate()` enforces the fabric contract (identifier shape, MTU math,
/// no local host/IP among peers, no duplicates).
pub fn to_plan(
    network_id: &str,
    vni: u32,
    proto_plan: &proto::FabricPlan,
    defaults: MtuDefaults,
) -> Result<StretchedL2Plan, ChvError> {
    let local_transport_ip: Ipv4Addr =
        proto_plan
            .local_fabric_ip
            .parse()
            .map_err(|_| ChvError::InvalidArgument {
                field: "fabric.local_fabric_ip".to_string(),
                reason: format!(
                    "'{}' is not a valid IPv4 fabric transport address",
                    proto_plan.local_fabric_ip
                ),
            })?;

    let vni = Vni::new(vni).map_err(|e| ChvError::InvalidArgument {
        field: "vni".to_string(),
        reason: e.to_string(),
    })?;

    let mut peers = Vec::with_capacity(proto_plan.peers.len());
    for (index, peer) in proto_plan.peers.iter().enumerate() {
        let public_key =
            PublicKey::new(peer.public_key.as_str()).map_err(|e| ChvError::InvalidArgument {
                field: format!("fabric.peers[{index}].public_key"),
                reason: e.to_string(),
            })?;
        let underlay_endpoint =
            UnderlayEndpoint::parse(peer.underlay_endpoint.as_str()).map_err(|e| {
                ChvError::InvalidArgument {
                    field: format!("fabric.peers[{index}].underlay_endpoint"),
                    reason: e.to_string(),
                }
            })?;
        let fabric_transport_ip: Ipv4Addr =
            peer.fabric_ip
                .parse()
                .map_err(|_| ChvError::InvalidArgument {
                    field: format!("fabric.peers[{index}].fabric_ip"),
                    reason: format!(
                        "'{}' is not a valid IPv4 fabric transport address",
                        peer.fabric_ip
                    ),
                })?;
        peers.push(FabricPeer {
            host_id: peer.node_id.clone(),
            public_key,
            underlay_endpoint,
            fabric_transport_ip,
        });
    }

    let plan = StretchedL2Plan {
        fabric_domain_id: proto_plan.fabric_domain_id.clone(),
        local_host_id: proto_plan.local_host_id.clone(),
        local_transport_ip,
        network_id: network_id.to_string(),
        vni,
        binding_generation: proto_plan.binding_generation,
        tenant_mtu: if proto_plan.tenant_mtu > 0 {
            proto_plan.tenant_mtu
        } else {
            defaults.tenant
        },
        fabric_mtu: if proto_plan.fabric_mtu > 0 {
            proto_plan.fabric_mtu
        } else {
            defaults.fabric
        },
        peers,
        plan_generation: proto_plan.plan_generation,
    };

    plan.validate().map_err(|e| ChvError::InvalidArgument {
        field: "fabric".to_string(),
        reason: e.to_string(),
    })?;
    Ok(plan)
}

/// Map a provider error onto the structured CHV error taxonomy.
///
/// The provider guarantees its error text never contains key material.
pub(crate) fn map_fabric_error(error: FabricError) -> ChvError {
    match error {
        FabricError::Invalid(reason) => ChvError::InvalidArgument {
            field: "fabric".to_string(),
            reason,
        },
        FabricError::Command(reason) => ChvError::NetworkUnavailable {
            resource: "fabric".to_string(),
            reason,
        },
        // Foreign kernel state is a conflict with objects this provider does
        // not own; it is never adopted or deleted (ADR-021 guardrail).
        FabricError::ForeignState {
            object,
            expected,
            observed,
        } => ChvError::Conflict {
            resource: "fabric".to_string(),
            id: format!("{object}: expected {expected}, observed {observed}"),
        },
        FabricError::Ownership(reason) => ChvError::Internal {
            reason: format!("fabric ownership state error: {reason}"),
        },
        FabricError::Unsupported(reason) => ChvError::Internal {
            reason: format!("fabric provider operation unsupported: {reason}"),
        },
        FabricError::Io(source) => ChvError::Io {
            path: "fabric state root".to_string(),
            source,
        },
    }
}

fn lock_poisoned() -> ChvError {
    ChvError::Internal {
        reason: "fabric provider lock poisoned".to_string(),
    }
}

fn join_error(context: &str, error: tokio::task::JoinError) -> ChvError {
    ChvError::Internal {
        reason: format!("fabric {context} task failed: {error}"),
    }
}

/// Wraps a shared runner so both the lazily-opened provider and direct key
/// operations (identity) can drive the same underlying [`FabricCommand`].
struct SharedRunner<R: FabricCommand> {
    inner: Arc<StdMutex<R>>,
}

impl<R: FabricCommand> SharedRunner<R> {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, R>, ChvError> {
        self.inner.lock().map_err(|_| lock_poisoned())
    }
}

impl<R: FabricCommand> FabricCommand for SharedRunner<R> {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<CommandOutput, FabricError> {
        self.lock()
            .map_err(|e| FabricError::Command(e.to_string()))?
            .run(program, args)
    }

    fn run_with_stdin(
        &mut self,
        program: &str,
        args: &[&str],
        stdin: &str,
    ) -> Result<CommandOutput, FabricError> {
        self.lock()
            .map_err(|e| FabricError::Command(e.to_string()))?
            .run_with_stdin(program, args, stdin)
    }
}

/// The production-grade [`FabricHandle`] over the shared Linux provider.
///
/// One instance lives for the daemon lifetime; the provider is opened
/// lazily (loading the ownership journal) on first use.
pub struct NwdFabricProvider<R: FabricCommand + Send + 'static> {
    runner: Arc<StdMutex<R>>,
    provider: Arc<StdMutex<Option<LinuxFabricProvider<SharedRunner<R>>>>>,
    config: FabricLinuxConfig,
    names: Names,
    defaults: MtuDefaults,
    private_key_path: PathBuf,
}

impl<R: FabricCommand + Send + 'static> NwdFabricProvider<R> {
    /// Build a provider over a private runner.
    pub fn new(
        config: FabricLinuxConfig,
        runner: R,
        defaults: MtuDefaults,
    ) -> Result<Self, ChvError> {
        Self::with_shared_runner(config, Arc::new(StdMutex::new(runner)), defaults)
    }

    /// Build a provider over an already-shared runner (test seam: the caller
    /// keeps a handle to inspect the recorded call journal).
    pub fn with_shared_runner(
        config: FabricLinuxConfig,
        runner: Arc<StdMutex<R>>,
        defaults: MtuDefaults,
    ) -> Result<Self, ChvError> {
        config.validate().map_err(map_fabric_error)?;
        let names = Names::new(config.name_prefix()).map_err(map_fabric_error)?;
        let private_key_path = config.private_key_path();
        Ok(Self {
            runner,
            provider: Arc::new(StdMutex::new(None)),
            config,
            names,
            defaults,
            private_key_path,
        })
    }

    fn open_provider(
        guard: &mut std::sync::MutexGuard<'_, Option<LinuxFabricProvider<SharedRunner<R>>>>,
        config: &FabricLinuxConfig,
        runner: &Arc<StdMutex<R>>,
    ) -> Result<(), ChvError> {
        if guard.is_none() {
            let shared = SharedRunner {
                inner: runner.clone(),
            };
            let opened =
                LinuxFabricProvider::open(config.clone(), shared).map_err(map_fabric_error)?;
            **guard = Some(opened);
        }
        Ok(())
    }
}

impl NwdFabricProvider<RealCommandRunner> {
    /// Build the production fabric handle from daemon configuration.
    ///
    /// Validates the configuration fail-closed (absolute persistent state
    /// root, bounded name prefix, distinct nonzero ports) before returning.
    pub fn real(config: &FabricNwdConfig) -> Result<Arc<dyn FabricHandle>, ChvError> {
        let linux_config = FabricLinuxConfig::new(config.state_dir.clone())
            .with_name_prefix(&config.name_prefix)
            .with_wireguard_port(config.wireguard_port)
            .with_vxlan_port(config.vxlan_port);
        let provider = Self::new(
            linux_config,
            RealCommandRunner,
            MtuDefaults {
                tenant: config.default_tenant_mtu,
                fabric: config.default_fabric_mtu,
            },
        )?;
        Ok(Arc::new(provider))
    }
}

#[async_trait]
impl<R: FabricCommand + Send + 'static> FabricHandle for NwdFabricProvider<R> {
    async fn apply(
        &self,
        network_id: &str,
        vni: u32,
        plan: &proto::FabricPlan,
    ) -> Result<AppliedFabric, ChvError> {
        let plan = to_plan(network_id, vni, plan, self.defaults)?;
        let consumer_veth = self.names.consumer_port_veth(network_id);
        let plan_generation = plan.plan_generation;
        let tenant_mtu = plan.tenant_mtu;
        let provider = self.provider.clone();
        let config = self.config.clone();
        let runner = self.runner.clone();

        let report = tokio::task::spawn_blocking(move || {
            let mut guard = provider.lock().map_err(|_| lock_poisoned())?;
            Self::open_provider(&mut guard, &config, &runner)?;
            let Some(provider) = guard.as_mut() else {
                return Err(lock_poisoned());
            };
            provider.apply_plan(&plan).map_err(map_fabric_error)
        })
        .await
        .map_err(|e| join_error("apply", e))??;

        Ok(AppliedFabric {
            report,
            plan_generation,
            tenant_mtu,
            consumer_veth,
        })
    }

    async fn remove_network(&self, network_id: &str) -> Result<(), ChvError> {
        let network_id = network_id.to_string();
        let provider = self.provider.clone();
        let config = self.config.clone();
        let runner = self.runner.clone();

        tokio::task::spawn_blocking(move || {
            let mut guard = provider.lock().map_err(|_| lock_poisoned())?;
            Self::open_provider(&mut guard, &config, &runner)?;
            let Some(provider) = guard.as_mut() else {
                return Err(lock_poisoned());
            };
            // Idempotent teardown: a network the provider does not own is
            // already gone (e.g. a retried delete after a partial failure).
            if !provider.ownership().networks.contains_key(&network_id) {
                return Ok(());
            }
            provider
                .remove_network(&network_id)
                .map_err(map_fabric_error)
        })
        .await
        .map_err(|e| join_error("remove", e))??;

        Ok(())
    }

    async fn identity(&self) -> Result<FabricIdentity, ChvError> {
        let underlay_mtu = measure_underlay_mtu().await;
        let runner = self.runner.clone();
        let key_path = self.private_key_path.clone();

        let public_key = tokio::task::spawn_blocking(move || {
            let mut guard = runner.lock().map_err(|_| lock_poisoned())?;
            let key_path = ensure_private_key(&key_path, &mut *guard).map_err(map_fabric_error)?;
            // The private key is read only to be piped to `wg pubkey` via
            // stdin; it is never logged, serialized, or placed in argv.
            let private_material =
                std::fs::read_to_string(&key_path).map_err(|e| ChvError::Io {
                    path: key_path.to_string_lossy().to_string(),
                    source: e,
                })?;
            let public = derive_public_key(&mut *guard, private_material.trim())
                .map_err(map_fabric_error)?;
            Ok::<_, ChvError>(public.as_str().to_string())
        })
        .await
        .map_err(|e| join_error("identity", e))??;

        Ok(FabricIdentity {
            public_key,
            underlay_mtu,
        })
    }

    async fn consumer_veth(&self, network_id: &str) -> Result<String, ChvError> {
        Ok(self.names.consumer_port_veth(network_id))
    }

    async fn overlay_status(&self, network_id: &str) -> Result<OverlayStatusInfo, ChvError> {
        let network_id = network_id.to_string();
        let provider = self.provider.clone();
        let config = self.config.clone();
        let runner = self.runner.clone();
        let names = self.names.clone();

        tokio::task::spawn_blocking(move || {
            let mut guard = provider.lock().map_err(|_| lock_poisoned())?;
            Self::open_provider(&mut guard, &config, &runner)?;
            let Some(provider) = guard.as_ref() else {
                return Err(lock_poisoned());
            };
            let Some(entry) = provider.ownership().networks.get(&network_id) else {
                return Ok(OverlayStatusInfo {
                    vxlan_interface_up: false,
                    fdb_entry_count: 0,
                });
            };
            let fdb_entry_count = entry.flood_peers.len() as u32;
            // Observe the VXLAN link inside the fabric namespace through the
            // runner (real `ip` in production, the fake kernel in tests).
            let ns = names.fabric_namespace();
            let vxlan = entry.vxlan_name.clone();
            let output = runner
                .lock()
                .map_err(|_| lock_poisoned())?
                .run(
                    "ip",
                    &["netns", "exec", &ns, "ip", "-d", "link", "show", &vxlan],
                )
                .map_err(map_fabric_error)?;
            let vxlan_interface_up = output.success && output.stdout.contains("UP");
            Ok(OverlayStatusInfo {
                vxlan_interface_up,
                fdb_entry_count,
            })
        })
        .await
        .map_err(|e| join_error("status", e))?
    }
}

/// Measure the default-route (underlay) interface MTU, raw — no VXLAN or
/// WireGuard overhead is subtracted; the control plane derives tenant/fabric
/// MTUs from this value (ADR-021 §3). Falls back to 1500 when the default
/// route or its MTU cannot be observed.
pub(crate) async fn measure_underlay_mtu() -> u32 {
    const FALLBACK_MTU: u32 = 1500;

    let route_output = match tokio::process::Command::new("ip")
        .args(["route", "show", "default"])
        .output()
        .await
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return FALLBACK_MTU,
    };

    // Parse "default via X.X.X.X dev eth0" to get the device name.
    let Some(dev) = route_output
        .split_whitespace()
        .skip_while(|w| *w != "dev")
        .nth(1)
    else {
        return FALLBACK_MTU;
    };
    let dev = dev.to_string();

    let mtu_output = match tokio::process::Command::new("ip")
        .args(["link", "show", "dev", &dev])
        .output()
        .await
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return FALLBACK_MTU,
    };

    mtu_output
        .split_whitespace()
        .skip_while(|w| *w != "mtu")
        .nth(1)
        .and_then(|m| m.parse::<u32>().ok())
        .unwrap_or(FALLBACK_MTU)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabric_linux::RecordingRunner;

    const VALID_PUBLIC_KEY: &str = "K7XbF9cV2mQpT3nZ8sL4dW6yH1jR5uA0eG9iO2pS7kM=";
    const SECOND_PUBLIC_KEY: &str = "Q8YcG0dW3nRqU4oA9tM5eX7zI2kS6vB1fH0jP3qT8lN=";

    fn defaults() -> MtuDefaults {
        MtuDefaults {
            tenant: 1380,
            fabric: 1440,
        }
    }

    fn proto_plan() -> proto::FabricPlan {
        proto::FabricPlan {
            fabric_domain_id: "fab-1".to_string(),
            local_host_id: "host-01".to_string(),
            local_fabric_ip: "100.100.0.1".to_string(),
            tenant_mtu: 1380,
            fabric_mtu: 1440,
            binding_generation: 1,
            plan_generation: 1,
            peers: vec![proto::FabricPeer {
                node_id: "host-02".to_string(),
                public_key: VALID_PUBLIC_KEY.to_string(),
                underlay_endpoint: "203.0.113.7:65001".to_string(),
                fabric_ip: "100.100.0.2".to_string(),
            }],
        }
    }

    fn invalid_argument_field(err: &ChvError) -> String {
        match err {
            ChvError::InvalidArgument { field, .. } => field.clone(),
            other => panic!("expected InvalidArgument, got {:?}", other),
        }
    }

    #[test]
    fn to_plan_maps_valid_proto_plan() {
        let plan =
            to_plan("net-1", 100, &proto_plan(), defaults()).expect("valid plan must convert");
        assert_eq!(plan.network_id, "net-1");
        assert_eq!(plan.vni.get(), 100);
        assert_eq!(plan.local_transport_ip.to_string(), "100.100.0.1");
        assert_eq!(plan.tenant_mtu, 1380);
        assert_eq!(plan.fabric_mtu, 1440);
        assert_eq!(plan.peers.len(), 1);
        assert_eq!(plan.peers[0].host_id, "host-02");
        assert_eq!(
            plan.peers[0].underlay_endpoint.to_string(),
            "203.0.113.7:65001"
        );
    }

    #[test]
    fn to_plan_applies_mtu_defaults_when_unset() {
        let mut plan = proto_plan();
        plan.tenant_mtu = 0;
        plan.fabric_mtu = 0;
        let converted = to_plan("net-1", 100, &plan, defaults())
            .expect("unset MTUs must fall back to defaults");
        assert_eq!(converted.tenant_mtu, 1380);
        assert_eq!(converted.fabric_mtu, 1440);
    }

    #[test]
    fn to_plan_rejects_bad_public_key() {
        let mut plan = proto_plan();
        plan.peers[0].public_key = "short".to_string();
        let err = to_plan("net-1", 100, &plan, defaults())
            .err()
            .expect("bad public key must be rejected");
        assert_eq!(invalid_argument_field(&err), "fabric.peers[0].public_key");
    }

    #[test]
    fn to_plan_rejects_bad_underlay_endpoint() {
        let mut plan = proto_plan();
        plan.peers[0].underlay_endpoint = "no-port".to_string();
        let err = to_plan("net-1", 100, &plan, defaults())
            .err()
            .expect("bad endpoint must be rejected");
        assert_eq!(
            invalid_argument_field(&err),
            "fabric.peers[0].underlay_endpoint"
        );
    }

    #[test]
    fn to_plan_rejects_bad_fabric_ip() {
        let mut plan = proto_plan();
        plan.peers[0].fabric_ip = "not-an-ip".to_string();
        let err = to_plan("net-1", 100, &plan, defaults())
            .err()
            .expect("bad fabric IP must be rejected");
        assert_eq!(invalid_argument_field(&err), "fabric.peers[0].fabric_ip");
    }

    #[test]
    fn to_plan_rejects_invalid_vni() {
        let err = to_plan("net-1", 0, &proto_plan(), defaults())
            .err()
            .expect("VNI 0 must be rejected");
        assert_eq!(invalid_argument_field(&err), "vni");
    }

    #[test]
    fn to_plan_rejects_mtu_math_violating_vxlan_headroom() {
        let mut plan = proto_plan();
        // tenant + 50 must not exceed fabric.
        plan.tenant_mtu = 1420;
        plan.fabric_mtu = 1440;
        let err = to_plan("net-1", 100, &plan, defaults())
            .err()
            .expect("MTU headroom violation must be rejected");
        assert_eq!(invalid_argument_field(&err), "fabric");
    }

    #[test]
    fn to_plan_rejects_local_host_in_peer_list() {
        let mut plan = proto_plan();
        plan.peers.push(proto::FabricPeer {
            node_id: "host-01".to_string(),
            public_key: SECOND_PUBLIC_KEY.to_string(),
            underlay_endpoint: "203.0.113.8:65001".to_string(),
            fabric_ip: "100.100.0.3".to_string(),
        });
        let err = to_plan("net-1", 100, &plan, defaults())
            .err()
            .expect("local host in peer list must be rejected");
        assert_eq!(invalid_argument_field(&err), "fabric");
    }

    // ---- NwdFabricProvider over the reference fake kernel ----------------

    fn test_provider(
        root: &std::path::Path,
        runner: Arc<StdMutex<RecordingRunner>>,
    ) -> NwdFabricProvider<RecordingRunner> {
        let config = FabricLinuxConfig::new(root).with_name_prefix("chv");
        NwdFabricProvider::with_shared_runner(config, runner, defaults())
            .expect("test provider must construct")
    }

    fn test_root(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "chv-fabric-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[tokio::test]
    async fn apply_realizes_fabric_and_network_idempotently() {
        let root = test_root("apply");
        let runner = Arc::new(StdMutex::new(RecordingRunner::new()));
        let provider = test_provider(&root, runner.clone());
        let names = Names::new("chv").expect("valid prefix");

        let first = provider
            .apply("net-1", 100, &proto_plan())
            .await
            .expect("first apply must succeed");
        assert!(
            first.report.created_fabric,
            "first apply creates the fabric"
        );
        assert!(
            first.report.created_network,
            "first apply creates the network"
        );
        assert_eq!(first.plan_generation, 1);
        assert_eq!(first.tenant_mtu, 1380);
        assert_eq!(first.consumer_veth, names.consumer_port_veth("net-1"));

        {
            let kernel = runner.lock().expect("runner lock");
            assert!(kernel.has_netns("chv-fabric"));
            assert!(kernel.has_link(&names.vxlan("net-1")));
            assert!(kernel.has_link(&first.consumer_veth));
        }

        let link_creates_after_first = runner
            .lock()
            .expect("runner lock")
            .calls()
            .iter()
            .filter(|call| call.args.iter().any(|arg| arg == "add"))
            .count();

        // Replaying the same plan must not recreate any object.
        let second = provider
            .apply("net-1", 100, &proto_plan())
            .await
            .expect("idempotent replay must succeed");
        assert!(!second.report.created_fabric);
        assert!(!second.report.created_network);
        assert_eq!(second.plan_generation, first.plan_generation);
        assert_eq!(second.tenant_mtu, first.tenant_mtu);
        assert_eq!(second.consumer_veth, first.consumer_veth);

        // The provider re-asserts WireGuard peer config on every apply
        // (`wg set` / `route replace`), but an unchanged plan must never
        // issue another `add`: no object is created twice.
        let link_creates_after_second = runner
            .lock()
            .expect("runner lock")
            .calls()
            .iter()
            .filter(|call| call.args.iter().any(|arg| arg == "add"))
            .count();
        assert_eq!(
            link_creates_after_first, link_creates_after_second,
            "replaying an unchanged plan must not create any object"
        );

        let _unused = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn remove_network_tears_down_and_preserves_the_fabric() {
        let root = test_root("remove");
        let runner = Arc::new(StdMutex::new(RecordingRunner::new()));
        let provider = test_provider(&root, runner.clone());
        let names = Names::new("chv").expect("valid prefix");

        provider
            .apply("net-1", 100, &proto_plan())
            .await
            .expect("apply must succeed");
        let vxlan = names.vxlan("net-1");

        provider
            .remove_network("net-1")
            .await
            .expect("remove must succeed");
        {
            let kernel = runner.lock().expect("runner lock");
            assert!(!kernel.has_link(&vxlan), "network VXLAN must be gone");
            assert!(
                kernel.has_netns("chv-fabric"),
                "the shared fabric must survive network teardown"
            );
        }

        // Teardown is idempotent: removing an unowned network is a no-op.
        provider
            .remove_network("net-1")
            .await
            .expect("remove must be idempotent");

        let _unused = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn identity_returns_public_key_without_leaking_private_material() {
        let root = test_root("identity");
        let runner = Arc::new(StdMutex::new(RecordingRunner::new()));
        let provider = test_provider(&root, runner.clone());

        let identity = provider.identity().await.expect("identity must succeed");
        assert_eq!(identity.public_key.len(), 44);
        assert!(identity.underlay_mtu > 0);

        {
            let kernel = runner.lock().expect("runner lock");
            // The fake kernel's genkey output is the stand-in private key:
            // it must never appear in any recorded command line.
            for call in kernel.calls() {
                assert!(
                    !call.joined().contains("fabric-test-private-key-material"),
                    "private key material leaked into argv: {}",
                    call.joined()
                );
            }
            // Derivation pipes the private key via stdin, not argv.
            let pubkey_call = kernel
                .calls()
                .iter()
                .find(|c| c.program == "wg" && c.args.first().map(String::as_str) == Some("pubkey"))
                .cloned()
                .expect("wg pubkey must have been invoked");
            assert_eq!(
                pubkey_call.stdin.as_deref(),
                Some("fabric-test-private-key-material")
            );
        }

        let _unused = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn overlay_status_reflects_apply_and_remove() {
        let root = test_root("status");
        let runner = Arc::new(StdMutex::new(RecordingRunner::new()));
        let provider = test_provider(&root, runner.clone());

        // Unknown network: honest zero status.
        let before = provider
            .overlay_status("net-1")
            .await
            .expect("status must succeed");
        assert!(!before.vxlan_interface_up);
        assert_eq!(before.fdb_entry_count, 0);

        provider
            .apply("net-1", 100, &proto_plan())
            .await
            .expect("apply must succeed");
        let during = provider
            .overlay_status("net-1")
            .await
            .expect("status must succeed");
        assert!(during.vxlan_interface_up);
        assert_eq!(during.fdb_entry_count, 1, "one HER flood peer");

        provider
            .remove_network("net-1")
            .await
            .expect("remove must succeed");
        let after = provider
            .overlay_status("net-1")
            .await
            .expect("status must succeed");
        assert!(!after.vxlan_interface_up);
        assert_eq!(after.fdb_entry_count, 0);

        let _unused = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn consumer_veth_name_is_deterministic() {
        let root = test_root("veth-name");
        let runner = Arc::new(StdMutex::new(RecordingRunner::new()));
        let provider = test_provider(&root, runner.clone());
        let names = Names::new("chv").expect("valid prefix");

        let veth = provider
            .consumer_veth("net-1")
            .await
            .expect("consumer veth name must resolve");
        assert_eq!(veth, names.consumer_port_veth("net-1"));
        assert!(veth.starts_with("chv-c-"));
        assert!(veth.len() < 16, "veth name must fit IFNAMSIZ: {veth}");

        let _unused = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fabric_error_mapping_is_structured() {
        let invalid = map_fabric_error(FabricError::Invalid("bad plan".to_string()));
        match invalid {
            ChvError::InvalidArgument { field, reason } => {
                assert_eq!(field, "fabric");
                assert_eq!(reason, "bad plan");
            }
            other => panic!("expected InvalidArgument, got {:?}", other),
        }

        let foreign = map_fabric_error(FabricError::ForeignState {
            object: "chv-x-abc".to_string(),
            expected: "vxlan id 100".to_string(),
            observed: "vxlan id 200".to_string(),
        });
        match foreign {
            ChvError::Conflict { resource, id } => {
                assert_eq!(resource, "fabric");
                assert!(id.contains("chv-x-abc"));
                assert!(id.contains("vxlan id 200"));
            }
            other => panic!("expected Conflict, got {:?}", other),
        }

        let command = map_fabric_error(FabricError::Command("wg set failed".to_string()));
        match command {
            ChvError::NetworkUnavailable { resource, reason } => {
                assert_eq!(resource, "fabric");
                assert_eq!(reason, "wg set failed");
            }
            other => panic!("expected NetworkUnavailable, got {:?}", other),
        }
    }
}
