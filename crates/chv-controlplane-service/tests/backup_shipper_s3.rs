//! Mock-S3 integration tests for the `object_store`-backed S3 shipper
//! (issue #177: rust-s3 replacement).
//!
//! These tests stand up an in-process HTTP server that plays a minimal
//! S3-compatible endpoint and verify the shipper's observable wiring:
//! the DELETE request carries the right method, path-style target and a
//! sigv4 (`AWS4-HMAC-SHA256`) Authorization header, success maps to
//! `Ok(())`, and an S3 error response maps to an `Internal` error.
//!
//! Non-goal: verifying sigv4 signature correctness itself — that is
//! object_store's responsibility and is covered by its own test suite.

use std::sync::{Arc, Mutex};

use axum::extract::State;
use chv_controlplane_service::shipper_from_destination;
use chv_errors::ChvError;
use tokio::net::TcpListener;

/// One recorded request: (method, path, Authorization header).
type RecordedRequest = (String, String, Option<String>);

#[derive(Default, Clone)]
struct RequestLog {
    entries: Arc<Mutex<Vec<RecordedRequest>>>,
}

async fn s3_handler(
    State((log, status)): State<(RequestLog, u16)>,
    req: axum::extract::Request,
) -> axum::response::Response {
    let (method, uri) = (req.method().clone(), req.uri().clone());
    let auth = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    log.entries
        .lock()
        .unwrap()
        .push((method.to_string(), uri.path().to_string(), auth));
    let body = if status == 204 {
        ""
    } else {
        "<?xml version=\"1.0\"?><Error><Code>AccessDenied</Code></Error>"
    };
    axum::http::Response::builder()
        .status(status)
        .body(axum::body::Body::from(body))
        .unwrap()
}

async fn spawn_mock_s3(status: u16) -> (String, RequestLog) {
    let log = RequestLog::default();
    let app = axum::Router::new().route(
        "/*rest",
        axum::routing::any(s3_handler).with_state((log.clone(), status)),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), log)
}

#[tokio::test]
async fn s3_shipper_delete_sends_signed_path_style_delete() {
    let (endpoint, log) = spawn_mock_s3(204).await;

    let shipper = shipper_from_destination(
        &format!("s3://qual-bucket/backups?region=us-east-1&endpoint={endpoint}"),
        Some("test-access-key".into()),
        Some("test-secret-key".into()),
    )
    .expect("shipper construction failed");

    shipper
        .delete("backups/qual-node-1/artifact.db")
        .await
        .expect("delete should succeed on 204");

    let entries = log.entries.lock().unwrap();
    assert_eq!(entries.len(), 1, "exactly one request expected");
    let (method, path, auth) = &entries[0];
    assert_eq!(method, "DELETE");
    assert_eq!(
        path, "/qual-bucket/backups/qual-node-1/artifact.db",
        "path-style addressing expected for a custom endpoint"
    );
    let auth = auth.as_deref().expect("Authorization header missing");
    assert!(
        auth.starts_with("AWS4-HMAC-SHA256"),
        "expected sigv4 authorization, got: {auth}"
    );
}

#[tokio::test]
async fn s3_shipper_delete_maps_s3_error_to_internal() {
    let (endpoint, _log) = spawn_mock_s3(403).await;

    let shipper = shipper_from_destination(
        &format!("s3://qual-bucket/backups?region=us-east-1&endpoint={endpoint}"),
        Some("test-access-key".into()),
        Some("test-secret-key".into()),
    )
    .expect("shipper construction failed");

    let err = shipper
        .delete("backups/qual-node-1/artifact.db")
        .await
        .expect_err("delete should fail on 403");
    match err {
        ChvError::Internal { reason } => {
            assert!(
                reason.contains("S3 delete failed"),
                "unexpected reason: {reason}"
            );
        }
        other => panic!("expected Internal error, got: {other:?}"),
    }
}

#[tokio::test]
async fn s3_shipper_delete_tolerates_leading_slash_key() {
    let (endpoint, log) = spawn_mock_s3(204).await;

    let shipper = shipper_from_destination(
        &format!("s3://qual-bucket/backups?region=us-east-1&endpoint={endpoint}"),
        Some("test-access-key".into()),
        Some("test-secret-key".into()),
    )
    .expect("shipper construction failed");

    shipper
        .delete("/backups/qual-node-1/artifact.db")
        .await
        .expect("delete should succeed on 204");

    let entries = log.entries.lock().unwrap();
    assert_eq!(entries.len(), 1);
    let (_, path, _) = &entries[0];
    assert_eq!(path, "/qual-bucket/backups/qual-node-1/artifact.db");
}
