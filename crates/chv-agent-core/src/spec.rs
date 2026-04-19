use chv_errors::ChvError;
use serde::Deserialize;

fn default_running() -> String {
    "Running".to_string()
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct VmSpec {
    pub name: String,
    pub cpus: u32,
    pub memory_bytes: u64,
    pub kernel_path: String,
    #[serde(default)]
    pub firmware_path: Option<String>,
    #[serde(default)]
    pub disk_seed_path: Option<String>,
    pub disks: Vec<DiskSpec>,
    pub nics: Vec<NicSpec>,
    #[serde(default = "default_running")]
    pub desired_state: String,
    #[serde(default)]
    pub cloud_init_userdata: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct DiskSpec {
    pub volume_id: String,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct NicSpec {
    pub network_id: String,
    pub mac_address: String,
    pub ip_address: String,
    #[serde(default)]
    pub tap_name: String,
    #[serde(default)]
    pub cidr: String,
    #[serde(default)]
    pub gateway: String,
}

impl VmSpec {
    pub fn from_json(raw: &str) -> Result<VmSpec, ChvError> {
        serde_json::from_str(raw).map_err(|e| ChvError::InvalidArgument {
            field: "vm_spec_json".to_string(),
            reason: e.to_string(),
        })
    }

    pub fn validate(&self) -> Result<(), ChvError> {
        if self.cpus == 0 {
            return Err(ChvError::InvalidArgument {
                field: "cpus".to_string(),
                reason: "cpus must be > 0".to_string(),
            });
        }
        if self.kernel_path.is_empty() {
            return Err(ChvError::InvalidArgument {
                field: "kernel_path".to_string(),
                reason: "kernel_path is required".to_string(),
            });
        }
        for nic in &self.nics {
            if nic.mac_address.is_empty() {
                return Err(ChvError::InvalidArgument {
                    field: "mac_address".to_string(),
                    reason: "mac_address is required".to_string(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_vm_spec() {
        let json = r#"{
            "name": "test-vm",
            "cpus": 2,
            "memory_bytes": 1073741824,
            "kernel_path": "/var/lib/chv/vmlinux",
            "disks": [
                {
                    "volume_id": "vol-1",
                    "read_only": true,
                    "size_bytes": 10737418240
                }
            ],
            "nics": [
                {
                    "network_id": "net-1",
                    "mac_address": "aa:bb:cc:dd:ee:ff",
                    "ip_address": "10.0.0.2"
                }
            ]
        }"#;
        let spec = VmSpec::from_json(json).unwrap();
        assert_eq!(spec.name, "test-vm");
        assert_eq!(spec.cpus, 2);
        assert_eq!(spec.memory_bytes, 1073741824);
        assert_eq!(spec.kernel_path, "/var/lib/chv/vmlinux");
        assert_eq!(spec.disk_seed_path, None);
        assert_eq!(spec.disks.len(), 1);
        assert!(spec.disks[0].read_only);
        assert_eq!(spec.disks[0].size_bytes, Some(10737418240));
        assert_eq!(spec.nics.len(), 1);
        assert_eq!(spec.nics[0].mac_address, "aa:bb:cc:dd:ee:ff");
        assert!(spec.validate().is_ok());
    }

    #[test]
    fn reject_zero_cpus() {
        let spec = VmSpec {
            name: "test".to_string(),
            cpus: 0,
            memory_bytes: 512,
            kernel_path: "/kernel".to_string(),
            firmware_path: None,
            disk_seed_path: None,
            disks: vec![],
            nics: vec![],
            desired_state: "Running".to_string(),
        };
        let err = spec.validate().unwrap_err();
        match err {
            ChvError::InvalidArgument { field, .. } => assert_eq!(field, "cpus"),
            _ => panic!("expected InvalidArgument error"),
        }
    }

    #[test]
    fn reject_missing_kernel() {
        let spec = VmSpec {
            name: "test".to_string(),
            cpus: 1,
            memory_bytes: 512,
            kernel_path: "".to_string(),
            firmware_path: None,
            disk_seed_path: None,
            disks: vec![],
            nics: vec![],
            desired_state: "Running".to_string(),
        };
        let err = spec.validate().unwrap_err();
        match err {
            ChvError::InvalidArgument { field, .. } => assert_eq!(field, "kernel_path"),
            _ => panic!("expected InvalidArgument error"),
        }
    }

    #[test]
    fn reject_empty_mac() {
        let spec = VmSpec {
            name: "test".to_string(),
            cpus: 1,
            memory_bytes: 512,
            kernel_path: "/kernel".to_string(),
            firmware_path: None,
            disk_seed_path: None,
            disks: vec![],
            nics: vec![NicSpec {
                network_id: "net-1".to_string(),
                mac_address: "".to_string(),
                ip_address: "10.0.0.2".to_string(),
                tap_name: "tap0".to_string(),
                cidr: "".to_string(),
                gateway: "".to_string(),
            }],
            desired_state: "Running".to_string(),
        };
        let err = spec.validate().unwrap_err();
        match err {
            ChvError::InvalidArgument { field, .. } => assert_eq!(field, "mac_address"),
            _ => panic!("expected InvalidArgument error"),
        }
    }
}
