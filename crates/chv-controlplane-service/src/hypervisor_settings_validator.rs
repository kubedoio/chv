use chv_agent_core::spec::HypervisorOverrides;
use chv_common::hypervisor;
use chv_controlplane_store::HypervisorSettingsPatchInput;

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("{field}: {message}")]
    InvalidField { field: String, message: String },
}

pub fn validate_settings_patch(input: &HypervisorSettingsPatchInput) -> Result<(), ValidationError> {
    if let Some(ref src) = input.rng_src {
        hypervisor::validate_rng_src(src)
            .map_err(|e| ValidationError::InvalidField { field: "rng_src".into(), message: e })?;
    }
    if let Some(ref mode) = input.serial_mode {
        hypervisor::validate_serial_mode(mode)
            .map_err(|e| ValidationError::InvalidField { field: "serial_mode".into(), message: e })?;
    }
    if let Some(ref mode) = input.console_mode {
        hypervisor::validate_console_mode(mode)
            .map_err(|e| ValidationError::InvalidField { field: "console_mode".into(), message: e })?;
    }
    if let Some(ref tpm) = input.tpm_type {
        hypervisor::validate_tpm_type(tpm)
            .map_err(|e| ValidationError::InvalidField { field: "tpm_type".into(), message: e })?;
    }
    if input.tpm_type.is_none() && input.tpm_socket_path.is_some() {
        return Err(ValidationError::InvalidField {
            field: "tpm_socket_path".into(),
            message: "tpm_socket_path cannot be set without tpm_type".into(),
        });
    }
    if input.iommu == Some(true) && input.memory_shared != Some(true) {
        return Err(ValidationError::InvalidField {
            field: "iommu".into(),
            message: "iommu=true requires memory_shared=true".into(),
        });
    }
    Ok(())
}

pub fn validate_vm_overrides(overrides: &HypervisorOverrides) -> Result<(), ValidationError> {
    if let Some(ref src) = overrides.rng_src {
        hypervisor::validate_rng_src(src)
            .map_err(|e| ValidationError::InvalidField { field: "rng_src".into(), message: e })?;
    }
    if let Some(ref mode) = overrides.serial_mode {
        hypervisor::validate_serial_mode(mode)
            .map_err(|e| ValidationError::InvalidField { field: "serial_mode".into(), message: e })?;
    }
    if let Some(ref mode) = overrides.console_mode {
        hypervisor::validate_console_mode(mode)
            .map_err(|e| ValidationError::InvalidField { field: "console_mode".into(), message: e })?;
    }
    if let Some(ref tpm) = overrides.tpm_type {
        hypervisor::validate_tpm_type(tpm)
            .map_err(|e| ValidationError::InvalidField { field: "tpm_type".into(), message: e })?;
    }
    if overrides.tpm_type.is_none() && overrides.tpm_socket_path.is_some() {
        return Err(ValidationError::InvalidField {
            field: "tpm_socket_path".into(),
            message: "tpm_socket_path cannot be set without tpm_type".into(),
        });
    }
    if overrides.iommu == Some(true) && overrides.memory_shared != Some(true) {
        return Err(ValidationError::InvalidField {
            field: "iommu".into(),
            message: "iommu=true requires memory_shared=true".into(),
        });
    }
    Ok(())
}
