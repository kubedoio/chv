use serde::Deserialize;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct VmSpec {
    pub cpu_count: Option<i32>,
    pub memory_bytes: Option<i64>,
    pub image_ref: Option<String>,
    pub boot_mode: Option<String>,
    pub desired_power_state: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct VolumeSpec {
    pub capacity_bytes: i64,
    pub volume_kind: Option<String>,
    pub storage_class: Option<String>,
    pub attached_vm_id: Option<String>,
    pub attachment_mode: Option<String>,
    pub device_name: Option<String>,
    pub read_only: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct NetworkSpec {
    pub network_class: Option<String>,
    pub exposures: Option<Vec<NetworkExposureSpec>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
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
    fn network_spec_parses_with_exposures() {
        let json = r#"{"network_class": "bridge", "exposures": [{"service_name": "web", "protocol": "tcp"}]}"#;
        let spec: NetworkSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.network_class.as_deref(), Some("bridge"));
        assert_eq!(spec.exposures.as_ref().unwrap()[0].service_name, "web");
    }

    #[test]
    fn node_spec_parses() {
        let json = r#"{"desired_state": "TenantReady", "state_reason": "initial bootstrap"}"#;
        let spec: NodeSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.desired_state, "TenantReady");
        assert_eq!(spec.state_reason.as_deref(), Some("initial bootstrap"));
    }
}
