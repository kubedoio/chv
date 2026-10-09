//! Golden-fixture fidelity: the simulator's actual list responses
//! must serialize to exactly the shapes pinned under
//! `tests/fixtures/netbox4/` (see the directory's README for
//! provenance and the refresh rules).

mod common;

use chv_netbox_sim::SimKind;
use serde_json::Value;

/// The fixtures are single-object list envelopes; the only run-vary
/// value is the ephemeral host:port inside `url` fields, which is
/// canonicalized to the fixtures' stable base.
#[tokio::test]
async fn list_responses_match_the_golden_fixtures() {
    let sim = common::start().await;
    let payload: Value = serde_json::from_str(include_str!("fixtures/netbox4/seed.json"))
        .expect("seed fixture parses");
    common::seed(&sim, &payload).await;

    let cases = [
        (
            SimKind::Device,
            include_str!("fixtures/netbox4/dcim-devices.json"),
        ),
        (
            SimKind::VirtualMachine,
            include_str!("fixtures/netbox4/virtualization-virtual-machines.json"),
        ),
        (
            SimKind::Interface,
            include_str!("fixtures/netbox4/virtualization-interfaces.json"),
        ),
        (
            SimKind::Prefix,
            include_str!("fixtures/netbox4/ipam-prefixes.json"),
        ),
        (
            SimKind::Vlan,
            include_str!("fixtures/netbox4/ipam-vlans.json"),
        ),
        (
            SimKind::IpAddress,
            include_str!("fixtures/netbox4/ipam-ip-addresses.json"),
        ),
    ];

    for (kind, fixture) in cases {
        let response = common::get(&sim, kind.api_path()).await;
        assert_eq!(response.status(), 200, "list {kind}");
        let body = response.text().await.expect("body");
        let canonical = body.replace(sim.base_url(), "http://netbox.example.com");
        let live: Value = serde_json::from_str(&canonical)
            .unwrap_or_else(|error| panic!("list {kind} body is not JSON: {error}\n{canonical}"));
        let expected: Value = serde_json::from_str(fixture)
            .unwrap_or_else(|error| panic!("fixture for {kind} is not JSON: {error}"));
        assert_eq!(live, expected, "fixture drift for {kind}");
    }
}

/// The seed fixture itself must stay loadable through the documented
/// seed-file format (the `netbox-sim` binary's `--seed-file` input).
#[tokio::test]
async fn seed_fixture_loads_through_the_seed_file_format() {
    let payload: chv_netbox_sim::SeedPayload =
        serde_json::from_str(include_str!("fixtures/netbox4/seed.json"))
            .expect("seed fixture parses as a SeedPayload");
    let sim = common::start().await;
    let counts = sim.seed(&payload).expect("seed applies");
    assert_eq!(counts[&SimKind::Device], 1);
    assert_eq!(counts[&SimKind::IpAddress], 1);
    // The dump agrees with the seed.
    let dump = common::state(&sim).await;
    assert_eq!(dump["objects"]["devices"][0]["id"], serde_json::json!(7));
}
