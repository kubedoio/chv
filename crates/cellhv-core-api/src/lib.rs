//! Versioned local HTTP/JSON transport for the single CellHV Core authority.

use axum::{
    extract::{
        rejection::{JsonRejection, PathRejection, QueryRejection},
        Path, Query, State,
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use cellhv_core_operations::{
    Acceptance, AcceptedOperation, ErrorClass, MutationCommand, OperationJournalEntry,
    OperationService, OperationServiceError, SubmitMutation,
};
use cellhv_core_types::{
    HostCapabilities, HostIdentity, IdempotencyKey, OperationEvent, OperationId, ResourceVersion,
    VmDefinition, VmId,
};
use serde::{Deserialize, Serialize};
use std::{path::Path as FsPath, sync::mpsc};
use thiserror::Error;
use tokio::sync::oneshot;

const IDEMPOTENCY_HEADER: &str = "idempotency-key";
const IF_MATCH_HEADER: &str = "if-match";
const LOCAL_SCOPE: &str = "core-api-v1";
pub const CONTRACT_V1: &str = include_str!("../contract/cellhv-core-api-v1.json");

type Reply<T> = oneshot::Sender<Result<T, OperationServiceError>>;

enum DbRequest {
    Host(Reply<cellhv_core_operations::HostRecord>),
    Vms(Reply<Vec<VmDefinition>>),
    Vm(VmId, Reply<VmDefinition>),
    Submit(Box<SubmitMutation>, Reply<AcceptedOperation>),
    Operations(Reply<Vec<OperationJournalEntry>>),
    Operation(OperationId, Reply<OperationJournalEntry>),
    Events(u64, u32, Reply<Vec<OperationEvent>>),
}

#[derive(Clone)]
struct DbActor {
    sender: mpsc::Sender<DbRequest>,
}

impl DbActor {
    fn start(mut service: OperationService) -> Result<Self, ApiStartError> {
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("cellhv-core-db".to_owned())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    match request {
                        DbRequest::Host(reply) => {
                            let _ = reply.send(service.host());
                        }
                        DbRequest::Vms(reply) => {
                            let _ = reply.send(service.vms());
                        }
                        DbRequest::Vm(id, reply) => {
                            let _ = reply.send(service.vm(&id));
                        }
                        DbRequest::Submit(value, reply) => {
                            let _ = reply.send(service.submit(*value));
                        }
                        DbRequest::Operations(reply) => {
                            let _ = reply.send(service.operations());
                        }
                        DbRequest::Operation(id, reply) => {
                            let _ = reply.send(service.operation(&id));
                        }
                        DbRequest::Events(after, limit, reply) => {
                            let _ = reply.send(service.events_after(after, limit));
                        }
                    }
                }
            })
            .map_err(ApiStartError::Thread)?;
        Ok(Self { sender })
    }

    async fn call<T>(&self, build: impl FnOnce(Reply<T>) -> DbRequest) -> Result<T, ApiError> {
        let (reply, receive) = oneshot::channel();
        self.sender
            .send(build(reply))
            .map_err(|_| ApiError::Unavailable)?;
        receive
            .await
            .map_err(|_| ApiError::Unavailable)?
            .map_err(ApiError::Service)
    }
}

#[derive(Debug, Error)]
pub enum ApiStartError {
    #[error("cannot start the Core database actor: {0}")]
    Thread(std::io::Error),
}

pub fn router(service: OperationService) -> Result<Router, ApiStartError> {
    let db = DbActor::start(service)?;
    Ok(Router::new()
        .route("/v1/host", get(get_host))
        .route("/v1/host/capabilities", get(get_capabilities))
        .route("/v1/vms", get(list_vms).post(create_vm))
        .route(
            "/v1/vms/:vm_id",
            get(get_vm).patch(update_vm).delete(delete_vm),
        )
        .route("/v1/vms/:vm_id/actions/start", post(start_vm))
        .route("/v1/vms/:vm_id/actions/stop", post(stop_vm))
        .route("/v1/vms/:vm_id/actions/reboot", post(reboot_vm))
        .route("/v1/operations", get(list_operations))
        .route("/v1/operations/:operation_id", get(get_operation))
        .route("/v1/events", get(list_events))
        .method_not_allowed_fallback(|| async { ApiError::MethodNotAllowed.into_response() })
        .fallback(|| async { ApiError::NotFound.into_response() })
        .with_state(db))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MutationBody {
    request_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VmMutationBody {
    request_id: String,
    definition: VmDefinition,
}

#[derive(Debug, Serialize)]
struct AcceptanceResponse {
    disposition: &'static str,
    operation_id: OperationId,
    resource_version: ResourceVersion,
}

#[derive(Debug, Serialize)]
struct HostResponse {
    identity: HostIdentity,
    capabilities: HostCapabilities,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct EventQuery {
    after: u64,
    limit: u32,
}
impl Default for EventQuery {
    fn default() -> Self {
        Self {
            after: 0,
            limit: 100,
        }
    }
}

#[derive(Debug, Error)]
enum ApiError {
    #[error("invalid request: {0}")]
    Invalid(String),
    #[error(transparent)]
    Service(OperationServiceError),
    #[error("Core authority is unavailable")]
    Unavailable,
    #[error("resource was not found")]
    NotFound,
    #[error("method is not allowed for this resource")]
    MethodNotAllowed,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Invalid(message) => (StatusCode::BAD_REQUEST, "invalid_request", message),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "resource was not found".to_owned(),
            ),
            Self::MethodNotAllowed => (
                StatusCode::METHOD_NOT_ALLOWED,
                "method_not_allowed",
                "method is not allowed for this resource".to_owned(),
            ),
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "authority_unavailable",
                "Core authority is unavailable".to_owned(),
            ),
            Self::Service(error) => match error.class() {
                ErrorClass::Invalid => (
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    error.to_string(),
                ),
                ErrorClass::Unsupported => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "unsupported",
                    error.to_string(),
                ),
                ErrorClass::NotFound => (
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "resource was not found".to_owned(),
                ),
                ErrorClass::Conflict => (StatusCode::CONFLICT, "conflict", error.to_string()),
                ErrorClass::Precondition => (
                    StatusCode::PRECONDITION_FAILED,
                    "precondition_failed",
                    error.to_string(),
                ),
                ErrorClass::Internal => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "authority_error",
                    "Core authority request failed".to_owned(),
                ),
            },
        };
        (status, Json(ErrorBody { code, message })).into_response()
    }
}

fn json<T>(value: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    value
        .map(|Json(value)| value)
        .map_err(|error| ApiError::Invalid(error.body_text()))
}
fn path(value: Result<Path<String>, PathRejection>) -> Result<String, ApiError> {
    value
        .map(|Path(value)| value)
        .map_err(|error| ApiError::Invalid(error.body_text()))
}

async fn get_host(State(db): State<DbActor>) -> Result<Json<HostResponse>, ApiError> {
    let host = db.call(DbRequest::Host).await?;
    Ok(Json(HostResponse {
        identity: host.identity,
        capabilities: host.capabilities,
    }))
}
async fn get_capabilities(State(db): State<DbActor>) -> Result<Json<HostCapabilities>, ApiError> {
    Ok(Json(db.call(DbRequest::Host).await?.capabilities))
}
async fn list_vms(State(db): State<DbActor>) -> Result<Json<Vec<VmDefinition>>, ApiError> {
    Ok(Json(db.call(DbRequest::Vms).await?))
}
async fn get_vm(
    State(db): State<DbActor>,
    id: Result<Path<String>, PathRejection>,
) -> Result<Json<VmDefinition>, ApiError> {
    let id = VmId::new(path(id)?).map_err(|error| ApiError::Invalid(error.to_string()))?;
    Ok(Json(db.call(|reply| DbRequest::Vm(id, reply)).await?))
}
async fn create_vm(
    State(db): State<DbActor>,
    headers: HeaderMap,
    body: Result<Json<VmMutationBody>, JsonRejection>,
) -> Result<(StatusCode, Json<AcceptanceResponse>), ApiError> {
    let body = json(body)?;
    let accepted = submit(
        &db,
        body.request_id,
        idempotency_key(&headers)?,
        ResourceVersion::new(1).expect("valid"),
        MutationCommand::CreateVm {
            definition: body.definition,
        },
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(response(accepted))))
}
async fn update_vm(
    State(db): State<DbActor>,
    id: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    body: Result<Json<VmMutationBody>, JsonRejection>,
) -> Result<(StatusCode, Json<AcceptanceResponse>), ApiError> {
    let id = path(id)?;
    let body = json(body)?;
    if body.definition.id.as_str() != id {
        return Err(ApiError::Invalid(
            "path VM identifier must match definition".to_owned(),
        ));
    }
    let accepted = submit(
        &db,
        body.request_id,
        idempotency_key(&headers)?,
        expected_version(&headers)?,
        MutationCommand::UpdateVm {
            definition: body.definition,
        },
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(response(accepted))))
}
async fn delete_vm(
    State(db): State<DbActor>,
    id: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    body: Result<Json<MutationBody>, JsonRejection>,
) -> Result<(StatusCode, Json<AcceptanceResponse>), ApiError> {
    let id = VmId::new(path(id)?).map_err(|error| ApiError::Invalid(error.to_string()))?;
    let body = json(body)?;
    let accepted = submit(
        &db,
        body.request_id,
        idempotency_key(&headers)?,
        expected_version(&headers)?,
        MutationCommand::DeleteVm { vm_id: id },
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(response(accepted))))
}
async fn start_vm(
    State(_): State<DbActor>,
    id: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    body: Result<Json<MutationBody>, JsonRejection>,
) -> Result<Response, ApiError> {
    unsupported_action(id, headers, body, "power start execution")
}
async fn stop_vm(
    State(_): State<DbActor>,
    id: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    body: Result<Json<MutationBody>, JsonRejection>,
) -> Result<Response, ApiError> {
    unsupported_action(id, headers, body, "power stop execution")
}
async fn reboot_vm(
    State(_): State<DbActor>,
    id: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    body: Result<Json<MutationBody>, JsonRejection>,
) -> Result<Response, ApiError> {
    unsupported_action(id, headers, body, "power reboot execution")
}

fn unsupported_action(
    id: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    body: Result<Json<MutationBody>, JsonRejection>,
    feature: &'static str,
) -> Result<Response, ApiError> {
    VmId::new(path(id)?).map_err(|error| ApiError::Invalid(error.to_string()))?;
    let body = json(body)?;
    native_operation_id(&body.request_id)?;
    idempotency_key(&headers)?;
    expected_version(&headers)?;
    Err(ApiError::Service(OperationServiceError::Unsupported(
        feature,
    )))
}

async fn submit(
    db: &DbActor,
    request_id: String,
    key: IdempotencyKey,
    expected_vm_version: ResourceVersion,
    command: MutationCommand,
) -> Result<AcceptedOperation, ApiError> {
    let operation_id = native_operation_id(&request_id)?;
    db.call(|reply| {
        DbRequest::Submit(
            Box::new(SubmitMutation {
                operation_id,
                idempotency_scope: LOCAL_SCOPE.to_owned(),
                idempotency_key: key,
                expected_vm_version,
                command,
            }),
            reply,
        )
    })
    .await
}
fn native_operation_id(request_id: &str) -> Result<OperationId, ApiError> {
    if request_id.trim().is_empty() || request_id.len() > 200 {
        return Err(ApiError::Invalid(
            "request_id must contain 1 through 200 characters".to_owned(),
        ));
    }
    OperationId::new(format!("native:v1:{request_id}"))
        .map_err(|error| ApiError::Invalid(error.to_string()))
}
async fn get_operation(
    State(db): State<DbActor>,
    id: Result<Path<String>, PathRejection>,
) -> Result<Json<OperationJournalEntry>, ApiError> {
    let id = OperationId::new(path(id)?).map_err(|error| ApiError::Invalid(error.to_string()))?;
    Ok(Json(
        db.call(|reply| DbRequest::Operation(id, reply)).await?,
    ))
}
async fn list_operations(
    State(db): State<DbActor>,
) -> Result<Json<Vec<OperationJournalEntry>>, ApiError> {
    Ok(Json(db.call(DbRequest::Operations).await?))
}
async fn list_events(
    State(db): State<DbActor>,
    query: Result<Query<EventQuery>, QueryRejection>,
) -> Result<Json<Vec<OperationEvent>>, ApiError> {
    let Query(query) = query.map_err(|error| ApiError::Invalid(error.body_text()))?;
    if query.limit == 0 || query.limit > 1_000 {
        return Err(ApiError::Invalid(
            "event limit must be between 1 and 1000".to_owned(),
        ));
    }
    Ok(Json(
        db.call(|reply| DbRequest::Events(query.after, query.limit, reply))
            .await?,
    ))
}
fn idempotency_key(headers: &HeaderMap) -> Result<IdempotencyKey, ApiError> {
    let value = headers
        .get(IDEMPOTENCY_HEADER)
        .ok_or_else(|| ApiError::Invalid("Idempotency-Key header is required".to_owned()))?
        .to_str()
        .map_err(|_| ApiError::Invalid("Idempotency-Key must be visible ASCII".to_owned()))?;
    IdempotencyKey::new(value).map_err(|error| ApiError::Invalid(error.to_string()))
}
fn expected_version(headers: &HeaderMap) -> Result<ResourceVersion, ApiError> {
    let value = headers
        .get(IF_MATCH_HEADER)
        .ok_or_else(|| ApiError::Invalid("If-Match header is required".to_owned()))?
        .to_str()
        .map_err(|_| ApiError::Invalid("If-Match must be visible ASCII".to_owned()))?;
    if value.len() < 3 || !value.starts_with('"') || !value.ends_with('"') {
        return Err(ApiError::Invalid(
            "If-Match must be one quoted positive resource version".to_owned(),
        ));
    }
    let number = &value[1..value.len() - 1];
    if number.starts_with('+')
        || number.starts_with('0')
        || !number.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(ApiError::Invalid(
            "If-Match must be one quoted positive resource version".to_owned(),
        ));
    }
    ResourceVersion::new(
        number.parse().map_err(|_| {
            ApiError::Invalid("If-Match resource version is out of range".to_owned())
        })?,
    )
    .map_err(|error| ApiError::Invalid(error.to_string()))
}
fn response(value: AcceptedOperation) -> AcceptanceResponse {
    AcceptanceResponse {
        disposition: match value.disposition {
            Acceptance::Accepted => "accepted",
            Acceptance::Replay => "replay",
        },
        operation_id: value.operation.id,
        resource_version: value.accepted_resource_version,
    }
}

#[derive(Debug, Error)]
pub enum BindError {
    #[error("Core API socket parent must be an owner-only directory: {0}")]
    UnsafeParent(String),
    #[error("refusing to replace existing Core API socket path: {0}")]
    ExistingPath(String),
    #[error("Core API socket operation failed: {0}")]
    Io(#[from] std::io::Error),
}
pub async fn bind_private(socket: &FsPath) -> Result<tokio::net::UnixListener, BindError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let parent = socket
        .parent()
        .ok_or_else(|| BindError::UnsafeParent(socket.display().to_string()))?;
    let metadata = std::fs::symlink_metadata(parent)
        .map_err(|_| BindError::UnsafeParent(parent.display().to_string()))?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(BindError::UnsafeParent(parent.display().to_string()));
    }
    if std::fs::symlink_metadata(socket).is_ok() {
        return Err(BindError::ExistingPath(socket.display().to_string()));
    }
    let listener = tokio::net::UnixListener::bind(socket)?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

#[cfg(test)]
mod tests;
