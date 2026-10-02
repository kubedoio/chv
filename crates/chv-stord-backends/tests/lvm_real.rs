//! Root-gated REAL-LVM integration tests for `LVMBackend` (M4.5 storage
//! qualification).
//!
//! These are the storage twins of `chv-nwd-core`'s host-safety tests: they
//! run the production backend against a real LVM2 stack (loopback PV → VG
//! → LV) as root. The qualification scenario
//! `scripts/integration/qual/m4.5-storage.sh` provisions a disposable VG
//! (`CHV_LVM_TEST_VG`), runs these tests via the built test binary, and
//! asserts afterwards that no residue remains.
//!
//! Layer truth (why provisioning is out-of-band here): `LVMBackend::open`
//! does not create LVs — provisioning (`lvcreate`) is the host operator's
//! job; stord consumes pre-provisioned volumes. Each test therefore
//! provisions its own LVs and removes them again (Drop guard), mirroring
//! the operator model. No VM-integrated path reaches an LVM volume today
//! (the agent's reconcile paths hardcode `backend_class "local"`); these
//! tests qualify the stord-layer contract a future integration will
//! build on.
//!
//! Run serially (snapshots/clones claim `100%FREE` of the shared VG, so
//! parallel tests would starve each other):
//!
//! ```text
//! sudo env "PATH=$PATH" CHV_LVM_TEST_VG=<vg> \
//!     cargo test -p chv-stord-backends --test lvm_real -- \
//!     --ignored --test-threads=1
//! ```

use chv_common::types::{BackendLocator, DevicePolicy};
use chv_common::AttachmentOwnership;
use chv_errors::ChvError;
use chv_stord_backends::lvm::LVMBackend;
use chv_stord_backends::r#trait::StorageBackend;
use std::path::PathBuf;
use std::process::Command;

/// The disposable VG provisioned by the harness. Panics with instructions
/// if unset — a silent skip would read as coverage that never ran.
fn vg() -> String {
    std::env::var("CHV_LVM_TEST_VG").unwrap_or_else(|_| {
        panic!(
            "CHV_LVM_TEST_VG is not set: these tests need a disposable VG \
             provisioned by the qualification harness (see \
             scripts/integration/qual/m4.5-storage.sh)"
        )
    })
}

fn backend() -> LVMBackend {
    LVMBackend::new(vg()).expect("valid vg name")
}

fn locator() -> BackendLocator {
    BackendLocator {
        backend_class: "lvm".to_string(),
        locator: format!("{}/ignored-by-lvm-backend", vg()),
        options: Default::default(),
    }
}

fn ownership() -> AttachmentOwnership {
    AttachmentOwnership {
        vm_id: "m45-qual-vm".to_string(),
        operation_id: None,
        requester: None,
    }
}

fn lv_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/dev/{}/{}", vg(), name))
}

/// Run a host command, asserting success (test failure, not error
/// return: harness setup/teardown failures are test failures).
fn run_ok(cmd: &str, args: &[&str]) {
    let out = Command::new(cmd).args(args).output().unwrap_or_else(|e| {
        panic!("failed to spawn {cmd}: {e}");
    });
    assert!(
        out.status.success(),
        "{cmd} {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn blockdev_get_u64(flag: &str, path: &std::path::Path) -> u64 {
    let out = Command::new("blockdev")
        .args([flag, &path.to_string_lossy()])
        .output()
        .expect("blockdev");
    assert!(
        out.status.success(),
        "blockdev {flag} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .expect("numeric blockdev output")
}

/// Removes the LV on drop, ignoring failures (best-effort cleanup so one
/// test's residue never hides the next test's real result; the harness's
/// no-residue assertions catch anything left behind).
struct LvGuard(String);

impl Drop for LvGuard {
    fn drop(&mut self) {
        let path = lv_path(&self.0).to_string_lossy().to_string();
        let _ = Command::new("lvremove").args(["-f", &path]).output();
    }
}

/// Provision an LV out-of-band (the operator model) and guard its removal.
fn provision_lv(name: &str, size_mb: u64) -> LvGuard {
    run_ok(
        "lvcreate",
        &["-L", &format!("{size_mb}M"), "-n", name, &vg()],
    );
    assert!(lv_path(name).exists(), "LV device node must exist: {name}");
    LvGuard(name.to_string())
}

/// Read `len` bytes at `offset` directly from a block device.
fn read_direct(path: &std::path::Path, offset: u64, len: usize) -> Vec<u8> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f =
        std::fs::File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    f.seek(SeekFrom::Start(offset)).expect("seek");
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf).expect("read_exact");
    buf
}

fn pattern(seed: u8, len: usize) -> Vec<u8> {
    (0..len).map(|i| seed.wrapping_add(i as u8)).collect()
}

/// The core stord-layer contract on real LVM: open exports the LV, block
/// writes land on the device, reads return them, size matches, close
/// releases the handle.
#[tokio::test]
#[ignore = "root-gated real-LVM test; requires CHV_LVM_TEST_VG (harness)"]
async fn lvm_real_open_export_and_block_roundtrip() {
    let vid = "m45rt-open1";
    let _lv = provision_lv(vid, 64);
    let backend = backend();

    let export = backend
        .open(vid, &locator(), &DevicePolicy::default())
        .await
        .expect("open");
    assert_eq!(export.export_kind, "lvm");
    assert_eq!(
        export.export_path,
        lv_path(vid).to_string_lossy().to_string(),
        "export path must be the LV device node"
    );
    assert_eq!(export.attachment_handle, format!("lvm-{}-{vid}", vg()));

    // Write → read back through the backend (the write/read contract).
    let data = pattern(0x11, 8192);
    backend
        .write_block(vid, &export.attachment_handle, 1 << 20, &data)
        .await
        .expect("write_block");
    let read = backend
        .read_block(vid, &export.attachment_handle, 1 << 20, data.len() as u64)
        .await
        .expect("read_block");
    assert_eq!(read, data, "read_block must return what write_block wrote");

    // The write landed on the real device (visible to a direct reader),
    // and never-written regions are still zero.
    assert_eq!(read_direct(&lv_path(vid), 1 << 20, 8192), data);
    let virgin = backend
        .read_block(vid, &export.attachment_handle, 2 << 20, 4096)
        .await
        .expect("read_block virgin");
    assert!(
        virgin.iter().all(|&b| b == 0),
        "unwritten region must be zeros"
    );

    // Size is the LV's real size (blockdev --getsize64 semantics).
    assert_eq!(
        backend
            .volume_size(vid, &export.attachment_handle)
            .await
            .expect("volume_size"),
        64 * 1024 * 1024
    );

    backend
        .close(vid, &export.attachment_handle)
        .await
        .expect("close");
}

/// Wrong-class locators are rejected before any device is touched.
#[tokio::test]
#[ignore = "root-gated real-LVM test; requires CHV_LVM_TEST_VG (harness)"]
async fn lvm_real_open_rejects_wrong_class() {
    let backend = backend();
    let bad = BackendLocator {
        backend_class: "local".to_string(),
        locator: "/dev/null".to_string(),
        options: Default::default(),
    };
    match backend
        .open("m45rt-none", &bad, &DevicePolicy::default())
        .await
    {
        Err(ChvError::BackendUnavailable { .. }) => {}
        other => panic!("expected BackendUnavailable for local-class locator, got {other:?}"),
    }
}

/// A snapshot is a real COW snapshot: after the origin is overwritten, the
/// snapshot device still reads the pre-snapshot bytes; delete_snapshot
/// removes it and leaves the origin alone.
#[tokio::test]
#[ignore = "root-gated real-LVM test; requires CHV_LVM_TEST_VG (harness)"]
async fn lvm_real_snapshot_is_copy_on_write() {
    let vid = "m45rt-snap1";
    let _lv = provision_lv(vid, 64);
    let backend = backend();

    let export = backend
        .open(vid, &locator(), &DevicePolicy::default())
        .await
        .expect("open");
    let handle = export.attachment_handle;

    let before = pattern(0x22, 8192);
    backend
        .write_block(vid, &handle, 4096, &before)
        .await
        .expect("write before snapshot");

    backend
        .prepare_snapshot(vid, &handle, ownership(), "s1")
        .await
        .expect("prepare_snapshot");
    let snap_dev = lv_path(&format!("{vid}-snap-s1"));
    assert!(
        snap_dev.exists(),
        "snapshot LV device must exist: {snap_dev:?}"
    );

    // Overwrite the origin AFTER the snapshot; the snapshot must still
    // read the pre-snapshot bytes (copy-on-write), the origin the new ones.
    let after = pattern(0x99, 8192);
    backend
        .write_block(vid, &handle, 4096, &after)
        .await
        .expect("write after snapshot");
    assert_eq!(
        read_direct(&snap_dev, 4096, 8192),
        before,
        "snapshot must hold the pre-snapshot bytes (COW)"
    );
    assert_eq!(
        read_direct(&lv_path(vid), 4096, 8192),
        after,
        "origin must hold the post-snapshot bytes"
    );

    backend
        .delete_snapshot(vid, &handle, "s1")
        .await
        .expect("delete_snapshot");
    assert!(!snap_dev.exists(), "snapshot LV must be removed");
    assert_eq!(
        read_direct(&lv_path(vid), 4096, 8192),
        after,
        "origin must be untouched by snapshot deletion"
    );
}

/// A clone is a real point-in-time copy: the clone device reads the origin
/// bytes at prepare time.
#[tokio::test]
#[ignore = "root-gated real-LVM test; requires CHV_LVM_TEST_VG (harness)"]
async fn lvm_real_clone_copies_data() {
    let vid = "m45rt-clone1";
    let _lv = provision_lv(vid, 64);
    let backend = backend();

    let export = backend
        .open(vid, &locator(), &DevicePolicy::default())
        .await
        .expect("open");
    let handle = export.attachment_handle;

    let data = pattern(0x33, 8192);
    backend
        .write_block(vid, &handle, 8192, &data)
        .await
        .expect("write before clone");

    backend
        .prepare_clone(vid, &handle, ownership(), "c1")
        .await
        .expect("prepare_clone");
    let clone_name = format!("{vid}-clone-c1");
    let clone_dev = lv_path(&clone_name);
    assert!(
        clone_dev.exists(),
        "clone LV device must exist: {clone_dev:?}"
    );
    let _clone_guard = LvGuard(clone_name);

    assert_eq!(
        read_direct(&clone_dev, 8192, 8192),
        data,
        "clone must read the origin bytes at prepare time"
    );
}

/// resize grows the real LV (blockdev-visible).
#[tokio::test]
#[ignore = "root-gated real-LVM test; requires CHV_LVM_TEST_VG (harness)"]
async fn lvm_real_resize_grows_volume() {
    let vid = "m45rt-grow1";
    let _lv = provision_lv(vid, 32);
    let backend = backend();

    let export = backend
        .open(vid, &locator(), &DevicePolicy::default())
        .await
        .expect("open");
    let handle = export.attachment_handle;

    assert_eq!(
        backend
            .volume_size(vid, &handle)
            .await
            .expect("size before"),
        32 * 1024 * 1024
    );
    backend
        .resize(vid, &handle, 64 * 1024 * 1024)
        .await
        .expect("resize");
    assert_eq!(
        blockdev_get_u64("--getsize64", &lv_path(vid)),
        64 * 1024 * 1024,
        "LV must really be 64MiB after resize"
    );
    assert_eq!(
        backend.volume_size(vid, &handle).await.expect("size after"),
        64 * 1024 * 1024
    );
}

/// read_only device policy really sets the device read-only (writes fail
/// at the block layer). NOTE: the backend's policy application is one-way
/// — there is no `--setrw` path — so the test restores rw itself; the
/// one-way limitation is recorded in the M4.5 evidence.
#[tokio::test]
#[ignore = "root-gated real-LVM test; requires CHV_LVM_TEST_VG (harness)"]
async fn lvm_real_read_only_policy_blocks_writes() {
    let vid = "m45rt-ro1";
    let _lv = provision_lv(vid, 64);
    let backend = backend();

    let export = backend
        .open(vid, &locator(), &DevicePolicy::default())
        .await
        .expect("open");
    let handle = export.attachment_handle;

    let data = pattern(0x44, 4096);
    backend
        .write_block(vid, &handle, 0, &data)
        .await
        .expect("writable with default policy");

    backend
        .set_device_policy(
            vid,
            &handle,
            &DevicePolicy {
                read_only: true,
                ..Default::default()
            },
        )
        .await
        .expect("set read-only policy");
    assert_eq!(
        blockdev_get_u64("--getro", &lv_path(vid)),
        1,
        "device must be read-only at the block layer"
    );
    assert!(
        backend.write_block(vid, &handle, 0, &data).await.is_err(),
        "write_block must fail on a read-only device"
    );

    // One-way limitation: restore rw manually (recorded in evidence).
    run_ok("blockdev", &["--setrw", &lv_path(vid).to_string_lossy()]);
    backend
        .write_block(vid, &handle, 0, &data)
        .await
        .expect("writable again after manual setrw");
}

/// health reflects the device's existence on the real stack.
#[tokio::test]
#[ignore = "root-gated real-LVM test; requires CHV_LVM_TEST_VG (harness)"]
async fn lvm_real_health_reflects_existence() {
    let vid = "m45rt-health1";
    let lv = provision_lv(vid, 64);
    let backend = backend();
    let handle = format!("lvm-{}-{vid}", vg());

    let healthy = backend.health(vid, &handle).await.expect("health");
    assert_eq!(healthy.status, "healthy");
    assert!(healthy.last_error.is_empty());

    // Remove the LV out-of-band; health must report it.
    run_ok("lvremove", &["-f", &lv_path(vid).to_string_lossy()]);
    drop(lv); // guard's own lvremove is now a no-op
    let unhealthy = backend.health(vid, &handle).await.expect("health");
    assert_eq!(unhealthy.status, "unhealthy");
    assert!(unhealthy.last_error.contains("path does not exist"));
}
