//! Hypervisor adapter types and trait.
//!
//! The canonical definitions live in [`chv_hypervisor_api`]. This module
//! re-exports them and provides the legacy `CloudHypervisorAdapter` alias
//! so that existing downstream code continues to compile without changes.

pub use chv_hypervisor_api::{
    AddDiskParams, AddNetParams, HypervisorAdapter as CloudHypervisorAdapter, VmConfig, VmCounters,
    VmDiskConfig, VmInfo, VmNicConfig,
};
