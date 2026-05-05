#[derive(Debug, thiserror::Error)]
pub enum ControlPlaneServiceError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("store error: {0}")]
    Store(chv_controlplane_store::StoreError),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("stale generation: expected {expected}, received {received}")]
    StaleGeneration { expected: String, received: String },
}

impl From<chv_controlplane_store::StoreError> for ControlPlaneServiceError {
    fn from(err: chv_controlplane_store::StoreError) -> Self {
        match err {
            chv_controlplane_store::StoreError::NotFound { entity, id } => {
                Self::NotFound(format!("{} with id {} not found", entity, id))
            }
            _ => Self::Store(err),
        }
    }
}

impl From<ControlPlaneServiceError> for tonic::Status {
    fn from(err: ControlPlaneServiceError) -> Self {
        use tonic::Status;
        match err {
            ControlPlaneServiceError::NotFound(msg) => Status::not_found(msg),
            ControlPlaneServiceError::InvalidArgument(msg) => Status::invalid_argument(msg),
            ControlPlaneServiceError::Unauthorized(msg) => Status::unauthenticated(msg),
            ControlPlaneServiceError::Conflict(msg) => Status::already_exists(msg),
            ControlPlaneServiceError::StaleGeneration { expected, received } => {
                Status::failed_precondition(format!(
                    "stale generation: expected {expected}, received {received}"
                ))
            }
            ControlPlaneServiceError::Store(ref e) => {
                tracing::error!(error = %e, "store error");
                Status::internal("internal error")
            }
            ControlPlaneServiceError::Internal(ref msg) => {
                tracing::error!(error = %msg, "internal error");
                Status::internal("internal error")
            }
            ControlPlaneServiceError::Io(ref e) => {
                tracing::error!(error = %e, "io error");
                Status::internal("internal error")
            }
        }
    }
}
