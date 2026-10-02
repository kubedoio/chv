//! First end-to-end sender↔receiver storage-migration test (M4.6, PR B).
//!
//! Everything on the path is production code: a real `LocalFileBackend` on
//! both sides (source and destination in separate tempdirs), the real
//! receiver served by [`chv_stord_core::server::serve_migration_tls`] on a
//! loopback mTLS TCP listener (rcgen certificate material, mandatory client
//! auth), and the real [`MigrationSender`] connecting with a real
//! [`MigrationTlsConfig`] over `https://127.0.0.1:<port>`.
//!
//! These tests pin the acknowledgment-protocol fixes for issue #391:
//!
//! - **Small volume** (2 chunks — not a multiple of the receiver's 64-chunk
//!   ack interval): previously the sender's bulk-phase drain waited for an
//!   ack that never came and timed out after 30 s. The receiver now flushes
//!   its ack window at phase boundaries, so the migration completes and the
//!   destination file matches the source bytes exactly.
//! - **Dirty rounds**: a concurrent `write_block` on the source (the only
//!   protocol-level dirty generator per issue #394 — the source bitmap is
//!   populated by `LocalFileBackend::write_block`) must be picked up by a
//!   dirty sync round, transferred, and present in the destination. Before
//!   the round-ack fix the sender blocked forever after `RoundComplete`.
//! - **Pause handshake**: with a task in a `MigrationTaskTable` (the stord
//!   handler path), the sender must reach `PausedFinalSync` and *stay*
//!   blocked until `ResumeDiskMigration{vm_paused:true}` semantics (a
//!   `true` on the task's `pause_tx` watch) release it into FinalSync.
//!
//! Fail-closed semantics (CRC mismatch, out-of-bounds rejection) are covered
//! by the receiver/sender unit tests and the negative mTLS listener tests in
//! `tests/migration_mtls.rs`.

use chv_common::types::{BackendLocator, DevicePolicy};
use chv_stord_backends::{LocalFileBackend, StorageBackend};
use chv_stord_core::migration::sender::{MigrationSender, MigrationTlsConfig};
use chv_stord_core::migration::service::StorageMigrationServiceImpl;
use chv_stord_core::migration::task::{MigrationPhase, MigrationTask, MigrationTaskTable};
use chv_stord_core::migration::tls_config::MigrationServerTls;
use chv_stord_core::server::serve_migration_tls;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

/// Install the rustls ring crypto provider (same as `cmd/chv-stord`
/// startup). Idempotent.
fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Migration block size (must match the backends' dirty-tracking block size
/// so bitmap bits map 1:1 to migration chunks).
const BLOCK: u64 = 4 * 1024 * 1024;

/// Overall guard so a protocol regression fails fast instead of hanging.
const MIGRATION_TIMEOUT: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------------
// rcgen certificate material (same pattern as tests/migration_mtls.rs)
// ---------------------------------------------------------------------

/// A test CA that can issue leaf certificates.
struct TestCa {
    params: rcgen::CertificateParams,
    key: rcgen::KeyPair,
    cert_pem: Vec<u8>,
}

fn test_ca(cn: &str) -> TestCa {
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::default();
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.key_usages = vec![
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
    ];
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, cn);
    let cert = params.self_signed(&key).unwrap();
    TestCa {
        params,
        key,
        cert_pem: cert.pem().into_bytes(),
    }
}

/// Issue a leaf certificate signed by `ca`. Server leaves carry the
/// `localhost`/127.0.0.1 identities and serverAuth EKU; client leaves carry
/// clientAuth EKU.
fn issue_leaf(ca: &TestCa, cn: &str, server: bool) -> (Vec<u8>, Vec<u8>) {
    use rcgen::string::Ia5String;

    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::default();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, cn);
    params.is_ca = rcgen::IsCa::NoCa;
    params.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![if server {
        rcgen::ExtendedKeyUsagePurpose::ServerAuth
    } else {
        rcgen::ExtendedKeyUsagePurpose::ClientAuth
    }];
    if server {
        params.subject_alt_names.push(rcgen::SanType::DnsName(
            Ia5String::try_from("localhost").unwrap(),
        ));
        params.subject_alt_names.push(rcgen::SanType::IpAddress(
            std::net::Ipv4Addr::LOCALHOST.into(),
        ));
    }
    let issuer = rcgen::Issuer::from_params(&ca.params, &ca.key);
    let cert = params.signed_by(&key, &issuer).unwrap();
    (cert.pem().into_bytes(), key.serialize_pem().into_bytes())
}

// ---------------------------------------------------------------------
// Test environment
// ---------------------------------------------------------------------

/// Serve the production migration receiver on an ephemeral loopback mTLS
/// TCP listener backed by `dest_dir`. Returns the bound address and the
/// test CA (which also issued the receiver's server leaf, so the same CA
/// can issue the sender's client identity).
async fn spawn_receiver(dest_dir: &Path) -> (SocketAddr, TestCa) {
    let ca = test_ca("chv-migration-e2e-ca");
    let (server_cert, server_key) = issue_leaf(&ca, "stord-receiver", true);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let tls = MigrationServerTls {
        listen_addr: addr,
        cert_pem: server_cert,
        key_pem: server_key,
        client_ca_pem: ca.cert_pem.clone(),
    };
    let backend = Arc::new(LocalFileBackend::new(dest_dir.to_path_buf()));
    let service = StorageMigrationServiceImpl::new(backend, dest_dir.to_path_buf());
    tokio::spawn(async move {
        if let Err(e) = serve_migration_tls(listener, tls, service).await {
            panic!("migration TLS listener failed: {e:?}");
        }
    });
    (addr, ca)
}

/// Build the sender's mTLS config from a client leaf issued by the same CA
/// the receiver trusts.
fn sender_tls(ca: &TestCa) -> MigrationTlsConfig {
    let (client_cert, client_key) = issue_leaf(ca, "stord-sender", false);
    MigrationTlsConfig {
        cert_pem: client_cert,
        key_pem: client_key,
        ca_pem: ca.cert_pem.clone(),
        dest_domain: "localhost".to_string(),
    }
}

/// A non-zero fill byte that varies per 4 MiB block, so bulk-copy
/// correctness (right block, right order) is observable in the destination.
fn block_fill(block_index: u64) -> u8 {
    (block_index as u8).wrapping_mul(37).wrapping_add(11) | 1
}

/// Write `blocks` 4 MiB blocks of per-block non-zero pattern to the source
/// image file and return the expected bytes.
fn write_source_image(dir: &Path, file: &str, blocks: u64) -> Vec<u8> {
    let mut image = Vec::with_capacity((blocks * BLOCK) as usize);
    for block in 0..blocks {
        image.extend(std::iter::repeat_n(block_fill(block), BLOCK as usize));
    }
    std::fs::write(dir.join(file), &image).unwrap();
    image
}

/// Open the source volume on the backend and enable dirty tracking (the
/// stord `TriggerDiskMigration` handler does exactly this before spawning
/// the sender). Returns the attachment handle.
async fn open_source_volume(
    backend: &LocalFileBackend,
    volume_id: &str,
    locator: &str,
    size_bytes: u64,
) -> String {
    let export = backend
        .open(
            volume_id,
            &BackendLocator {
                backend_class: "local".to_string(),
                locator: locator.to_string(),
                options: [("size_bytes".to_string(), size_bytes.to_string())]
                    .into_iter()
                    .collect(),
            },
            &DevicePolicy::default(),
        )
        .await
        .expect("source volume must open");
    backend
        .enable_dirty_tracking(volume_id, &export.attachment_handle, size_bytes)
        .await
        .expect("dirty tracking must be enabled");
    export.attachment_handle
}

/// Simulate `ResumeDiskMigration{vm_paused:true}` as soon as the sender
/// requests the pause (used by the tests that are not about the handshake
/// itself).
fn spawn_auto_pause(task: Arc<MigrationTask>) {
    tokio::spawn(async move {
        loop {
            let (needs_pause, failed) = {
                let state = task.state.read().await;
                (state.needs_vm_pause, state.phase == MigrationPhase::Failed)
            };
            if needs_pause {
                task.pause_tx.send(true).unwrap();
                return;
            }
            if failed {
                return; // the migration failed; the test asserts on the result
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    });
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

/// **Small volume** (issue #391, ack-window flush): an 8 MiB volume is 2
/// chunks — not a multiple of the receiver's 64-chunk ack interval. Before
/// the flush fix the sender's bulk-phase drain waited for an interval ack
/// that never came and failed with a 30 s timeout; now the boundary flush
/// completes it. The destination file must match the source bytes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn small_volume_completes_and_matches() {
    install_crypto_provider();

    let src_dir = tempfile::tempdir().unwrap();
    let dest_dir = tempfile::tempdir().unwrap();

    // 8 MiB = 2 chunks of 4 MiB.
    let expected = write_source_image(src_dir.path(), "vol.img", 2);

    let src_backend = Arc::new(LocalFileBackend::new(src_dir.path().to_path_buf()));
    let handle = open_source_volume(&src_backend, "vol-e2e-small", "vol.img", 2 * BLOCK).await;

    let (addr, ca) = spawn_receiver(dest_dir.path()).await;

    let (task, _pause_rx) = MigrationTask::new(
        "vol-e2e-small".to_string(),
        handle.clone(),
        format!("https://{addr}"),
    );
    spawn_auto_pause(task.clone());

    let sender = MigrationSender::new(src_backend, "vol-e2e-small".to_string(), handle)
        .with_tls(sender_tls(&ca))
        .with_task(task.clone());

    let endpoint = format!("https://{addr}");
    tokio::time::timeout(MIGRATION_TIMEOUT, sender.start_migration(endpoint))
        .await
        .expect("migration must not hang (ack-window flush regression?)")
        .expect("migration must succeed");
    assert_eq!(task.state.read().await.phase, MigrationPhase::Completed);

    let dest = std::fs::read(dest_dir.path().join("vol-e2e-small.img"))
        .expect("receiving volume file must exist");
    assert_eq!(
        dest, expected,
        "destination must match the source bytes exactly"
    );
}

/// **Dirty rounds transfer real data** (issue #391, round acknowledgment):
/// while bulk copy is running, a concurrent task writes to the source via
/// `write_block` — the only legitimate protocol-level dirty generator
/// (issue #394: the source bitmap is populated exclusively by
/// `LocalFileBackend::write_block`). The migration must converge through a
/// DIRTY_SYNC round, complete, and the destination must contain both the
/// pre-seeded dirty bytes and the concurrently written bytes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dirty_rounds_transfer_concurrent_writes() {
    install_crypto_provider();

    let src_dir = tempfile::tempdir().unwrap();
    let dest_dir = tempfile::tempdir().unwrap();

    // 32 MiB = 8 chunks (still below the 64-chunk ack interval, so the
    // round path also depends on the boundary flush).
    let blocks: u64 = 8;
    let mut expected = write_source_image(src_dir.path(), "vol.img", blocks);

    let src_backend = Arc::new(LocalFileBackend::new(src_dir.path().to_path_buf()));
    let volume_id = "vol-e2e-dirty".to_string();
    let handle = open_source_volume(&src_backend, &volume_id, "vol.img", blocks * BLOCK).await;

    // Pre-seed one deterministic dirty block (block 1): guarantees the
    // first dirty sync round has real payload regardless of scheduling.
    let d0 = vec![0xD0u8; 64 * 1024];
    src_backend
        .write_block(&volume_id, &handle, BLOCK, &d0)
        .await
        .expect("pre-seed dirty write must succeed");
    expected[BLOCK as usize..BLOCK as usize + d0.len()].copy_from_slice(&d0);

    let (addr, ca) = spawn_receiver(dest_dir.path()).await;

    let (task, _pause_rx) =
        MigrationTask::new(volume_id.clone(), handle.clone(), format!("https://{addr}"));
    spawn_auto_pause(task.clone());

    // Concurrent dirty writer (simulates the guest writing during bulk
    // copy): once the bulk phase is observed, keep writing the same payload
    // to the LAST block (offset 28 MiB) for as long as the bulk phase is
    // running. Targeting the last block maximizes the window: the sender
    // reads blocks in order, so the write lands before the sender's read of
    // that block and is transferred both by bulk copy and by the dirty
    // round that re-sends the marked block.
    let writer_backend = src_backend.clone();
    let writer_volume = volume_id.clone();
    let writer_handle = handle.clone();
    let writer_task = task.clone();
    let d1 = vec![0xD1u8; 64 * 1024];
    let d1_payload = d1.clone();
    let d1_offset = (blocks - 1) * BLOCK;
    let writer = tokio::spawn(async move {
        loop {
            let phase = writer_task.state.read().await.phase;
            match phase {
                MigrationPhase::Pending => {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                MigrationPhase::BulkCopy => {
                    writer_backend
                        .write_block(&writer_volume, &writer_handle, d1_offset, &d1_payload)
                        .await
                        .expect("concurrent dirty write must succeed");
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                _ => break, // DirtySync or later: stop writing
            }
        }
    });

    let sender = MigrationSender::new(src_backend, volume_id, handle)
        .with_tls(sender_tls(&ca))
        .with_task(task.clone());
    let endpoint = format!("https://{addr}");
    tokio::time::timeout(MIGRATION_TIMEOUT, sender.start_migration(endpoint))
        .await
        .expect("migration must not hang (round-ack deadlock regression?)")
        .expect("migration must succeed");
    writer
        .await
        .expect("concurrent writer must finish without panicking");

    let state = task.state.read().await;
    assert_eq!(state.phase, MigrationPhase::Completed);
    assert!(
        state.convergence_round >= 1,
        "migration must converge through at least one dirty sync round"
    );
    assert!(
        state.bytes_transferred > 0,
        "dirty rounds must transfer real bytes"
    );
    drop(state);

    // The concurrently written bytes must be present in the destination.
    expected[d1_offset as usize..d1_offset as usize + d1.len()].copy_from_slice(&d1);

    let dest = std::fs::read(dest_dir.path().join("vol-e2e-dirty.img"))
        .expect("receiving volume file must exist");
    assert_eq!(
        dest, expected,
        "destination must contain the bulk image plus both dirty writes"
    );
}

/// **Pause handshake**: with the task registered in a `MigrationTaskTable`
/// (the stord handler path), the sender must reach `PausedFinalSync`, set
/// `needs_vm_pause`, and stay blocked until the pause is signaled — then
/// complete and match the source.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pause_handshake_releases_final_sync() {
    install_crypto_provider();

    let src_dir = tempfile::tempdir().unwrap();
    let dest_dir = tempfile::tempdir().unwrap();

    let expected = write_source_image(src_dir.path(), "vol.img", 2);

    let src_backend = Arc::new(LocalFileBackend::new(src_dir.path().to_path_buf()));
    let volume_id = "vol-e2e-pause".to_string();
    let handle = open_source_volume(&src_backend, &volume_id, "vol.img", 2 * BLOCK).await;

    let (addr, ca) = spawn_receiver(dest_dir.path()).await;

    // Register the task in the table exactly like `TriggerDiskMigration`
    // does, and drive the sender from the table entry.
    let table = MigrationTaskTable::new();
    let (task, _pause_rx) =
        MigrationTask::new(volume_id.clone(), handle.clone(), format!("https://{addr}"));
    table.insert("mig-e2e-pause".to_string(), task.clone());
    let task = table.get("mig-e2e-pause").unwrap();

    let sender = MigrationSender::new(src_backend, volume_id, handle)
        .with_tls(sender_tls(&ca))
        .with_task(task.clone());
    let endpoint = format!("https://{addr}");
    let migration = tokio::spawn(async move {
        tokio::time::timeout(MIGRATION_TIMEOUT, sender.start_migration(endpoint))
            .await
            .expect("migration must not hang")
            .expect("migration must succeed");
    });

    // Wait for the pause request (the sender is about to send FinalSync).
    let deadline = tokio::time::Instant::now() + MIGRATION_TIMEOUT;
    loop {
        let state = task.state.read().await;
        if state.needs_vm_pause {
            assert_eq!(
                state.phase,
                MigrationPhase::PausedFinalSync,
                "needs_vm_pause must be observed in PausedFinalSync"
            );
            break;
        }
        assert_ne!(
            state.phase,
            MigrationPhase::Completed,
            "sender must not complete before the VM pause is signaled"
        );
        assert_ne!(
            state.phase,
            MigrationPhase::Failed,
            "migration failed while waiting for pause: {}",
            state.error_message
        );
        drop(state);
        assert!(
            tokio::time::Instant::now() < deadline,
            "sender never requested the VM pause (stuck in bulk/dirty sync?)"
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }

    // The sender must genuinely be blocked on the pause: give it a wide
    // window to (incorrectly) proceed on its own.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        !migration.is_finished(),
        "sender must stay blocked in PausedFinalSync until the pause is signaled"
    );
    assert_eq!(
        task.state.read().await.phase,
        MigrationPhase::PausedFinalSync
    );

    // Simulate `ResumeDiskMigration{vm_paused: true}`.
    task.pause_tx.send(true).unwrap();

    tokio::time::timeout(MIGRATION_TIMEOUT, migration)
        .await
        .expect("migration future must not hang")
        .expect("migration task must not panic");
    assert_eq!(task.state.read().await.phase, MigrationPhase::Completed);

    let dest = std::fs::read(dest_dir.path().join("vol-e2e-pause.img"))
        .expect("receiving volume file must exist");
    assert_eq!(dest, expected, "destination must match the source bytes");
}
