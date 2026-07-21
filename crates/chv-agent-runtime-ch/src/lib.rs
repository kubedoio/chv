pub mod adapter;
pub mod ch_api;
#[cfg(target_os = "linux")]
// Phase C deliberately compiles this observer without production composition.
#[allow(dead_code)]
pub(crate) mod linux_observation;
pub mod mock;
pub mod process;

pub use adapter::{CloudHypervisorAdapter, VmConfig};
pub use ch_api::CloudHypervisorApiClient;
pub use process::ProcessCloudHypervisorAdapter;
