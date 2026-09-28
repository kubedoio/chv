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

    #[error("starter seed failed: {0}")]
    Seed(#[from] chv_controlplane_seed::SeedError),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("stale generation: expected {expected}, received {received}")]
    StaleGeneration { expected: String, received: String },

    /// The requested RPC is a node-scoped operator action and is not routed
    /// through the control plane (fail closed with a greppable message
    /// instead of a half-implemented forward).
    #[error("unsupported on the control plane: {0}")]
    Unsupported(String),

    /// The build requested `CHV_ALLOW_INSECURE=1` (insecure peer-identity mode)
    /// but was not compiled with the `dev` Cargo feature. This is a typed,
    /// non-panicking startup failure — a production build must never run with
    /// mTLS peer-identity enforcement disabled. See ADR-014 and issue #233.
    #[error("insecure peer-identity mode is locked out without the dev feature: {0}")]
    InsecureModeLockedOut(String),
}

impl From<chv_controlplane_store::StoreError> for ControlPlaneServiceError {
    fn from(err: chv_controlplane_store::StoreError) -> Self {
        match err {
            chv_controlplane_store::StoreError::NotFound { entity, id } => {
                Self::NotFound(format!("{} with id {} not found", entity, id))
            }
            chv_controlplane_store::StoreError::StaleGeneration {
                entity,
                id,
                incoming,
            } => Self::StaleGeneration {
                expected: format!(">{} for {} '{}'", incoming, entity, id),
                received: incoming.to_string(),
            },
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
            ControlPlaneServiceError::PermissionDenied(msg) => Status::permission_denied(msg),
            ControlPlaneServiceError::Conflict(msg) => Status::already_exists(msg),
            ControlPlaneServiceError::StaleGeneration { expected, received } => {
                Status::failed_precondition(format!(
                    "stale generation: expected {expected}, received {received}"
                ))
            }
            ControlPlaneServiceError::Unsupported(msg) => Status::unimplemented(msg),
            ControlPlaneServiceError::Store(ref e) => {
                tracing::error!(error = %e, "store error");
                Status::internal("internal error")
            }
            ControlPlaneServiceError::Internal(ref msg) => {
                tracing::error!(error = %msg, "internal error");
                Status::internal("internal error")
            }
            ControlPlaneServiceError::Seed(ref e) => {
                tracing::error!(error = %e, "starter seed error");
                Status::internal("internal error")
            }
            ControlPlaneServiceError::InsecureModeLockedOut(ref msg) => {
                tracing::error!(error = %msg, "insecure mode locked out");
                Status::internal("insecure peer-identity mode is not available in this build")
            }
            ControlPlaneServiceError::Io(ref e) => {
                tracing::error!(error = %e, "io error");
                Status::internal("internal error")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Node-scoped operator RPCs (e.g. resolve_inspect_required_operation)
    /// fail closed on the control plane and must surface as gRPC
    /// `Unimplemented`, never as an internal error.
    #[test]
    fn unsupported_maps_to_tonic_unimplemented() {
        let status = tonic::Status::from(ControlPlaneServiceError::Unsupported(
            "resolve_inspect_required_operation is a node-scoped operator action".into(),
        ));
        assert_eq!(status.code(), tonic::Code::Unimplemented);
        assert!(status.message().contains("node-scoped"));
    }
}
