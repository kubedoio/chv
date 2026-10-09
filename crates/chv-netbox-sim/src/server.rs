//! The axum server: the six NetBox endpoint families, the `__`-
//! prefixed control plane, and the in-process [`NetboxSim`] handle.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_stream::once as stream_once;
use tracing::{debug, warn};

use crate::config::NetboxSimConfig;
use crate::fault::FaultConfig;
use crate::kind::SimKind;
use crate::state::{SeedPayload, SimShared};
use crate::wire::{effective_limit, page_links, WireError, NOT_FOUND_DETAIL};

// ---------------------------------------------------------------------------
// Handle
// ---------------------------------------------------------------------------

/// A running simulator instance.
///
/// Dropping the handle triggers a graceful shutdown of the server
/// task (in-flight requests complete); [`NetboxSim::shutdown`] waits
/// for it explicitly. The handle also exposes the control-plane
/// operations directly ([`NetboxSim::seed`], [`NetboxSim::reset`]) so
/// the `netbox-sim` binary can load a seed file race-free at startup.
pub struct NetboxSim {
    shared: Arc<SimShared>,
    shutdown_tx: watch::Sender<()>,
    local_addr: SocketAddr,
    base_url: String,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl NetboxSim {
    /// Start a simulator on an ephemeral port (`127.0.0.1:0`).
    pub async fn start(config: NetboxSimConfig) -> std::io::Result<Self> {
        Self::start_on(SocketAddr::from(([127, 0, 0, 1], 0)), config).await
    }

    /// Start a simulator bound to `addr` (used by the `netbox-sim`
    /// binary and tests that need a fixed port).
    pub async fn start_on(addr: SocketAddr, config: NetboxSimConfig) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;
        let shared = Arc::new(SimShared::new(config));
        let app = router(shared.clone());
        let (shutdown_tx, mut shutdown_rx) = watch::channel(());
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    // Completes on an explicit shutdown or when the
                    // last handle (and the server task's own copy of
                    // the app state) is dropped.
                    let _ = shutdown_rx.changed().await;
                })
                .await
        });
        debug!(%local_addr, "netbox simulator listening");
        Ok(Self {
            shared,
            shutdown_tx,
            local_addr,
            base_url: format!("http://{local_addr}"),
            task,
        })
    }

    /// The simulator's base URL, e.g. `http://127.0.0.1:39291` (no
    /// trailing slash — the same shape the adapter client expects).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The address the server is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// The shared state (for the control-plane helpers below and for
    /// the `netbox-sim` binary).
    pub fn shared(&self) -> &Arc<SimShared> {
        &self.shared
    }

    /// Bulk-load objects (same payload and rules as `POST /__seed`).
    pub fn seed(
        &self,
        payload: &SeedPayload,
    ) -> Result<std::collections::BTreeMap<SimKind, usize>, WireError> {
        self.shared.lock().seed(payload)
    }

    /// Clear objects and faults (same as `POST /__reset`).
    pub fn reset(&self) {
        self.shared.lock().reset();
    }

    /// Gracefully stop the server and wait for it to finish.
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
        if let Err(err) = self.task.await {
            warn!(%err, "netbox simulator server task failed during shutdown");
        }
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

fn router(shared: Arc<SimShared>) -> Router {
    Router::new()
        // The six endpoint families. Only the client's used methods
        // are registered: GET/POST on the list paths and PATCH/DELETE
        // on the detail paths — anything else (e.g. GET on a detail
        // path, PUT anywhere) is outside the contract surface.
        .route("/api/:app/:resource/", get(list).post(create))
        .route(
            "/api/:app/:resource/:id/",
            patch(patch_object).delete(delete_object),
        )
        // The `__`-prefixed test-control plane (ADR-024): obviously
        // never provided by a real NetBox, unauthenticated, never
        // referenced by production code.
        .route("/__seed", post(seed))
        .route("/__state", get(state_dump))
        .route("/__reset", post(reset))
        .route("/__faults", post(set_faults))
        .fallback(not_found)
        .with_state(shared)
}

// ---------------------------------------------------------------------------
// Shared plumbing
// ---------------------------------------------------------------------------

/// The absolute URL base for the request (the simulator serves plain
/// HTTP; `url` fields and pagination links are built from the Host
/// header so they always match the origin the client reached).
fn request_base(headers: &HeaderMap) -> String {
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("127.0.0.1");
    format!("http://{host}")
}

async fn not_found() -> Response {
    not_found_response()
}

fn not_found_response() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "detail": NOT_FOUND_DETAIL })),
    )
        .into_response()
}

fn unauthorized(detail: &str) -> Response {
    let mut response =
        (StatusCode::UNAUTHORIZED, Json(json!({ "detail": detail }))).into_response();
    response.headers_mut().insert(
        axum::http::header::WWW_AUTHENTICATE,
        HeaderValue::from_static("Token"),
    );
    response
}

/// Validate `Authorization: Token <t>` against the configured tokens
/// (the only auth scheme the adapter client uses). Returns the 401
/// response when the request must be rejected.
fn check_auth(shared: &SimShared, headers: &HeaderMap) -> Option<Response> {
    let header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let presented = header
        .and_then(|value| value.strip_prefix("Token "))
        .map(str::trim);
    match presented {
        None if header.is_none() => Some(unauthorized(
            "Authentication credentials were not provided.",
        )),
        None => Some(unauthorized("Invalid token")),
        Some(token) => {
            if shared.config().tokens.iter().any(|valid| valid == token) {
                None
            } else {
                Some(unauthorized("Invalid token"))
            }
        }
    }
}

/// Apply the configured fault for `kind`, if any: latency first, then
/// at most one terminal fault. Returns the response to short-circuit
/// with, or `None` to proceed normally.
async fn apply_faults(shared: &Arc<SimShared>, kind: SimKind) -> Option<Response> {
    let fault = shared.lock().effective_fault(kind);
    if !fault.is_active() {
        return None;
    }
    debug!(kind = %kind, "netbox simulator fault applied");
    if fault.latency_ms > 0 {
        tokio::time::sleep(Duration::from_millis(fault.latency_ms)).await;
    }
    if fault.connection_drop {
        return Some(connection_drop_response());
    }
    if fault.auth_failure {
        return Some(unauthorized("Invalid token"));
    }
    if fault.rate_limit {
        let mut response = (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "detail": "Request was throttled." })),
        )
            .into_response();
        response.headers_mut().insert(
            axum::http::header::RETRY_AFTER,
            HeaderValue::from_static("1"),
        );
        return Some(response);
    }
    if let Some(status) = fault.server_error {
        let status = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        return Some((status, Json(json!({ "detail": "Internal server error." }))).into_response());
    }
    None
}

/// Abort the response mid-transmission: the response begins and is
/// then cut off, so clients observe a transport/body-read failure
/// rather than a well-formed response.
fn connection_drop_response() -> Response {
    let error = std::io::Error::new(
        std::io::ErrorKind::ConnectionAborted,
        "connection dropped by chv-netbox-sim fault injection",
    );
    let mut response = Response::new(Body::from_stream(stream_once(Err::<
        axum::body::Bytes,
        std::io::Error,
    >(error))));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
}

/// Parse an optional `usize` query parameter, failing closed on
/// garbage (DRF rejects unparseable pagination parameters).
fn usize_param(
    params: &[(String, String)],
    name: &'static str,
) -> Result<Option<usize>, WireError> {
    match params.iter().find(|(key, _)| key == name) {
        None => Ok(None),
        Some((_, value)) => value
            .parse::<usize>()
            .map(Some)
            .map_err(|_| WireError::Field {
                field: name,
                message: "A valid integer is required.",
            }),
    }
}

fn bad_json_body() -> Response {
    WireError::Detail {
        message: "Invalid JSON body.",
    }
    .into_response()
}

/// NetBox-shaped error responses for [`WireError`].
impl IntoResponse for WireError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status()).unwrap_or(StatusCode::BAD_REQUEST);
        (status, Json(self.body())).into_response()
    }
}

// ---------------------------------------------------------------------------
// NetBox surface handlers
// ---------------------------------------------------------------------------

async fn list(
    State(shared): State<Arc<SimShared>>,
    Path((app, resource)): Path<(String, String)>,
    headers: HeaderMap,
    query: Result<Query<Vec<(String, String)>>, QueryRejection>,
) -> Response {
    let Some(kind) = SimKind::from_api_segments(&app, &resource) else {
        return not_found_response();
    };
    let Query(params) = match query {
        Ok(params) => params,
        Err(_) => {
            return WireError::Detail {
                message: "Invalid query string.",
            }
            .into_response()
        }
    };
    if let Some(response) = apply_faults(&shared, kind).await {
        return response;
    }
    if let Some(response) = check_auth(&shared, &headers) {
        return response;
    }
    let limit = match usize_param(&params, "limit") {
        Ok(value) => value,
        Err(error) => return error.into_response(),
    };
    let offset = match usize_param(&params, "offset") {
        Ok(value) => value.unwrap_or(0),
        Err(error) => return error.into_response(),
    };
    let base = request_base(&headers);
    let effective = effective_limit(
        limit,
        shared.config().default_page_size,
        shared.config().max_page_size,
    );
    let (count, results, links) = {
        let state = shared.lock();
        let (count, matched) = match state.list(kind, &params) {
            Ok(found) => found,
            Err(error) => return error.into_response(),
        };
        let results: Vec<Value> = matched
            .iter()
            .skip(offset)
            .take(effective)
            .map(|row| state.read_form(kind, row, &base))
            .collect();
        let filters: Vec<(String, String)> = params
            .iter()
            .filter(|(key, _)| key != "limit" && key != "offset")
            .cloned()
            .collect();
        let links = page_links(&base, kind.api_path(), &filters, count, effective, offset);
        (count, results, links)
    };
    Json(json!({
        "count": count,
        "next": links.0,
        "previous": links.1,
        "results": results,
    }))
    .into_response()
}

async fn create(
    State(shared): State<Arc<SimShared>>,
    Path((app, resource)): Path<(String, String)>,
    headers: HeaderMap,
    body: Result<Json<Value>, JsonRejection>,
) -> Response {
    let Some(kind) = SimKind::from_api_segments(&app, &resource) else {
        return not_found_response();
    };
    if let Some(response) = apply_faults(&shared, kind).await {
        return response;
    }
    if let Some(response) = check_auth(&shared, &headers) {
        return response;
    }
    let Json(body) = match body {
        Ok(body) => body,
        Err(_) => return bad_json_body(),
    };
    let id = match shared.lock().create(kind, &body) {
        Ok(id) => id,
        Err(error) => return error.into_response(),
    };
    let base = request_base(&headers);
    let object = {
        let state = shared.lock();
        match state.row(kind, id) {
            Some(row) => state.read_form(kind, row, &base),
            None => return not_found_response(),
        }
    };
    (StatusCode::CREATED, Json(object)).into_response()
}

async fn patch_object(
    State(shared): State<Arc<SimShared>>,
    Path((app, resource, id)): Path<(String, String, String)>,
    headers: HeaderMap,
    body: Result<Json<Value>, JsonRejection>,
) -> Response {
    let Some(kind) = SimKind::from_api_segments(&app, &resource) else {
        return not_found_response();
    };
    let Ok(id) = id.parse::<i64>() else {
        return not_found_response();
    };
    if let Some(response) = apply_faults(&shared, kind).await {
        return response;
    }
    if let Some(response) = check_auth(&shared, &headers) {
        return response;
    }
    let Json(body) = match body {
        Ok(body) => body,
        Err(_) => return bad_json_body(),
    };
    if let Err(error) = shared.lock().patch(kind, id, &body) {
        return error.into_response();
    }
    let base = request_base(&headers);
    let object = {
        let state = shared.lock();
        match state.row(kind, id) {
            Some(row) => state.read_form(kind, row, &base),
            None => return not_found_response(),
        }
    };
    Json(object).into_response()
}

async fn delete_object(
    State(shared): State<Arc<SimShared>>,
    Path((app, resource, id)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Response {
    let Some(kind) = SimKind::from_api_segments(&app, &resource) else {
        return not_found_response();
    };
    let Ok(id) = id.parse::<i64>() else {
        return not_found_response();
    };
    if let Some(response) = apply_faults(&shared, kind).await {
        return response;
    }
    if let Some(response) = check_auth(&shared, &headers) {
        return response;
    }
    match shared.lock().delete(kind, id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => error.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Control-plane handlers
// ---------------------------------------------------------------------------

async fn seed(
    State(shared): State<Arc<SimShared>>,
    body: Result<Json<SeedPayload>, JsonRejection>,
) -> Response {
    let Json(payload) = match body {
        Ok(payload) => payload,
        Err(_) => return bad_json_body(),
    };
    let counts = match shared.lock().seed(&payload) {
        Ok(counts) => counts,
        Err(error) => return error.into_response(),
    };
    let mut seeded = Map::new();
    for kind in SimKind::ALL {
        seeded.insert(
            kind.collection().to_string(),
            json!(counts.get(&kind).copied().unwrap_or(0)),
        );
    }
    Json(json!({ "seeded": Value::Object(seeded) })).into_response()
}

async fn state_dump(State(shared): State<Arc<SimShared>>, headers: HeaderMap) -> Response {
    let base = request_base(&headers);
    Json(shared.lock().dump(&base)).into_response()
}

async fn reset(State(shared): State<Arc<SimShared>>) -> Response {
    shared.lock().reset();
    Json(json!({ "reset": true })).into_response()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetFaultRequest {
    /// Fault scope: a kind string (`"vlan"`, …) or absent/null for
    /// the global scope.
    #[serde(default)]
    kind: Option<SimKind>,
    #[serde(default)]
    auth_failure: bool,
    #[serde(default)]
    rate_limit: bool,
    #[serde(default)]
    server_error: Option<u16>,
    #[serde(default)]
    latency_ms: u64,
    #[serde(default)]
    connection_drop: bool,
}

async fn set_faults(
    State(shared): State<Arc<SimShared>>,
    body: Result<Json<SetFaultRequest>, JsonRejection>,
) -> Response {
    let Json(request) = match body {
        Ok(request) => request,
        Err(_) => return bad_json_body(),
    };
    if let Some(status) = request.server_error {
        if !(500..=599).contains(&status) {
            return WireError::Field {
                field: "server_error",
                message: "Must be a 5xx status code.",
            }
            .into_response();
        }
    }
    let fault = FaultConfig {
        auth_failure: request.auth_failure,
        rate_limit: request.rate_limit,
        server_error: request.server_error,
        latency_ms: request.latency_ms,
        connection_drop: request.connection_drop,
    };
    let snapshot = {
        let mut state = shared.lock();
        state.set_fault(request.kind, fault);
        state.faults_value()
    };
    Json(snapshot).into_response()
}
