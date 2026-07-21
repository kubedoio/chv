//! Versioned local HTTP/JSON transport for the single CellHV Core authority.

mod listener;

pub use listener::{CoreApiListener, ListenerError};

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
    Acceptance, AcceptedOperation, AuthorityActorError, AuthorityHandle, ErrorClass,
    MutationCommand, OperationJournalEntry, OperationServiceError, SubmitMutation,
};
use cellhv_core_types::{
    HostCapabilities, HostIdentity, IdempotencyKey, OperationEvent, OperationId, ResourceVersion,
    VmDefinition, VmId,
};
use serde::{Deserialize, Serialize};
use std::path::Path as FsPath;
use thiserror::Error;

const IDEMPOTENCY_HEADER: &str = "idempotency-key";
const IF_MATCH_HEADER: &str = "if-match";
const LOCAL_SCOPE: &str = "core-api-v1";
pub const CONTRACT_V1: &str = include_str!("../contract/cellhv-core-api-v1.json");

/// Builds the native transport over the process-wide Core authority.
///
/// The caller owns actor startup and shutdown. This constructor neither opens
/// a database nor creates another serialization boundary.
pub fn router(authority: AuthorityHandle) -> Router {
    Router::new()
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
        .with_state(authority)
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

impl From<AuthorityActorError> for ApiError {
    fn from(error: AuthorityActorError) -> Self {
        match error {
            AuthorityActorError::Service(error) => Self::Service(error),
            AuthorityActorError::InvalidCapacity
            | AuthorityActorError::Unavailable
            | AuthorityActorError::Join(_)
            | AuthorityActorError::Spawn(_)
            | AuthorityActorError::ThreadPanicked => Self::Unavailable,
        }
    }
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

async fn get_host(
    State(authority): State<AuthorityHandle>,
) -> Result<Json<HostResponse>, ApiError> {
    let host = authority.host().await?;
    Ok(Json(HostResponse {
        identity: host.identity,
        capabilities: host.capabilities,
    }))
}
async fn get_capabilities(
    State(authority): State<AuthorityHandle>,
) -> Result<Json<HostCapabilities>, ApiError> {
    Ok(Json(authority.host().await?.capabilities))
}
async fn list_vms(
    State(authority): State<AuthorityHandle>,
) -> Result<Json<Vec<VmDefinition>>, ApiError> {
    Ok(Json(authority.vms().await?))
}
async fn get_vm(
    State(authority): State<AuthorityHandle>,
    id: Result<Path<String>, PathRejection>,
) -> Result<Json<VmDefinition>, ApiError> {
    let id = VmId::new(path(id)?).map_err(|error| ApiError::Invalid(error.to_string()))?;
    Ok(Json(authority.vm(id).await?))
}
async fn create_vm(
    State(authority): State<AuthorityHandle>,
    headers: HeaderMap,
    body: Result<Json<VmMutationBody>, JsonRejection>,
) -> Result<(StatusCode, Json<AcceptanceResponse>), ApiError> {
    let body = json(body)?;
    let accepted = submit(
        &authority,
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
    State(authority): State<AuthorityHandle>,
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
        &authority,
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
    State(authority): State<AuthorityHandle>,
    id: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    body: Result<Json<MutationBody>, JsonRejection>,
) -> Result<(StatusCode, Json<AcceptanceResponse>), ApiError> {
    let id = VmId::new(path(id)?).map_err(|error| ApiError::Invalid(error.to_string()))?;
    let body = json(body)?;
    let accepted = submit(
        &authority,
        body.request_id,
        idempotency_key(&headers)?,
        expected_version(&headers)?,
        MutationCommand::DeleteVm { vm_id: id },
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(response(accepted))))
}
async fn start_vm(
    State(_): State<AuthorityHandle>,
    id: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    body: Result<Json<MutationBody>, JsonRejection>,
) -> Result<Response, ApiError> {
    unsupported_action(id, headers, body, "power start execution")
}
async fn stop_vm(
    State(_): State<AuthorityHandle>,
    id: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    body: Result<Json<MutationBody>, JsonRejection>,
) -> Result<Response, ApiError> {
    unsupported_action(id, headers, body, "power stop execution")
}
async fn reboot_vm(
    State(_): State<AuthorityHandle>,
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
    authority: &AuthorityHandle,
    request_id: String,
    key: IdempotencyKey,
    expected_vm_version: ResourceVersion,
    command: MutationCommand,
) -> Result<AcceptedOperation, ApiError> {
    let operation_id = native_operation_id(&request_id)?;
    Ok(authority
        .submit(SubmitMutation {
            operation_id,
            idempotency_scope: LOCAL_SCOPE.to_owned(),
            idempotency_key: key,
            expected_vm_version,
            command,
        })
        .await?)
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
    State(authority): State<AuthorityHandle>,
    id: Result<Path<String>, PathRejection>,
) -> Result<Json<OperationJournalEntry>, ApiError> {
    let id = OperationId::new(path(id)?).map_err(|error| ApiError::Invalid(error.to_string()))?;
    Ok(Json(authority.operation(id).await?))
}
async fn list_operations(
    State(authority): State<AuthorityHandle>,
) -> Result<Json<Vec<OperationJournalEntry>>, ApiError> {
    Ok(Json(authority.operations().await?))
}
async fn list_events(
    State(authority): State<AuthorityHandle>,
    query: Result<Query<EventQuery>, QueryRejection>,
) -> Result<Json<Vec<OperationEvent>>, ApiError> {
    let Query(query) = query.map_err(|error| ApiError::Invalid(error.body_text()))?;
    if query.limit == 0 || query.limit > 1_000 {
        return Err(ApiError::Invalid(
            "event limit must be between 1 and 1000".to_owned(),
        ));
    }
    Ok(Json(
        authority.events_after(query.after, query.limit).await?,
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
    #[error("Core API socket path does not identify the bound socket: {0}")]
    IdentityMismatch(String),
    #[error("Core API socket operation failed: {0}")]
    Io(#[from] std::io::Error),
}
pub async fn bind_private(socket: &FsPath) -> Result<tokio::net::UnixListener, BindError> {
    bind_private_owned(socket, ExistingSocketPolicy::Refuse)
        .await
        .map(|(listener, _)| listener)
}

#[derive(Clone, Copy)]
pub(crate) enum ExistingSocketPolicy {
    Refuse,
    RecoverStale,
}

async fn bind_private_owned(
    socket: &FsPath,
    existing_socket_policy: ExistingSocketPolicy,
) -> Result<(tokio::net::UnixListener, crate::listener::SocketIdentity), BindError> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
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
    match std::fs::symlink_metadata(socket) {
        Ok(metadata)
            if metadata.file_type().is_socket()
                && matches!(existing_socket_policy, ExistingSocketPolicy::RecoverStale) =>
        {
            recover_stale_socket(socket).await?;
        }
        Ok(_) => return Err(BindError::ExistingPath(socket.display().to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(BindError::Io(error)),
    }
    let listener = tokio::net::UnixListener::bind(socket)?;
    let (path_file, identity) = crate::listener::open_socket_path(socket).map_err(BindError::Io)?;
    if let Err(error) = crate::listener::set_socket_mode(socket, &path_file, identity) {
        crate::listener::remove_matching_socket(socket, identity);
        return Err(BindError::Io(error));
    }
    let path_metadata = match std::fs::symlink_metadata(socket) {
        Ok(metadata) => metadata,
        Err(error) => {
            crate::listener::remove_matching_socket(socket, identity);
            return Err(BindError::Io(error));
        }
    };
    if !path_metadata.file_type().is_socket()
        || path_metadata.dev() != identity.device
        || path_metadata.ino() != identity.inode
        || path_metadata.permissions().mode() & 0o777 != 0o600
    {
        crate::listener::remove_matching_socket(socket, identity);
        return Err(BindError::IdentityMismatch(socket.display().to_string()));
    }
    Ok((listener, identity))
}

async fn recover_stale_socket(socket: &FsPath) -> Result<(), BindError> {
    let (_, identity) = crate::listener::open_socket_path(socket)
        .map_err(|_| BindError::ExistingPath(socket.display().to_string()))?;
    match tokio::net::UnixStream::connect(socket).await {
        Ok(stream) => {
            drop(stream);
            return Err(BindError::ExistingPath(socket.display().to_string()));
        }
        Err(error) if error.raw_os_error() == Some(nix::libc::ECONNREFUSED) => {}
        Err(_) => return Err(BindError::ExistingPath(socket.display().to_string())),
    }

    // Revalidate after the liveness probe. The directory is owner-only, and
    // removal still checks the captured inode immediately before unlinking.
    crate::listener::remove_matching_socket(socket, identity);
    match std::fs::symlink_metadata(socket) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(BindError::ExistingPath(socket.display().to_string())),
        Err(error) => Err(BindError::Io(error)),
    }
}

#[cfg(test)]
mod tests;
