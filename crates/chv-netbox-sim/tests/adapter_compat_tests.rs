//! Wire-compatibility suite: the **real** `NetBoxClient` from
//! `chv-netbox-adapter` (via its dev-only `test-http` constructor)
//! running against the in-process simulator.
//!
//! Direction matters: the adapter does NOT depend on the simulator;
//! this crate dev-depends on the adapter. These tests prove in PR 2
//! already that the simulator's responses satisfy the client's
//! fail-closed parser and its exact request surface (filters,
//! pagination `next`-link following, mutation status codes, auth).

mod common;

use chv_netbox_adapter::{
    ClientError, DeviceStatus, NetBoxClient, NetBoxDevice, NetBoxInterface, NetBoxIpAddress,
    NetBoxKind, NetBoxObject, NetBoxPrefix, NetBoxToken, NetBoxVirtualMachine, NetBoxVlan,
    VmStatus,
};
use chv_netbox_sim::{NetboxSim, NetboxSimConfig, SimKind};
use serde_json::{json, Value};

use common::TOKEN;

fn client_for(sim: &NetboxSim, token: &str) -> NetBoxClient {
    NetBoxClient::new_unchecked_for_tests(sim.base_url(), NetBoxToken::new(token.to_string()))
        .expect("test client")
}

/// Ownership custom fields every desired object carries — what the
/// projection always writes.
fn ownership_custom_fields(external_id: &str) -> std::collections::BTreeMap<String, String> {
    [
        ("chv_external_id".to_string(), external_id.to_string()),
        ("chv_architecture_id".to_string(), "arch-1".to_string()),
        ("chv_managed_by".to_string(), "chv".to_string()),
        ("chv_mapping_version".to_string(), "v1".to_string()),
    ]
    .into()
}

fn vlan_object(vid: u32) -> NetBoxObject {
    NetBoxObject::Vlan(NetBoxVlan {
        vid,
        name: format!("net-{vid}"),
        tags: vec!["chv-team".to_string()],
        custom_fields: ownership_custom_fields(&format!("arch:arch-1:network/net-{vid}:1")),
    })
}

fn device_object(name: &str, site: Option<&str>) -> NetBoxObject {
    NetBoxObject::Device(NetBoxDevice {
        name: name.to_string(),
        site: site.map(str::to_string),
        status: DeviceStatus::Active,
        tags: vec!["chv-team".to_string()],
        custom_fields: ownership_custom_fields(&format!("arch:arch-1:server/{name}:1")),
    })
}

// ---------------------------------------------------------------------------
// List + natural-key lookups round-trip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_with_filters_round_trips_through_the_fail_closed_parser() {
    let sim = common::start().await;
    common::seed(
        &sim,
        &json!({
            "devices": [
                { "id": 7, "name": "chv-node-01", "status": "active",
                  "site": { "name": "dc1" }, "tags": ["chv-team"],
                  "custom_fields": {
                      "chv_architecture_id": "arch-1",
                      "chv_external_id": "arch:arch-1:server/chv-node-01:1",
                      "chv_managed_by": "chv",
                      "chv_cpu_cores": 8
                  } },
                { "id": 8, "name": "foreign-node", "status": "active",
                  "custom_fields": { "chv_architecture_id": "arch-other" } }
            ],
            "virtual_machines": [
                { "id": 9, "name": "vm-01", "status": "active",
                  "device": { "name": "chv-node-01" }, "vcpus": 2, "memory": 2048 }
            ],
            "interfaces": [
                { "id": 11, "name": "backend", "virtual_machine": { "name": "vm-01" },
                  "description": "backend", "type": "virtual" }
            ],
            "prefixes": [ { "id": 3, "prefix": "10.42.0.0/24", "vlan": { "vid": 42 } } ],
            "vlans": [ { "id": 42, "vid": 42, "name": "backend" } ],
            "ip_addresses": [
                { "id": 21, "address": "10.42.0.5/24",
                  "assigned_object_type": "virtualization.vminterface",
                  "assigned_object_id": 11 }
            ]
        }),
    )
    .await;
    let client = client_for(&sim, TOKEN);

    // Custom-field filter (the by-architecture list the runner uses).
    let devices = client
        .list_devices_by_architecture("chv_architecture_id", "arch-1")
        .await
        .expect("list devices");
    assert_eq!(devices.len(), 1, "only this architecture's device");
    assert_eq!(devices[0].netbox_id, 7);
    assert_eq!(devices[0].object.natural_key["name"], "chv-node-01");
    // Enrichment custom fields survive the parser's scalar
    // stringification.
    assert_eq!(devices[0].object.custom_fields["chv_cpu_cores"], "8");

    // Natural-key probes, one per kind — every row must parse.
    let found = client
        .get_devices_by_name("chv-node-01")
        .await
        .expect("probe");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].object.content["site"], "dc1");
    assert_eq!(found[0].object.content["tags"], "chv-team");
    assert!(client
        .get_devices_by_name("missing")
        .await
        .expect("probe")
        .is_empty());

    let vms = client
        .get_virtual_machines_by_name("vm-01")
        .await
        .expect("probe");
    assert_eq!(vms.len(), 1);
    assert_eq!(vms[0].object.content["cpu"], "2");
    assert_eq!(vms[0].object.content["memory_mb"], "2048");
    assert_eq!(vms[0].object.content["device"], "chv-node-01");

    let interfaces = client
        .get_interfaces_by_name("backend", "vm-01")
        .await
        .expect("probe");
    assert_eq!(interfaces.len(), 1);
    assert_eq!(interfaces[0].object.natural_key["virtual_machine"], "vm-01");

    let prefixes = client
        .get_prefixes_by_cidr("10.42.0.0/24")
        .await
        .expect("probe");
    assert_eq!(prefixes.len(), 1);
    assert_eq!(prefixes[0].object.content["vlan"], "42");

    let vlans = client.get_vlans_by_vid(42).await.expect("probe");
    assert_eq!(vlans.len(), 1);
    assert_eq!(vlans[0].object.natural_key["vid"], "42");

    // Mask independence: the client probes maskless, the stored
    // address carries a mask.
    let ips = client
        .get_ip_addresses_by_address("10.42.0.5")
        .await
        .expect("probe");
    assert_eq!(ips.len(), 1);
    assert_eq!(ips[0].object.natural_key["address"], "10.42.0.5");
    // The assignment round-trips as "<vm>/<interface>".
    assert_eq!(
        ips[0].object.content["assigned_to_interface"],
        "vm-01/backend"
    );
}

// ---------------------------------------------------------------------------
// Create → list → update → delete lifecycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_list_update_delete_round_trip_across_all_kinds() {
    let sim = common::start().await;
    let client = client_for(&sim, TOKEN);

    let objects = vec![
        vlan_object(42),
        NetBoxObject::Prefix(NetBoxPrefix {
            prefix: "10.42.0.0/24".to_string(),
            vlan: Some(42),
            description: "backend (vlan)".to_string(),
            network_name: "backend".to_string(),
            tags: vec!["chv-team".to_string()],
            custom_fields: ownership_custom_fields("arch:arch-1:network/backend:1"),
        }),
        NetBoxObject::Interface(NetBoxInterface {
            name: "backend".to_string(),
            virtual_machine: "vm-01".to_string(),
            description: "backend".to_string(),
            tags: vec![],
            custom_fields: ownership_custom_fields("arch:arch-1:instance/vm-01/backend:1"),
        }),
        NetBoxObject::VirtualMachine(NetBoxVirtualMachine {
            name: "vm-01".to_string(),
            status: VmStatus::Active,
            cluster: None,
            // The device is created after the VM (contract kind
            // order) — the sim must accept the forward reference.
            device: Some("chv-node-01".to_string()),
            cpu: Some(2),
            memory_mb: Some(2048),
            tags: vec![],
            custom_fields: ownership_custom_fields("arch:arch-1:instance/vm-01:1"),
        }),
        device_object("chv-node-01", Some("dc1")),
    ];
    let mut ids = Vec::new();
    for object in &objects {
        let id = client.create_object(object).await.expect("create");
        ids.push(id);
    }
    // The IP address is created after its interface so the client's
    // assignment lookup resolves.
    let ip = NetBoxObject::IpAddress(NetBoxIpAddress {
        address: "10.42.0.5".to_string(),
        assigned_to_interface: Some("vm-01/backend".to_string()),
        tags: vec![],
        custom_fields: ownership_custom_fields("arch:arch-1:instance/vm-01/backend#10.42.0.5:1"),
    });
    let ip_id = client.create_object(&ip).await.expect("create ip");
    ids.push(ip_id);

    // Every created object is visible through the by-architecture
    // lists (which parse every row fail-closed).
    let kinds = [
        (NetBoxKind::Vlan, SimKind::Vlan),
        (NetBoxKind::Prefix, SimKind::Prefix),
        (NetBoxKind::IpAddress, SimKind::IpAddress),
        (NetBoxKind::Interface, SimKind::Interface),
        (NetBoxKind::VirtualMachine, SimKind::VirtualMachine),
        (NetBoxKind::Device, SimKind::Device),
    ];
    for (kind, _) in kinds {
        let listed = match kind {
            NetBoxKind::Vlan => {
                client
                    .list_vlans_by_architecture("chv_architecture_id", "arch-1")
                    .await
            }
            NetBoxKind::Prefix => {
                client
                    .list_prefixes_by_architecture("chv_architecture_id", "arch-1")
                    .await
            }
            NetBoxKind::IpAddress => {
                client
                    .list_ip_addresses_by_architecture("chv_architecture_id", "arch-1")
                    .await
            }
            NetBoxKind::Interface => {
                client
                    .list_interfaces_by_architecture("chv_architecture_id", "arch-1")
                    .await
            }
            NetBoxKind::VirtualMachine => {
                client
                    .list_virtual_machines_by_architecture("chv_architecture_id", "arch-1")
                    .await
            }
            NetBoxKind::Device => {
                client
                    .list_devices_by_architecture("chv_architecture_id", "arch-1")
                    .await
            }
        }
        .expect("list by architecture");
        assert_eq!(listed.len(), 1, "one {:?} visible after create", kind);
    }

    // The IP assignment was resolved by the client (interface lookup)
    // and round-trips back through the parser.
    let ips = client
        .get_ip_addresses_by_address("10.42.0.5")
        .await
        .expect("probe");
    assert_eq!(
        ips[0].object.content["assigned_to_interface"],
        "vm-01/backend"
    );
    // The maskless create was normalized to /32 and the parser strips
    // it for the natural key.
    assert_eq!(ips[0].object.natural_key["address"], "10.42.0.5");

    // Update: change the device's site, re-probe.
    let device_id = ids[4];
    client
        .update_object(device_id, &device_object("chv-node-01", Some("dc2")))
        .await
        .expect("update");
    let devices = client
        .get_devices_by_name("chv-node-01")
        .await
        .expect("probe");
    assert_eq!(devices[0].object.content["site"], "dc2");

    // mark_stale patches a PARTIAL custom_fields map plus a string
    // status — the merge semantics must keep ownership intact.
    client
        .mark_stale(device_id, NetBoxKind::Device, "chv_managed_state")
        .await
        .expect("mark stale");
    let devices = client
        .get_devices_by_name("chv-node-01")
        .await
        .expect("probe");
    assert_eq!(devices[0].object.content["status"], "decommissioning");
    assert_eq!(devices[0].object.custom_fields["chv_managed_by"], "chv");
    assert_eq!(
        devices[0].object.custom_fields["chv_managed_state"],
        "stale"
    );

    // Delete everything; the by-architecture lists empty out.
    // (id, kind) pairs in creation order: vlan, prefix, interface,
    // vm, device, ip.
    let deletions = [
        (ids[0], NetBoxKind::Vlan),
        (ids[1], NetBoxKind::Prefix),
        (ids[2], NetBoxKind::Interface),
        (ids[3], NetBoxKind::VirtualMachine),
        (ids[4], NetBoxKind::Device),
        (ip_id, NetBoxKind::IpAddress),
    ];
    for (id, kind) in deletions {
        client.delete_object(id, kind).await.expect("delete");
    }
    let devices = client
        .list_devices_by_architecture("chv_architecture_id", "arch-1")
        .await
        .unwrap();
    assert!(devices.is_empty());
    let vlans = client
        .list_vlans_by_architecture("chv_architecture_id", "arch-1")
        .await
        .unwrap();
    assert!(vlans.is_empty());
}

// ---------------------------------------------------------------------------
// Pagination across multiple pages
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pagination_follows_next_links_across_multiple_pages() {
    // Page size forced to 2 so five objects span three pages — well
    // under the client's MAX_PAGES bound of 10.
    let sim = common::start_with(NetboxSimConfig::new(TOKEN).with_page_sizes(2, 100)).await;
    let client = client_for(&sim, TOKEN);

    for vid in 41..=45 {
        client
            .create_object(&vlan_object(vid))
            .await
            .expect("create vlan");
    }
    let vlans = client
        .list_vlans_by_architecture("chv_architecture_id", "arch-1")
        .await
        .expect("list across pages");
    assert_eq!(vlans.len(), 5, "all pages were followed");
    let mut vids: Vec<String> = vlans
        .iter()
        .map(|vlan| vlan.object.natural_key["vid"].clone())
        .collect();
    vids.sort();
    assert_eq!(vids, ["41", "42", "43", "44", "45"]);
}

// ---------------------------------------------------------------------------
// Auth rejection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn wrong_token_is_an_auth_failure() {
    let sim = common::start().await;
    let client = client_for(&sim, "wrong-token");
    let err = client
        .list_devices_by_architecture("chv_architecture_id", "arch-1")
        .await
        .expect_err("auth must fail");
    assert!(
        matches!(err, ClientError::AuthFailed),
        "unexpected: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Error classification against the sim's NetBox-shaped errors
// ---------------------------------------------------------------------------

#[tokio::test]
async fn duplicate_create_is_a_permanent_api_error() {
    let sim = common::start().await;
    let client = client_for(&sim, TOKEN);

    client
        .create_object(&device_object("dup", None))
        .await
        .expect("create");
    let err = client
        .create_object(&device_object("dup", None))
        .await
        .expect_err("duplicate must fail");
    assert!(
        matches!(err, ClientError::Api { status: 400, .. }),
        "unexpected: {err:?}"
    );
    assert!(!err.is_transient(), "4xx semantics are permanent");
    // The NetBox error body is surfaced for operators.
    assert!(err.to_string().contains("This field must be unique."));
}

#[tokio::test]
async fn injected_faults_surface_as_the_right_client_errors() {
    let sim = common::start().await;
    let client = client_for(&sim, TOKEN);

    // 429 → transient Api error.
    common::set_fault(&sim, json!({ "rate_limit": true })).await;
    let err = client
        .list_vlans_by_architecture("chv_architecture_id", "arch-1")
        .await
        .expect_err("rate limit");
    assert!(
        matches!(err, ClientError::Api { status: 429, .. }),
        "unexpected: {err:?}"
    );
    assert!(err.is_transient());

    // 5xx → transient Api error.
    common::set_fault(&sim, json!({ "server_error": 503 })).await;
    let err = client
        .list_vlans_by_architecture("chv_architecture_id", "arch-1")
        .await
        .expect_err("server error");
    assert!(
        matches!(err, ClientError::Api { status: 503, .. }),
        "unexpected: {err:?}"
    );
    assert!(err.is_transient());

    // Forced auth failure → AuthFailed (transient per the client's
    // token-rotation policy).
    common::set_fault(&sim, json!({ "auth_failure": true })).await;
    let err = client
        .list_vlans_by_architecture("chv_architecture_id", "arch-1")
        .await
        .expect_err("auth failure");
    assert!(
        matches!(err, ClientError::AuthFailed),
        "unexpected: {err:?}"
    );

    // Connection drop → a transport/body-read failure, never a clean
    // response.
    common::set_fault(&sim, json!({ "connection_drop": true })).await;
    let err = client
        .list_vlans_by_architecture("chv_architecture_id", "arch-1")
        .await
        .expect_err("connection drop");
    assert!(
        matches!(
            err,
            ClientError::Unreachable { .. } | ClientError::MalformedResponse { .. }
        ),
        "unexpected: {err:?}"
    );
}

#[tokio::test]
async fn the_state_dump_reflects_client_writes() {
    // The PR-3 composed suite will assert via /__state; prove the
    // control-plane view agrees with what the client wrote.
    let sim = common::start().await;
    let client = client_for(&sim, TOKEN);

    let id = client
        .create_object(&vlan_object(42))
        .await
        .expect("create");
    let dump: Value = common::state(&sim).await;
    let vlans = dump["objects"]["vlans"].as_array().expect("vlans");
    assert_eq!(vlans.len(), 1);
    assert_eq!(vlans[0]["id"], json!(id));
    assert_eq!(vlans[0]["vid"], json!(42));
    assert_eq!(
        vlans[0]["custom_fields"]["chv_external_id"],
        json!("arch:arch-1:network/net-42:1")
    );
}
