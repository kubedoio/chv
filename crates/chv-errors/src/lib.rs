use chv_stord_api::chv_stord_api as proto;

#[derive(Debug, thiserror::Error)]
pub enum ChvError {
    #[error("not found: {resource} {id}")]
    NotFound { resource: String, id: String },

    #[error("already exists: {resource} {id}")]
    AlreadyExists { resource: String, id: String },

    #[error("invalid argument: {field} — {reason}")]
    InvalidArgument { field: String, reason: String },

    #[error("backend unavailable: {backend} — {reason}")]
    BackendUnavailable { backend: String, reason: String },

    #[error("network unavailable: {resource} — {reason}")]
    NetworkUnavailable { resource: String, reason: String },

    #[error("conflict: {resource} {id}")]
    Conflict { resource: String, id: String },

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
    pub const BACKEND_UNAVAILABLE: &str = "BACKEND_UNAVAILABLE";
    pub const NETWORK_UNAVAILABLE: &str = "NETWORK_UNAVAILABLE";
    pub const CONFLICT: &str = "CONFLICT";
    pub const IO: &str = "IO_ERROR";
    pub const INTERNAL: &str = "INTERNAL_ERROR";
}

impl ChvError {
    pub fn error_code(&self) -> &'static str {
        match self {
            ChvError::NotFound { .. } => ErrorCode::NOT_FOUND,
            ChvError::AlreadyExists { .. } => ErrorCode::ALREADY_EXISTS,
            ChvError::InvalidArgument { .. } => ErrorCode::INVALID_ARGUMENT,
            ChvError::BackendUnavailable { .. } => ErrorCode::BACKEND_UNAVAILABLE,
            ChvError::NetworkUnavailable { .. } => ErrorCode::NETWORK_UNAVAILABLE,
            ChvError::Conflict { .. } => ErrorCode::CONFLICT,
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

    pub fn to_proto_result(&self) -> proto::Result {
        let (status, error_code, human_summary) = self.to_result_fields();
        proto::Result {
            status: status.to_string(),
            error_code: error_code.to_string(),
            human_summary,
        }
    }

    pub fn ok_proto_result() -> proto::Result {
        proto::Result {
            status: ErrorCode::OK.to_string(),
            error_code: ErrorCode::OK.to_string(),
            human_summary: String::new(),
        }
    }
}
