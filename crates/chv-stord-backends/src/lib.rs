//! Storage backend implementations for chv-stord.
//!
//! Each module under this crate provides a [`StorageBackend`] implementation
//! for a specific storage class:
//!
//! - [`local`]: file-backed volumes (raw and qcow2)
//! - [`lvm`]: LVM2 logical volumes
//! - [`iscsi`]: iSCSI LUNs via `iscsiadm` / `targetcli`
//! - [`ceph`]: Ceph RBD images via the `rbd` CLI
//!
//! ## Testing
//!
//! Backend implementations have unit tests covering:
//!
//! - Config / construction-time validation (empty fields, malicious vg/pool names).
//! - `Clone` round-trips and stable handle-format strings (these are part of
//!   the ABI between chv-agent and chv-stord).
//! - `volume_id` and other id sanitization (path traversal, shell injection).
//! - Health-probe error paths (unreachable endpoints, missing files/binaries).
//!
//! End-to-end tests against real clusters (Ceph RBD, iSCSI targets, LVM
//! volume groups) are NOT in the unit test suite.  They live in
//! `tests/integration/` (TBD) and require explicit cluster fixtures
//! provisioned in CI.  Tests in this crate that require a host-provided
//! tool (`iscsiadm`, `rbd`) and a reachable target are marked `#[ignore]`
//! so they do not bloat default `cargo test` runs; invoke them with
//! `cargo test -p chv-stord-backends -- --ignored` on a suitably
//! provisioned host.

pub mod ceph;
pub mod iscsi;
pub mod local;
pub mod lvm;
pub mod r#trait;

pub use ceph::CephRbdBackend;
pub use iscsi::IscsiBackend;
pub use local::LocalFileBackend;
pub use lvm::LVMBackend;
pub use r#trait::{BackendHealth, StorageBackend, VolumeExport, DIRTY_TRACKING_BLOCK_SIZE};
