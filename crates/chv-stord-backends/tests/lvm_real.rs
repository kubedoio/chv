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
//! Layer truth: since #379 DP2, `LVMBackend::open` does create-on-open —
//! an absent LV with a `size_bytes` option provisions via `lvcreate`
//! (parity with the local backend's sparse-file create-on-open), and an
//! absent LV WITHOUT a size refuses (LVM volumes are not sparse; a
//! default-size LV would silently consume real extents). Tests that
//! exercise the pre-provisioned contract provision their own LVs and
//! remove them again (Drop guard), mirroring the operator model; the
//! create-on-open tests let the backend provision and guard the result
//! the same way. The VM-integrated path is real since PR 3 (the agent
//! shapes LVM-class opens, DP5; qualified end-to-end by m4.5 Leg G);
//! these tests pin the stord-layer contract it builds on.
//!
//! `seed_from` is unsupported on LVM (DP2 scope cut): the create-on-open
//! test below pins the explicit rejection.
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

/// A create-on-open locator: `size_bytes` rides the options map exactly
/// as the agent's open sites send it (#379 DP2).
fn creating_locator(size_bytes: u64) -> BackendLocator {
    BackendLocator {
        backend_class: "lvm".to_string(),
        locator: format!("{}/ignored-by-lvm-backend", vg()),
        options: [("size_bytes".to_string(), size_bytes.to_string())].into(),
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

/// #379 DP2 create-on-open: an absent LV with a `size_bytes` option is
/// provisioned by the open itself (`lvcreate`, the
/// create_receiving_volume shape) and the export is the new LV device —
/// the parity the VM-integrated LVM path (m4.5 Leg G) depends on.
#[tokio::test]
#[ignore = "root-gated real-LVM test; requires CHV_LVM_TEST_VG (harness)"]
async fn lvm_real_open_provisions_absent_lv_with_size() {
    let vid = "m45rt-create1";
    assert!(
        !lv_path(vid).exists(),
        "precondition: the LV must not exist before the create-on-open"
    );
    let _lv = LvGuard(vid.to_string());
    let backend = backend();

    let export = backend
        .open(
            vid,
            &creating_locator(64 * 1024 * 1024),
            &DevicePolicy::default(),
        )
        .await
        .expect("create-on-open");
    assert_eq!(export.export_kind, "lvm");
    assert_eq!(
        export.export_path,
        lv_path(vid).to_string_lossy().to_string(),
        "export path must be the provisioned LV device node"
    );
    assert_eq!(export.attachment_handle, format!("lvm-{}-{vid}", vg()));
    assert!(lv_path(vid).exists(), "the LV must exist after the open");

    // The provisioned LV really is the requested size (rounded up to
    // extents by lvcreate; 64MiB is extent-aligned).
    assert_eq!(
        blockdev_get_u64("--getsize64", &lv_path(vid)),
        64 * 1024 * 1024,
        "the provisioned LV must be 64MiB"
    );

    // The new volume is writable (the guest's first-boot contract).
    let data = pattern(0x55, 4096);
    backend
        .write_block(vid, &export.attachment_handle, 0, &data)
        .await
        .expect("write_block on the provisioned LV");
    assert_eq!(read_direct(&lv_path(vid), 0, 4096), data);

    // Idempotence: a second open of the now-existing LV (no size option)
    // does NOT re-provision — it exports the same device and the data
    // survives.
    let again = backend
        .open(vid, &locator(), &DevicePolicy::default())
        .await
        .expect("re-open of existing LV");
    assert_eq!(again.export_path, export.export_path);
    assert_eq!(
        read_direct(&lv_path(vid), 0, 4096),
        data,
        "re-open must not re-provision (data survives)"
    );
}

/// #379 DP2: an absent LV WITHOUT a size refuses. LVM volumes are not
/// sparse — a default-size LV would silently consume real extents, so
/// unlike the local backend there is no default size.
#[tokio::test]
#[ignore = "root-gated real-LVM test; requires CHV_LVM_TEST_VG (harness)"]
async fn lvm_real_open_refuses_absent_lv_without_size() {
    let vid = "m45rt-nosize";
    let backend = backend();
    match backend
        .open(vid, &locator(), &DevicePolicy::default())
        .await
    {
        Err(ChvError::InvalidArgument { field, .. }) => {
            assert_eq!(field, "size_bytes");
        }
        other => panic!("expected InvalidArgument(size_bytes), got {other:?}"),
    }
    assert!(
        !lv_path(vid).exists(),
        "the refused open must not leave an LV behind"
    );
}

/// #379 DP2 scope cut, pinned: `seed_from` is unsupported on LVM — the
/// open rejects explicitly instead of silently provisioning an empty LV
/// under an operator who asked for a seeded image.
#[tokio::test]
#[ignore = "root-gated real-LVM test; requires CHV_LVM_TEST_VG (harness)"]
async fn lvm_real_open_rejects_seed_from() {
    let vid = "m45rt-seed1";
    let backend = backend();
    let seeded = BackendLocator {
        backend_class: "lvm".to_string(),
        locator: format!("{}/ignored-by-lvm-backend", vg()),
        options: [
            ("size_bytes".to_string(), (64 * 1024 * 1024).to_string()),
            (
                "seed_from".to_string(),
                "/tmp/no-such-image.qcow2".to_string(),
            ),
        ]
        .into(),
    };
    match backend.open(vid, &seeded, &DevicePolicy::default()).await {
        Err(ChvError::InvalidArgument { field, .. }) => {
            assert_eq!(field, "seed_from");
        }
        other => panic!("expected InvalidArgument(seed_from), got {other:?}"),
    }
    assert!(
        !lv_path(vid).exists(),
        "the refused open must not leave an LV behind"
    );
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

    // Clone independence (review of #382): this backend implements
    // prepare_clone as an LVM COW snapshot, NOT a full block copy — so
    // the load-bearing property is that later origin writes never show
    // through. Reading the clone only at prepare time cannot distinguish
    // a linked clone from an independent one; overwrite the origin and
    // assert the clone still reads the ORIGINAL bytes.
    let overwrite = pattern(0x99, 8192);
    backend
        .write_block(vid, &handle, 8192, &overwrite)
        .await
        .expect("write after clone");
    assert_eq!(
        read_direct(&clone_dev, 8192, 8192),
        data,
        "clone must be independent of later origin writes (COW snapshot of the point-in-time state)"
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

/// #522 PR 1 (DP3): the destroy primitive against real LVM — the
/// reclaim actually runs `lvremove -y` on the carrier's LV, the LV
/// device node is gone afterwards, and a SECOND destroy of the
/// already-absent LV is `Ok(())` (the idempotency contract PR 2's
/// crash-redrive/retry story depends on — a replayed delete must never
/// manufacture a failure on the reclaimed extents).
#[tokio::test]
#[ignore = "root-gated real-LVM test; requires CHV_LVM_TEST_VG (harness)"]
async fn lvm_real_destroy_removes_the_lv_and_is_idempotent() {
    let vid = "m45rt-destroy1";
    // Provision out-of-band and deliberately let the guard's best-effort
    // lvremove find nothing (the destroy under test must have removed
    // the LV already) — the no-residue assertion is the test's own.
    let guard = provision_lv(vid, 32);
    let backend = backend();

    assert!(lv_path(vid).exists(), "precondition: LV exists");
    backend
        .destroy(vid, &locator())
        .await
        .expect("destroy of a present LV");
    assert!(
        !lv_path(vid).exists(),
        "the LV device node must be gone after destroy"
    );

    // Idempotent: the absent case is a success, and no command runs
    // (the exists-gate in the implementation).
    backend
        .destroy(vid, &locator())
        .await
        .expect("destroy of an absent LV is Ok (idempotency contract)");
    assert!(!lv_path(vid).exists());

    drop(guard);
}
