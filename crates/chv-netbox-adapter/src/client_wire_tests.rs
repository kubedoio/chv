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
