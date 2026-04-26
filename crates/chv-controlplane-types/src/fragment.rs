use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FirewallRuleSpec {
    pub direction: String,
    pub protocol: String,
    pub source_cidr: Option<String>,
    pub dest_port: Option<String>,
    pub action: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NatRuleSpec {
    pub source_cidr: String,
    pub dest_cidr: Option<String>,
    pub masquerade: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DhcpScopeSpec {
    pub range_start: String,
    pub range_end: String,
    pub dns_servers: Option<Vec<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DnsScopeSpec {
    pub forwarders: Option<Vec<String>>,
    pub static_records: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VmSpec {
    pub cpu_count: Option<i32>,
    pub memory_bytes: Option<i64>,
    pub image_ref: Option<String>,
    pub boot_mode: Option<String>,
    pub desired_power_state: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeSpec {
    pub capacity_bytes: i64,
    pub volume_kind: Option<String>,
    pub storage_class: Option<String>,
    pub attached_vm_id: Option<String>,
    pub attachment_mode: Option<String>,
    pub device_name: Option<String>,
    #[serde(default)]
    pub read_only: bool,
    pub snapshot_op: Option<String>,
    pub snapshot_name: Option<String>,
    pub clone_source_volume_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkSpec {
    pub network_class: Option<String>,
    pub exposures: Option<Vec<NetworkExposureSpec>>,
    #[serde(default)]
    pub cidr: Option<String>,
    #[serde(default)]
    pub gateway: Option<String>,
    #[serde(default)]
    pub nat_enabled: Option<bool>,
    #[serde(default)]
    pub dhcp_enabled: Option<bool>,
    #[serde(default)]
    pub ipam_mode: Option<String>,
    #[serde(default)]
    pub firewall_rules: Option<Vec<FirewallRuleSpec>>,
    #[serde(default)]
    pub nat_rules: Option<Vec<NatRuleSpec>>,
    #[serde(default)]
    pub dhcp_scope: Option<DhcpScopeSpec>,
    #[serde(default)]
    pub dns_enabled: Option<bool>,
    #[serde(default)]
    pub dns_scope: Option<DnsScopeSpec>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkExposureSpec {
    pub service_name: String,
    pub protocol: String,
    pub listen_address: Option<String>,
    pub listen_port: Option<i32>,
    pub target_address: Option<String>,
    pub target_port: Option<i32>,
    pub exposure_policy: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeSpec {
    pub desired_state: String,
    pub state_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_spec_parses_minimal() {
        let json = r#"{"cpu_count": 2, "memory_bytes": 4294967296}"#;
        let spec: VmSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.cpu_count, Some(2));
        assert_eq!(spec.memory_bytes, Some(4294967296));
    }

    #[test]
    fn vm_spec_rejects_unknown_field() {
        let json = r#"{"cpu_count": 2, "unknown": true}"#;
        assert!(serde_json::from_str::<VmSpec>(json).is_err());
    }

    #[test]
    fn volume_spec_parses_and_defaults_read_only() {
        let json = r#"{"capacity_bytes": 10737418240}"#;
        let spec: VolumeSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.capacity_bytes, 10737418240);
        assert!(!spec.read_only);
    }

    #[test]
    fn volume_spec_rejects_missing_required_field() {
        let json = r#"{"volume_kind": "ssd"}"#;
        assert!(serde_json::from_str::<VolumeSpec>(json).is_err());
    }

    #[test]
    fn network_spec_parses_with_exposures() {
        let json = r#"{"network_class": "bridge", "exposures": [{"service_name": "web", "protocol": "tcp"}]}"#;
        let spec: NetworkSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.network_class.as_deref(), Some("bridge"));
        assert_eq!(spec.exposures.as_ref().unwrap()[0].service_name, "web");
    }

    #[test]
    fn network_exposure_spec_rejects_unknown_field() {
        let json = r#"{"service_name": "web", "protocol": "tcp", "extra": 1}"#;
        assert!(serde_json::from_str::<NetworkExposureSpec>(json).is_err());
    }

    #[test]
    fn node_spec_parses() {
        let json = r#"{"desired_state": "TenantReady", "state_reason": "initial bootstrap"}"#;
        let spec: NodeSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.desired_state, "TenantReady");
        assert_eq!(spec.state_reason.as_deref(), Some("initial bootstrap"));
    }

    #[test]
    fn node_spec_rejects_missing_required_field() {
        let json = r#"{"state_reason": "reason"}"#;
        assert!(serde_json::from_str::<NodeSpec>(json).is_err());
    }
}
