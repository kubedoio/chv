//! Filesystem exclusion protocols for the single CellHV Core authority.
//!
//! [`AuthorityLock`] serializes short NodeCache save/cutover transactions.
//! [`RuntimeAuthorityLease`] is a distinct, non-blocking process-lifetime lease
//! for the future production Core authority. Holding one does not replace the
//! short transaction lock and neither primitive is wired into `chv-agent` here.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// An exclusive process-lifetime lease for one Core database authority.
///
/// The lease is released by the kernel when this value is dropped or its
/// process exits. Acquisition never waits: an already-running authority is
/// reported as [`io::ErrorKind::WouldBlock`].
#[derive(Debug)]
pub struct RuntimeAuthorityLease {
    _file: File,
    path: PathBuf,
}

impl RuntimeAuthorityLease {
    pub fn acquire(core_database: &Path) -> io::Result<Self> {
        acquire_runtime_lease(core_database, |_| {})
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn acquire_runtime_lease(
    core_database: &Path,
    after_lock: impl FnOnce(&Path),
) -> io::Result<RuntimeAuthorityLease> {
    let path = runtime_lease_path(core_database)?;
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "runtime lease has no parent")
    })?;
    validate_private_parent(parent)?;

    if paths_alias(core_database, &path)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime lease aliases the Core database",
        ));
    }
    validate_existing_regular_file(&path, "runtime lease")?;

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)?;
    validate_open_file(&file, "runtime lease")?;

    // SAFETY: `file` owns a valid descriptor for at least the lease lifetime.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let error = io::Error::last_os_error();
        return if matches!(error.raw_os_error(), Some(code) if code == libc::EAGAIN || code == libc::EACCES)
        {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "another CellHV Core authority holds the runtime lease",
            ))
        } else {
            Err(error)
        };
    }

    after_lock(&path);
    validate_path_matches_file(&path, &file)?;

    Ok(RuntimeAuthorityLease { _file: file, path })
}

pub struct AuthorityLock {
    _file: File,
}

impl AuthorityLock {
    /// Takes the exclusive lock shared by NodeCache saves and Core cutover.
    pub fn acquire(node_cache: &Path) -> io::Result<Self> {
        let path = lock_path(node_cache)?;
        let parent = path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "authority lock has no parent")
        })?;
        let parent_metadata = fs::symlink_metadata(parent)?;
        if !parent_metadata.file_type().is_dir()
            || parent_metadata.uid() != unsafe { libc::geteuid() }
            || parent_metadata.permissions().mode() & 0o022 != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "authority lock parent must be owner-owned and not writable by group or others",
            ));
        }
        match fs::symlink_metadata(&path) {
            Ok(metadata) if !metadata.file_type().is_file() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "authority lock must be a regular file",
                ));
            }
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)?;
        let metadata = file.metadata()?;
        if !metadata.file_type().is_file()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.permissions().mode() & 0o777 != 0o600
            || metadata.nlink() != 1
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "authority lock must be owner-owned 0600 regular file with one link",
            ));
        }
        // SAFETY: flock only observes the valid descriptor for the lifetime of `file`.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { _file: file })
    }
}

pub fn lock_path(node_cache: &Path) -> io::Result<PathBuf> {
    let name = node_cache.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "NodeCache path has no file name",
        )
    })?;
    Ok(node_cache.with_file_name(format!(".{}.cellhv-authority.lock", name.to_string_lossy())))
}

/// Derives the runtime authority lease beside the Core database.
pub fn runtime_lease_path(core_database: &Path) -> io::Result<PathBuf> {
    let name = core_database.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Core database path has no file name",
        )
    })?;
    Ok(core_database.with_file_name(format!(
        ".{}.cellhv-runtime-authority.lease",
        name.to_string_lossy()
    )))
}

/// Returns whether two configured paths identify the same lexical path or,
/// when both exist, the same filesystem inode.
pub fn paths_alias(left: &Path, right: &Path) -> io::Result<bool> {
    if normalize_lexically(left)? == normalize_lexically(right)? {
        return Ok(true);
    }
    match (optional_metadata(left)?, optional_metadata(right)?) {
        (Some(left), Some(right)) => Ok(left.dev() == right.dev() && left.ino() == right.ino()),
        _ => Ok(false),
    }
}

fn optional_metadata(path: &Path) -> io::Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn normalize_lexically(path: &Path) -> io::Result<PathBuf> {
    use std::path::Component;

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "path escapes its filesystem root",
                    ));
                }
            }
            Component::Normal(value) => normalized.push(value),
        }
    }
    Ok(normalized)
}

fn validate_private_parent(parent: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::geteuid() }
        || (metadata.permissions().mode() & 0o777 != 0o700
            && metadata.permissions().mode() & 0o777 != 0o750)
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "runtime lease parent must be a real owner-owned 0700 or 0750 directory",
        ));
    }
    Ok(())
}

fn validate_existing_regular_file(path: &Path, label: &str) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if !metadata.file_type().is_file()
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.permissions().mode() & 0o777 != 0o600
                || metadata.nlink() != 1 =>
        {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{label} must be an owner-owned 0600 regular file with one link"),
            ))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn validate_open_file(file: &File, label: &str) -> io::Result<()> {
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{label} must be an owner-owned 0600 regular file with one link"),
        ));
    }
    Ok(())
}

fn validate_path_matches_file(path: &Path, file: &File) -> io::Result<()> {
    let path_metadata = fs::symlink_metadata(path)?;
    let file_metadata = file.metadata()?;
    if path_metadata.dev() != file_metadata.dev() || path_metadata.ino() != file_metadata.ino() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "runtime lease pathname changed while acquiring the lock",
        ));
    }
    if !path_metadata.file_type().is_file()
        || path_metadata.uid() != unsafe { libc::geteuid() }
        || path_metadata.permissions().mode() & 0o777 != 0o600
        || path_metadata.nlink() != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "runtime lease pathname must identify the locked owner-owned 0600 regular file with one link",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn rejects_symlink_lock_and_unsafe_parent() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache.json");
        let target = dir.path().join("target");
        fs::write(&target, b"").unwrap();
        symlink(&target, lock_path(&cache).unwrap()).unwrap();
        assert!(AuthorityLock::acquire(&cache).is_err());

        let unsafe_dir = tempfile::tempdir().unwrap();
        fs::set_permissions(unsafe_dir.path(), fs::Permissions::from_mode(0o777)).unwrap();
        assert!(AuthorityLock::acquire(&unsafe_dir.path().join("cache.json")).is_err());
    }

    fn private_directory() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        directory
    }

    fn lease_child(database: &Path, expectation: &str) -> std::process::ExitStatus {
        static NEXT_MARKER: AtomicU64 = AtomicU64::new(0);
        let marker = database.with_extension(format!(
            "lease-child-{}-{}",
            std::process::id(),
            NEXT_MARKER.fetch_add(1, Ordering::Relaxed)
        ));
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::runtime_lease_subprocess_helper",
                "--nocapture",
            ])
            .env("CELLHV_LEASE_TEST_DATABASE", database)
            .env("CELLHV_LEASE_TEST_EXPECT", expectation)
            .env("CELLHV_LEASE_TEST_MARKER", &marker)
            .status()
            .unwrap();
        assert_eq!(
            fs::read_to_string(&marker).unwrap_or_else(|error| panic!(
                "lease child helper did not write {}: {error}",
                marker.display()
            )),
            format!("{expectation}\n")
        );
        fs::remove_file(marker).unwrap();
        status
    }

    #[test]
    fn runtime_lease_subprocess_helper() {
        let Ok(database) = std::env::var("CELLHV_LEASE_TEST_DATABASE") else {
            return;
        };
        let expectation = std::env::var("CELLHV_LEASE_TEST_EXPECT").unwrap();
        let marker = std::env::var("CELLHV_LEASE_TEST_MARKER").unwrap();
        let result = RuntimeAuthorityLease::acquire(Path::new(&database));
        match expectation.as_str() {
            "acquired" => drop(result.unwrap()),
            "exit-held" => std::mem::forget(result.unwrap()),
            "blocked" => assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock),
            other => panic!("unknown lease test expectation {other}"),
        }
        fs::write(marker, format!("{expectation}\n")).unwrap();
    }

    #[test]
    fn runtime_lease_contends_across_processes_and_is_reusable_after_exit() {
        let directory = private_directory();
        let database = directory.path().join("core.db");
        let lease = RuntimeAuthorityLease::acquire(&database).unwrap();
        assert_eq!(lease.path(), runtime_lease_path(&database).unwrap());
        let persistent_inode = fs::symlink_metadata(lease.path()).unwrap().ino();
        assert_eq!(
            RuntimeAuthorityLease::acquire(&database)
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(lease_child(&database, "blocked").success());
        drop(lease);
        assert_eq!(
            fs::symlink_metadata(runtime_lease_path(&database).unwrap())
                .unwrap()
                .ino(),
            persistent_inode
        );

        assert!(lease_child(&database, "exit-held").success());
        assert_eq!(
            fs::symlink_metadata(runtime_lease_path(&database).unwrap())
                .unwrap()
                .ino(),
            persistent_inode
        );
        RuntimeAuthorityLease::acquire(&database).unwrap();
    }

    #[test]
    fn runtime_lease_rejects_path_replacement_after_lock() {
        let directory = private_directory();
        let database = directory.path().join("core.db");
        let displaced = directory.path().join("displaced.lease");
        let result = acquire_runtime_lease(&database, |lease_path| {
            fs::rename(lease_path, &displaced).unwrap();
            let replacement = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(lease_path)
                .unwrap();
            replacement.sync_all().unwrap();
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_ne!(
            fs::symlink_metadata(runtime_lease_path(&database).unwrap())
                .unwrap()
                .ino(),
            fs::symlink_metadata(displaced).unwrap().ino()
        );
    }

    #[test]
    fn runtime_lease_rejects_unsafe_files_and_detects_aliases() {
        let directory = private_directory();
        let database = directory.path().join("core.db");
        let lease_path = runtime_lease_path(&database).unwrap();
        let target = directory.path().join("target");
        fs::write(&target, b"").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&target, &lease_path).unwrap();
        assert!(RuntimeAuthorityLease::acquire(&database).is_err());
        fs::remove_file(&lease_path).unwrap();

        fs::hard_link(&target, &lease_path).unwrap();
        assert!(RuntimeAuthorityLease::acquire(&database).is_err());
        assert!(paths_alias(&target, &lease_path).unwrap());
        assert!(paths_alias(&directory.path().join("nested/../target"), &target).unwrap());

        fs::remove_file(&lease_path).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
        fs::rename(&target, &lease_path).unwrap();
        assert!(RuntimeAuthorityLease::acquire(&database).is_err());

        let unsafe_directory = tempfile::tempdir().unwrap();
        fs::set_permissions(unsafe_directory.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(RuntimeAuthorityLease::acquire(&unsafe_directory.path().join("core.db")).is_err());
    }
}
