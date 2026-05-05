#[derive(Debug, thiserror::Error)]
pub enum ChvError {
    #[error("not found: {resource} {id}")]
    NotFound { resource: String, id: String },

    #[error("already exists: {resource} {id}")]
    AlreadyExists { resource: String, id: String },

    #[error("invalid argument: {field} — {reason}")]
    InvalidArgument { field: String, reason: String },

    #[error("bad request: {reason}")]
    BadRequest { reason: String },

    #[error("unauthorized: {reason}")]
    Unauthorized { reason: String },

    #[error("quota exceeded: {resource} — limit {limit}, used {used}, requested {requested}")]
    QuotaExceeded {
        resource: String,
        limit: i64,
        used: i64,
        requested: i64,
    },

    #[error("backend unavailable: {backend} — {reason}")]
    BackendUnavailable { backend: String, reason: String },

    #[error("network unavailable: {resource} — {reason}")]
    NetworkUnavailable { resource: String, reason: String },

    #[error("conflict: {resource} {id}")]
    Conflict { resource: String, id: String },

    #[error("stale generation: {resource} {id} — expected >= {expected}, got {got}")]
    StaleGeneration {
        resource: String,
        id: String,
        expected: String,
        got: String,
    },

    #[error("control plane unavailable: {reason}")]
    ControlPlaneUnavailable { reason: String },

    #[error("io error on {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("internal error: {reason}")]
    Internal { reason: String },
}

pub struct ErrorCode;

#[allow(non_upper_case_globals)]
impl ErrorCode {
    pub const OK: &str = "OK";
    pub const NOT_FOUND: &str = "NOT_FOUND";
    pub const ALREADY_EXISTS: &str = "ALREADY_EXISTS";
    pub const INVALID_ARGUMENT: &str = "INVALID_ARGUMENT";
    pub const BAD_REQUEST: &str = "BAD_REQUEST";
    pub const UNAUTHORIZED: &str = "UNAUTHORIZED";
    pub const QUOTA_EXCEEDED: &str = "QUOTA_EXCEEDED";
    pub const BACKEND_UNAVAILABLE: &str = "BACKEND_UNAVAILABLE";
    pub const NETWORK_UNAVAILABLE: &str = "NETWORK_UNAVAILABLE";
    pub const CONFLICT: &str = "CONFLICT";
    pub const STALE_GENERATION: &str = "STALE_GENERATION";
    pub const CONTROL_PLANE_UNAVAILABLE: &str = "CONTROL_PLANE_UNAVAILABLE";
    pub const IO: &str = "IO_ERROR";
    pub const INTERNAL: &str = "INTERNAL_ERROR";
}

impl ChvError {
    pub fn error_code(&self) -> &'static str {
        match self {
            ChvError::NotFound { .. } => ErrorCode::NOT_FOUND,
            ChvError::AlreadyExists { .. } => ErrorCode::ALREADY_EXISTS,
            ChvError::InvalidArgument { .. } => ErrorCode::INVALID_ARGUMENT,
            ChvError::BadRequest { .. } => ErrorCode::BAD_REQUEST,
            ChvError::Unauthorized { .. } => ErrorCode::UNAUTHORIZED,
            ChvError::QuotaExceeded { .. } => ErrorCode::QUOTA_EXCEEDED,
            ChvError::BackendUnavailable { .. } => ErrorCode::BACKEND_UNAVAILABLE,
            ChvError::NetworkUnavailable { .. } => ErrorCode::NETWORK_UNAVAILABLE,
            ChvError::Conflict { .. } => ErrorCode::CONFLICT,
            ChvError::StaleGeneration { .. } => ErrorCode::STALE_GENERATION,
            ChvError::ControlPlaneUnavailable { .. } => ErrorCode::CONTROL_PLANE_UNAVAILABLE,
            ChvError::Io { .. } => ErrorCode::IO,
            ChvError::Internal { .. } => ErrorCode::INTERNAL,
        }
    }

    pub fn status(&self) -> &'static str {
        "error"
    }

    pub fn to_result_fields(&self) -> (&'static str, &'static str, String) {
        (self.status(), self.error_code(), self.to_string())
    }

    pub fn ok_result_fields() -> (&'static str, &'static str, String) {
        (ErrorCode::OK, ErrorCode::OK, String::new())
    }
}

impl From<ChvError> for tonic::Status {
    fn from(err: ChvError) -> tonic::Status {
        match &err {
            ChvError::NotFound { resource, id } => {
                tonic::Status::not_found(format!("{resource} {id}"))
            }
            ChvError::AlreadyExists { resource, id } => {
                tonic::Status::already_exists(format!("{resource} {id}"))
            }
            ChvError::InvalidArgument { field, reason } => {
                tonic::Status::invalid_argument(format!("{field}: {reason}"))
            }
            ChvError::BadRequest { reason } => tonic::Status::invalid_argument(reason.clone()),
            ChvError::Unauthorized { .. } => {
                tonic::Status::unauthenticated("unauthorized")
            }
            ChvError::QuotaExceeded { resource, .. } => {
                tonic::Status::resource_exhausted(format!("{resource} quota exceeded"))
            }
            ChvError::Conflict { resource, id } => {
                tonic::Status::already_exists(format!("{resource} {id}"))
            }
            ChvError::StaleGeneration {
                resource,
                id,
                expected,
                got,
            } => tonic::Status::failed_precondition(format!(
                "stale generation on {resource} {id}: expected >= {expected}, got {got}"
            )),
            ChvError::BackendUnavailable { backend, .. } => {
                tonic::Status::unavailable(format!("{backend} unavailable"))
            }
            ChvError::ControlPlaneUnavailable { .. } => {
                tonic::Status::unavailable("control plane unavailable")
            }
            ChvError::NetworkUnavailable { resource, .. } => {
                tonic::Status::unavailable(format!("{resource} unavailable"))
            }
            ChvError::Io { .. } | ChvError::Internal { .. } => {
                tonic::Status::internal("internal error")
            }
        }
    }
}
