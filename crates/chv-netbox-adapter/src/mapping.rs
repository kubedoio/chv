//! Pure CHV → NetBox object builders (mapping contract v1).
//!
//! Implements the "Object mapping" table and "Mapping rules" 1–6 of
//! `docs/specs/architecture-designer/contracts/netbox-mapping-contract.md`
//! on top of the applied [`CHVArchitecture`] model, optionally enriched
//! with live [`InventorySnapshot`] host facts (design §4.2 / DP3: the
//! applied version is the source of truth; the snapshot is enrichment
//! only).
//!
//! # PR-2 boundary
//!
//! Everything here is pure and deterministic: no HTTP, no tokio, no
//! sqlx, no clock. The REST client that materializes these objects and
//! the runner that executes plans arrive in PR 4.
//!
//! # Invalid names
//!
//! Names project verbatim (rule 1). A name that fails
//! [`validate_netbox_name`] is **never silently renamed** — the object is
//! omitted from [`MappingOutput::objects`] and reported as a
//! [`MappingIssue`] instead, which [`crate::plan::compute_plan`] turns
//! into a `conflict` plan entry with the reason. Nothing is dropped
//! silently.

use chv_architecture_validate::fleet::InventorySnapshot;
use chv_architecture_validate::model::{CHVArchitecture, NetworkType};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

use crate::ownership::{
    external_id, is_secret_field_name, validate_custom_field_prefix, CustomFieldNames,
    ManagedMarker, ManagedState, MANAGED_BY_CHV, MAPPING_VERSION, RESOURCE_SLUG_INSTANCE,
    RESOURCE_SLUG_NETWORK, RESOURCE_SLUG_SERVER,
};

/// Maximum length of a NetBox object name / slug.
pub const NETBOX_NAME_MAX_LEN: usize = 100;

/// Manufacturer slug the projection's device writes reference
/// (`device_type.manufacturer.slug`). A **provisioning prerequisite**:
/// the manufacturer must exist before the first device write or
/// NetBox answers 400 — the projection never creates it. The
/// qualification lane and the fixture recorder provision exactly this
/// slug; see the mapping contract's "Provisioning prerequisites".
pub const CHV_NETBOX_MANUFACTURER: &str = "chv";

/// Device-type slug the projection's device writes reference
/// (`device_type.slug`). Same provisioning prerequisite as the
/// manufacturer; NetBox 4.7's `DeviceSerializer` requires `device_type`
/// on every device write.
pub const CHV_NETBOX_DEVICE_TYPE: &str = "chv-host";

/// Device-role slug the projection's device writes reference
/// (`role.slug`). NetBox 4.7's `DeviceSerializer` also requires `role`
/// on every device write (the model FK is non-nullable). Matches the
/// mapping contract's object table ("role `chv-node`").
pub const CHV_NETBOX_DEVICE_ROLE: &str = "chv-node";

/// Placeholder used when diffing a field that is unset on one side.
/// Never written to NetBox — only used inside `changes` strings.
const UNSET: &str = "(unset)";

// ---------------------------------------------------------------------------
// NetBox object kinds
// ---------------------------------------------------------------------------

/// NetBox object kinds the v1 mapping projects, in plan order.
///
/// Declaration order **is** the contract's kind rank (mapping contract
/// rule 3): `vlan → prefix → device → virtual_machine → interface →
/// ip_address`. The order is FK-dependency-safe for execution: real
/// NetBox resolves nested-FK writes by **existence** (a dict or PK
/// reference must match an existing row), so parents must be created
/// before the children that reference them — prefix→vlan, VM→device,
/// interface→VM — and the IP address lands last, after the interface
/// its assignment references.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetBoxKind {
    Vlan,
    Prefix,
    Device,
    VirtualMachine,
    Interface,
    IpAddress,
}

impl NetBoxKind {
    /// The contract's `kind` string used in plan entries.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vlan => "vlan",
            Self::Prefix => "prefix",
            Self::IpAddress => "ip_address",
            Self::Interface => "interface",
            Self::VirtualMachine => "virtual_machine",
            Self::Device => "device",
        }
    }

    /// Deterministic ordering rank; lower sorts first.
    pub const fn rank(self) -> u8 {
        self as u8
    }
}

/// NetBox device status. The pure core projects the *applied* topology,
/// so devices are `active`; lifecycle-driven transitions are the
/// runner's concern (PR 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    Active,
    Offline,
}

impl DeviceStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Offline => "offline",
        }
    }
}

/// NetBox virtual machine status. `Active` when the projected version is
/// the applied one (the trigger model only fires post-successful-apply
/// or on explicit export); `Staged` is available for the runner when
/// projecting not-yet-applied state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VmStatus {
    Active,
    Staged,
}

impl VmStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Staged => "staged",
        }
    }
}

// ---------------------------------------------------------------------------
// NetBox object models (only what the mapping contract needs)
// ---------------------------------------------------------------------------

/// DCIM Device projected from `servers[]` (+ live node facts).
///
/// CPU and memory are **not** top-level content fields: per the mapping
/// contract ("CPU/memory as custom fields when live facts exist") they
/// ride in [`NetBoxDevice::custom_fields`] under the prefixed names
/// [`CustomFieldNames::cpu_cores`] / [`CustomFieldNames::memory_gb`],
/// and therefore diff as custom-field changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetBoxDevice {
    pub name: String,
    pub site: Option<String>,
    pub status: DeviceStatus,
    /// Deterministic, sorted NetBox tags derived from `metadata`.
    pub tags: Vec<String>,
    pub custom_fields: BTreeMap<String, String>,
}

/// DCIM VirtualMachine projected from `instances[]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetBoxVirtualMachine {
    pub name: String,
    pub status: VmStatus,
    /// No cluster concept exists in CHV v1; left unset rather than
    /// fabricated (contract rule 2).
    pub cluster: Option<String>,
    /// `placement.server` when it resolves to a mapped server.
    pub device: Option<String>,
    /// Declared `instances[].resources.cpu` — the snapshot has no VM
    /// facts (design G2), so declared wins for VMs (contract rule 4).
    pub cpu: Option<u32>,
    /// Declared `instances[].resources.memory_mb`.
    pub memory_mb: Option<u32>,
    pub tags: Vec<String>,
    pub custom_fields: BTreeMap<String, String>,
}

/// DCIM (virtualization) Interface projected from
/// `instances[].networks[]`. `type: virtual` per the contract; MAC
/// addresses are not modelled in v1.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetBoxInterface {
    pub name: String,
    /// Parent VM name — the second half of the natural key.
    pub virtual_machine: String,
    /// CHV network name, per the contract.
    pub description: String,
    pub tags: Vec<String>,
    pub custom_fields: BTreeMap<String, String>,
}

/// IPAM Prefix projected from `networks[]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetBoxPrefix {
    /// The CIDR (`networks[].cidr`).
    pub prefix: String,
    /// Linked VLAN vid when the network carries `vlan_id`.
    pub vlan: Option<u32>,
    pub description: String,
    /// The CHV source network name. Kept so `chv_resource_ref` can be
    /// built as `networks/<network_name>` (the `description` only
    /// carries "name (type)" and would produce malformed refs). **Not**
    /// projected as content: excluded from `content_fields()` and from
    /// the natural key.
    pub network_name: String,
    pub tags: Vec<String>,
    pub custom_fields: BTreeMap<String, String>,
}

/// IPAM VLAN projected from `networks[]` with `vlan_id`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetBoxVlan {
    pub vid: u32,
    /// The CHV network name.
    pub name: String,
    pub tags: Vec<String>,
    pub custom_fields: BTreeMap<String, String>,
}

/// IPAM IPAddress projected from `instances[].networks[].ip`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetBoxIpAddress {
    pub address: String,
    /// Identifies the owning VM interface as `"<vm>/<interface>"`;
    /// `None` means unassigned.
    pub assigned_to_interface: Option<String>,
    pub tags: Vec<String>,
    pub custom_fields: BTreeMap<String, String>,
}

/// One projected NetBox object. Only the six kinds the v1 contract maps
/// are modelled — anything else is an error upstream, never fabricated.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NetBoxObject {
    Device(NetBoxDevice),
    VirtualMachine(NetBoxVirtualMachine),
    Interface(NetBoxInterface),
    Prefix(NetBoxPrefix),
    Vlan(NetBoxVlan),
    IpAddress(NetBoxIpAddress),
}

impl NetBoxObject {
    /// Contract kind of this object.
    pub fn kind(&self) -> NetBoxKind {
        match self {
            Self::Device(_) => NetBoxKind::Device,
            Self::VirtualMachine(_) => NetBoxKind::VirtualMachine,
            Self::Interface(_) => NetBoxKind::Interface,
            Self::Prefix(_) => NetBoxKind::Prefix,
            Self::Vlan(_) => NetBoxKind::Vlan,
            Self::IpAddress(_) => NetBoxKind::IpAddress,
        }
    }

    /// Primary sort/collision name: the object `name` for named kinds,
    /// the CIDR for prefixes and the address for IP addresses.
    pub fn name(&self) -> &str {
        match self {
            Self::Device(d) => &d.name,
            Self::VirtualMachine(v) => &v.name,
            Self::Interface(i) => &i.name,
            Self::Prefix(p) => &p.prefix,
            Self::Vlan(v) => &v.name,
            Self::IpAddress(a) => &a.address,
        }
    }

    /// The natural key used for collision detection (design §6.1):
    /// Device/VM `name`; Prefix `prefix`; IPAddress `address`; VLAN
    /// `vid`; Interface `name` + parent VM.
    pub fn natural_key(&self) -> BTreeMap<String, String> {
        let mut key = BTreeMap::new();
        match self {
            Self::Device(d) => {
                key.insert("name".to_string(), d.name.clone());
            }
            Self::VirtualMachine(v) => {
                key.insert("name".to_string(), v.name.clone());
            }
            Self::Interface(i) => {
                key.insert("name".to_string(), i.name.clone());
                key.insert("virtual_machine".to_string(), i.virtual_machine.clone());
            }
            Self::Prefix(p) => {
                key.insert("prefix".to_string(), p.prefix.clone());
            }
            Self::Vlan(v) => {
                key.insert("vid".to_string(), v.vid.to_string());
            }
            Self::IpAddress(a) => {
                key.insert("address".to_string(), a.address.clone());
            }
        }
        key
    }

    /// The object's custom fields (ownership marker + enrichment).
    pub fn custom_fields(&self) -> &BTreeMap<String, String> {
        match self {
            Self::Device(d) => &d.custom_fields,
            Self::VirtualMachine(v) => &v.custom_fields,
            Self::Interface(i) => &i.custom_fields,
            Self::Prefix(p) => &p.custom_fields,
            Self::Vlan(v) => &v.custom_fields,
            Self::IpAddress(a) => &a.custom_fields,
        }
    }

    /// The mapped content fields (everything except custom fields),
    /// flattened to strings for deterministic diffing. This is the
    /// `no_op` vs `update` comparison surface consumed by
    /// [`crate::plan::compute_plan`].
    pub fn content_fields(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let tags = |t: &[String]| t.join(",");
        match self {
            Self::Device(d) => {
                fields.insert("name".to_string(), d.name.clone());
                fields.insert("site".to_string(), opt_str(&d.site));
                fields.insert("status".to_string(), d.status.as_str().to_string());
                fields.insert("tags".to_string(), tags(&d.tags));
            }
            Self::VirtualMachine(v) => {
                fields.insert("name".to_string(), v.name.clone());
                fields.insert("status".to_string(), v.status.as_str().to_string());
                fields.insert("cluster".to_string(), opt_str(&v.cluster));
                fields.insert("device".to_string(), opt_str(&v.device));
                fields.insert("cpu".to_string(), opt_u32(v.cpu));
                fields.insert("memory_mb".to_string(), opt_u32(v.memory_mb));
                fields.insert("tags".to_string(), tags(&v.tags));
            }
            Self::Interface(i) => {
                fields.insert("name".to_string(), i.name.clone());
                fields.insert("virtual_machine".to_string(), i.virtual_machine.clone());
                fields.insert("description".to_string(), i.description.clone());
                fields.insert("tags".to_string(), tags(&i.tags));
            }
            Self::Prefix(p) => {
                fields.insert("prefix".to_string(), p.prefix.clone());
                fields.insert("vlan".to_string(), opt_u32(p.vlan));
                fields.insert("description".to_string(), p.description.clone());
                fields.insert("tags".to_string(), tags(&p.tags));
            }
            Self::Vlan(v) => {
                fields.insert("vid".to_string(), v.vid.to_string());
                fields.insert("name".to_string(), v.name.clone());
                fields.insert("tags".to_string(), tags(&v.tags));
            }
            Self::IpAddress(a) => {
                fields.insert("address".to_string(), a.address.clone());
                fields.insert(
                    "assigned_to_interface".to_string(),
                    opt_str(&a.assigned_to_interface),
                );
                fields.insert("tags".to_string(), tags(&a.tags));
            }
        }
        fields
    }
}

fn opt_str(value: &Option<String>) -> String {
    value.clone().unwrap_or_else(|| UNSET.to_string())
}

fn opt_u32(value: Option<u32>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| UNSET.to_string())
}

// ---------------------------------------------------------------------------
// Projection input / output
// ---------------------------------------------------------------------------

/// The pure slice of `NetboxProjectionConfig` the builders need. The
/// full config (endpoint, token secret ref, retention, …) is PR 3+.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionConfigView {
    /// Custom-field name prefix (default `chv_`).
    pub custom_field_prefix: String,
    /// NetBox site to place devices in; falls back to a slugified
    /// `metadata.environment` when unset.
    pub site_name: Option<String>,
}

impl Default for ProjectionConfigView {
    fn default() -> Self {
        Self {
            custom_field_prefix: crate::ownership::DEFAULT_CUSTOM_FIELD_PREFIX.to_string(),
            site_name: None,
        }
    }
}

/// Everything the pure builders consume: the applied architecture
/// version, optionally enriched with a live inventory snapshot.
#[derive(Clone, Debug)]
pub struct ProjectionInput<'a> {
    /// The applied, validated CHVArchitecture model (authoritative).
    pub architecture: &'a CHVArchitecture,
    /// Architecture id used in external ids and ownership fields.
    pub architecture_id: &'a str,
    /// Applied architecture version number (provenance).
    pub architecture_version: u64,
    /// Live fleet facts; `None` projects declared state only.
    pub snapshot: Option<&'a InventorySnapshot>,
    /// Pure projection config.
    pub config: ProjectionConfigView,
}

/// An object that could not be projected because its name fails NetBox
/// validation. Reported (never silently dropped) and turned into a
/// `conflict` plan entry by [`crate::plan::compute_plan`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingIssue {
    pub kind: NetBoxKind,
    /// `servers/<name>`-style CHV resource reference.
    pub chv_resource_ref: String,
    /// The offending name, verbatim.
    pub name: String,
    /// The external id the object would have carried.
    pub external_id: String,
    /// Why the name failed validation.
    pub reason: String,
}

/// Result of [`build_objects`]: valid objects plus reported issues.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingOutput {
    /// Valid objects, deterministically ordered (kind rank, then name,
    /// then natural key).
    pub objects: Vec<NetBoxObject>,
    /// Invalid-name issues; nothing is dropped silently.
    pub issues: Vec<MappingIssue>,
}

/// Hard failures of the pure builders. Invalid *names* are not errors —
/// they become [`MappingIssue`]s so a plan can still be produced.
#[derive(Debug, Error)]
pub enum MappingError {
    /// The projection input itself is unusable (e.g. empty architecture
    /// id — external ids would be ambiguous).
    #[error("invalid projection input: {reason}")]
    InvalidInput { reason: String },

    /// The configured custom-field prefix is unusable: empty (the
    /// removal-diff loop would treat every remote custom field as
    /// projected) or secret-shaped (derived field names would trip the
    /// fail-closed scrubber). See
    /// [`crate::ownership::validate_custom_field_prefix`].
    #[error("invalid custom-field prefix {prefix:?}: {reason}")]
    InvalidCustomFieldPrefix { prefix: String, reason: String },

    /// A custom-field name carrying secret material reached the output.
    /// Fail-closed: the builders only write contract-named fields, so
    /// this variant firing means a builder bug, and scrubbing silently
    /// would hide it.
    #[error("secret material detected in custom field {field:?}; refusing to project")]
    SecretMaterial { field: String },
}

// ---------------------------------------------------------------------------
// Name validation (contract rule 1)
// ---------------------------------------------------------------------------

/// Validate a name against the NetBox charset: non-empty, at most
/// [`NETBOX_NAME_MAX_LEN`] characters, lowercase alphanumerics plus
/// `-`, `_` and `.`. Returns the failure reason on `Err`.
pub fn validate_netbox_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("name is empty".to_string());
    }
    if name.len() > NETBOX_NAME_MAX_LEN {
        return Err(format!("name exceeds {NETBOX_NAME_MAX_LEN} characters"));
    }
    if let Some(bad) = name
        .chars()
        .find(|c| !matches!(c, 'a'..='z' | '0'..='9' | '-' | '_' | '.'))
    {
        return Err(format!(
            "character {bad:?} is not allowed (lowercase alphanumerics, '-', '_' and '.')"
        ));
    }
    Ok(())
}

/// Validate a name against the strict NetBox **slug** charset:
/// non-empty, at most [`NETBOX_NAME_MAX_LEN`] characters, lowercase
/// alphanumerics plus `-` and `_` — no `.`.
///
/// NetBox VLAN `name`/`slug` fields are strict slugs, so a network
/// named `backend.v2` with a `vlan_id` fails here and produces a
/// mapping issue (contract rule 1: conflict, never a silent rename).
/// Device, VM and interface names allow dots — use
/// [`validate_netbox_name`] for those.
pub fn validate_netbox_slug(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("name is empty".to_string());
    }
    if name.len() > NETBOX_NAME_MAX_LEN {
        return Err(format!("name exceeds {NETBOX_NAME_MAX_LEN} characters"));
    }
    if let Some(bad) = name
        .chars()
        .find(|c| !matches!(c, 'a'..='z' | '0'..='9' | '-' | '_'))
    {
        return Err(format!(
            "character {bad:?} is not allowed (VLAN names are NetBox slugs: lowercase alphanumerics, '-' and '_')"
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Secret exclusion (contract rule 5)
// ---------------------------------------------------------------------------

/// Scrub helper enforcing contract rule 5 on the built objects: any
/// custom field whose *name* marks it as a secret carrier aborts the
/// projection with [`MappingError::SecretMaterial`].
///
/// This is defense in depth — the builders below only ever write
/// contract-named fields and never read secret-bearing model fields —
/// but the invariant "secret-shaped names never reach NetBox" is
/// enforced mechanically, not by convention.
fn scrub_secret_fields(custom_fields: &BTreeMap<String, String>) -> Result<(), MappingError> {
    if let Some(field) = custom_fields.keys().find(|k| is_secret_field_name(k)) {
        return Err(MappingError::SecretMaterial {
            field: field.clone(),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------

/// Build the desired NetBox objects (and reported issues) from a
/// projection input. Deterministic: identical inputs produce
/// byte-identical output.
///
/// Exclusions (contract rule 6) are enforced structurally — only
/// servers, instances, instance networks and networks are ever read;
/// `users`, `roles`, `instance_users`, `ssh_keys`, `backup_targets`,
/// `backup_policies`, `projects`, `images`, `templates` and
/// `datastores` never produce objects (datastore facts may surface as
/// device custom-field enrichment only).
pub fn build_objects(input: &ProjectionInput) -> Result<MappingOutput, MappingError> {
    if input.architecture_id.is_empty() {
        return Err(MappingError::InvalidInput {
            reason: "architecture_id must not be empty".to_string(),
        });
    }
    if let Err(reason) = validate_custom_field_prefix(&input.config.custom_field_prefix) {
        return Err(MappingError::InvalidCustomFieldPrefix {
            prefix: input.config.custom_field_prefix.clone(),
            reason,
        });
    }

    let names = CustomFieldNames::new(&input.config.custom_field_prefix);
    let arch = input.architecture;
    let arch_id = input.architecture_id;
    let version = input.architecture_version;

    let mut objects = Vec::new();
    let mut issues = Vec::new();

    let tags = architecture_tags(arch);

    // servers[] → devices (live node facts win over declared resources).
    for server in &arch.servers {
        let ext = external_id(arch_id, RESOURCE_SLUG_SERVER, &server.name, version);
        let resource_ref = format!("servers/{}", server.name);
        if let Err(reason) = validate_netbox_name(&server.name) {
            issues.push(MappingIssue {
                kind: NetBoxKind::Device,
                chv_resource_ref: resource_ref,
                name: server.name.clone(),
                external_id: ext,
                reason,
            });
            continue;
        }

        let (cpu_cores, memory_gb) = match input
            .snapshot
            .and_then(|s| s.nodes.iter().find(|n| n.name == server.name))
        {
            Some(node) => (Some(node.cpu_cores), Some(node.memory_gb)),
            None => (
                server.resources.as_ref().and_then(|r| r.cpu_cores),
                server.resources.as_ref().and_then(|r| r.memory_gb),
            ),
        };

        let mut custom_fields = ownership_custom_fields(&names, &ext, arch_id, version, arch);
        // CPU/memory project as custom fields (mapping contract,
        // object-mapping table), only when a fact exists — contract
        // rule 2: nullable facts stay unset, no placeholder values.
        if let Some(cpu) = cpu_cores {
            custom_fields.insert(names.cpu_cores(), cpu.to_string());
        }
        if let Some(memory) = memory_gb {
            custom_fields.insert(names.memory_gb(), memory.to_string());
        }
        if let Some(snap) = input.snapshot {
            let mut stores: Vec<String> = snap
                .datastores
                .iter()
                .filter(|d| d.host.as_deref() == Some(server.name.as_str()))
                .map(|d| format!("{}:{}", d.name, d.kind))
                .collect();
            stores.sort();
            if !stores.is_empty() {
                custom_fields.insert(names.datastores(), stores.join(","));
            }
        }

        objects.push(NetBoxObject::Device(NetBoxDevice {
            name: server.name.clone(),
            site: input
                .config
                .site_name
                .clone()
                .or_else(|| arch.metadata.environment.as_deref().map(slugify)),
            status: DeviceStatus::Active,
            tags: tags.clone(),
            custom_fields,
        }));
    }

    // instances[] → virtual machines (+ interfaces, + ip addresses).
    for instance in &arch.instances {
        let ext = external_id(arch_id, RESOURCE_SLUG_INSTANCE, &instance.name, version);
        let resource_ref = format!("instances/{}", instance.name);
        if let Err(reason) = validate_netbox_name(&instance.name) {
            // The VM name is part of every child natural key — report the
            // whole subtree rather than silently dropping it.
            issues.push(MappingIssue {
                kind: NetBoxKind::VirtualMachine,
                chv_resource_ref: resource_ref.clone(),
                name: instance.name.clone(),
                external_id: ext.clone(),
                reason: reason.clone(),
            });
            for net in &instance.networks {
                issues.push(MappingIssue {
                    kind: NetBoxKind::Interface,
                    chv_resource_ref: format!("{resource_ref}/networks/{}", net.name),
                    name: net.name.clone(),
                    external_id: external_id(
                        arch_id,
                        "instance",
                        &format!("{}/{}", instance.name, net.name),
                        version,
                    ),
                    reason: format!("parent virtual machine name is invalid: {reason}"),
                });
                if let Some(ip) = &net.ip {
                    issues.push(MappingIssue {
                        kind: NetBoxKind::IpAddress,
                        chv_resource_ref: format!("{resource_ref}/{}", net.name),
                        name: ip.clone(),
                        external_id: external_id(
                            arch_id,
                            RESOURCE_SLUG_INSTANCE,
                            &format!("{}/{}#{}", instance.name, net.name, ip),
                            version,
                        ),
                        reason: format!("parent virtual machine name is invalid: {reason}"),
                    });
                }
            }
            continue;
        }

        // Declared wins for VMs (contract rule 4): the snapshot carries no
        // VM facts (design G2), so resources come from the model only.
        objects.push(NetBoxObject::VirtualMachine(NetBoxVirtualMachine {
            name: instance.name.clone(),
            status: VmStatus::Active,
            cluster: None,
            device: instance
                .placement
                .as_ref()
                .and_then(|p| p.server.clone())
                .filter(|server| arch.servers.iter().any(|s| &s.name == server)),
            cpu: instance.resources.as_ref().and_then(|r| r.cpu),
            memory_mb: instance.resources.as_ref().and_then(|r| r.memory_mb),
            tags: tags.clone(),
            custom_fields: ownership_custom_fields(&names, &ext, arch_id, version, arch),
        }));

        for net in &instance.networks {
            let iface_ext = external_id(
                arch_id,
                RESOURCE_SLUG_INSTANCE,
                &format!("{}/{}", instance.name, net.name),
                version,
            );
            let iface_ref = format!("{resource_ref}/networks/{}", net.name);
            if let Err(reason) = validate_netbox_name(&net.name) {
                issues.push(MappingIssue {
                    kind: NetBoxKind::Interface,
                    chv_resource_ref: iface_ref.clone(),
                    name: net.name.clone(),
                    external_id: iface_ext.clone(),
                    reason: reason.clone(),
                });
                if let Some(ip) = &net.ip {
                    issues.push(MappingIssue {
                        kind: NetBoxKind::IpAddress,
                        // Same ref form as the live IP object
                        // (`instances/<vm>/<net>`), not the interface's
                        // `instances/<vm>/networks/<net>`.
                        chv_resource_ref: format!("{resource_ref}/{}", net.name),
                        name: ip.clone(),
                        external_id: external_id(
                            arch_id,
                            RESOURCE_SLUG_INSTANCE,
                            &format!("{}/{}#{}", instance.name, net.name, ip),
                            version,
                        ),
                        reason: format!("parent interface name is invalid: {reason}"),
                    });
                }
                continue;
            }

            objects.push(NetBoxObject::Interface(NetBoxInterface {
                name: net.name.clone(),
                virtual_machine: instance.name.clone(),
                description: net.name.clone(),
                tags: tags.clone(),
                custom_fields: ownership_custom_fields(&names, &iface_ext, arch_id, version, arch),
            }));

            if let Some(ip) = &net.ip {
                objects.push(NetBoxObject::IpAddress(NetBoxIpAddress {
                    address: ip.clone(),
                    assigned_to_interface: Some(format!("{}/{}", instance.name, net.name)),
                    tags: tags.clone(),
                    custom_fields: ownership_custom_fields(
                        &names,
                        &external_id(
                            arch_id,
                            RESOURCE_SLUG_INSTANCE,
                            &format!("{}/{}#{}", instance.name, net.name, ip),
                            version,
                        ),
                        arch_id,
                        version,
                        arch,
                    ),
                }));
            }
        }
    }

    // networks[] → prefixes (+ VLANs when vlan_id present). The snapshot
    // supplements missing vlan/cidr facts (design DP3); declared values
    // stay authoritative for the topology shape.
    for net in &arch.networks {
        let snap_net = input
            .snapshot
            .and_then(|s| s.networks.iter().find(|n| n.name == net.name));
        let cidr = net
            .cidr
            .clone()
            .or_else(|| snap_net.and_then(|n| n.cidr.clone()));
        let vlan_id = net.vlan_id.or_else(|| snap_net.and_then(|n| n.vlan_id));

        if let Some(cidr) = cidr {
            let ext = external_id(arch_id, RESOURCE_SLUG_NETWORK, &net.name, version);
            objects.push(NetBoxObject::Prefix(NetBoxPrefix {
                prefix: cidr,
                vlan: vlan_id,
                description: format!("{} ({})", net.name, network_type_str(&net.network_type)),
                network_name: net.name.clone(),
                tags: tags.clone(),
                custom_fields: ownership_custom_fields(&names, &ext, arch_id, version, arch),
            }));
        }

        if let Some(vid) = vlan_id {
            let ext = external_id(
                arch_id,
                RESOURCE_SLUG_NETWORK,
                &format!("{}#vlan", net.name),
                version,
            );
            // NetBox VLAN name/slug is a strict slug (no dots), unlike
            // device/VM/interface names — see `validate_netbox_slug`.
            match validate_netbox_slug(&net.name) {
                Ok(()) => objects.push(NetBoxObject::Vlan(NetBoxVlan {
                    vid,
                    name: net.name.clone(),
                    tags: tags.clone(),
                    custom_fields: ownership_custom_fields(&names, &ext, arch_id, version, arch),
                })),
                Err(reason) => issues.push(MappingIssue {
                    kind: NetBoxKind::Vlan,
                    chv_resource_ref: format!("networks/{}", net.name),
                    name: net.name.clone(),
                    external_id: ext,
                    reason,
                }),
            }
        }
    }

    // Deterministic ordering: kind rank, then name, then natural key.
    objects.sort_by_key(object_sort_key);

    // Secret exclusion (contract rule 5), enforced mechanically.
    for object in &objects {
        scrub_secret_fields(object.custom_fields())?;
    }

    Ok(MappingOutput { objects, issues })
}

fn ownership_custom_fields(
    names: &CustomFieldNames,
    ext_id: &str,
    arch_id: &str,
    version: u64,
    arch: &CHVArchitecture,
) -> BTreeMap<String, String> {
    let marker = ManagedMarker {
        external_id: ext_id.to_string(),
        architecture_id: arch_id.to_string(),
        managed_by: MANAGED_BY_CHV.to_string(),
        managed_state: ManagedState::Active,
        architecture_version: version,
        mapping_version: MAPPING_VERSION.to_string(),
    };
    let mut fields = marker.to_custom_fields(names);
    if let Some(owner) = &arch.metadata.owner {
        fields.insert(names.owner(), owner.clone());
    }
    fields
}

/// Tags derived from `metadata` (mapping contract, object-mapping table):
/// `chv-<label>` slugified plus `chv-env-<environment>`. Sorted and
/// deduplicated for determinism.
fn architecture_tags(arch: &CHVArchitecture) -> Vec<String> {
    let mut tags: Vec<String> = arch
        .metadata
        .labels
        .keys()
        .map(|label| format!("chv-{}", slugify(label)))
        .collect();
    if let Some(environment) = &arch.metadata.environment {
        tags.push(format!("chv-env-{}", slugify(environment)));
    }
    tags.sort();
    tags.dedup();
    tags
}

fn network_type_str(network_type: &NetworkType) -> &'static str {
    match network_type {
        NetworkType::Bridge => "bridge",
        NetworkType::Vlan => "vlan",
        NetworkType::Nat => "nat",
        NetworkType::Isolated => "isolated",
        NetworkType::Routed => "routed",
        NetworkType::Unknown => "unknown",
    }
}

/// NetBox-slug-safe rendering of a free-form value (used for tags and
/// the environment-derived site fallback).
fn slugify(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut pending_dash = false;
    for c in value.chars() {
        match c {
            'a'..='z' | '0'..='9' => {
                if pending_dash && !out.is_empty() {
                    out.push('-');
                }
                pending_dash = false;
                out.push(c);
            }
            'A'..='Z' => {
                if pending_dash && !out.is_empty() {
                    out.push('-');
                }
                pending_dash = false;
                out.push(c.to_ascii_lowercase());
            }
            _ => pending_dash = true,
        }
    }
    out
}

fn object_sort_key(object: &NetBoxObject) -> (u8, String, String) {
    (
        object.kind().rank(),
        object.name().to_string(),
        natural_key_string(&object.natural_key()),
    )
}

fn natural_key_string(natural_key: &BTreeMap<String, String>) -> String {
    natural_key
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(";")
}

// ---------------------------------------------------------------------------
// Tests + shared fixtures
// ---------------------------------------------------------------------------

/// Fixtures shared with `plan.rs` tests.
#[cfg(test)]
pub(crate) mod testsupport {
    use std::collections::BTreeMap;

    use chrono::Utc;
    use chv_architecture_validate::fleet::{DatastoreInfo, InventorySnapshot, NodeInfo};
    use chv_architecture_validate::model::{
        CHVArchitecture, Instance, InstanceNetwork, InstancePlacement, InstanceResources, Metadata,
        Network, NetworkType, Server, ServerResources,
    };

    use crate::mapping::{ProjectionConfigView, ProjectionInput};

    pub(crate) const ARCH_ID: &str = "arch_01HX";

    pub(crate) fn test_architecture() -> CHVArchitecture {
        CHVArchitecture {
            api_version: "chv.kubedo.io/v1alpha1".to_string(),
            kind: "CHVArchitecture".to_string(),
            metadata: Metadata {
                name: "t1".to_string(),
                display_name: None,
                description: None,
                environment: Some("production".to_string()),
                owner: Some("alice".to_string()),
                labels: [("team".to_string(), "platform".to_string())].into(),
            },
            servers: vec![Server {
                name: "chv-node-01".to_string(),
                management_ip: None,
                role: None,
                labels: BTreeMap::new(),
                resources: Some(ServerResources {
                    cpu_cores: Some(4),
                    memory_gb: Some(8),
                }),
                networks: None,
            }],
            networks: vec![
                Network {
                    name: "backend".to_string(),
                    network_type: NetworkType::Vlan,
                    bridge: None,
                    vlan_id: Some(42),
                    cidr: Some("10.42.0.0/24".to_string()),
                    gateway: None,
                    dns: Vec::new(),
                    dhcp: None,
                },
                Network {
                    name: "storage".to_string(),
                    network_type: NetworkType::Bridge,
                    bridge: Some("br-storage".to_string()),
                    vlan_id: None,
                    cidr: Some("10.42.1.0/24".to_string()),
                    gateway: None,
                    dns: Vec::new(),
                    dhcp: None,
                },
            ],
            datastores: Vec::new(),
            backup_targets: Vec::new(),
            backup_policies: Vec::new(),
            images: Vec::new(),
            templates: Vec::new(),
            instances: vec![Instance {
                name: "vm-01".to_string(),
                template: None,
                placement: Some(InstancePlacement {
                    server: Some("chv-node-01".to_string()),
                }),
                resources: Some(InstanceResources {
                    cpu: Some(2),
                    memory_mb: Some(2048),
                }),
                disks: Vec::new(),
                networks: vec![InstanceNetwork {
                    name: "backend".to_string(),
                    ip: Some("10.42.0.5".to_string()),
                }],
                cloud_init: None,
                backup: None,
                tags: Vec::new(),
            }],
            ssh_keys: Vec::new(),
            instance_users: Vec::new(),
            roles: Vec::new(),
            users: Vec::new(),
            projects: Vec::new(),
        }
    }

    /// A second architecture with one extra server (`chv-node-02`), used
    /// to build "remote" state for stale-detection tests.
    pub(crate) fn architecture_with_extra_server() -> CHVArchitecture {
        let mut arch = test_architecture();
        arch.servers.push(Server {
            name: "chv-node-02".to_string(),
            management_ip: None,
            role: None,
            labels: BTreeMap::new(),
            resources: None,
            networks: None,
        });
        arch
    }

    /// An architecture with no projectable resources at all (empty
    /// topology); used for the empty-plan edge case.
    pub(crate) fn empty_architecture() -> CHVArchitecture {
        CHVArchitecture {
            api_version: "chv.kubedo.io/v1alpha1".to_string(),
            kind: "CHVArchitecture".to_string(),
            metadata: Metadata {
                name: "empty".to_string(),
                display_name: None,
                description: None,
                environment: None,
                owner: None,
                labels: BTreeMap::new(),
            },
            servers: Vec::new(),
            networks: Vec::new(),
            datastores: Vec::new(),
            backup_targets: Vec::new(),
            backup_policies: Vec::new(),
            images: Vec::new(),
            templates: Vec::new(),
            instances: Vec::new(),
            ssh_keys: Vec::new(),
            instance_users: Vec::new(),
            roles: Vec::new(),
            users: Vec::new(),
            projects: Vec::new(),
        }
    }

    /// Snapshot whose node facts differ from the declared resources, so
    /// enrichment precedence is observable, plus one datastore reported
    /// for `chv-node-01`.
    pub(crate) fn test_snapshot() -> InventorySnapshot {
        InventorySnapshot {
            captured_at: Utc::now(),
            source: "test".to_string(),
            nodes: vec![NodeInfo {
                name: "chv-node-01".to_string(),
                schedulable: true,
                cpu_cores: 8,
                memory_gb: 16,
                bridges: Vec::new(),
                vlans: Vec::new(),
                used_ips: Vec::new(),
            }],
            networks: Vec::new(),
            datastores: vec![DatastoreInfo {
                name: "ds-pool".to_string(),
                kind: "ceph-rbd".to_string(),
                capacity_gb: Some(1024),
                free_gb: Some(512),
                host: Some("chv-node-01".to_string()),
            }],
            images: Vec::new(),
            backup_targets: Vec::new(),
            backup_targets_complete: true,
            secrets: Vec::new(),
            secrets_complete: true,
            network_facts_complete: true,
            deploy_allowed: true,
        }
    }

    pub(crate) fn projection_input<'a>(
        architecture: &'a CHVArchitecture,
        version: u64,
        snapshot: Option<&'a InventorySnapshot>,
    ) -> ProjectionInput<'a> {
        ProjectionInput {
            architecture,
            architecture_id: ARCH_ID,
            architecture_version: version,
            snapshot,
            config: ProjectionConfigView::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use chv_architecture_validate::model::{
        BackupTarget, Datastore, Instance, InstanceNetwork, InstanceUser, Network, Server, SshKey,
        User,
    };
    use serde_json;

    use super::*;
    use testsupport::{projection_input, test_architecture, test_snapshot, ARCH_ID};

    #[test]
    fn builds_all_six_kinds_with_ownership_fields() {
        let arch = test_architecture();
        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");

        assert!(output.issues.is_empty());
        let kinds: Vec<&str> = output.objects.iter().map(|o| o.kind().as_str()).collect();
        for expected in [
            "vlan",
            "prefix",
            "ip_address",
            "interface",
            "virtual_machine",
            "device",
        ] {
            assert!(
                kinds.contains(&expected),
                "missing kind {expected} in {kinds:?}"
            );
        }

        // Every object carries the full ownership marker.
        for object in &output.objects {
            let fields = object.custom_fields();
            for key in [
                "chv_external_id",
                "chv_architecture_id",
                "chv_managed_by",
                "chv_managed_state",
                "chv_architecture_version",
                "chv_mapping_version",
            ] {
                assert!(fields.contains_key(key), "{key} missing on {object:?}");
            }
            assert_eq!(
                fields.get("chv_managed_by").map(String::as_str),
                Some("chv")
            );
            assert_eq!(
                fields.get("chv_architecture_id").map(String::as_str),
                Some(ARCH_ID)
            );
            assert_eq!(
                fields.get("chv_architecture_version").map(String::as_str),
                Some("3")
            );
            assert_eq!(
                fields.get("chv_mapping_version").map(String::as_str),
                Some("v1")
            );
            // Metadata: tags from labels + environment, owner as custom field.
            assert!(object_tags(object).contains(&"chv-team".to_string()));
            assert!(object_tags(object).contains(&"chv-env-production".to_string()));
            assert_eq!(
                fields.get("chv_owner").map(String::as_str),
                Some("alice"),
                "owner custom field on {:?}",
                object.kind()
            );
        }
    }

    fn object_tags(object: &NetBoxObject) -> &[String] {
        match object {
            NetBoxObject::Device(d) => &d.tags,
            NetBoxObject::VirtualMachine(v) => &v.tags,
            NetBoxObject::Interface(i) => &i.tags,
            NetBoxObject::Prefix(p) => &p.tags,
            NetBoxObject::Vlan(v) => &v.tags,
            NetBoxObject::IpAddress(a) => &a.tags,
        }
    }

    /// The provisioning-surface tripwire, derived from the mapping's
    /// actual output: every custom-field name `build_objects` writes
    /// (for the maximal fixture — owner set, live snapshot facts, a
    /// datastore) must be enumerated by
    /// `CustomFieldNames::ownership_fields` + `enrichment_fields`.
    /// A new mapping write that is not in those lists would be
    /// silently unprovisioned against a real NetBox (400 on every
    /// write carrying it) — this test fails first, in PR CI.
    #[test]
    fn mapping_custom_fields_stay_within_the_provisioning_surface() {
        let mut arch = test_architecture();
        arch.datastores = vec![Datastore {
            name: "ds-local".to_string(),
            datastore_type: chv_architecture_validate::model::DatastoreType::Qcow2Dir,
            path: Some("/var/lib/chv".to_string()),
            pool: None,
            capabilities: None,
            secret_ref: None,
        }];
        let snapshot = test_snapshot();
        let output = build_objects(&projection_input(&arch, 3, Some(&snapshot))).expect("builds");
        assert!(output.objects.len() >= 6);

        let mut written: std::collections::BTreeSet<String> = Default::default();
        for object in &output.objects {
            written.extend(object.custom_fields().keys().cloned());
        }
        assert!(
            written.contains("chv_datastores"),
            "fixture must exercise the datastore enrichment field"
        );

        let names = crate::ownership::CustomFieldNames::default();
        let surface: std::collections::BTreeSet<String> = names
            .ownership_fields()
            .iter()
            .map(|field| field.to_string())
            .chain(names.enrichment_fields())
            .collect();
        assert_eq!(
            written, surface,
            "the mapping's custom-field writes and the provisioning surface diverged"
        );
    }

    #[test]
    fn external_id_on_device_matches_contract_format() {
        let arch = test_architecture();
        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");
        let device = output
            .objects
            .iter()
            .find(|o| o.kind() == NetBoxKind::Device)
            .expect("device");
        assert_eq!(
            device.custom_fields().get("chv_external_id"),
            Some(&"arch:arch_01HX:server/chv-node-01:3".to_string())
        );
    }

    #[test]
    fn identity_and_config_kinds_are_never_mapped() {
        let mut arch = test_architecture();
        arch.users.push(User {
            name: "alice".to_string(),
            display_name: None,
            email: None,
            auth: None,
            password: None,
            token: None,
            roles: Vec::new(),
        });
        arch.roles.push(chv_architecture_validate::model::Role {
            name: "ops".to_string(),
            permissions: Vec::new(),
        });
        arch.ssh_keys.push(SshKey {
            name: "key-1".to_string(),
            public_key: "ssh-ed25519 AAAA".to_string(),
        });
        arch.instance_users.push(InstanceUser {
            name: "root".to_string(),
            sudo: true,
            shell: None,
            password: None,
            ssh_authorized_keys: Vec::new(),
        });
        arch.backup_targets.push(BackupTarget {
            name: "offsite".to_string(),
            target_type: "s3".to_string(),
            endpoint: None,
            datastore: None,
            user: None,
            secret_ref: None,
        });
        arch.datastores.push(Datastore {
            name: "ds-1".to_string(),
            datastore_type: chv_architecture_validate::model::DatastoreType::Nfs,
            path: None,
            pool: None,
            capabilities: None,
            secret_ref: None,
        });

        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");
        // Only topology kinds appear; none of the excluded resource names
        // shows up as an object name.
        for object in &output.objects {
            let name = object.name().to_string();
            assert!(
                !["alice", "ops", "key-1", "root", "offsite", "ds-1"].contains(&name.as_str()),
                "excluded kind leaked as object: {name}"
            );
        }
        let names: Vec<String> = output
            .objects
            .iter()
            .map(|o| o.name().to_string())
            .collect();
        assert!(names.iter().all(|n| !n.starts_with("ssh-ed25519")));
    }

    #[test]
    fn secret_values_never_reach_mapped_objects() {
        let mut arch = test_architecture();
        arch.users.push(User {
            name: "eve".to_string(),
            display_name: None,
            email: None,
            auth: Some(chv_architecture_validate::model::UserAuth {
                auth_type: Some("oidc".to_string()),
                subject: Some("SECRET-AUTH-SUBJECT".to_string()),
            }),
            password: Some("SECRET-USER-PASSWORD".to_string()),
            token: Some("SECRET-USER-TOKEN".to_string()),
            roles: Vec::new(),
        });
        arch.ssh_keys.push(SshKey {
            name: "k".to_string(),
            public_key: "SECRET-PUBLIC-KEY-MATERIAL".to_string(),
        });
        arch.instance_users.push(InstanceUser {
            name: "root".to_string(),
            sudo: false,
            shell: None,
            password: Some("SECRET-INSTANCE-PASSWORD".to_string()),
            ssh_authorized_keys: Vec::new(),
        });
        arch.datastores.push(Datastore {
            name: "ds".to_string(),
            datastore_type: chv_architecture_validate::model::DatastoreType::Nfs,
            path: None,
            pool: None,
            capabilities: None,
            secret_ref: Some("SECRET-DATASTORE-REF".to_string()),
        });
        arch.backup_targets.push(BackupTarget {
            name: "off".to_string(),
            target_type: "s3".to_string(),
            endpoint: None,
            datastore: None,
            user: None,
            secret_ref: Some("SECRET-BACKUP-REF".to_string()),
        });

        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");
        let serialized = serde_json::to_string(&output).expect("serializable");
        for secret in [
            "SECRET-AUTH-SUBJECT",
            "SECRET-USER-PASSWORD",
            "SECRET-USER-TOKEN",
            "SECRET-PUBLIC-KEY-MATERIAL",
            "SECRET-INSTANCE-PASSWORD",
            "SECRET-DATASTORE-REF",
            "SECRET-BACKUP-REF",
        ] {
            assert!(
                !serialized.contains(secret),
                "secret material {secret} reached the projection output"
            );
        }
    }

    #[test]
    fn secret_named_custom_field_fails_closed() {
        // Direct scrubber check: a secret-carrier field name aborts the
        // build instead of being silently scrubbed away.
        let mut fields = BTreeMap::new();
        fields.insert("chv_password".to_string(), "x".to_string());
        let err = scrub_secret_fields(&fields).expect_err("secret name must fail closed");
        assert!(matches!(err, MappingError::SecretMaterial { .. }));

        // Contract-named fields (including the "auth"-free architecture
        // fields) pass the scrubber.
        let names = CustomFieldNames::default();
        let mut ok = ManagedMarker {
            external_id: "arch:a:server/n:1".to_string(),
            architecture_id: "a".to_string(),
            managed_by: "chv".to_string(),
            managed_state: ManagedState::Active,
            architecture_version: 1,
            mapping_version: MAPPING_VERSION.to_string(),
        }
        .to_custom_fields(&names);
        ok.insert(names.owner(), "alice".to_string());
        ok.insert(names.datastores(), "ds:nfs".to_string());
        ok.insert(names.cpu_cores(), "8".to_string());
        ok.insert(names.memory_gb(), "16".to_string());
        assert!(scrub_secret_fields(&ok).is_ok());
    }

    #[test]
    fn snapshot_facts_override_declared_server_resources() {
        let arch = test_architecture();
        // Declared: 4 cores / 8 GB. Live: 8 cores / 16 GB. Live wins.
        let with_snapshot =
            build_objects(&projection_input(&arch, 3, Some(&test_snapshot()))).expect("builds");
        let device = with_snapshot
            .objects
            .iter()
            .find_map(|o| match o {
                NetBoxObject::Device(d) => Some(d),
                _ => None,
            })
            .expect("device");
        // CPU/memory are custom fields (mapping contract), with live
        // facts winning over declared (contract rule 4).
        assert_eq!(
            device.custom_fields.get("chv_cpu_cores"),
            Some(&"8".to_string())
        );
        assert_eq!(
            device.custom_fields.get("chv_memory_gb"),
            Some(&"16".to_string())
        );
        // Datastore facts ride along as device custom-field enrichment.
        assert_eq!(
            device.custom_fields.get("chv_datastores"),
            Some(&"ds-pool:ceph-rbd".to_string())
        );

        // Without a snapshot the declared resources project.
        let declared = build_objects(&projection_input(&arch, 3, None)).expect("builds");
        let device = declared
            .objects
            .iter()
            .find_map(|o| match o {
                NetBoxObject::Device(d) => Some(d),
                _ => None,
            })
            .expect("device");
        assert_eq!(
            device.custom_fields.get("chv_cpu_cores"),
            Some(&"4".to_string())
        );
        assert_eq!(
            device.custom_fields.get("chv_memory_gb"),
            Some(&"8".to_string())
        );
        assert!(!device.custom_fields.contains_key("chv_datastores"));
    }

    #[test]
    fn missing_facts_leave_cpu_memory_custom_fields_unset() {
        // Contract rule 2: nullable facts stay unset — no placeholder
        // values, no zero-valued custom fields.
        let mut arch = test_architecture();
        arch.servers[0].resources = None;
        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");
        let device = output
            .objects
            .iter()
            .find_map(|o| match o {
                NetBoxObject::Device(d) => Some(d),
                _ => None,
            })
            .expect("device");
        assert!(!device.custom_fields.contains_key("chv_cpu_cores"));
        assert!(!device.custom_fields.contains_key("chv_memory_gb"));
    }

    #[test]
    fn declared_resources_win_for_virtual_machines() {
        let arch = test_architecture();
        // The snapshot reports a *node* with the VM's name; VM facts are
        // not taken from it (design G2) — declared wins.
        let mut snapshot = test_snapshot();
        snapshot
            .nodes
            .push(chv_architecture_validate::fleet::NodeInfo {
                name: "vm-01".to_string(),
                schedulable: true,
                cpu_cores: 64,
                memory_gb: 256,
                bridges: Vec::new(),
                vlans: Vec::new(),
                used_ips: Vec::new(),
            });
        let output = build_objects(&projection_input(&arch, 3, Some(&snapshot))).expect("builds");
        let vm = output
            .objects
            .iter()
            .find_map(|o| match o {
                NetBoxObject::VirtualMachine(v) => Some(v),
                _ => None,
            })
            .expect("vm");
        assert_eq!(vm.cpu, Some(2));
        assert_eq!(vm.memory_mb, Some(2048));
        assert_eq!(vm.device.as_deref(), Some("chv-node-01"));
    }

    #[test]
    fn vlan_emitted_only_when_vlan_id_present_and_links_prefix() {
        let arch = test_architecture();
        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");

        let vlan = output
            .objects
            .iter()
            .find_map(|o| match o {
                NetBoxObject::Vlan(v) => Some(v),
                _ => None,
            })
            .expect("vlan for the vlan-typed network");
        assert_eq!(vlan.vid, 42);
        assert_eq!(vlan.name, "backend");

        // Exactly one vlan — the bridge network has no vlan_id.
        assert_eq!(
            output
                .objects
                .iter()
                .filter(|o| o.kind() == NetBoxKind::Vlan)
                .count(),
            1
        );

        // Prefix linkage: backend's prefix carries vlan 42, storage's
        // does not.
        let prefixes: Vec<&NetBoxPrefix> = output
            .objects
            .iter()
            .filter_map(|o| match o {
                NetBoxObject::Prefix(p) => Some(p),
                _ => None,
            })
            .collect();
        assert_eq!(prefixes.len(), 2);
        let backend = prefixes
            .iter()
            .find(|p| p.prefix == "10.42.0.0/24")
            .expect("backend prefix");
        assert_eq!(backend.vlan, Some(42));
        let storage = prefixes
            .iter()
            .find(|p| p.prefix == "10.42.1.0/24")
            .expect("storage prefix");
        assert_eq!(storage.vlan, None);
    }

    #[test]
    fn invalid_names_become_issues_not_objects() {
        let mut arch = test_architecture();
        arch.servers.push(Server {
            name: "Bad Node!".to_string(),
            management_ip: None,
            role: None,
            labels: BTreeMap::new(),
            resources: None,
            networks: None,
        });
        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");

        let issue = output
            .issues
            .iter()
            .find(|i| i.name == "Bad Node!")
            .expect("issue reported for the invalid name");
        assert_eq!(issue.kind, NetBoxKind::Device);
        assert_eq!(issue.chv_resource_ref, "servers/Bad Node!");
        assert!(issue.reason.contains("not allowed"));
        assert_eq!(issue.external_id, "arch:arch_01HX:server/Bad Node!:3");

        // Verbatim: no object was created under a renamed key.
        assert!(!output.objects.iter().any(|o| o.name().contains("Bad Node")));
    }

    #[test]
    fn invalid_vm_name_reports_whole_subtree() {
        let mut arch = test_architecture();
        arch.instances.push(Instance {
            name: "VM 2".to_string(),
            template: None,
            placement: None,
            resources: None,
            disks: Vec::new(),
            networks: vec![InstanceNetwork {
                name: "backend".to_string(),
                ip: Some("10.42.0.9".to_string()),
            }],
            cloud_init: None,
            backup: None,
            tags: Vec::new(),
        });
        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");
        // VM + interface + ip all reported; nothing silently dropped.
        assert_eq!(output.issues.len(), 3);
        assert!(output
            .issues
            .iter()
            .any(|i| i.kind == NetBoxKind::VirtualMachine && i.name == "VM 2"));
        assert!(output.issues.iter().any(|i| i.kind == NetBoxKind::Interface
            && i.chv_resource_ref == "instances/VM 2/networks/backend"));
        assert!(output
            .issues
            .iter()
            .any(|i| i.kind == NetBoxKind::IpAddress && i.name == "10.42.0.9"));
    }

    #[test]
    fn objects_ordered_by_kind_rank_then_name() {
        let mut arch = test_architecture();
        arch.servers.push(Server {
            name: "chv-node-00".to_string(),
            management_ip: None,
            role: None,
            labels: BTreeMap::new(),
            resources: None,
            networks: None,
        });
        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");
        let ranks: Vec<u8> = output.objects.iter().map(|o| o.kind().rank()).collect();
        let mut sorted = ranks.clone();
        sorted.sort();
        assert_eq!(ranks, sorted, "kind ranks must be non-decreasing");

        // Within the device rank, names are alphabetical.
        let device_names: Vec<&str> = output
            .objects
            .iter()
            .filter(|o| o.kind() == NetBoxKind::Device)
            .map(NetBoxObject::name)
            .collect();
        assert_eq!(device_names, vec!["chv-node-00", "chv-node-01"]);
    }

    #[test]
    fn configurable_prefix_flows_into_built_objects() {
        let arch = test_architecture();
        let input = ProjectionInput {
            architecture: &arch,
            architecture_id: ARCH_ID,
            architecture_version: 3,
            snapshot: None,
            config: ProjectionConfigView {
                custom_field_prefix: "acme_".to_string(),
                site_name: None,
            },
        };
        let output = build_objects(&input).expect("builds");
        let device = output
            .objects
            .iter()
            .find(|o| o.kind() == NetBoxKind::Device)
            .expect("device");
        let fields = device.custom_fields();
        assert_eq!(
            fields.get("acme_external_id"),
            Some(&"arch:arch_01HX:server/chv-node-01:3".to_string())
        );
        assert!(fields.contains_key("acme_managed_by"));
        assert!(!fields.keys().any(|k| k.starts_with("chv_")));
        // The marker parses under the configured prefix.
        let names = CustomFieldNames::new("acme_");
        let marker = ManagedMarker::parse(fields, &names).expect("marker parses");
        assert!(marker.is_owned_by_chv());
    }

    #[test]
    fn site_falls_back_to_environment_and_config_wins() {
        let arch = test_architecture();
        let with_config = ProjectionInput {
            architecture: &arch,
            architecture_id: ARCH_ID,
            architecture_version: 3,
            snapshot: None,
            config: ProjectionConfigView {
                custom_field_prefix: "chv_".to_string(),
                site_name: Some("dc-berlin".to_string()),
            },
        };
        let output = build_objects(&with_config).expect("builds");
        let device = output
            .objects
            .iter()
            .find_map(|o| match o {
                NetBoxObject::Device(d) => Some(d),
                _ => None,
            })
            .expect("device");
        assert_eq!(device.site.as_deref(), Some("dc-berlin"));

        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");
        let device = output
            .objects
            .iter()
            .find_map(|o| match o {
                NetBoxObject::Device(d) => Some(d),
                _ => None,
            })
            .expect("device");
        assert_eq!(device.site.as_deref(), Some("production"));
    }

    #[test]
    fn empty_architecture_id_is_rejected() {
        let arch = test_architecture();
        let input = ProjectionInput {
            architecture: &arch,
            architecture_id: "",
            architecture_version: 3,
            snapshot: None,
            config: ProjectionConfigView::default(),
        };
        assert!(matches!(
            build_objects(&input),
            Err(MappingError::InvalidInput { .. })
        ));
    }

    #[test]
    fn snapshot_supplements_missing_network_facts() {
        let mut arch = test_architecture();
        // Declared network without cidr/vlan; the snapshot knows both.
        arch.networks.push(Network {
            name: "live-only".to_string(),
            network_type: NetworkType::Vlan,
            bridge: None,
            vlan_id: None,
            cidr: None,
            gateway: None,
            dns: Vec::new(),
            dhcp: None,
        });
        let mut snapshot = test_snapshot();
        snapshot
            .networks
            .push(chv_architecture_validate::fleet::NetworkInfo {
                name: "live-only".to_string(),
                bridge: None,
                vlan_id: Some(7),
                cidr: Some("10.42.7.0/24".to_string()),
            });

        let output = build_objects(&projection_input(&arch, 3, Some(&snapshot))).expect("builds");
        let prefix = output
            .objects
            .iter()
            .find_map(|o| match o {
                NetBoxObject::Prefix(p) if p.prefix == "10.42.7.0/24" => Some(p),
                _ => None,
            })
            .expect("prefix from live fact");
        assert_eq!(prefix.vlan, Some(7));
        assert!(output
            .objects
            .iter()
            .any(|o| matches!(o, NetBoxObject::Vlan(v) if v.vid == 7)));

        // Without the snapshot, nothing is fabricated (rule 2).
        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");
        assert!(!output.objects.iter().any(|o| o.name() == "live-only"
            || matches!(&o, NetBoxObject::Prefix(p) if p.prefix == "10.42.7.0/24")));
    }

    #[test]
    fn deterministic_output_for_identical_inputs() {
        let arch = test_architecture();
        let a = build_objects(&projection_input(&arch, 3, None)).expect("builds");
        let b = build_objects(&projection_input(&arch, 3, None)).expect("builds");
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn invalid_custom_field_prefixes_are_rejected() {
        let arch = test_architecture();
        for prefix in [
            "",
            "token_",
            "auth_",
            "MY_PASSWORD_",
            "ssh_key_",
            "secret_",
            "key_",
            "credential_",
        ] {
            let input = ProjectionInput {
                architecture: &arch,
                architecture_id: ARCH_ID,
                architecture_version: 3,
                snapshot: None,
                config: ProjectionConfigView {
                    custom_field_prefix: prefix.to_string(),
                    site_name: None,
                },
            };
            assert!(
                matches!(
                    build_objects(&input),
                    Err(MappingError::InvalidCustomFieldPrefix { .. })
                ),
                "prefix {prefix:?} must be rejected"
            );
        }
    }

    #[test]
    fn vlan_name_with_dot_is_an_issue_and_a_plan_conflict() {
        // NetBox VLAN name/slug is a strict slug: `backend.v2` is legal
        // for a prefix-bearing network but must not pass VLAN name
        // validation — it becomes a mapping issue (conflict path),
        // never a silent rename.
        let mut arch = test_architecture();
        arch.networks[0].name = "backend.v2".to_string();
        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");

        let issue = output
            .issues
            .iter()
            .find(|i| i.kind == NetBoxKind::Vlan)
            .expect("vlan issue for the dotted name");
        assert_eq!(issue.name, "backend.v2");
        assert!(issue.reason.contains("not allowed"));
        assert!(!output.objects.iter().any(|o| o.kind() == NetBoxKind::Vlan));

        // The prefix still projects (dots are legal outside VLAN
        // names), carrying the source name for the resource ref.
        assert!(output.objects.iter().any(|o| matches!(
            &o,
            NetBoxObject::Prefix(p) if p.network_name == "backend.v2"
        )));

        // Conflict path: the issue becomes a conflict plan entry, never
        // a write.
        let plan =
            crate::plan::compute_plan(&output, &[], &crate::plan::PlanContext::new(ARCH_ID, 3))
                .expect("plan");
        let conflict = plan
            .entries
            .iter()
            .find(|e| {
                e.kind == NetBoxKind::Vlan && e.action == crate::plan::NetboxPlanAction::Conflict
            })
            .expect("conflict entry for the vlan name");
        assert_eq!(
            conflict.netbox_natural_key.get("name").map(String::as_str),
            Some("backend.v2")
        );
        assert!(conflict
            .reason
            .contains("name fails NetBox charset validation"));
    }

    #[test]
    fn vlan_id_without_cidr_emits_vlan_but_no_prefix() {
        let mut arch = test_architecture();
        arch.networks.push(Network {
            name: "vlan-only".to_string(),
            network_type: NetworkType::Vlan,
            bridge: None,
            vlan_id: Some(99),
            cidr: None,
            gateway: None,
            dns: Vec::new(),
            dhcp: None,
        });
        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");
        assert!(output
            .objects
            .iter()
            .any(|o| matches!(&o, NetBoxObject::Vlan(v) if v.vid == 99)));
        assert!(!output
            .objects
            .iter()
            .any(|o| matches!(&o, NetBoxObject::Prefix(p) if p.network_name == "vlan-only")));
        assert!(output.issues.is_empty());
    }

    #[test]
    fn version_zero_external_ids_format_correctly() {
        assert_eq!(
            external_id("arch_01HX", "server", "chv-node-01", 0),
            "arch:arch_01HX:server/chv-node-01:0"
        );
        let arch = test_architecture();
        let output = build_objects(&projection_input(&arch, 0, None)).expect("builds");
        let device = output
            .objects
            .iter()
            .find(|o| o.kind() == NetBoxKind::Device)
            .expect("device");
        assert_eq!(
            device.custom_fields().get("chv_external_id"),
            Some(&"arch:arch_01HX:server/chv-node-01:0".to_string())
        );
        assert_eq!(
            device.custom_fields().get("chv_architecture_version"),
            Some(&"0".to_string())
        );
        // Version 0 still parses and round-trips through the marker.
        let marker = ManagedMarker::parse(device.custom_fields(), &CustomFieldNames::default())
            .expect("marker parses");
        assert_eq!(marker.architecture_version, 0);
    }

    #[test]
    fn external_id_resource_slugs_are_pinned() {
        // Cross-crate contract: the external-id kind slugs must be
        // exactly this set, matching `resource_type_as_str` in
        // chv-architecture-reconcile/src/apply/mod.rs. A divergence
        // breaks external-id ↔ resource-ref joins and must be a
        // deliberate, reviewed change in both crates.
        assert_eq!(RESOURCE_SLUG_SERVER, "server");
        assert_eq!(RESOURCE_SLUG_NETWORK, "network");
        assert_eq!(RESOURCE_SLUG_INSTANCE, "instance");

        // And the builders only ever use these three slugs.
        let arch = test_architecture();
        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");
        let names = CustomFieldNames::default();
        for object in &output.objects {
            let ext_id = object
                .custom_fields()
                .get(&names.external_id)
                .expect("external id");
            let kind = ext_id
                .split(':')
                .nth(2)
                .and_then(|segment| segment.split('/').next())
                .expect("kind slug");
            assert!(
                [
                    RESOURCE_SLUG_SERVER,
                    RESOURCE_SLUG_NETWORK,
                    RESOURCE_SLUG_INSTANCE
                ]
                .contains(&kind),
                "unexpected resource slug {kind:?} in {ext_id:?}"
            );
        }
    }
}
