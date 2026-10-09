//! Wire-level client tests over a real HTTP wire (wiremock, plain HTTP
//! via the test-only constructor). These cover the transport-hardening
//! guarantees that pure unit tests cannot: redirects are never
//! followed (F1) and pagination `next` links are strictly same-origin
//! (F9). Request recording proves the client never sent the request it
//! should have refused.

#![cfg(test)]

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::client::{ClientError, NetBoxClient, NetBoxToken};
use crate::mapping::{
    DeviceStatus, NetBoxDevice, NetBoxInterface, NetBoxIpAddress, NetBoxObject, NetBoxPrefix,
    NetBoxVirtualMachine, NetBoxVlan, VmStatus, CHV_NETBOX_DEVICE_ROLE, CHV_NETBOX_DEVICE_TYPE,
    CHV_NETBOX_MANUFACTURER,
};

const TOKEN: &str = "wire-test-token";

fn vlan_row(id: i64, vid: u32, name: &str) -> serde_json::Value {
    json!({ "id": id, "vid": vid, "name": name, "tags": [] })
}

fn page(results: Vec<serde_json::Value>, next: Option<String>) -> serde_json::Value {
    json!({
        "count": results.len(),
        "next": next,
        "results": results,
    })
}

/// A 302 response is an error, never a followed redirect: exactly one
/// request reaches the wire and the redirect target is never contacted
/// (following it could replay the `Authorization: Token …` header to an
/// `https → http` downgrade or a cross-origin host).
#[tokio::test]
async fn redirect_is_not_followed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/ipam/vlans/"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", "/api/elsewhere/"))
        .mount(&server)
        .await;

    let client =
        NetBoxClient::new_unchecked_for_tests(&server.uri(), NetBoxToken::new(TOKEN.into()))
            .expect("test client");
    let err = client
        .list_vlans_by_architecture("chv_architecture_id", "arch-1")
        .await
        .expect_err("302 must be an error");

    match &err {
        ClientError::Api { status, .. } => assert_eq!(*status, 302, "unexpected error: {err}"),
        other => panic!("expected Api(302), got {other:?}"),
    }

    // Exactly one request: the redirect target was never contacted.
    let requests = server.received_requests().await.expect("recording");
    assert_eq!(requests.len(), 1, "no second request after the 302");
    assert_eq!(requests[0].url.path(), "/api/ipam/vlans/");
}

/// A pagination `next` link with userinfo or a foreign host is rejected
/// fail-closed: the client errors and never sends the request.
#[tokio::test]
async fn pagination_next_link_with_userinfo_or_foreign_host_is_rejected() {
    for hostile_next in [
        // Userinfo trick: `netbox.example.com@evil.example.com` parses
        // as host evil.example.com with username netbox.example.com —
        // a plain string-prefix check would pass it.
        "https://netbox.example.com@evil.example.com/api/ipam/vlans/?page=2".to_string(),
        // Same host, scheme downgrade (https while the base is http).
        "https://127.0.0.1/api/ipam/vlans/?page=2".to_string(),
        // Plainly foreign host.
        "http://evil.example.com/api/ipam/vlans/?page=2".to_string(),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/ipam/vlans/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(page(vec![vlan_row(1, 42, "backend")], Some(hostile_next))),
            )
            .mount(&server)
            .await;

        let client =
            NetBoxClient::new_unchecked_for_tests(&server.uri(), NetBoxToken::new(TOKEN.into()))
                .expect("test client");
        let err = client
            .list_vlans_by_architecture("chv_architecture_id", "arch-1")
            .await
            .expect_err("hostile next link must be rejected");

        assert!(
            matches!(err, ClientError::MalformedResponse { ref reason } if reason.contains("escapes the endpoint")),
            "unexpected error: {err}"
        );

        // The follow-up request was never sent.
        let requests = server.received_requests().await.expect("recording");
        assert_eq!(requests.len(), 1, "only the first page was fetched");
    }
}

/// Positive control: a same-origin `next` link is still followed — the
/// guard hardens pagination without breaking it.
#[tokio::test]
async fn pagination_next_link_same_origin_is_followed() {
    let server = MockServer::start().await;
    let page2_url = format!("{}/api/ipam/vlans/?page=2", server.uri());

    Mock::given(method("GET"))
        .and(path("/api/ipam/vlans/"))
        .and(query_param("cf_chv_architecture_id", "arch-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(page(vec![vlan_row(1, 42, "backend")], Some(page2_url))),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/ipam/vlans/"))
        .and(query_param("page", "2"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(page(vec![vlan_row(2, 43, "storage")], None)),
        )
        .mount(&server)
        .await;

    let client =
        NetBoxClient::new_unchecked_for_tests(&server.uri(), NetBoxToken::new(TOKEN.into()))
            .expect("test client");
    let vlans = client
        .list_vlans_by_architecture("chv_architecture_id", "arch-1")
        .await
        .expect("pagination succeeds");

    assert_eq!(vlans.len(), 2);
    assert_eq!(vlans[0].netbox_id, 1);
    assert_eq!(vlans[1].netbox_id, 2);
    assert_eq!(
        server.received_requests().await.expect("recording").len(),
        2,
        "both pages were fetched"
    );
}

/// The write bodies carry NetBox 4.7's accepted reference forms, pinned
/// by inspecting the recorded requests (real-NetBox write-path
/// conformance, issue #586 PR 6):
///
/// - **tags** are name-dicts, `[{"name": …}]` — plain strings are a
///   400 on a real NetBox (`NestedTagSerializer` →
///   `get_related_object_by_attrs` accepts only a numeric PK or a
///   dict of attrs);
/// - **device creates** carry the required `device_type`
///   (manufacturer-scoped — a DeviceType slug alone is not globally
///   unique) and `role` references.
#[tokio::test]
async fn create_bodies_carry_netbox_write_reference_forms() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/dcim/devices/"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "id": 7 })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/ipam/vlans/"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "id": 42 })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/ipam/prefixes/"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "id": 43 })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/virtualization/virtual-machines/"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "id": 44 })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/virtualization/interfaces/"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "id": 45 })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/ipam/ip-addresses/"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "id": 46 })))
        .mount(&server)
        .await;
    // The IP create with an assigned interface first resolves the
    // interface's NetBox id by natural key (the inline half of the
    // assignment path; the runner's post-loop fix-up is the other).
    // The probe filters by `name` only — `virtual_machine` must NOT
    // be a query param (real NetBox types it as a choice filter that
    // 400s on values that do not exist yet; see
    // `get_interfaces_by_name`) — and the VM half of the key is
    // applied client-side, so a same-named interface on another VM
    // must not be picked (id 99 is the decoy; id 45 must win).
    Mock::given(method("GET"))
        .and(path("/api/virtualization/interfaces/"))
        .and(query_param("name", "net-backend"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "results": [
                {
                    "id": 99,
                    "name": "net-backend",
                    "virtual_machine": { "id": 98, "name": "other-vm" },
                    "tags": []
                },
                {
                    "id": 45,
                    "name": "net-backend",
                    "virtual_machine": { "id": 44, "name": "chv-vm-01" },
                    "tags": []
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        NetBoxClient::new_unchecked_for_tests(&server.uri(), NetBoxToken::new(TOKEN.into()))
            .expect("test client");
    let tags = vec!["chv-team".to_string(), "chv-env-production".to_string()];
    client
        .create_object(&NetBoxObject::Vlan(NetBoxVlan {
            vid: 42,
            name: "backend".to_string(),
            tags: tags.clone(),
            custom_fields: Default::default(),
        }))
        .await
        .expect("vlan create");
    client
        .create_object(&NetBoxObject::Prefix(NetBoxPrefix {
            prefix: "10.42.0.0/24".to_string(),
            vlan: Some(42),
            description: "backend (vlan)".to_string(),
            network_name: "backend".to_string(),
            tags: tags.clone(),
            custom_fields: Default::default(),
        }))
        .await
        .expect("prefix create");
    client
        .create_object(&NetBoxObject::Device(NetBoxDevice {
            name: "chv-node-01".to_string(),
            site: Some("dc1".to_string()),
            status: DeviceStatus::Active,
            tags: tags.clone(),
            custom_fields: Default::default(),
        }))
        .await
        .expect("device create");
    client
        .create_object(&NetBoxObject::VirtualMachine(NetBoxVirtualMachine {
            name: "chv-vm-01".to_string(),
            status: VmStatus::Active,
            cluster: None,
            device: Some("chv-node-01".to_string()),
            cpu: Some(2),
            memory_mb: Some(2048),
            tags: tags.clone(),
            custom_fields: Default::default(),
        }))
        .await
        .expect("vm create");
    client
        .create_object(&NetBoxObject::Interface(NetBoxInterface {
            name: "net-backend".to_string(),
            virtual_machine: "chv-vm-01".to_string(),
            description: "backend".to_string(),
            tags: tags.clone(),
            custom_fields: Default::default(),
        }))
        .await
        .expect("interface create");
    client
        .create_object(&NetBoxObject::IpAddress(NetBoxIpAddress {
            address: "10.42.0.10/24".to_string(),
            assigned_to_interface: Some("chv-vm-01/net-backend".to_string()),
            tags: tags.clone(),
            custom_fields: Default::default(),
        }))
        .await
        .expect("ip create");

    let requests = server.received_requests().await.expect("recording");
    let posts: Vec<&wiremock::Request> = requests
        .iter()
        .filter(|request| request.method.as_str() == "POST")
        .collect();
    assert_eq!(posts.len(), 6, "exactly the six creates");
    let mut bodies: Vec<serde_json::Value> = Vec::with_capacity(6);
    for request in posts {
        bodies.push(serde_json::from_slice(&request.body).expect("create body is JSON"));
    }
    // Every kind serializes tags as name-dicts, never plain strings.
    for body in &bodies {
        assert_eq!(
            body["tags"],
            json!([{ "name": "chv-team" }, { "name": "chv-env-production" }]),
            "tags serialize as name-dicts on every create body"
        );
    }
    let vlan_body = bodies
        .iter()
        .find(|body| body.get("vid").is_some())
        .expect("vlan body");
    assert_eq!(vlan_body["vid"], json!(42));
    let prefix_body = bodies
        .iter()
        .find(|body| body.get("prefix").is_some())
        .expect("prefix body");
    assert_eq!(prefix_body["prefix"], json!("10.42.0.0/24"));
    let vm_body = bodies
        .iter()
        .find(|body| body.get("cluster").is_some())
        .expect("vm body");
    assert_eq!(vm_body["device"], json!({ "name": "chv-node-01" }));
    let interface_body = bodies
        .iter()
        .find(|body| body.get("virtual_machine").is_some())
        .expect("interface body");
    assert_eq!(
        interface_body["virtual_machine"],
        json!({ "name": "chv-vm-01" })
    );
    let ip_body = bodies
        .iter()
        .find(|body| body.get("address").is_some())
        .expect("ip body");
    assert_eq!(ip_body["address"], json!("10.42.0.10/24"));
    assert_eq!(
        ip_body["assigned_object_type"],
        json!("virtualization.vminterface")
    );
    assert_eq!(
        ip_body["assigned_object_id"],
        json!(45),
        "an unambiguous interface match is assigned inline"
    );
    let device_body = bodies
        .iter()
        .find(|body| body.get("device_type").is_some())
        .expect("device body");
    assert_eq!(
        device_body["device_type"],
        json!({
            "manufacturer": { "slug": CHV_NETBOX_MANUFACTURER },
            "slug": CHV_NETBOX_DEVICE_TYPE,
        }),
        "device_type is referenced manufacturer-scoped (slug alone is not unique)"
    );
    assert_eq!(
        device_body["role"],
        json!({ "slug": CHV_NETBOX_DEVICE_ROLE }),
        "NetBox 4.7 requires a role on device creates"
    );
    // Wire form of the natural-key probe: name only. `virtual_machine`
    // as a query param is forbidden — real NetBox's choice filter
    // 400s on values that do not exist yet (see
    // `get_interfaces_by_name`), and the VM half of the key is
    // applied client-side on the parsed rows.
    for request in &requests {
        if request.method.as_str() == "GET" {
            let query = request.url.query().unwrap_or_default();
            assert!(
                !query
                    .split('&')
                    .any(|pair| pair.starts_with("virtual_machine=")),
                "interface probes must filter by name only, never a virtual_machine param: {query}"
            );
        }
    }
}
