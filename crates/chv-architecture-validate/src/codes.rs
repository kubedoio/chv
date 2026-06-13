//! Stable finding code registry for the Architecture Designer validator.
//!
//! Every code surfaced by static-validation or schema-validation is declared
//! here as a `pub const &'static str`. Codes are part of the public BFF
//! contract: UIs and tests pin against these strings, so once a code ships it
//! must not change.
//!
//! # Lifecycle policy
//!
//! Treat codes the way ADR-014 treats proto field numbers:
//!
//! - New codes may be added at the bottom of the file at any time.
//! - A code that needs to be removed is **retired**, not deleted: comment the
//!   line out with a `// retired YYYY-MM-DD: <reason>` annotation and leave
//!   the line in place. Future additions MUST NOT reuse a retired string.
//! - The `codes_are_unique` test below guards the live (non-retired) set.
//!
//! Stability matters because consumer dashboards alert on specific codes; a
//! reused code would silently change semantics.

/// JSON-Schema validation error (one per schema violation).
pub const SCHEMA_INVALID: &str = "SCHEMA_INVALID";

/// Two list items in the same section share a `name`.
pub const DUPLICATE_NAME: &str = "DUPLICATE_NAME";

/// A cross-reference points at a name that does not exist in the referenced
/// section (e.g. `instance.template` → `templates[]`).
pub const MISSING_REFERENCE: &str = "MISSING_REFERENCE";

/// `network.cidr` does not parse as an IPv4 / IPv6 network.
pub const INVALID_CIDR: &str = "INVALID_CIDR";

/// Two networks have CIDRs that overlap (one is a subset of the other or
/// shares any address). Pairwise; deterministic ordering.
pub const NETWORK_CIDR_OVERLAP: &str = "NETWORK_CIDR_OVERLAP";

/// Two distinct addresses (instance static IPs, gateway IPs) collide.
pub const DUPLICATE_IP: &str = "DUPLICATE_IP";

/// An instance static IP is not contained in the CIDR of the network it
/// attaches to.
pub const IP_OUTSIDE_NETWORK: &str = "IP_OUTSIDE_NETWORK";

/// `network.gateway` is set but does not fall inside `network.cidr`.
pub const GATEWAY_OUTSIDE_NETWORK: &str = "GATEWAY_OUTSIDE_NETWORK";

/// DHCP range is malformed: `range_start > range_end`, or the range falls
/// outside the network CIDR. The `message` distinguishes the two cases.
pub const DHCP_RANGE_INVALID: &str = "DHCP_RANGE_INVALID";

/// A field whose name suggests a secret (`password`, `token`, `private_key`,
/// `secret`) holds a plain-string literal instead of a `secret_ref`.
pub const RAW_SECRET_FORBIDDEN: &str = "RAW_SECRET_FORBIDDEN";

/// A `role.permissions[]` entry is not a recognised CHV permission string.
pub const INVALID_PERMISSION: &str = "INVALID_PERMISSION";

/// An implicit edge in the YAML model points at a wrong section
/// (`instance.placement.server` must be a server, etc.).
pub const INVALID_EDGE: &str = "INVALID_EDGE";

/// A name appears in both `users[]` (platform) and `instance_users[]` (OS).
/// Per the YAML contract these namespaces must remain disjoint.
pub const USER_NAMESPACE_COLLISION: &str = "USER_NAMESPACE_COLLISION";

/// A static instance IP falls inside the DHCP allocation range of its
/// network. Severity: warning (non-blocking) — DHCP servers can still
/// reserve it, but the operator should know.
pub const STATIC_IP_IN_DHCP_RANGE: &str = "STATIC_IP_IN_DHCP_RANGE";

// retired YYYY-MM-DD: <reason>
//
// (Intentional sentinel — keeps the lifecycle pattern documented and
// machine-grep-friendly. Real retirements append a real entry above this
// sentinel; the parser does not require it.)

/// All live (non-retired) codes. Keep alphabetised within stable groups so
/// adding a code does not produce noisy diffs in the test below.
pub const ALL_CODES: &[&str] = &[
    SCHEMA_INVALID,
    DUPLICATE_NAME,
    MISSING_REFERENCE,
    INVALID_CIDR,
    NETWORK_CIDR_OVERLAP,
    DUPLICATE_IP,
    IP_OUTSIDE_NETWORK,
    GATEWAY_OUTSIDE_NETWORK,
    DHCP_RANGE_INVALID,
    RAW_SECRET_FORBIDDEN,
    INVALID_PERMISSION,
    INVALID_EDGE,
    USER_NAMESPACE_COLLISION,
    STATIC_IP_IN_DHCP_RANGE,
];

/// Canonical CHV permission strings used by `INVALID_PERMISSION`. Wildcard
/// `"*"` is accepted because the canonical example uses it for the
/// platform-admin role.
pub const ALLOWED_PERMISSIONS: &[&str] = &[
    "*",
    "architecture:read",
    "architecture:write",
    "architecture:apply",
    "architecture:destroy",
    "instance:read",
    "instance:create",
    "instance:delete",
    "instance:start",
    "instance:stop",
    "instance:restart",
    "instance:resize",
    "console:access",
    "network:read",
    "network:write",
    "datastore:read",
    "datastore:write",
    "backup:read",
    "backup:write",
    "user:read",
    "user:write",
    "role:read",
    "role:write",
    "settings:read",
    "settings:write",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn codes_are_unique() {
        let set: HashSet<&&str> = ALL_CODES.iter().collect();
        assert_eq!(
            set.len(),
            ALL_CODES.len(),
            "duplicate code detected in ALL_CODES; codes must never be reused"
        );
    }

    #[test]
    fn permissions_are_unique() {
        let set: HashSet<&&str> = ALLOWED_PERMISSIONS.iter().collect();
        assert_eq!(
            set.len(),
            ALLOWED_PERMISSIONS.len(),
            "duplicate permission in ALLOWED_PERMISSIONS"
        );
    }
}
