//! Runtime-neutral ownership evidence and classification. Deliberately unwired.

use cellhv_core_types::{HostId, OperationId, VmId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(target_os = "linux")]
pub mod linux {
    use super::*;
    use std::ffi::CString;
    use std::fs::File;
    use std::io::{Read, Seek, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;

    const MAX_MARKER_BYTES: u64 = 16 * 1024;

    #[derive(Debug, Error)]
    pub enum StoreError {
        #[error("runtime root is not a private real directory")]
        UnsafeRoot,
        #[error("unsafe marker file")]
        UnsafeMarker,
        #[error("marker already exists")]
        Exists,
        #[error("marker identity changed")]
        IdentityChanged,
        #[error(transparent)]
        Io(#[from] std::io::Error),
        #[error(transparent)]
        Json(#[from] serde_json::Error),
        #[error(transparent)]
        Marker(#[from] MarkerError),
    }

    pub struct MarkerStore {
        root: File,
        uid: u32,
    }
    impl MarkerStore {
        pub fn open(path: &Path) -> Result<Self, StoreError> {
            use std::path::Component;
            if path.as_os_str().is_empty() {
                return Err(StoreError::UnsafeRoot);
            }
            let absolute = path.is_absolute();
            let base = CString::new(if absolute { "/" } else { "." }).unwrap();
            let fd = unsafe {
                libc::open(
                    base.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                )
            };
            if fd < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let mut root = unsafe { File::from_raw_fd(fd) };
            for component in path.components() {
                let Component::Normal(component) = component else {
                    if matches!(component, Component::RootDir) {
                        continue;
                    }
                    return Err(StoreError::UnsafeRoot);
                };
                let component = CString::new(component.as_encoded_bytes())
                    .map_err(|_| StoreError::UnsafeRoot)?;
                let next = unsafe {
                    libc::openat(
                        root.as_raw_fd(),
                        component.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                    )
                };
                if next < 0 {
                    return Err(StoreError::UnsafeRoot);
                }
                root = unsafe { File::from_raw_fd(next) };
            }
            let metadata = root.metadata()?;
            let uid = unsafe { libc::geteuid() };
            if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
                return Err(StoreError::UnsafeRoot);
            }
            Ok(Self { root, uid })
        }
        fn open_marker(&self, flags: i32, mode: u32, name: &str) -> Result<File, StoreError> {
            let name = CString::new(name).map_err(|_| StoreError::UnsafeMarker)?;
            let fd = unsafe {
                libc::openat(
                    self.root.as_raw_fd(),
                    name.as_ptr(),
                    flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                    mode,
                )
            };
            if fd < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(unsafe { File::from_raw_fd(fd) })
        }
        pub fn publish(&self, marker: &OwnerMarkerV1) -> Result<(), StoreError> {
            marker.validate()?;
            let bytes = serde_json::to_vec(marker)?;
            if bytes.len() as u64 > MAX_MARKER_BYTES {
                return Err(StoreError::UnsafeMarker);
            }
            let temp = format!(".owner-v1.{}.tmp", marker.publication_nonce);
            let mut file =
                match self.open_marker(libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL, 0o600, &temp)
                {
                    Ok(file) => file,
                    Err(StoreError::Io(error)) if error.raw_os_error() == Some(libc::EEXIST) => {
                        return Err(StoreError::Exists)
                    }
                    Err(error) => return Err(error),
                };
            if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
                let name = CString::new(temp).expect("validated nonce produces a C string");
                unsafe { libc::unlinkat(self.root.as_raw_fd(), name.as_ptr(), 0) };
                return Err(error.into());
            }
            let from = CString::new(temp.clone()).unwrap();
            let to = CString::new("owner-v1.json").unwrap();
            let rc = unsafe {
                libc::syscall(
                    libc::SYS_renameat2,
                    self.root.as_raw_fd(),
                    from.as_ptr(),
                    self.root.as_raw_fd(),
                    to.as_ptr(),
                    libc::RENAME_NOREPLACE,
                )
            };
            if rc < 0 {
                let error = std::io::Error::last_os_error();
                unsafe { libc::unlinkat(self.root.as_raw_fd(), from.as_ptr(), 0) };
                return if error.raw_os_error() == Some(libc::EEXIST) {
                    Err(StoreError::Exists)
                } else {
                    Err(error.into())
                };
            }
            self.root.sync_all()?;
            Ok(())
        }
        fn read_named(&self, name: &str) -> Result<(OwnerMarkerV1, FileIdentity), StoreError> {
            self.read_named_with_hook(name, || {})
        }
        pub(crate) fn read_named_with_hook<F: FnOnce()>(
            &self,
            name: &str,
            before_revalidate: F,
        ) -> Result<(OwnerMarkerV1, FileIdentity), StoreError> {
            let mut file = self.open_marker(libc::O_RDONLY | libc::O_NONBLOCK, 0, name)?;
            let before = file.metadata()?;
            if !before.is_file()
                || before.nlink() != 1
                || before.uid() != self.uid
                || before.mode() & 0o777 != 0o600
                || before.len() > MAX_MARKER_BYTES
            {
                return Err(StoreError::UnsafeMarker);
            }
            let mut bytes = Vec::new();
            std::io::Read::by_ref(&mut file)
                .take(MAX_MARKER_BYTES + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_MARKER_BYTES {
                return Err(StoreError::UnsafeMarker);
            }
            let marker: OwnerMarkerV1 = serde_json::from_slice(&bytes)?;
            marker.validate()?;
            before_revalidate();
            file.rewind()?;
            let mut repeated = Vec::new();
            std::io::Read::by_ref(&mut file)
                .take(MAX_MARKER_BYTES + 1)
                .read_to_end(&mut repeated)?;
            let after = file.metadata()?;
            if before.dev() != after.dev()
                || before.ino() != after.ino()
                || before.len() != after.len()
                || bytes != repeated
                || !after.is_file()
                || after.nlink() != 1
                || after.uid() != self.uid
                || after.mode() & 0o777 != 0o600
            {
                return Err(StoreError::IdentityChanged);
            }
            let name = CString::new(name).map_err(|_| StoreError::UnsafeMarker)?;
            let mut path_stat = std::mem::MaybeUninit::<libc::stat>::uninit();
            if unsafe {
                libc::fstatat(
                    self.root.as_raw_fd(),
                    name.as_ptr(),
                    path_stat.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } < 0
            {
                return Err(StoreError::IdentityChanged);
            }
            let path_stat = unsafe { path_stat.assume_init() };
            if path_stat.st_dev != after.dev()
                || path_stat.st_ino != after.ino()
                || path_stat.st_size as u64 != after.len()
                || path_stat.st_nlink != 1
                || path_stat.st_uid != self.uid
                || path_stat.st_mode & libc::S_IFMT != libc::S_IFREG
                || path_stat.st_mode & 0o777 != 0o600
            {
                return Err(StoreError::IdentityChanged);
            }
            Ok((
                marker,
                FileIdentity {
                    device: after.dev(),
                    inode: after.ino(),
                },
            ))
        }
        pub fn read(&self) -> Result<OwnerMarkerV1, StoreError> {
            self.read_named("owner-v1.json").map(|(marker, _)| marker)
        }
        pub fn remove_if(&self, expected: &OwnerMarkerV1) -> Result<(), StoreError> {
            self.remove_if_with_hook(expected, || {})
        }
        pub(crate) fn remove_if_with_hook<F: FnOnce()>(
            &self,
            expected: &OwnerMarkerV1,
            before_quarantine: F,
        ) -> Result<(), StoreError> {
            expected.validate()?;
            let (actual, identity) = self.read_named("owner-v1.json")?;
            if &actual != expected {
                return Err(StoreError::IdentityChanged);
            }
            before_quarantine();
            let source = CString::new("owner-v1.json").unwrap();
            let tomb_name = format!(".owner-v1.remove.{}.tmp", expected.publication_nonce);
            let tomb =
                CString::new(tomb_name.clone()).expect("validated nonce produces a C string");
            let moved = unsafe {
                libc::syscall(
                    libc::SYS_renameat2,
                    self.root.as_raw_fd(),
                    source.as_ptr(),
                    self.root.as_raw_fd(),
                    tomb.as_ptr(),
                    libc::RENAME_NOREPLACE,
                )
            };
            if moved < 0 {
                let error = std::io::Error::last_os_error();
                return if error.raw_os_error() == Some(libc::EEXIST) {
                    Err(StoreError::Exists)
                } else {
                    Err(error.into())
                };
            }
            let moved_matches = self
                .read_named(&tomb_name)
                .map(|(marker, moved_identity)| marker == *expected && moved_identity == identity)
                .unwrap_or(false);
            if !moved_matches {
                let restored = unsafe {
                    libc::syscall(
                        libc::SYS_renameat2,
                        self.root.as_raw_fd(),
                        tomb.as_ptr(),
                        self.root.as_raw_fd(),
                        source.as_ptr(),
                        libc::RENAME_NOREPLACE,
                    )
                };
                if restored < 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
                return Err(StoreError::IdentityChanged);
            }
            if unsafe { libc::unlinkat(self.root.as_raw_fd(), tomb.as_ptr(), 0) } < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            self.root.sync_all()?;
            Ok(())
        }

        pub fn supersede(
            &self,
            expected: &OwnerMarkerV1,
            replacement: &OwnerMarkerV1,
            decision: RecoveryDecision,
        ) -> Result<(), StoreError> {
            expected.validate()?;
            replacement.validate()?;
            if expected == replacement {
                return Ok(());
            }

            let (actual, _) = self.read_named("owner-v1.json")?;
            if &actual != expected {
                return Err(StoreError::IdentityChanged);
            }

            match decision {
                RecoveryDecision::SupersedeExited => {}
                RecoveryDecision::SupersedeActive => {
                    if expected.host_id != replacement.host_id {
                        return Err(StoreError::IdentityChanged);
                    }
                }
            }

            let bytes = serde_json::to_vec(replacement)?;
            if bytes.len() as u64 > MAX_MARKER_BYTES {
                return Err(StoreError::UnsafeMarker);
            }

            let temp = format!(".owner-v1.{}.tmp", replacement.publication_nonce);
            let mut file =
                match self.open_marker(libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL, 0o600, &temp)
                {
                    Ok(file) => file,
                    Err(StoreError::Io(error)) if error.raw_os_error() == Some(libc::EEXIST) => {
                        return Err(StoreError::Exists)
                    }
                    Err(error) => return Err(error),
                };

            if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
                let name = CString::new(temp).expect("validated nonce produces a C string");
                unsafe { libc::unlinkat(self.root.as_raw_fd(), name.as_ptr(), 0) };
                return Err(error.into());
            }

            let from = CString::new(temp.clone()).unwrap();
            let to = CString::new("owner-v1.json").unwrap();

            let rc = unsafe {
                libc::syscall(
                    libc::SYS_renameat2,
                    self.root.as_raw_fd(),
                    from.as_ptr(),
                    self.root.as_raw_fd(),
                    to.as_ptr(),
                    libc::RENAME_EXCHANGE,
                )
            };

            if rc < 0 {
                let error = std::io::Error::last_os_error();
                unsafe { libc::unlinkat(self.root.as_raw_fd(), from.as_ptr(), 0) };
                return Err(error.into());
            }

            let moved_matches = self
                .read_named(&temp)
                .map(|(marker, _)| marker == *expected)
                .unwrap_or(false);

            if !moved_matches {
                let restored = unsafe {
                    libc::syscall(
                        libc::SYS_renameat2,
                        self.root.as_raw_fd(),
                        to.as_ptr(),
                        self.root.as_raw_fd(),
                        from.as_ptr(),
                        libc::RENAME_EXCHANGE,
                    )
                };
                if restored < 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
                unsafe { libc::unlinkat(self.root.as_raw_fd(), from.as_ptr(), 0) };
                return Err(StoreError::IdentityChanged);
            }

            unsafe { libc::unlinkat(self.root.as_raw_fd(), from.as_ptr(), 0) };
            self.root.sync_all()?;
            Ok(())
        }
    }
}

pub const MARKER_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryDecision {
    /// Safe to overwrite because the existing attempt has definitely terminated.
    SupersedeExited,
    /// Overwrite an attempt that may still be active. Requires ownership intent.
    SupersedeActive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileIdentity {
    pub device: u64,
    pub inode: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerMarkerV1 {
    pub schema_version: u16,
    pub host_id: HostId,
    pub vm_id: VmId,
    pub operation_id: OperationId,
    pub runtime_generation: String,
    pub active_attempt_token: String,
    pub config_fingerprint: String,
    pub publication_nonce: String,
    pub pid: u32,
    pub proc_start_ticks: u64,
    pub boot_id: String,
    pub executable: FileIdentity,
    pub uid: u32,
    pub gid: u32,
    pub cgroup_fingerprint: String,
    pub runtime_directory_name: String,
    pub api_socket_name: String,
    pub runtime_directory: FileIdentity,
    pub api_socket: FileIdentity,
}

impl OwnerMarkerV1 {
    pub fn validate(&self) -> Result<(), MarkerError> {
        if self.schema_version != MARKER_SCHEMA_VERSION {
            return Err(MarkerError::Schema);
        }
        validate_uuid(&self.runtime_generation)?;
        validate_visible_ascii(&self.active_attempt_token, 1, 128)?;
        validate_hex(&self.config_fingerprint, 64)?;
        validate_component(&self.publication_nonce, 16, 128)?;
        validate_uuid(&self.boot_id)?;
        validate_cgroup(&self.cgroup_fingerprint)?;
        validate_basename(&self.runtime_directory_name, 1, 128)?;
        validate_basename(&self.api_socket_name, 1, 128)?;
        if self.runtime_directory_name != self.vm_id.as_str() || self.api_socket_name != "vm.sock" {
            return Err(MarkerError::Identity);
        }
        if self.pid == 0
            || self.proc_start_ticks == 0
            || [self.executable, self.runtime_directory, self.api_socket]
                .iter()
                .any(|x| x.inode == 0)
        {
            return Err(MarkerError::Identity);
        }
        Ok(())
    }
}

fn validate_visible_ascii(value: &str, min: usize, max: usize) -> Result<(), MarkerError> {
    if !(min..=max).contains(&value.len())
        || !value.bytes().all(|byte| (b' '..=b'~').contains(&byte))
    {
        return Err(MarkerError::Token);
    }
    Ok(())
}

fn validate_component(value: &str, min: usize, max: usize) -> Result<(), MarkerError> {
    if !(min..=max).contains(&value.len())
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(MarkerError::Token);
    }
    Ok(())
}

fn validate_basename(value: &str, min: usize, max: usize) -> Result<(), MarkerError> {
    if value == "."
        || value == ".."
        || !(min..=max).contains(&value.len())
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(MarkerError::Token);
    }
    Ok(())
}

fn validate_uuid(value: &str) -> Result<(), MarkerError> {
    if value.len() != 36
        || value.bytes().enumerate().any(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte != b'-'
            } else {
                !byte.is_ascii_hexdigit()
            }
        })
    {
        return Err(MarkerError::Token);
    }
    Ok(())
}

fn validate_hex(value: &str, length: usize) -> Result<(), MarkerError> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(MarkerError::Token);
    }
    Ok(())
}

fn validate_cgroup(value: &str) -> Result<(), MarkerError> {
    if value.len() > 256
        || !value.starts_with('/')
        || value.contains("//")
        || value.split('/').any(|part| part == "." || part == "..")
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b':'))
    {
        return Err(MarkerError::Token);
    }
    Ok(())
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum MarkerError {
    #[error("unsupported marker schema")]
    Schema,
    #[error("invalid bounded token")]
    Token,
    #[error("invalid identity")]
    Identity,
    #[error("invalid proc stat")]
    ProcStat,
}

/// Parse Linux `/proc/PID/stat` field 22 without trusting spaces or parentheses in comm.
pub fn parse_proc_start_ticks(stat: &str) -> Result<u64, MarkerError> {
    let close = stat.rfind(')').ok_or(MarkerError::ProcStat)?;
    let prefix = &stat[..close];
    if !prefix.contains('(') {
        return Err(MarkerError::ProcStat);
    }
    // Fields following comm begin at field 3; starttime is the twentieth token.
    stat[close + 1..]
        .split_ascii_whitespace()
        .nth(19)
        .ok_or(MarkerError::ProcStat)?
        .parse()
        .map_err(|_| MarkerError::ProcStat)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub start_ticks: u64,
    pub boot_id: String,
    pub executable: FileIdentity,
    pub uid: u32,
    pub gid: u32,
    pub cgroup_fingerprint: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketIdentity {
    pub runtime_directory: FileIdentity,
    pub socket: FileIdentity,
    pub peer_pid: u32,
    pub peer_uid: u32,
    pub api_live: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestedOwner {
    pub host_id: HostId,
    pub vm_id: VmId,
    pub operation_id: OperationId,
    pub runtime_generation: String,
    pub active_attempt_token: String,
    pub config_fingerprint: String,
}

/// Result of checking whether another process may own the VM.
///
/// `Exclusive` is a positive proof, not merely the absence of a candidate in
/// a best-effort process scan. Implementations that cannot establish that
/// proof must return `Indeterminate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateEvidence {
    Conflict,
    Exclusive,
    Indeterminate,
}

pub trait Observation {
    type Error;
    fn process_before(&self, pid: u32) -> Result<Option<ProcessIdentity>, Self::Error>;
    fn socket(&self, vm: &VmId) -> Result<Option<SocketIdentity>, Self::Error>;
    fn process_after(&self, pid: u32) -> Result<Option<ProcessIdentity>, Self::Error>;
    fn pidfd_alive(&self, pid: u32) -> Result<bool, Self::Error>;
    fn duplicate_evidence(&self, vm: &VmId) -> Result<DuplicateEvidence, Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classification {
    /// Identity evidence matched. This is not a control or adoption capability.
    OwnershipMatched,
    OwnedAliveSocketUnavailable,
    ExitedOwned,
    ForeignConflict,
    AmbiguousPreserve,
    DuplicateConflict,
    CorruptOwnership,
}

pub fn inspect<O: Observation>(
    requested: &RequestedOwner,
    marker: Result<OwnerMarkerV1, MarkerError>,
    observations: &O,
) -> Classification {
    let marker = match marker {
        Ok(m) if m.validate().is_ok() => m,
        _ => return Classification::CorruptOwnership,
    };
    if marker.host_id != requested.host_id
        || marker.vm_id != requested.vm_id
        || marker.operation_id != requested.operation_id
        || marker.runtime_generation != requested.runtime_generation
        || marker.active_attempt_token != requested.active_attempt_token
        || marker.config_fingerprint != requested.config_fingerprint
    {
        return Classification::ForeignConflict;
    }
    match observations.duplicate_evidence(&marker.vm_id) {
        Ok(DuplicateEvidence::Conflict) => return Classification::DuplicateConflict,
        Ok(DuplicateEvidence::Exclusive) => {}
        Ok(DuplicateEvidence::Indeterminate) => return Classification::AmbiguousPreserve,
        Err(_) => return Classification::AmbiguousPreserve,
    }
    let process_before = match observations.process_before(marker.pid) {
        Ok(v) => v,
        Err(_) => return Classification::AmbiguousPreserve,
    };
    let socket = match observations.socket(&marker.vm_id) {
        Ok(v) => v,
        Err(_) => return Classification::AmbiguousPreserve,
    };
    let process_after = match observations.process_after(marker.pid) {
        Ok(v) => v,
        Err(_) => return Classification::AmbiguousPreserve,
    };
    let pidfd_alive = match observations.pidfd_alive(marker.pid) {
        Ok(value) => value,
        Err(_) => return Classification::AmbiguousPreserve,
    };
    match (process_before, socket, process_after) {
        (None, None, None) if !pidfd_alive => Classification::ExitedOwned,
        (None, None, None) => Classification::AmbiguousPreserve,
        (None, _, _) | (_, _, None) => Classification::AmbiguousPreserve,
        (Some(before), None, Some(after)) => {
            if process_matches(&before, &after, pidfd_alive, &marker) {
                Classification::OwnedAliveSocketUnavailable
            } else {
                Classification::AmbiguousPreserve
            }
        }
        (Some(before), Some(s), Some(after)) => {
            let matches = process_matches(&before, &after, pidfd_alive, &marker)
                && s.runtime_directory == marker.runtime_directory
                && s.socket == marker.api_socket
                && s.peer_pid == marker.pid
                && s.peer_uid == marker.uid;
            if matches {
                if s.api_live {
                    Classification::OwnershipMatched
                } else {
                    Classification::OwnedAliveSocketUnavailable
                }
            } else {
                Classification::AmbiguousPreserve
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptedRuntime {
    pub marker: OwnerMarkerV1,
}

pub fn adopt<O: Observation>(
    requested: &RequestedOwner,
    marker: Result<OwnerMarkerV1, MarkerError>,
    observations: &O,
) -> Result<AdoptedRuntime, Classification> {
    let m = match marker {
        Ok(m) => m,
        Err(_) => return Err(Classification::CorruptOwnership),
    };
    let classification = inspect(requested, Ok(m.clone()), observations);
    if classification == Classification::OwnershipMatched {
        Ok(AdoptedRuntime { marker: m })
    } else {
        Err(classification)
    }
}

fn process_matches(
    before: &ProcessIdentity,
    after: &ProcessIdentity,
    pidfd_alive: bool,
    marker: &OwnerMarkerV1,
) -> bool {
    pidfd_alive
        && before == after
        && before.pid == marker.pid
        && before.start_ticks == marker.proc_start_ticks
        && before.boot_id == marker.boot_id
        && before.executable == marker.executable
        && before.uid == marker.uid
        && before.gid == marker.gid
        && before.cgroup_fingerprint == marker.cgroup_fingerprint
}

#[cfg(test)]
mod tests;
