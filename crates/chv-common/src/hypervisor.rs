use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq, Default, Serialize)]
pub struct HypervisorOverrides {
    pub cpu_nested: Option<bool>,
    pub cpu_amx: Option<bool>,
    pub cpu_kvm_hyperv: Option<bool>,
    pub memory_mergeable: Option<bool>,
    pub memory_hugepages: Option<bool>,
    pub memory_shared: Option<bool>,
    pub memory_prefault: Option<bool>,
    pub iommu: Option<bool>,
    pub rng_src: Option<String>,
    pub watchdog: Option<bool>,
    pub landlock_enable: Option<bool>,
    pub serial_mode: Option<String>,
    pub console_mode: Option<String>,
    pub pvpanic: Option<bool>,
    pub tpm_type: Option<String>,
    pub tpm_socket_path: Option<String>,
}

pub const DEFAULT_CPU_NESTED: bool = true;
pub const DEFAULT_CPU_AMX: bool = false;
pub const DEFAULT_CPU_KVM_HYPERV: bool = false;
pub const DEFAULT_MEMORY_MERGEABLE: bool = false;
pub const DEFAULT_MEMORY_HUGEPAGES: bool = false;
pub const DEFAULT_MEMORY_SHARED: bool = false;
pub const DEFAULT_MEMORY_PREFAULT: bool = false;
pub const DEFAULT_IOMMU: bool = false;
pub const DEFAULT_RNG_SRC: &str = "/dev/urandom";
pub const DEFAULT_WATCHDOG: bool = false;
pub const DEFAULT_LANDLOCK_ENABLE: bool = false;
pub const DEFAULT_SERIAL_MODE: &str = "Pty";
pub const DEFAULT_CONSOLE_MODE: &str = "Off";
pub const DEFAULT_PVPANIC: bool = false;
pub const DEFAULT_TPM_TYPE: Option<&str> = None;
pub const DEFAULT_TPM_SOCKET_PATH: Option<&str> = None;

pub const VALID_SERIAL_MODES: &[&str] = &["Pty", "File", "Off", "Null"];
pub const VALID_CONSOLE_MODES: &[&str] = &["Pty", "File", "Off", "Null"];
pub const VALID_TPM_TYPES: &[&str] = &["swtpm"];

pub fn validate_rng_src(src: &str) -> Result<(), String> {
    if src.is_empty() {
        return Err("rng_src must be non-empty".to_string());
    }
    if !src.starts_with('/') {
        return Err("rng_src must be an absolute path".to_string());
    }
    Ok(())
}

pub fn validate_serial_mode(mode: &str) -> Result<(), String> {
    if VALID_SERIAL_MODES.contains(&mode) {
        Ok(())
    } else {
        Err(format!(
            "serial_mode must be one of {:?}",
            VALID_SERIAL_MODES
        ))
    }
}

pub fn validate_console_mode(mode: &str) -> Result<(), String> {
    if VALID_CONSOLE_MODES.contains(&mode) {
        Ok(())
    } else {
        Err(format!(
            "console_mode must be one of {:?}",
            VALID_CONSOLE_MODES
        ))
    }
}

pub fn validate_tpm_type(tpm: &str) -> Result<(), String> {
    if VALID_TPM_TYPES.contains(&tpm) {
        Ok(())
    } else {
        Err(format!("tpm_type must be one of {:?}", VALID_TPM_TYPES))
    }
}
