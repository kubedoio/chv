//! The six NetBox object kinds the simulator serves.

use std::fmt;

use serde::{Deserialize, Serialize};

/// One of the six NetBox object kinds the adapter client touches
/// (mapping contract "NetBox REST surface used (v1)").
///
/// Declaration order is the contract's kind rank
/// (`vlan → prefix → device → vm → interface → ip`), so
/// [`SimKind::ALL`] iterates parents before children — the natural
/// order for seeding (and the FK-dependency order the client's
/// create path needs against a real NetBox, which resolves nested
/// references by existence).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimKind {
    Vlan,
    Prefix,
    Device,
    VirtualMachine,
    Interface,
    IpAddress,
}

impl SimKind {
    /// All kinds, in contract rank order.
    pub const ALL: [SimKind; 6] = [
        SimKind::Vlan,
        SimKind::Prefix,
        SimKind::Device,
        SimKind::VirtualMachine,
        SimKind::Interface,
        SimKind::IpAddress,
    ];

    /// The contract's kind string (also the `/__faults` scope key and
    /// the plan-entry `kind` value).
    pub const fn as_str(self) -> &'static str {
        match self {
            SimKind::Vlan => "vlan",
            SimKind::Prefix => "prefix",
            SimKind::IpAddress => "ip_address",
            SimKind::Interface => "interface",
            SimKind::VirtualMachine => "virtual_machine",
            SimKind::Device => "device",
        }
    }

    /// REST path of the kind's list endpoint — exactly the paths the
    /// adapter client's `kind_api_path` produces.
    pub const fn api_path(self) -> &'static str {
        match self {
            SimKind::Vlan => "/api/ipam/vlans/",
            SimKind::Prefix => "/api/ipam/prefixes/",
            SimKind::IpAddress => "/api/ipam/ip-addresses/",
            SimKind::Interface => "/api/virtualization/interfaces/",
            SimKind::VirtualMachine => "/api/virtualization/virtual-machines/",
            SimKind::Device => "/api/dcim/devices/",
        }
    }

    /// Collection key in `/__seed`, `/__state`, and the seed-file
    /// format used by the `netbox-sim` binary.
    pub const fn collection(self) -> &'static str {
        match self {
            SimKind::Vlan => "vlans",
            SimKind::Prefix => "prefixes",
            SimKind::IpAddress => "ip_addresses",
            SimKind::Interface => "interfaces",
            SimKind::VirtualMachine => "virtual_machines",
            SimKind::Device => "devices",
        }
    }

    /// Resolve `(app, resource)` path segments to a kind; `None` for
    /// anything outside the six families (those get a 404, like a
    /// real NetBox).
    pub fn from_api_segments(app: &str, resource: &str) -> Option<Self> {
        match (app, resource) {
            ("dcim", "devices") => Some(SimKind::Device),
            ("virtualization", "virtual-machines") => Some(SimKind::VirtualMachine),
            ("virtualization", "interfaces") => Some(SimKind::Interface),
            ("ipam", "prefixes") => Some(SimKind::Prefix),
            ("ipam", "vlans") => Some(SimKind::Vlan),
            ("ipam", "ip-addresses") => Some(SimKind::IpAddress),
            _ => None,
        }
    }

    /// The request field a natural-key uniqueness violation is
    /// reported under, mirroring NetBox's
    /// `{"<field>": ["This field must be unique."]}` error shape.
    pub const fn unique_error_field(self) -> &'static str {
        match self {
            SimKind::Vlan => "vid",
            SimKind::Prefix => "prefix",
            SimKind::IpAddress => "address",
            SimKind::Interface | SimKind::VirtualMachine | SimKind::Device => "name",
        }
    }
}

impl fmt::Display for SimKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_paths_match_the_adapter_client() {
        // Extracted from chv-netbox-adapter/src/client.rs
        // (`kind_api_path`): the sim must serve exactly these paths.
        assert_eq!(SimKind::Device.api_path(), "/api/dcim/devices/");
        assert_eq!(
            SimKind::VirtualMachine.api_path(),
            "/api/virtualization/virtual-machines/"
        );
        assert_eq!(
            SimKind::Interface.api_path(),
            "/api/virtualization/interfaces/"
        );
        assert_eq!(SimKind::Prefix.api_path(), "/api/ipam/prefixes/");
        assert_eq!(SimKind::Vlan.api_path(), "/api/ipam/vlans/");
        assert_eq!(SimKind::IpAddress.api_path(), "/api/ipam/ip-addresses/");
    }

    #[test]
    fn segments_resolve_only_the_six_families() {
        assert_eq!(
            SimKind::from_api_segments("dcim", "devices"),
            Some(SimKind::Device)
        );
        assert_eq!(SimKind::from_api_segments("dcim", "sites"), None);
        assert_eq!(SimKind::from_api_segments("extras", "objects"), None);
        assert_eq!(SimKind::from_api_segments("api", "status"), None);
    }

    #[test]
    fn kind_strings_match_the_contract() {
        assert_eq!(SimKind::Vlan.as_str(), "vlan");
        assert_eq!(SimKind::IpAddress.as_str(), "ip_address");
        assert_eq!(SimKind::VirtualMachine.as_str(), "virtual_machine");
        assert_eq!(SimKind::IpAddress.collection(), "ip_addresses");
    }
}
