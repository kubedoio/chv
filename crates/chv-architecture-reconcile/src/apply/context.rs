//! Caller-supplied context for [`super::apply_plan`].
//!
//! The reconcile-side apply path is intentionally pure-data; the BFF (or any
//! other host) builds an [`ApplyContext`] from the request body, the
//! plan record, and the resolved authentication subject, then hands it off
//! to [`super::apply_plan`].

use chv_controlplane_types::architecture::{
    ArchitectureId, ArchitecturePlanId, ArchitectureVersionId,
};

/// Caller-supplied context for an apply attempt.
///
/// Keep this narrow and immutable. All fields are passed by value or
/// reference at the call site so the apply path itself does no I/O before
/// touching the store.
#[derive(Clone, Debug)]
pub struct ApplyContext {
    /// Architecture being applied. Looked up from the plan record by the
    /// BFF and passed in here so the reconcile crate stays single-purpose.
    pub architecture_id: ArchitectureId,
    /// Concrete version of the architecture the plan was generated against.
    pub architecture_version_id: ArchitectureVersionId,
    /// `version_number` of the topology row at the moment the BFF read it.
    /// The apply path uses this for the topology lifecycle-status CAS
    /// (`draft → applying`); a mismatch means a concurrent writer touched
    /// the row and we treat the apply as advisory rather than wedging the
    /// topology in an inconsistent state. Plumbed in from the BFF so the
    /// reconcile crate does no I/O before its first store call.
    pub topology_version: i64,
    /// Topology display name used for typed-name confirmation. The
    /// destructive-apply guard requires `confirmation.typed_name` to match
    /// this string verbatim.
    pub topology_name: String,
    /// Optional environment label (`"production"`, `"staging"`, ...). The
    /// BFF may stack additional guards on top of this; the apply path
    /// itself does not branch on the value, but it is recorded in tracing
    /// fields for audit.
    pub environment: Option<String>,
    /// Plan identifier being applied.
    pub plan_id: ArchitecturePlanId,
    /// Subject (user id) that initiated the apply, propagated to the
    /// `apply_run.requested_by` and `operation.requested_by` columns.
    pub requested_by: Option<String>,
    /// Typed-name confirmation token. Required for destructive plans;
    /// ignored otherwise.
    pub confirmation: ConfirmationToken,
    /// Whether the caller acknowledges plan warnings. If `false` and the
    /// plan has warnings, [`super::apply_plan`] rejects the request with
    /// [`super::ApplyError::MissingWarningAck`].
    pub acknowledged_warnings: bool,
}

/// Typed-name confirmation token.
///
/// The destructive-apply guard requires the caller to type the topology
/// name as a sentence that the UI displays in the confirmation dialog.
/// This struct carries that string from the request body to the apply
/// path.
#[derive(Clone, Debug, Default)]
pub struct ConfirmationToken {
    /// Topology name typed by the user, or `None` if the request body
    /// omitted the field.
    pub typed_name: Option<String>,
}

impl ConfirmationToken {
    /// Returns `true` when `typed_name` is present and matches
    /// `topology_name` exactly.
    ///
    /// The match is byte-for-byte; case differences are *not* normalized
    /// because the topology name is a domain identifier with the same
    /// casing rules as a Kubernetes object name.
    pub fn matches(&self, topology_name: &str) -> bool {
        self.typed_name.as_deref() == Some(topology_name)
    }
}
