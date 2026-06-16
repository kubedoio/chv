//! Deterministic topology synthesizer for perf benchmarking.
//!
//! Used by the Criterion bench at `benches/large_graph.rs` and by the
//! assertion-gate test at `tests/perf_large_graph.rs` (the latter is the
//! actual CI gate; see that file's module-level doc).
//!
//! Determinism: no `rand`, no clocks. Names are zero-padded
//! (`srv-0001..srv-NNNN`) so the topology is byte-stable across runs and
//! the bench/gate are reproducible.

#![allow(dead_code)] // shared by tests and benches; both targets compile this file

use chv_architecture_validate::model::{
    CHVArchitecture, Instance, InstanceNetwork, Metadata, Network, NetworkType, Server,
};

/// Build a [`CHVArchitecture`] with `n_servers` servers, `n_networks`
/// networks, and `n_instances` instances such that each instance has one
/// NIC attached (round-robin) to one of the synthesised networks. The
/// resulting topology has `n_instances` NIC-attachment edges and
/// `n_instances` placement edges (instance -> server), giving
/// `2 * n_instances` total edges if you count both kinds.
///
/// The model is shaped to:
/// - parse cleanly,
/// - exercise the heaviest static checks (duplicate-name, missing-ref,
///   ip-attachment, duplicate-ip, cidr-overlap),
/// - produce zero validation errors at synthesis time.
///
/// Each network is given a unique non-overlapping `/24` CIDR drawn from
/// `10.0.0.0/16` so the CIDR-overlap check has real work to do without
/// emitting findings. Instance IPs are issued from the corresponding
/// network's `/24` (host octet derived from the instance index), so the
/// ip-scope check runs without findings either.
pub fn synthesize_topology(
    n_servers: usize,
    n_networks: usize,
    n_instances: usize,
) -> CHVArchitecture {
    assert!(n_networks >= 1, "need at least one network");
    assert!(n_servers >= 1, "need at least one server");
    assert!(
        n_networks <= 256,
        "synthesizer addresses /24s out of 10.0.x.0/24; max 256 networks"
    );

    let servers: Vec<Server> = (0..n_servers)
        .map(|i| Server {
            name: format!("srv-{:04}", i + 1),
            management_ip: None,
            role: None,
            labels: Default::default(),
            resources: None,
            networks: None,
        })
        .collect();

    let networks: Vec<Network> = (0..n_networks)
        .map(|i| Network {
            name: format!("net-{:04}", i + 1),
            network_type: NetworkType::Bridge,
            bridge: Some(format!("br{}", i)),
            vlan_id: None,
            cidr: Some(format!("10.0.{}.0/24", i)),
            gateway: Some(format!("10.0.{}.1", i)),
            dns: vec![],
            dhcp: None,
        })
        .collect();

    let instances: Vec<Instance> = (0..n_instances)
        .map(|i| {
            let net_idx = i % n_networks;
            let net_name = format!("net-{:04}", net_idx + 1);
            // Host octet starts at 10 to avoid clashing with the gateway
            // (.1) and DHCP boundary noise. With /24 we have 245 usable
            // hosts per network. The Phase-7 800-edge profile is
            // (500, 50, 800) → 800/50 = 16 instances per network, well
            // within the /24 capacity envelope. The assertion below
            // makes any future bench-profile bump fail loudly instead
            // of silently wrapping octets.
            let host_octet = 10 + (i / n_networks);
            assert!(
                host_octet <= 254,
                "host_octet {host_octet} exceeds /24 capacity; reduce n_instances or grow n_networks"
            );
            Instance {
                name: format!("inst-{:04}", i + 1),
                template: None,
                placement: None,
                resources: None,
                disks: vec![],
                networks: vec![InstanceNetwork {
                    name: net_name,
                    ip: Some(format!("10.0.{}.{}", net_idx, host_octet)),
                }],
                cloud_init: None,
                backup: None,
                tags: vec![],
            }
        })
        .collect();

    CHVArchitecture {
        api_version: "chv.kubedo.io/v1alpha1".to_string(),
        kind: "CHVArchitecture".to_string(),
        metadata: Metadata {
            name: format!("synth-{}-{}-{}", n_servers, n_networks, n_instances),
            display_name: None,
            description: None,
            environment: None,
            owner: None,
            labels: Default::default(),
        },
        servers,
        networks,
        datastores: vec![],
        backup_targets: vec![],
        backup_policies: vec![],
        images: vec![],
        templates: vec![],
        instances,
        ssh_keys: vec![],
        instance_users: vec![],
        roles: vec![],
        users: vec![],
        projects: vec![],
    }
}
