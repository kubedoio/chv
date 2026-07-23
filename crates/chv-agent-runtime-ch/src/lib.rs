pub mod adapter;
pub mod ch_api;
pub mod core_runtime;
#[cfg(target_os = "linux")]
// Phase C deliberately compiles this observer without production composition.
#[allow(dead_code)]
pub(crate) mod linux_observation;
pub mod mock;
pub mod process;

pub use adapter::{CloudHypervisorAdapter, VmConfig};
pub use ch_api::CloudHypervisorApiClient;
pub use chv_hypervisor_api::HypervisorAdapter;
pub use process::ProcessCloudHypervisorAdapter;
