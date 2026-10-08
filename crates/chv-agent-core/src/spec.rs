use chv_common::hypervisor;
use chv_errors::ChvError;
use serde::Deserialize;

fn default_running() -> String {
    "Running".to_string()
}

pub use chv_common::hypervisor::HypervisorOverrides;

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
    #[serde(default)]
    pub hypervisor_overrides: Option<HypervisorOverrides>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct DiskSpec {
    pub volume_id: String,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    /// Stord backend class for this disk's volume opens (#379 PR 1, the
    /// A6 model field): absent means the historical `"local"` default
    /// ([`DiskSpec::backend_class_or_local`]). The model is
    /// serde-tolerant (no `deny_unknown_fields`), so the field is purely
    /// additive — a pre-#379 spec JSON deserializes identically. No
    /// producer sets it yet (the CP carry is PR 2); the name matches what
    /// the attach handler's spec_json parser already reads
    /// (`backend_class`, B5).
    #[serde(default)]
    pub backend_class: Option<String>,
}

impl DiskSpec {
    /// The stord backend class to open this disk's volume with: the
    /// spec's value when set, else the historical `"local"` default.
    ///
    /// #379 PR 1's single default seam for the agent-side open sites —
    /// while no producer sets the field this returns exactly what the
    /// old inline literal sent (zero behavior change).
    pub fn backend_class_or_local(&self) -> &str {
        self.backend_class
            .as_deref()
            .unwrap_or(chv_hypervisor_api::resources::DEFAULT_BACKEND_CLASS)
    }
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
    /// Operator-configured firewall policy for the network (the CP's
    /// `firewall_rules_json` snapshot). Applied by the Core executor at
    /// attach time (#355); an absent or empty snapshot resolves to the
    /// shared DP4 baseline (DHCP/DNS/conntrack + default-deny) at the
    /// attach path — there is no unfiltered "bare-table" outcome
    /// anymore.
    #[serde(default)]
    pub firewall_policy_json: Option<String>,
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
        // Volume ids become path components of stord locators
        // (`{volume_id}.img`): reject anything that is not a single safe
        // component (a crafted id is a write traversal). The control plane
        // is a trusted-but-buggy peer; this is the node-side boundary.
        for disk in &self.disks {
            if !chv_common::is_safe_id(&disk.volume_id) {
                return Err(ChvError::InvalidArgument {
                    field: "volume_id".to_string(),
                    reason: format!(
                        "'{}' is not a safe volume id (must be a single path component)",
                        disk.volume_id
                    ),
                });
            }
        }
        if let Some(ref hv) = self.hypervisor_overrides {
            if let Some(ref src) = hv.rng_src {
                hypervisor::validate_rng_src(src).map_err(|e| ChvError::InvalidArgument {
                    field: "rng_src".to_string(),
                    reason: e,
                })?;
            }
            if let Some(ref mode) = hv.serial_mode {
                hypervisor::validate_serial_mode(mode).map_err(|e| ChvError::InvalidArgument {
                    field: "serial_mode".to_string(),
                    reason: e,
                })?;
            }
            if let Some(ref mode) = hv.console_mode {
                hypervisor::validate_console_mode(mode).map_err(|e| ChvError::InvalidArgument {
                    field: "console_mode".to_string(),
                    reason: e,
                })?;
            }
            if let Some(ref tpm) = hv.tpm_type {
                hypervisor::validate_tpm_type(tpm).map_err(|e| ChvError::InvalidArgument {
                    field: "tpm_type".to_string(),
                    reason: e,
                })?;
            }
            if hv.tpm_type.is_none() && hv.tpm_socket_path.is_some() {
                return Err(ChvError::InvalidArgument {
                    field: "tpm_socket_path".to_string(),
                    reason: "tpm_socket_path cannot be set without tpm_type".to_string(),
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
    fn parse_vm_spec_nic_firewall_policy_snapshot() {
        // #355: the CP attaches the network's firewall_rules_json to the
        // nic spec; it must deserialize (absent → None for pre-#355
        // specs).
        let with_policy = r#"{
            "name": "test-vm",
            "cpus": 1,
            "memory_bytes": 512,
            "kernel_path": "/var/lib/chv/vmlinux",
            "disks": [],
            "nics": [
                {
                    "network_id": "net-1",
                    "mac_address": "aa:bb:cc:dd:ee:ff",
                    "ip_address": "10.0.0.2",
                    "firewall_policy_json": "[{\"direction\":\"inbound\",\"action\":\"accept\",\"protocol\":\"icmp\"}]"
                }
            ]
        }"#;
        let spec = VmSpec::from_json(with_policy).unwrap();
        assert_eq!(
            spec.nics[0].firewall_policy_json.as_deref(),
            Some("[{\"direction\":\"inbound\",\"action\":\"accept\",\"protocol\":\"icmp\"}]")
        );
        assert!(spec.validate().is_ok());

        let without_policy = with_policy.replace(
            ",\n                    \"firewall_policy_json\": \"[{\\\"direction\\\":\\\"inbound\\\",\\\"action\\\":\\\"accept\\\",\\\"protocol\\\":\\\"icmp\\\"}]\"",
            "",
        );
        let spec = VmSpec::from_json(&without_policy).unwrap();
        assert_eq!(spec.nics[0].firewall_policy_json, None);
    }

    #[test]
    fn disk_spec_backend_class_is_additive_and_parse_stable() {
        // #379 PR 1 back-compat pin (A6): the field is purely additive —
        // a pre-#379 DiskSpec JSON (no backend_class key) deserializes
        // identically (None), and a JSON carrying the field parses to the
        // same value a hand-built struct holds (the shape PR 2's
        // producers will emit). DiskSpec has no Serialize impl — this
        // pins the deserialization contract, not a serialize round-trip.
        let without = r#"{"volume_id":"vol-1","read_only":true,"size_bytes":1024}"#;
        let parsed: DiskSpec = serde_json::from_str(without).unwrap();
        assert_eq!(
            parsed,
            DiskSpec {
                volume_id: "vol-1".to_string(),
                read_only: true,
                size_bytes: Some(1024),
                backend_class: None,
            }
        );

        let with = r#"{"volume_id":"vol-1","backend_class":"lvm"}"#;
        let parsed: DiskSpec = serde_json::from_str(with).unwrap();
        assert_eq!(
            parsed,
            DiskSpec {
                volume_id: "vol-1".to_string(),
                read_only: false,
                size_bytes: None,
                backend_class: Some("lvm".to_string()),
            }
        );
        // Re-parsing the same JSON is deterministic (same value back).
        let reparsed: DiskSpec = serde_json::from_str(with).unwrap();
        assert_eq!(reparsed, parsed);
    }

    #[test]
    fn disk_spec_backend_class_or_local_is_the_default_seam() {
        // #379 PR 1 pin (B5 default): absent field resolves to exactly
        // "local" — the historical literal — and a set field passes
        // through verbatim.
        let absent: DiskSpec = serde_json::from_str(r#"{"volume_id":"v"}"#).unwrap();
        assert_eq!(absent.backend_class_or_local(), "local");
        let set: DiskSpec =
            serde_json::from_str(r#"{"volume_id":"v","backend_class":"lvm"}"#).unwrap();
        assert_eq!(set.backend_class_or_local(), "lvm");
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
            cloud_init_userdata: None,
            hypervisor_overrides: None,
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
            cloud_init_userdata: None,
            hypervisor_overrides: None,
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
                firewall_policy_json: None,
            }],
            desired_state: "Running".to_string(),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        };
        let err = spec.validate().unwrap_err();
        match err {
            ChvError::InvalidArgument { field, .. } => assert_eq!(field, "mac_address"),
            _ => panic!("expected InvalidArgument error"),
        }
    }
}
