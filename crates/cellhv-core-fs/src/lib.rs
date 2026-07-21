//! Shared filesystem exclusion protocol for NodeCache authority transitions.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};

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
}
