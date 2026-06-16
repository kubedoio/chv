//! Starter topologies — six embedded YAML fixtures + the seeder algorithm.
//!
//! The fixtures are the canonical reference topologies described in
//! `docs/plans/2026-06-16-starter-topologies-and-auto-seed.md`:
//!
//! 1. Single Linux Dev VM (smallest useful topology)
//! 2. LAMP / WordPress single-server (classic web)
//! 3. Three-tier Web (multi-network, multi-NIC instance)
//! 4. Kubernetes HA stacked etcd (3+3, scale-out)
//! 5. Prometheus + Grafana observability
//! 6. K3s single-node edge (VLAN-tagged)
//!
//! All fixtures are embedded via `include_str!` so the binary is a
//! self-contained seeder.

mod graph;
mod seeder;

pub use seeder::{seed_if_first_deployment, seed_one, SeedOutcome};

/// Static metadata for one starter fixture: its slug (kebab-case),
/// human-readable label, environment override, and embedded YAML.
#[derive(Clone, Copy, Debug)]
pub struct StarterFixture {
    /// Kebab-case slug used inside the deterministic ID (e.g. `single-vm`).
    pub slug: &'static str,
    /// Human-readable name (matches `metadata.name` inside the YAML).
    pub name: &'static str,
    /// Environment recorded on the topology row.
    pub environment: &'static str,
    /// Verbatim YAML body, included at compile time.
    pub yaml: &'static str,
}

/// The six starter topologies, in seed order.
///
/// IDs are derived from the position in this array (`starter-NN-<slug>`),
/// so the order is part of the public contract. Re-ordering would change
/// the IDs and break any operator who has linked or scripted against them.
pub const STARTER_FIXTURES: &[StarterFixture] = &[
    StarterFixture {
        slug: "single-vm",
        name: "starter-single-vm",
        environment: "development",
        yaml: include_str!("fixtures/01-single-vm.yaml"),
    },
    StarterFixture {
        slug: "lamp-wordpress",
        name: "starter-lamp-wordpress",
        environment: "development",
        yaml: include_str!("fixtures/02-lamp-wordpress.yaml"),
    },
    StarterFixture {
        slug: "three-tier-web",
        name: "starter-three-tier-web",
        environment: "staging",
        yaml: include_str!("fixtures/03-three-tier-web.yaml"),
    },
    StarterFixture {
        slug: "k8s-ha",
        name: "starter-k8s-ha",
        environment: "staging",
        yaml: include_str!("fixtures/04-k8s-ha.yaml"),
    },
    StarterFixture {
        slug: "observability",
        name: "starter-observability",
        environment: "staging",
        yaml: include_str!("fixtures/05-observability.yaml"),
    },
    StarterFixture {
        slug: "k3s-edge",
        name: "starter-k3s-edge",
        environment: "development",
        yaml: include_str!("fixtures/06-k3s-edge.yaml"),
    },
];
