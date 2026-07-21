use super::*;

fn file(device: u64, inode: u64) -> FileIdentity {
    FileIdentity { device, inode }
}

fn marker(nonce: &str) -> OwnerMarkerV1 {
    OwnerMarkerV1 {
        schema_version: MARKER_SCHEMA_VERSION,
        host_id: HostId::new("host-1").unwrap(),
        vm_id: VmId::new("vm-1").unwrap(),
        operation_id: OperationId::new("op-1").unwrap(),
        runtime_generation: "018f6f20-7b6d-7d10-8000-000000000001".to_owned(),
        active_attempt_token: "attempt-token-1".to_owned(),
        config_fingerprint: "a".repeat(64),
        publication_nonce: nonce.to_owned(),
        pid: 123,
        proc_start_ticks: 456,
        boot_id: "018f6f20-7b6d-7d10-8000-000000000002".to_owned(),
        executable: file(1, 10),
        uid: unsafe { libc::geteuid() },
        gid: unsafe { libc::getegid() },
        cgroup_fingerprint: "/cellhv/vm-1".to_owned(),
        runtime_directory_name: "vm-1".to_owned(),
        api_socket_name: "vm.sock".to_owned(),
        runtime_directory: file(1, 20),
        api_socket: file(1, 30),
    }
}

#[cfg(target_os = "linux")]
mod linux_tests {
    use super::*;
    use crate::linux::{MarkerStore, StoreError};
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::{symlink, OpenOptionsExt, PermissionsExt};
    use std::path::Path;
    use std::sync::{Arc, Barrier};

    fn store() -> (tempfile::TempDir, MarkerStore) {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let store = MarkerStore::open(directory.path()).unwrap();
        (directory, store)
    }

    fn write_owner(path: &Path, bytes: &[u8], mode: u32) {
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(path.join("owner-v1.json"))
            .unwrap();
        output.write_all(bytes).unwrap();
    }

    #[test]
    fn unsafe_root_and_path_capable_nonce_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        assert!(matches!(
            MarkerStore::open(directory.path()),
            Err(StoreError::UnsafeRoot)
        ));
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let store = MarkerStore::open(directory.path()).unwrap();
        for nonce in [
            "../../outside-marker",
            "sixteen/chars-ok",
            "................",
        ] {
            let mut value = marker("safe-publication-nonce");
            value.publication_nonce = nonce.to_owned();
            assert!(matches!(
                store.publish(&value),
                Err(StoreError::Marker(MarkerError::Token))
            ));
        }
    }

    #[test]
    fn intermediate_symlink_in_runtime_root_is_rejected() {
        let outer = tempfile::tempdir().unwrap();
        let real = outer.path().join("real");
        let child = real.join("private");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::set_permissions(&child, std::fs::Permissions::from_mode(0o700)).unwrap();
        symlink(&real, outer.path().join("linked")).unwrap();
        assert!(matches!(
            MarkerStore::open(&outer.path().join("linked/private")),
            Err(StoreError::UnsafeRoot)
        ));
    }

    #[test]
    fn hostile_marker_kinds_sizes_links_and_modes_fail_closed_without_blocking() {
        type HostileMarker = Box<dyn Fn(&Path)>;
        let cases: Vec<HostileMarker> = vec![
            Box::new(|root| {
                let target = root.join("target");
                std::fs::write(&target, b"{}").unwrap();
                symlink(target, root.join("owner-v1.json")).unwrap();
            }),
            Box::new(|root| {
                let name = std::ffi::CString::new(
                    root.join("owner-v1.json").as_os_str().as_encoded_bytes(),
                )
                .unwrap();
                assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
            }),
            Box::new(|root| {
                write_owner(
                    root,
                    &serde_json::to_vec(&marker("hardlink-publication")).unwrap(),
                    0o600,
                );
                std::fs::hard_link(root.join("owner-v1.json"), root.join("second-link")).unwrap();
            }),
            Box::new(|root| write_owner(root, &vec![b'x'; 16 * 1024 + 1], 0o600)),
            Box::new(|root| {
                write_owner(
                    root,
                    &serde_json::to_vec(&marker("wrongmode-publication")).unwrap(),
                    0o640,
                )
            }),
        ];
        for create in cases {
            let (directory, store) = store();
            create(directory.path());
            assert!(store.read().is_err());
        }
    }

    #[test]
    fn temp_collision_is_preserved_and_concurrent_publish_has_one_winner() {
        let (directory, store) = store();
        let value = marker("collision-publication");
        let temp = directory.path().join(".owner-v1.collision-publication.tmp");
        std::fs::write(&temp, b"do-not-replace").unwrap();
        assert!(matches!(store.publish(&value), Err(StoreError::Exists)));
        assert_eq!(std::fs::read(&temp).unwrap(), b"do-not-replace");
        std::fs::remove_file(temp).unwrap();

        let store = Arc::new(store);
        let barrier = Arc::new(Barrier::new(3));
        let mut threads = Vec::new();
        for nonce in ["concurrent-owner-one", "concurrent-owner-two"] {
            let store = store.clone();
            let barrier = barrier.clone();
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                store.publish(&marker(nonce))
            }));
        }
        barrier.wait();
        let results: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(StoreError::Exists)))
                .count(),
            1
        );
        assert!(store.read().is_ok());
        assert!(std::fs::read_dir(directory.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
    }

    #[test]
    fn anchored_read_rejects_a_path_swap_after_read() {
        let (directory, store) = store();
        let original = marker("original-publication");
        let replacement = marker("replacement-publication");
        store.publish(&original).unwrap();
        let root = directory.path().to_owned();
        let replacement_bytes = serde_json::to_vec(&replacement).unwrap();
        let result = store.read_named_with_hook("owner-v1.json", move || {
            std::fs::rename(root.join("owner-v1.json"), root.join("old-owner")).unwrap();
            write_owner(&root, &replacement_bytes, 0o600);
        });
        assert!(matches!(result, Err(StoreError::IdentityChanged)));
        assert_eq!(store.read().unwrap(), replacement);
    }

    #[test]
    fn repeated_snapshot_rejects_same_inode_same_length_mutation() {
        let (directory, store) = store();
        let original = marker("snapshot-original-1");
        let replacement = marker("snapshot-replaced-1");
        let replacement_bytes = serde_json::to_vec(&replacement).unwrap();
        store.publish(&original).unwrap();
        let path = directory.path().join("owner-v1.json");
        let result = store.read_named_with_hook("owner-v1.json", move || {
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(path)
                .unwrap();
            file.write_all(&replacement_bytes).unwrap();
            file.sync_all().unwrap();
        });
        assert!(matches!(result, Err(StoreError::IdentityChanged)));
        assert_eq!(store.read().unwrap(), replacement);
    }

    #[test]
    fn second_snapshot_rejects_chmod_and_hardlink_races() {
        {
            let (directory, marker_store) = store();
            let value = marker("chmod-race-marker");
            marker_store.publish(&value).unwrap();
            let path = directory.path().join("owner-v1.json");
            let chmod = marker_store.read_named_with_hook("owner-v1.json", move || {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o640)).unwrap();
            });
            assert!(matches!(chmod, Err(StoreError::IdentityChanged)));
        }

        let (directory, marker_store) = store();
        let value = marker("hardlink-race-owner");
        marker_store.publish(&value).unwrap();
        let root = directory.path().to_owned();
        let hardlink = marker_store.read_named_with_hook("owner-v1.json", move || {
            std::fs::hard_link(root.join("owner-v1.json"), root.join("raced-link")).unwrap();
        });
        assert!(matches!(hardlink, Err(StoreError::IdentityChanged)));
    }

    #[test]
    fn conditional_remove_preserves_a_replacement_detected_after_read() {
        let (directory, store) = store();
        let expected = marker("expected-publication");
        let replacement = marker("replacement-owner-two");
        store.publish(&expected).unwrap();
        let root = directory.path().to_owned();
        let replacement_bytes = serde_json::to_vec(&replacement).unwrap();
        let result = store.remove_if_with_hook(&expected, move || {
            std::fs::rename(root.join("owner-v1.json"), root.join("expected-old")).unwrap();
            write_owner(&root, &replacement_bytes, 0o600);
        });
        assert!(matches!(result, Err(StoreError::IdentityChanged)));
        assert_eq!(store.read().unwrap(), replacement);
        assert!(directory.path().join("expected-old").exists());
        assert!(!directory
            .path()
            .join(".owner-v1.remove.expected-publication.tmp")
            .exists());
    }

    #[test]
    fn matching_remove_deletes_only_the_published_inode() {
        let (directory, store) = store();
        let value = marker("remove-publication");
        store.publish(&value).unwrap();
        store.remove_if(&value).unwrap();
        assert!(!directory.path().join("owner-v1.json").exists());
    }
}

#[derive(Clone)]
struct FakeObservation {
    before: Result<Option<ProcessIdentity>, ()>,
    after: Result<Option<ProcessIdentity>, ()>,
    alive: Result<bool, ()>,
    socket: Result<Option<SocketIdentity>, ()>,
    duplicate: Result<DuplicateEvidence, ()>,
}

impl Observation for FakeObservation {
    type Error = ();
    fn process_before(&self, _: u32) -> Result<Option<ProcessIdentity>, Self::Error> {
        self.before.clone()
    }
    fn socket(&self, _: &VmId) -> Result<Option<SocketIdentity>, Self::Error> {
        self.socket.clone()
    }
    fn process_after(&self, _: u32) -> Result<Option<ProcessIdentity>, Self::Error> {
        self.after.clone()
    }
    fn pidfd_alive(&self, _: u32) -> Result<bool, Self::Error> {
        self.alive
    }
    fn duplicate_evidence(&self, _: &VmId) -> Result<DuplicateEvidence, Self::Error> {
        self.duplicate
    }
}

fn requested() -> RequestedOwner {
    RequestedOwner {
        host_id: HostId::new("host-1").unwrap(),
        vm_id: VmId::new("vm-1").unwrap(),
        operation_id: OperationId::new("op-1").unwrap(),
        runtime_generation: "018f6f20-7b6d-7d10-8000-000000000001".to_owned(),
        active_attempt_token: "attempt-token-1".to_owned(),
        config_fingerprint: "a".repeat(64),
    }
}

fn observation(value: &OwnerMarkerV1) -> FakeObservation {
    let process = ProcessIdentity {
        pid: value.pid,
        start_ticks: value.proc_start_ticks,
        boot_id: value.boot_id.clone(),
        executable: value.executable,
        uid: value.uid,
        gid: value.gid,
        cgroup_fingerprint: value.cgroup_fingerprint.clone(),
    };
    FakeObservation {
        before: Ok(Some(process.clone())),
        after: Ok(Some(process)),
        alive: Ok(true),
        socket: Ok(Some(SocketIdentity {
            runtime_directory: value.runtime_directory,
            socket: value.api_socket,
            peer_pid: value.pid,
            peer_uid: value.uid,
            api_live: true,
        })),
        duplicate: Ok(DuplicateEvidence::Exclusive),
    }
}

#[test]
fn proc_stat_handles_hostile_comm_and_rejects_truncation() {
    let mut tail = vec!["S"; 20];
    tail[19] = "4242";
    let stat = format!("7 (name ) with spaces) {}", tail.join(" "));
    assert_eq!(parse_proc_start_ticks(&stat), Ok(4242));
    assert_eq!(
        parse_proc_start_ticks("7 (x) S"),
        Err(MarkerError::ProcStat)
    );
}

#[test]
fn launch_correlation_and_runtime_names_have_strict_canonical_forms() {
    let base = marker("validate-publication");
    let mut visible_attempt = base.clone();
    visible_attempt.active_attempt_token = "attempt token !#$%".to_owned();
    assert_eq!(visible_attempt.validate(), Ok(()));
    let invalid = [
        {
            let mut value = base.clone();
            value.runtime_generation = "not-a-uuid".to_owned();
            value
        },
        {
            let mut value = base.clone();
            value.active_attempt_token = "bad\ntoken".to_owned();
            value
        },
        {
            let mut value = base.clone();
            value.config_fingerprint = "A".repeat(64);
            value
        },
        {
            let mut value = base.clone();
            value.runtime_directory_name = "..".to_owned();
            value
        },
        {
            let mut value = base.clone();
            value.api_socket_name = "nested/vm.sock".to_owned();
            value
        },
        {
            let mut value = base.clone();
            value.runtime_directory_name = "other-vm".to_owned();
            value
        },
        {
            let mut value = base.clone();
            value.api_socket_name = "api.sock".to_owned();
            value
        },
        {
            let mut value = base;
            value.proc_start_ticks = 0;
            value
        },
    ];
    for value in invalid {
        assert!(value.validate().is_err());
    }
}

#[test]
fn classification_requires_revalidated_process_socket_peer_and_liveness_identity() {
    let value = marker("classify-publication");
    assert!(matches!(
        inspect(&requested(), Ok(value.clone()), &observation(&value)),
        Classification::OwnershipMatched
    ));

    let mut variants: Vec<FakeObservation> = Vec::new();
    for mutate in [
        |p: &mut ProcessIdentity| p.start_ticks += 1,
        |p: &mut ProcessIdentity| p.boot_id.push('x'),
        |p: &mut ProcessIdentity| p.executable.inode += 1,
        |p: &mut ProcessIdentity| p.uid += 1,
        |p: &mut ProcessIdentity| p.gid += 1,
        |p: &mut ProcessIdentity| p.cgroup_fingerprint.push('x'),
    ] {
        let mut item = observation(&value);
        mutate(item.before.as_mut().unwrap().as_mut().unwrap());
        variants.push(item);
    }
    let mut after_changed = observation(&value);
    after_changed
        .after
        .as_mut()
        .unwrap()
        .as_mut()
        .unwrap()
        .start_ticks += 1;
    variants.push(after_changed);
    let mut dead = observation(&value);
    dead.alive = Ok(false);
    variants.push(dead);
    for mutate in [
        |s: &mut SocketIdentity| s.runtime_directory.device += 1,
        |s: &mut SocketIdentity| s.runtime_directory.inode += 1,
        |s: &mut SocketIdentity| s.socket.device += 1,
        |s: &mut SocketIdentity| s.socket.inode += 1,
        |s: &mut SocketIdentity| s.peer_pid += 1,
        |s: &mut SocketIdentity| s.peer_uid += 1,
    ] {
        let mut item = observation(&value);
        mutate(item.socket.as_mut().unwrap().as_mut().unwrap());
        variants.push(item);
    }
    for item in variants {
        assert_eq!(
            inspect(&requested(), Ok(value.clone()), &item),
            Classification::AmbiguousPreserve
        );
    }

    let mut no_api = observation(&value);
    no_api.socket.as_mut().unwrap().as_mut().unwrap().api_live = false;
    assert_eq!(
        inspect(&requested(), Ok(value.clone()), &no_api),
        Classification::OwnedAliveSocketUnavailable
    );
    let mut no_socket = observation(&value);
    no_socket.socket = Ok(None);
    assert_eq!(
        inspect(&requested(), Ok(value.clone()), &no_socket),
        Classification::OwnedAliveSocketUnavailable
    );
    let mut exited = observation(&value);
    exited.before = Ok(None);
    exited.after = Ok(None);
    exited.socket = Ok(None);
    exited.alive = Ok(false);
    assert_eq!(
        inspect(&requested(), Ok(value.clone()), &exited),
        Classification::ExitedOwned
    );
    let mut absent_but_live = exited.clone();
    absent_but_live.alive = Ok(true);
    assert_eq!(
        inspect(&requested(), Ok(value.clone()), &absent_but_live),
        Classification::AmbiguousPreserve
    );
    let mut duplicate = observation(&value);
    duplicate.duplicate = Ok(DuplicateEvidence::Conflict);
    assert_eq!(
        inspect(&requested(), Ok(value.clone()), &duplicate),
        Classification::DuplicateConflict
    );
    let mut indeterminate = observation(&value);
    indeterminate.duplicate = Ok(DuplicateEvidence::Indeterminate);
    assert_eq!(
        inspect(&requested(), Ok(value.clone()), &indeterminate),
        Classification::AmbiguousPreserve
    );
    let mut duplicate_error = observation(&value);
    duplicate_error.duplicate = Err(());
    assert_eq!(
        inspect(&requested(), Ok(value.clone()), &duplicate_error),
        Classification::AmbiguousPreserve
    );
    let mut failed = observation(&value);
    failed.before = Err(());
    assert_eq!(
        inspect(&requested(), Ok(value.clone()), &failed),
        Classification::AmbiguousPreserve
    );
}

#[test]
fn matched_evidence_is_not_stable_across_socket_swap_or_pid_exit() {
    let value = marker("reinspect-publication");
    assert_eq!(
        inspect(&requested(), Ok(value.clone()), &observation(&value)),
        Classification::OwnershipMatched
    );
    let mut socket_swapped = observation(&value);
    socket_swapped
        .socket
        .as_mut()
        .unwrap()
        .as_mut()
        .unwrap()
        .socket
        .inode += 1;
    assert_eq!(
        inspect(&requested(), Ok(value.clone()), &socket_swapped),
        Classification::AmbiguousPreserve
    );
    let mut exited = observation(&value);
    exited.after = Ok(None);
    exited.alive = Ok(false);
    assert_eq!(
        inspect(&requested(), Ok(value), &exited),
        Classification::AmbiguousPreserve
    );
}

#[test]
fn foreign_and_corrupt_markers_never_reach_observation_adoption() {
    let value = marker("foreign-publication");
    let mut foreign = value.clone();
    foreign.host_id = HostId::new("other-host").unwrap();
    assert_eq!(
        inspect(&requested(), Ok(foreign), &observation(&value)),
        Classification::ForeignConflict
    );
    for mutate in [
        |request: &mut RequestedOwner| request.runtime_generation.push('0'),
        |request: &mut RequestedOwner| request.active_attempt_token.push('0'),
        |request: &mut RequestedOwner| request.config_fingerprint.replace_range(..1, "b"),
    ] {
        let mut request = requested();
        mutate(&mut request);
        assert_eq!(
            inspect(&request, Ok(value.clone()), &observation(&value)),
            Classification::ForeignConflict
        );
    }
    let mut corrupt = value.clone();
    corrupt.pid = 0;
    assert_eq!(
        inspect(&requested(), Ok(corrupt), &observation(&value)),
        Classification::CorruptOwnership
    );
}
