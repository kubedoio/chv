//! The v1 metric registry — the explicit allowlist every sample's
//! `metric_id` must exist in (`docs/specs/contracts/
//! chv-monitoring-metrics-v1.md`, "Registry of initial metrics").
//!
//! Unknown metrics are never sampled, stored or queried; adding a metric is
//! a reviewed registry extension, and breaking unit/identity changes
//! require v2 IDs or a new schema version — never a silent interpretation
//! change.
//!
//! Registry notes:
//! - `derived` metrics (`vm.cpu.capacity_ratio`, `vm.cpu.assigned_vcpus`,
//!   `vm.memory.provisioned_bytes`,
//!   `monitoring.agent.last_seen_age_seconds`) are computed at query time
//!   from stored samples and configuration. They appear here so query-side
//!   lookups resolve, but sources must not emit them; the only allowed
//!   source is [`Source::Derived`].
//! - Guest-agent and check metrics appear here so the registry is the
//!   complete v1 allowlist, but nothing produces them until ADR-026's
//!   optional guest agent lands (G3/G4).
//! - `check.status` is a typed state (`ok`/`warning`/`critical`/`unknown`),
//!   never a float; its value modelling arrives with the guest-agent
//!   implementation. No PR-1 producer exists.

use crate::model::{MetricKind, Source, Unit};

/// One registry entry: identity, kind, unit, allowed sources and the
/// dimension allowlist for the metric.
#[derive(Debug)]
pub struct MetricDef {
    /// Canonical dotted ID (Prometheus export uses a separately maintained
    /// `chv_*` underscore name registry, not this string).
    pub id: &'static str,
    pub kind: MetricKind,
    pub unit: Unit,
    /// Observation layers that may produce this metric. Values are
    /// attributed only to the layer that actually observed them.
    pub allowed_sources: &'static [Source],
    /// Registered dimension keys a sample of this metric may carry
    /// (bounded to [`crate::model::MAX_DIMENSIONS`] per sample).
    pub dimensions: &'static [&'static str],
}

use MetricKind::{Counter, Gauge, State};
use Source::{Derived, GuestAgent, NetworkProvider, NodeOs, StorageProvider, VmCgroup, Vmm};
use Unit::{Boolean, Bytes, Cores, Count, Ratio, Seconds};

const NODE_OS: &[Source] = &[NodeOs];
const VMM_OR_CGROUP: &[Source] = &[Vmm, VmCgroup];
const DERIVED_ONLY: &[Source] = &[Derived];
const GUEST_ONLY: &[Source] = &[GuestAgent];

/// The complete v1 metric allowlist. Order mirrors the contract's registry
/// table.
pub static REGISTRY: &[MetricDef] = &[
    // ---- node metrics (node_os) ----
    MetricDef {
        id: "node.cpu.capacity_ratio",
        kind: Gauge,
        unit: Ratio,
        allowed_sources: NODE_OS,
        dimensions: &[],
    },
    MetricDef {
        id: "node.cpu.load1",
        kind: Gauge,
        unit: Count,
        allowed_sources: NODE_OS,
        dimensions: &[],
    },
    MetricDef {
        id: "node.cpu.load5",
        kind: Gauge,
        unit: Count,
        allowed_sources: NODE_OS,
        dimensions: &[],
    },
    MetricDef {
        id: "node.cpu.load15",
        kind: Gauge,
        unit: Count,
        allowed_sources: NODE_OS,
        dimensions: &[],
    },
    MetricDef {
        id: "node.memory.total_bytes",
        kind: Gauge,
        unit: Bytes,
        allowed_sources: NODE_OS,
        dimensions: &[],
    },
    MetricDef {
        id: "node.memory.available_bytes",
        kind: Gauge,
        unit: Bytes,
        allowed_sources: NODE_OS,
        dimensions: &[],
    },
    MetricDef {
        id: "node.swap.used_bytes",
        kind: Gauge,
        unit: Bytes,
        allowed_sources: NODE_OS,
        dimensions: &[],
    },
    MetricDef {
        id: "node.memory.psi_some_ratio",
        kind: Gauge,
        unit: Ratio,
        allowed_sources: NODE_OS,
        dimensions: &["window"],
    },
    MetricDef {
        id: "node.fs.available_bytes",
        kind: Gauge,
        unit: Bytes,
        allowed_sources: NODE_OS,
        dimensions: &["mount_id"],
    },
    MetricDef {
        id: "node.fs.total_bytes",
        kind: Gauge,
        unit: Bytes,
        allowed_sources: NODE_OS,
        dimensions: &["mount_id"],
    },
    MetricDef {
        id: "node.net.rx_bytes_total",
        kind: Counter,
        unit: Bytes,
        allowed_sources: NODE_OS,
        dimensions: &["interface_id"],
    },
    MetricDef {
        id: "node.net.tx_bytes_total",
        kind: Counter,
        unit: Bytes,
        allowed_sources: NODE_OS,
        dimensions: &["interface_id"],
    },
    MetricDef {
        id: "node.block.read_bytes_total",
        kind: Counter,
        unit: Bytes,
        allowed_sources: NODE_OS,
        dimensions: &["block_device_id"],
    },
    MetricDef {
        id: "node.block.write_bytes_total",
        kind: Counter,
        unit: Bytes,
        allowed_sources: NODE_OS,
        dimensions: &["block_device_id"],
    },
    // ---- VM metrics ----
    MetricDef {
        id: "vm.cpu.cores_used",
        kind: Gauge,
        unit: Cores,
        allowed_sources: VMM_OR_CGROUP,
        dimensions: &[],
    },
    MetricDef {
        id: "vm.cpu.capacity_ratio",
        kind: Gauge,
        unit: Ratio,
        allowed_sources: DERIVED_ONLY,
        dimensions: &[],
    },
    MetricDef {
        id: "vm.cpu.assigned_vcpus",
        kind: Gauge,
        unit: Count,
        allowed_sources: DERIVED_ONLY,
        dimensions: &[],
    },
    MetricDef {
        id: "vm.memory.provisioned_bytes",
        kind: Gauge,
        unit: Bytes,
        allowed_sources: DERIVED_ONLY,
        dimensions: &[],
    },
    MetricDef {
        id: "vm.memory.host_accounted_bytes",
        kind: Gauge,
        unit: Bytes,
        allowed_sources: &[VmCgroup, Vmm],
        dimensions: &[],
    },
    MetricDef {
        id: "vm.memory.guest_available_bytes",
        kind: Gauge,
        unit: Bytes,
        allowed_sources: GUEST_ONLY,
        dimensions: &[],
    },
    MetricDef {
        id: "vm.block.read_bytes_total",
        kind: Counter,
        unit: Bytes,
        allowed_sources: &[Vmm, StorageProvider],
        dimensions: &["block_device_id"],
    },
    MetricDef {
        id: "vm.block.write_bytes_total",
        kind: Counter,
        unit: Bytes,
        allowed_sources: &[Vmm, StorageProvider],
        dimensions: &["block_device_id"],
    },
    MetricDef {
        id: "vm.net.rx_bytes_total",
        kind: Counter,
        unit: Bytes,
        allowed_sources: &[Vmm, NetworkProvider],
        dimensions: &["interface_id"],
    },
    MetricDef {
        id: "vm.net.tx_bytes_total",
        kind: Counter,
        unit: Bytes,
        allowed_sources: &[Vmm, NetworkProvider],
        dimensions: &["interface_id"],
    },
    // ---- guest metrics (ADR-026; produced from G3/G4) ----
    MetricDef {
        id: "vm.guest.fs.available_bytes",
        kind: Gauge,
        unit: Bytes,
        allowed_sources: GUEST_ONLY,
        dimensions: &["mount_id"],
    },
    MetricDef {
        id: "vm.guest.fs.total_bytes",
        kind: Gauge,
        unit: Bytes,
        allowed_sources: GUEST_ONLY,
        dimensions: &["mount_id"],
    },
    MetricDef {
        id: "vm.guest.service.up",
        kind: State,
        unit: Boolean,
        allowed_sources: GUEST_ONLY,
        dimensions: &["service_key"],
    },
    MetricDef {
        id: "vm.guest.process.count",
        kind: Gauge,
        unit: Count,
        allowed_sources: GUEST_ONLY,
        dimensions: &["process_selector"],
    },
    MetricDef {
        id: "vm.guest.net.rx_errors_total",
        kind: Counter,
        unit: Count,
        allowed_sources: GUEST_ONLY,
        dimensions: &["interface_id"],
    },
    MetricDef {
        id: "check.duration_seconds",
        kind: Gauge,
        unit: Seconds,
        allowed_sources: GUEST_ONLY,
        dimensions: &["check_id"],
    },
    MetricDef {
        id: "check.status",
        kind: State,
        unit: Count,
        allowed_sources: GUEST_ONLY,
        dimensions: &["check_id"],
    },
    // ---- manager-side derived ----
    MetricDef {
        id: "monitoring.agent.last_seen_age_seconds",
        kind: Gauge,
        unit: Seconds,
        allowed_sources: DERIVED_ONLY,
        dimensions: &[],
    },
];

/// Registry lookup. `None` means the metric is not in the allowlist and
/// must be rejected everywhere (sampling, ingestion, query).
pub fn lookup(metric_id: &str) -> Option<&'static MetricDef> {
    REGISTRY.iter().find(|d| d.id == metric_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_ids_are_unique() {
        let ids: HashSet<&str> = REGISTRY.iter().map(|d| d.id).collect();
        assert_eq!(ids.len(), REGISTRY.len(), "duplicate metric ids");
    }

    #[test]
    fn lookup_resolves_contract_ids() {
        assert_eq!(lookup("vm.cpu.cores_used").map(|d| d.unit), Some(Cores));
        assert_eq!(
            lookup("node.net.rx_bytes_total").map(|d| d.kind),
            Some(Counter)
        );
        assert!(lookup("node.cpu.capacity_ratio").is_some());
        assert!(lookup("monitoring.agent.last_seen_age_seconds").is_some());
        assert!(lookup("made.up.metric").is_none());
    }

    #[test]
    fn counters_declare_epoch_relevant_sources_only() {
        // Counter metrics must never allow Derived (derived values are
        // query-time only and cannot carry counter epochs).
        for def in REGISTRY.iter().filter(|d| d.kind == Counter) {
            assert!(
                !def.allowed_sources.contains(&Derived),
                "counter {} allows derived source",
                def.id
            );
        }
    }

    #[test]
    fn dimensions_stay_within_allowlist() {
        const ALLOWED: &[&str] = &[
            "interface_id",
            "block_device_id",
            "mount_id",
            "service_key",
            "process_selector",
            "window",
            "direction",
            "check_id",
        ];
        for def in REGISTRY {
            for dim in def.dimensions {
                assert!(
                    ALLOWED.contains(dim),
                    "metric {} declares unknown dimension {}",
                    def.id,
                    dim
                );
            }
            assert!(
                def.dimensions.len() <= crate::model::MAX_DIMENSIONS,
                "metric {} declares too many dimensions",
                def.id
            );
        }
    }

    #[test]
    fn psi_requires_window_dimension() {
        // Contract: window-defined memory PSI requires a `window`
        // dimension.
        let def = lookup("node.memory.psi_some_ratio").unwrap();
        assert!(def.dimensions.contains(&"window"));
    }
}
