//! CHV ownership contract for projected NetBox objects (mapping contract
//! "Ownership custom fields" table, design §7).
//!
//! Every NetBox object this adapter creates carries six custom fields that
//! together form the [`ManagedMarker`]:
//!
//! | Custom field | Value | Purpose |
//! |---|---|---|
//! | `chv_external_id` | `arch:<arch_id>:<kind>/<name>:<version>` | Idempotency match key |
//! | `chv_architecture_id` | architecture id | Grouping / scoping |
//! | `chv_managed_by` | literal `chv` | Ownership marker — the write guard |
//! | `chv_managed_state` | `active` \| `stale` | Retention marker |
//! | `chv_architecture_version` | version number | Provenance |
//! | `chv_mapping_version` | `v1` | Contract version |
//!
//! The name prefix (`chv_` by default) is configurable per projection
//! config; the field-name tails are stable contract surface.
//!
//! Pure data + pure functions only (PR-2 boundary of
//! `docs/plans/2026-10-08-netbox-projection-implementation-plan.md`): no
//! I/O, no clock, no randomness — everything here is deterministic so
//! plans built on top are byte-stable.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Version of the CHV → NetBox mapping contract this crate implements.
/// Carried on every projected object as the `chv_mapping_version` custom
/// field.
pub const MAPPING_VERSION: &str = "v1";

/// Default custom-field name prefix.
pub const DEFAULT_CUSTOM_FIELD_PREFIX: &str = "chv_";

/// Literal `managed_by` value marking a NetBox object as owned by CHV.
/// Objects whose `chv_managed_by` differs from this are never written.
pub const MANAGED_BY_CHV: &str = "chv";

/// Resource-type slugs used in external-id construction.
///
/// **Cross-crate contract:** these MUST stay in sync with
/// `resource_type_as_str` in `chv-architecture-reconcile/src/apply/mod.rs`
/// — the reconcile plan builds its resource refs / correlation ids from
/// the same slugs, and external ids are joined against them (see
/// [`resource_ref_from_external_id`](crate::plan) consumers). A
/// divergence would silently break that join; the
/// `external_id_resource_slugs_are_pinned` test in `mapping.rs` pins
/// the exact set so any change surfaces in review of this crate.
pub const RESOURCE_SLUG_SERVER: &str = "server";
/// See [`RESOURCE_SLUG_SERVER`].
pub const RESOURCE_SLUG_NETWORK: &str = "network";
/// See [`RESOURCE_SLUG_SERVER`].
pub const RESOURCE_SLUG_INSTANCE: &str = "instance";

/// The ownership custom-field names for one prefix.
///
/// The six struct fields are the stable contract surface; the prefix is
/// configurable per projection config (`netbox_projection_config.
/// custom_field_prefix`). The [`CustomFieldNames::owner`] and
/// [`CustomFieldNames::datastores`] helpers derive the enrichment field
/// names from the same prefix (mapping contract, object-mapping table).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomFieldNames {
    /// Configurable prefix all names share (default `chv_`).
    pub prefix: String,
    /// `chv_external_id` — idempotency match key.
    pub external_id: String,
    /// `chv_architecture_id` — grouping / scoping.
    pub architecture_id: String,
    /// `chv_managed_by` — ownership marker (the write guard).
    pub managed_by: String,
    /// `chv_managed_state` — retention marker (`active` | `stale`).
    pub managed_state: String,
    /// `chv_architecture_version` — provenance.
    pub architecture_version: String,
    /// `chv_mapping_version` — mapping contract version.
    pub mapping_version: String,
}

impl CustomFieldNames {
    /// Build the six contract field names for `prefix`.
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
            external_id: format!("{prefix}external_id"),
            architecture_id: format!("{prefix}architecture_id"),
            managed_by: format!("{prefix}managed_by"),
            managed_state: format!("{prefix}managed_state"),
            architecture_version: format!("{prefix}architecture_version"),
            mapping_version: format!("{prefix}mapping_version"),
        }
    }

    /// The six ownership field names, in contract-table order.
    pub fn ownership_fields(&self) -> [&str; 6] {
        [
            &self.external_id,
            &self.architecture_id,
            &self.managed_by,
            &self.managed_state,
            &self.architecture_version,
            &self.mapping_version,
        ]
    }

    /// Enrichment custom field recording `metadata.owner` (mapping
    /// contract, object-mapping table: "owner label recorded as a custom
    /// field").
    pub fn owner(&self) -> String {
        format!("{}owner", self.prefix)
    }

    /// Device-enrichment custom field listing live datastore facts for a
    /// host (mapping contract rule 6: datastore facts may appear as device
    /// custom-field enrichment only).
    pub fn datastores(&self) -> String {
        format!("{}datastores", self.prefix)
    }

    /// Device-enrichment custom field carrying the CPU fact (live
    /// snapshot wins over declared, contract rule 4). Mapping contract,
    /// object-mapping table: "CPU/memory as custom fields when live
    /// facts exist".
    pub fn cpu_cores(&self) -> String {
        format!("{}cpu_cores", self.prefix)
    }

    /// Device-enrichment custom field carrying the memory fact in GiB
    /// (live snapshot wins over declared, contract rule 4). See
    /// [`CustomFieldNames::cpu_cores`].
    pub fn memory_gb(&self) -> String {
        format!("{}memory_gb", self.prefix)
    }
}

impl Default for CustomFieldNames {
    fn default() -> Self {
        Self::new(DEFAULT_CUSTOM_FIELD_PREFIX)
    }
}

/// Derive the stable external-id key for one CHV resource:
/// `arch:<architecture_id>:<kind>/<name>:<version>`.
///
/// `kind` uses the stable resource-type slug already used by the
/// architecture reconcile plan (`server`, `network`, `instance`, …).
/// Derived objects (interfaces, VLANs, IP addresses) reuse their parent
/// kind slug with a composite name segment, e.g.
/// `arch:arch_01HX:instance/vm-01/backend:3` for a VM interface.
pub fn external_id(architecture_id: &str, kind: &str, name: &str, version: u64) -> String {
    format!("arch:{architecture_id}:{kind}/{name}:{version}")
}

/// Custom-field name fragments that mark a field as a secret carrier
/// (contract rule 5: `secret_ref`, `password`, `token`,
/// `ssh_keys.public_key`, `User.auth` material).
///
/// Shared by the fail-closed scrubber in [`crate::mapping`] and by
/// [`validate_custom_field_prefix`] so a configured prefix can never
/// generate scrubber-tripping field names of our own.
pub const SECRET_FIELD_NAME_PATTERNS: &[&str] = &[
    "secret_ref",
    "password",
    "token",
    "public_key",
    "private_key",
    "ssh_key",
    "auth",
];

/// `true` when a custom-field *name* marks it as a secret carrier.
pub fn is_secret_field_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    SECRET_FIELD_NAME_PATTERNS.iter().any(|p| lower.contains(p))
}

/// Validate a configurable custom-field name prefix.
///
/// Rejects:
///
/// - the empty prefix — every custom-field name starts with `""`, so
///   the removal-diff loop in [`crate::plan::content_changes`] would
///   treat *every* remote custom field (foreign ones included) as a
///   projected field pending removal;
/// - prefixes containing any [`SECRET_FIELD_NAME_PATTERNS`] fragment —
///   the six ownership fields derived from such a prefix
///   (`token_managed_by`, …) would trip the fail-closed secret
///   scrubber on our own output.
///
/// Enforced at the entry points ([`crate::mapping::build_objects`] and
/// [`crate::plan::compute_plan`]) because the config structs have
/// public fields; the config layer (PR 3) may also call this directly.
pub fn validate_custom_field_prefix(prefix: &str) -> Result<(), String> {
    if prefix.is_empty() {
        return Err("custom-field prefix must not be empty (every remote custom field would be treated as projected)".to_string());
    }
    let lower = prefix.to_ascii_lowercase();
    if let Some(pattern) = SECRET_FIELD_NAME_PATTERNS
        .iter()
        .find(|p| lower.contains(*p))
    {
        return Err(format!(
            "custom-field prefix {prefix:?} matches the secret-name pattern {pattern:?}; \
             derived ownership fields would trip the fail-closed secret scrubber"
        ));
    }
    Ok(())
}

/// Retention marker carried on every projected object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedState {
    /// The CHV source still exists.
    Active,
    /// The CHV source disappeared; the object is retained but marked.
    Stale,
}

impl ManagedState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Stale => "stale",
        }
    }
}

/// The ownership marker parsed from (or written into) a NetBox object's
/// custom fields. `parse` returns `None` unless **all six** contract
/// fields are present and well-formed — a partially-written marker is not
/// a marker (this is what distinguishes a partial-failure resume from a
/// foreign object in [`crate::plan::compute_plan`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedMarker {
    pub external_id: String,
    pub architecture_id: String,
    pub managed_by: String,
    pub managed_state: ManagedState,
    pub architecture_version: u64,
    pub mapping_version: String,
}

impl ManagedMarker {
    /// Parse a marker out of a NetBox object's custom fields. `None` when
    /// any of the six fields is missing or malformed.
    ///
    /// A marker whose `chv_mapping_version` differs from
    /// [`MAPPING_VERSION`] still parses — as a *foreign* marker — but
    /// [`ManagedMarker::is_owned_by_chv`] then returns `false`, so the
    /// planner emits a `conflict` and never a write. Fail-closed by
    /// design: a future v2 writer's objects must not be silently
    /// rewritten by a v1 adapter, and a v1 adapter cannot know what
    /// semantics a v2 marker carries.
    pub fn parse(
        custom_fields: &BTreeMap<String, String>,
        names: &CustomFieldNames,
    ) -> Option<Self> {
        let managed_state = match custom_fields.get(&names.managed_state)?.as_str() {
            "active" => ManagedState::Active,
            "stale" => ManagedState::Stale,
            _ => return None,
        };
        Some(Self {
            external_id: custom_fields.get(&names.external_id)?.clone(),
            architecture_id: custom_fields.get(&names.architecture_id)?.clone(),
            managed_by: custom_fields.get(&names.managed_by)?.clone(),
            managed_state,
            architecture_version: custom_fields
                .get(&names.architecture_version)?
                .parse()
                .ok()?,
            mapping_version: custom_fields.get(&names.mapping_version)?.clone(),
        })
    }

    /// The write guard: `true` only when `chv_managed_by == "chv"` and
    /// the marker was written by this mapping contract version
    /// ([`MAPPING_VERSION`]).
    ///
    /// An object with `chv_managed_by == "chv"` but a different
    /// `chv_mapping_version` (a future v2 writer's object) is **not**
    /// ours to rewrite: this adapter cannot interpret v2 semantics, so
    /// it fails closed — the planner turns such objects into
    /// `conflict` entries, never writes.
    pub fn is_owned_by_chv(&self) -> bool {
        self.managed_by == MANAGED_BY_CHV && self.mapping_version == MAPPING_VERSION
    }

    /// Render the marker back into the six custom fields.
    pub fn to_custom_fields(&self, names: &CustomFieldNames) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        fields.insert(names.external_id.clone(), self.external_id.clone());
        fields.insert(names.architecture_id.clone(), self.architecture_id.clone());
        fields.insert(names.managed_by.clone(), self.managed_by.clone());
        fields.insert(
            names.managed_state.clone(),
            self.managed_state.as_str().to_string(),
        );
        fields.insert(
            names.architecture_version.clone(),
            self.architecture_version.to_string(),
        );
        fields.insert(names.mapping_version.clone(), self.mapping_version.clone());
        fields
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_id_format_is_exact() {
        // Contract example shape, character for character.
        assert_eq!(
            external_id("arch_01HX", "server", "chv-node-01", 3),
            "arch:arch_01HX:server/chv-node-01:3"
        );
        assert_eq!(
            external_id("arch_01HX", "network", "backend#vlan", 7),
            "arch:arch_01HX:network/backend#vlan:7"
        );
    }

    #[test]
    fn default_custom_field_names_use_chv_prefix() {
        let names = CustomFieldNames::default();
        assert_eq!(names.prefix, "chv_");
        assert_eq!(names.external_id, "chv_external_id");
        assert_eq!(names.architecture_id, "chv_architecture_id");
        assert_eq!(names.managed_by, "chv_managed_by");
        assert_eq!(names.managed_state, "chv_managed_state");
        assert_eq!(names.architecture_version, "chv_architecture_version");
        assert_eq!(names.mapping_version, "chv_mapping_version");
        assert_eq!(names.owner(), "chv_owner");
        assert_eq!(names.datastores(), "chv_datastores");
        assert_eq!(names.cpu_cores(), "chv_cpu_cores");
        assert_eq!(names.memory_gb(), "chv_memory_gb");
    }

    #[test]
    fn configurable_prefix_changes_all_six_names() {
        let names = CustomFieldNames::new("acme_");
        assert_eq!(names.external_id, "acme_external_id");
        assert_eq!(names.architecture_id, "acme_architecture_id");
        assert_eq!(names.managed_by, "acme_managed_by");
        assert_eq!(names.managed_state, "acme_managed_state");
        assert_eq!(names.architecture_version, "acme_architecture_version");
        assert_eq!(names.mapping_version, "acme_mapping_version");
        // No default-prefixed name survives.
        for name in names.ownership_fields() {
            assert!(!name.starts_with("chv_"), "{name} kept the default prefix");
        }
    }

    #[test]
    fn marker_roundtrips_through_custom_fields() {
        let names = CustomFieldNames::default();
        let marker = ManagedMarker {
            external_id: external_id("arch_01HX", "server", "chv-node-01", 3),
            architecture_id: "arch_01HX".to_string(),
            managed_by: MANAGED_BY_CHV.to_string(),
            managed_state: ManagedState::Active,
            architecture_version: 3,
            mapping_version: MAPPING_VERSION.to_string(),
        };
        let fields = marker.to_custom_fields(&names);
        assert_eq!(ManagedMarker::parse(&fields, &names), Some(marker.clone()));
        assert!(marker.is_owned_by_chv());
    }

    #[test]
    fn marker_parse_rejects_partial_or_foreign_markers() {
        let names = CustomFieldNames::default();
        let full = ManagedMarker {
            external_id: "arch:a:server/n:1".to_string(),
            architecture_id: "a".to_string(),
            managed_by: "netops".to_string(),
            managed_state: ManagedState::Stale,
            architecture_version: 1,
            mapping_version: MAPPING_VERSION.to_string(),
        }
        .to_custom_fields(&names);

        // Foreign owner parses fine but is not owned by chv.
        let parsed = ManagedMarker::parse(&full, &names).expect("full marker parses");
        assert!(!parsed.is_owned_by_chv());

        // Missing any one of the six fields → None.
        for key in names.ownership_fields() {
            let mut partial = full.clone();
            partial.remove(key);
            assert_eq!(
                ManagedMarker::parse(&partial, &names),
                None,
                "missing {key}"
            );
        }

        // Malformed version / state values → None.
        let mut bad = full.clone();
        bad.insert(
            names.architecture_version.clone(),
            "not-a-number".to_string(),
        );
        assert_eq!(ManagedMarker::parse(&bad, &names), None);
        let mut bad = full;
        bad.insert(names.managed_state.clone(), "half-baked".to_string());
        assert_eq!(ManagedMarker::parse(&bad, &names), None);
    }

    #[test]
    fn marker_with_foreign_mapping_version_parses_but_is_not_owned() {
        // Fail-closed (F7): a v2 writer's marker is a well-formed
        // *foreign* marker — it parses (so the planner can emit a
        // conflict naming it) but is never treated as chv-owned, so no
        // write is ever proposed against it.
        let names = CustomFieldNames::default();
        let fields = ManagedMarker {
            external_id: "arch:a:server/n:1".to_string(),
            architecture_id: "a".to_string(),
            managed_by: MANAGED_BY_CHV.to_string(),
            managed_state: ManagedState::Active,
            architecture_version: 1,
            mapping_version: "v2".to_string(),
        }
        .to_custom_fields(&names);
        let parsed = ManagedMarker::parse(&fields, &names).expect("foreign marker parses");
        assert!(!parsed.is_owned_by_chv());
    }

    #[test]
    fn custom_field_prefix_validation_rejects_empty_and_secret_shaped() {
        assert!(validate_custom_field_prefix("chv_").is_ok());
        assert!(validate_custom_field_prefix("acme_").is_ok());
        // Empty: the removal-diff loop would match every remote field.
        assert_eq!(
            validate_custom_field_prefix("").unwrap_err(),
            "custom-field prefix must not be empty (every remote custom field would be treated as projected)"
        );
        // Secret-shaped prefixes would trip the fail-closed scrubber on
        // our own derived field names.
        for prefix in ["token_", "auth_", "SECRET_REF_", "my_password_", "ssh_key_"] {
            assert!(
                validate_custom_field_prefix(prefix).is_err(),
                "prefix {prefix:?} must be rejected"
            );
        }
    }
}
