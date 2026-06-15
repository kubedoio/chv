//! Error type for the [`super::apply_plan`] entry point.
//!
//! Failure modes are split into pre-condition violations (the plan is not
//! ready, the user did not type the topology name, warnings are
//! unacknowledged), expiry, and downstream store/identifier propagation.
//! The two-tier split lets the BFF translate pre-condition variants into
//! distinct HTTP error codes without string-matching error messages.

use chv_controlplane_store::StoreError;
use chv_controlplane_types::domain::IdentifierError;
use thiserror::Error;

/// Errors returned by [`super::apply_plan`].
///
/// Each variant maps to a distinct user-visible failure mode. The BFF
/// translates variants to HTTP status codes (`400`, `409`) without
/// inspecting the inner message.
#[derive(Debug, Error)]
pub enum ApplyError {
    /// The plan is destructive and the caller did not pass a typed-name
    /// confirmation that matches the topology name.
    #[error(
        "plan {plan_id} requires typed-name confirmation but none was provided or it did not match \"{topology_name}\""
    )]
    MissingConfirmation {
        /// Plan identifier the apply attempt targeted.
        plan_id: String,
        /// Topology name that the caller's `typed_name` had to match.
        topology_name: String,
    },

    /// The plan carries warnings and the caller did not pass
    /// `acknowledged_warnings = true`.
    #[error(
        "plan {plan_id} has unacknowledged warnings; pass acknowledged_warnings=true to proceed"
    )]
    MissingWarningAck {
        /// Plan identifier the apply attempt targeted.
        plan_id: String,
        /// Number of warnings carried by the plan at the time of apply.
        warnings: usize,
    },

    /// The plan is not in [`chv_controlplane_types::architecture::PlanStatus::ReadyToApply`].
    #[error("plan {plan_id} is not in a state that allows apply (current: {current_status})")]
    PlanNotApplicable {
        /// Plan identifier the apply attempt targeted.
        plan_id: String,
        /// Current `plan.status` at the time of the apply attempt.
        current_status: String,
    },

    /// The plan's `expires_at` is in the past relative to the supplied clock.
    #[error("plan {plan_id} expired at {expires_at}")]
    PlanExpired {
        /// Plan identifier the apply attempt targeted.
        plan_id: String,
        /// RFC3339 timestamp at which the plan expired.
        expires_at: String,
    },

    /// A `PlanChange.resource_name` contains characters reserved for the
    /// idempotency-key separator (`::`) or the resource_ref separator
    /// (`/`). Permitting those would let two distinct (resource_type,
    /// resource_name) pairs collide on the same idempotency key, which the
    /// operations-table unique index would silently treat as a retry of
    /// the wrong change. We reject the apply pre-emptively rather than
    /// risk persisting a malformed key.
    #[error("resource_name {resource_name:?} is invalid: {reason}")]
    InvalidResourceName {
        /// The offending resource_name copied verbatim for diagnostics.
        resource_name: String,
        /// Human-readable reason (which separator hit, etc.).
        reason: String,
    },

    /// Underlying SQL/store failure. Propagated verbatim so the BFF can
    /// surface a `500` with a structured message.
    #[error("store error: {0}")]
    Store(#[from] StoreError),

    /// Identifier construction failed (empty/invalid resource id, run id,
    /// operation id). Should not happen for inputs produced by Phase-4
    /// plan generation but the error is propagated rather than panicked.
    #[error("invalid identifier: {0}")]
    Identifier(#[from] IdentifierError),
}
