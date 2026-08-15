use super::*;
use axum::body::Body;
use axum::http::Request;
use cellhv_core_operations::{AuthorityActor, AuthorityActorJoin, OperationService};
use cellhv_core_types::{HostId, HostIdentity};
use http_body_util::BodyExt;
use std::os::unix::fs::PermissionsExt;
use tower::ServiceExt;

fn service(dir: &tempfile::TempDir) -> OperationService {
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    OperationService::create_new(
        &dir.path().join("core.db"),
        &HostIdentity {
            id: HostId::new("host-1").unwrap(),
            resource_version: ResourceVersion::new(1).unwrap(),
        },
    )
    .unwrap()
}
fn app(dir: &tempfile::TempDir) -> (Router, AuthorityActorJoin) {
    let (authority, owner) = AuthorityActor::spawn(service(dir), 16).unwrap();
    (router(authority), owner)
}
async fn join_app(app: Router, owner: AuthorityActorJoin) {
    drop(app);
    owner.join().await.unwrap();
}
fn vm_request(request_id: &str, version: u64) -> serde_json::Value {
    serde_json::json!({"request_id":request_id,"definition":{
        "id":"vm-1","name":"vm-1","boot":{"kernel":"/kernel","firmware":null,"initial_disk":null},
        "compute":{"vcpus":1,"memory_bytes":1048576},"storage":[],"networks":[],
        "requested_power_state":"stopped","observed_power_state":"unknown","resource_version":version
    }})
}
async fn body_json(response: Response) -> serde_json::Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

#[tokio::test]
async fn contract_snapshot_is_exact_and_capabilities_are_false() {
    let contract: serde_json::Value = serde_json::from_str(CONTRACT_V1).unwrap();
    assert_eq!(contract["version"], 1);
    assert_eq!(contract["unknown_method_status"], 405);
    assert_eq!(
        contract["routes"],
        serde_json::json!([
            "GET /v1/host",
            "GET /v1/host/capabilities",
            "GET /v1/vms",
            "POST /v1/vms",
            "GET /v1/vms/{vm_id}",
            "PATCH /v1/vms/{vm_id}",
            "DELETE /v1/vms/{vm_id}",
            "POST /v1/vms/{vm_id}/actions/start",
            "POST /v1/vms/{vm_id}/actions/stop",
            "POST /v1/vms/{vm_id}/actions/reboot",
            "GET /v1/operations",
            "GET /v1/operations/{operation_id}",
            "GET /v1/events"
        ])
    );
    let dir = tempfile::tempdir().unwrap();
    let (app, owner) = app(&dir);
    let response = app
        .clone()
        .oneshot(
            Request::get("/v1/host/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        body_json(response).await,
        serde_json::to_value(HostCapabilities::default()).unwrap()
    );
    join_app(app, owner).await;
}

#[tokio::test]
async fn definition_mutations_reject_invalid_request_ids_as_structured_400() {
    let dir = tempfile::tempdir().unwrap();
    let (app, owner) = app(&dir);
    let mut invalid = vm_request("   ", 1);
    let create = Request::post("/v1/vms")
        .header("content-type", "application/json")
        .header("idempotency-key", "create")
        .body(Body::from(serde_json::to_vec(&invalid).unwrap()))
        .unwrap();
    invalid["definition"]["resource_version"] = serde_json::json!(2);
    let update = Request::patch("/v1/vms/vm-1")
        .header("content-type", "application/json")
        .header("idempotency-key", "update")
        .header("if-match", "\"1\"")
        .body(Body::from(serde_json::to_vec(&invalid).unwrap()))
        .unwrap();
    let delete = Request::delete("/v1/vms/vm-1")
        .header("content-type", "application/json")
        .header("idempotency-key", "delete")
        .header("if-match", "\"1\"")
        .body(Body::from(r#"{"request_id":" "}"#))
        .unwrap();
    for request in [create, update, delete] {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body_json(response).await["code"], "invalid_request");
    }
    join_app(app, owner).await;
}

#[tokio::test]
async fn wrong_method_is_structured_405() {
    let dir = tempfile::tempdir().unwrap();
    let (app, owner) = app(&dir);
    let response = app
        .clone()
        .oneshot(Request::put("/v1/host").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    let body = body_json(response).await;
    assert_eq!(body["code"], "method_not_allowed");
    join_app(app, owner).await;
}

#[tokio::test]
async fn create_is_namespaced_durable_idempotent_and_readable() {
    let dir = tempfile::tempdir().unwrap();
    let (app, owner) = app(&dir);
    let create = |request_id: &str| {
        Request::post("/v1/vms")
            .header("content-type", "application/json")
            .header("idempotency-key", "create-1")
            .body(Body::from(
                serde_json::to_vec(&vm_request(request_id, 1)).unwrap(),
            ))
            .unwrap()
    };
    assert_eq!(
        app.clone()
            .oneshot(create("request-1"))
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );
    let replay = app.clone().oneshot(create("ignored")).await.unwrap();
    let replay = body_json(replay).await;
    assert_eq!(replay["disposition"], "replay");
    assert_eq!(replay["operation_id"], "native:v1:request-1");
    for uri in [
        "/v1/vms/vm-1",
        "/v1/operations/native:v1:request-1",
        "/v1/events?after=0&limit=10",
    ] {
        assert_eq!(
            app.clone()
                .oneshot(Request::get(uri).body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
    join_app(app, owner).await;
}

#[tokio::test]
async fn lifecycle_validates_complete_request_before_unsupported_and_never_journals() {
    let dir = tempfile::tempdir().unwrap();
    let (app, owner) = app(&dir);
    let action = |path: &str, key: Option<&str>, etag: Option<&str>, body: &str| {
        let mut request = Request::post(path).header("content-type", "application/json");
        if let Some(value) = key {
            request = request.header("idempotency-key", value);
        }
        if let Some(value) = etag {
            request = request.header("if-match", value);
        }
        request.body(Body::from(body.to_owned())).unwrap()
    };
    assert_eq!(
        app.clone()
            .oneshot(action(
                "/v1/vms/%20/actions/start",
                Some("k"),
                Some("\"1\""),
                r#"{"request_id":"r"}"#
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.clone()
            .oneshot(action(
                "/v1/vms/vm-1/actions/start",
                None,
                Some("\"1\""),
                r#"{"request_id":"r"}"#
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.clone()
            .oneshot(action(
                "/v1/vms/vm-1/actions/start",
                Some("k"),
                Some("1"),
                r#"{"request_id":"r"}"#
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.clone()
            .oneshot(action(
                "/v1/vms/vm-1/actions/start",
                Some("k"),
                Some("\"1\""),
                "{"
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let valid = app
        .clone()
        .oneshot(action(
            "/v1/vms/vm-1/actions/start",
            Some("k"),
            Some("\"1\""),
            r#"{"request_id":"r"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(valid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let operations = app
        .clone()
        .oneshot(Request::get("/v1/operations").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(body_json(operations).await, serde_json::json!([]));
    join_app(app, owner).await;
}

#[tokio::test]
async fn malformed_inputs_and_internal_failures_are_structured_and_redacted() {
    let dir = tempfile::tempdir().unwrap();
    let (app, owner) = app(&dir);
    for request in [
        Request::post("/v1/vms")
            .header("content-type", "application/json")
            .body(Body::from("{"))
            .unwrap(),
        Request::get("/v1/events?after=nope")
            .body(Body::empty())
            .unwrap(),
        Request::get("/v1/vms/%20").body(Body::empty()).unwrap(),
    ] {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body_json(response).await["code"], "invalid_request");
    }
    let parse_error = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
    let response = ApiError::Service(OperationServiceError::Json(parse_error)).into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = body_json(response).await;
    assert_eq!(body["message"], "Core authority request failed");
    assert!(!body.to_string().contains("line 1"));
    join_app(app, owner).await;
}

#[tokio::test]
async fn stopped_shared_authority_is_structured_503() {
    let dir = tempfile::tempdir().unwrap();
    let (authority, owner) = AuthorityActor::spawn(service(&dir), 1).unwrap();
    let app = router(authority.clone());
    authority.shutdown().await.unwrap();

    let response = app
        .clone()
        .oneshot(Request::get("/v1/host").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body_json(response).await,
        serde_json::json!({
            "code": "authority_unavailable",
            "message": "Core authority is unavailable"
        })
    );
    drop(app);
    owner.join().await.unwrap();
}

#[tokio::test]
async fn strict_preconditions_use_412_for_stale_versions() {
    let dir = tempfile::tempdir().unwrap();
    let (app, owner) = app(&dir);
    let create = Request::post("/v1/vms")
        .header("content-type", "application/json")
        .header("idempotency-key", "create")
        .body(Body::from(
            serde_json::to_vec(&vm_request("create", 1)).unwrap(),
        ))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(create).await.unwrap().status(),
        StatusCode::ACCEPTED
    );
    for invalid in ["1", "W/\"1\"", "\"01\"", "\"0\"", "\"+1\"", "\"1\", \"2\""] {
        let request = Request::patch("/v1/vms/vm-1")
            .header("content-type", "application/json")
            .header("idempotency-key", format!("k-{invalid}"))
            .header("if-match", invalid)
            .body(Body::from(
                serde_json::to_vec(&vm_request("update", 2)).unwrap(),
            ))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );
    }
    let stale = Request::patch("/v1/vms/vm-1")
        .header("content-type", "application/json")
        .header("idempotency-key", "stale")
        .header("if-match", "\"2\"")
        .body(Body::from(
            serde_json::to_vec(&vm_request("stale", 3)).unwrap(),
        ))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(stale).await.unwrap().status(),
        StatusCode::PRECONDITION_FAILED
    );
    join_app(app, owner).await;
}
