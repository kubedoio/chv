//! Linux ownership evidence collection for recovery. Deliberately unwired.
//!
//! Every runtime-root component must be root- or service-owned and not writable
//! by group/other. A process with the same effective UID can still mutate these
//! directories and attempt an ABA replacement; inode revalidation detects
//! ordinary replacement, but this slice cannot prove absence of same-UID ABA.
//! Duplicate proof therefore remains unavailable and classification preserves
//! ambiguity instead of permitting adoption.

use cellhv_core_runtime_ownership::{
    DuplicateEvidence, FileIdentity, Observation, ProcessIdentity, SocketIdentity,
};
use cellhv_core_types::VmId;
use nix::libc;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use thiserror::Error;

const MAX_PROC_BYTES: usize = 64 * 1024;
const MAX_API_BYTES: usize = 64 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SessionState {
    Initial,
    DuplicateChecked,
    ProcessBefore,
    Socket,
    ProcessAfter,
    Complete,
    Terminal,
}

#[derive(Debug, Error)]
pub enum LinuxObservationError {
    #[error("unsafe observation path")]
    UnsafePath,
    #[error("observation identity changed")]
    IdentityChanged,
    #[error("required observation capability is unavailable")]
    CapabilityUnavailable,
    #[error("observation input exceeded its bound")]
    TooLarge,
    #[error("invalid process evidence")]
    InvalidProcess,
    #[error("observer lock is poisoned")]
    Poisoned,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Retains the pidfd from the first process sample through the ordered inspection.
pub struct LinuxOwnershipObservation {
    proc_root: File,
    runtime_root: File,
    pidfd: Mutex<Option<OwnedFd>>,
    state: Mutex<SessionState>,
    expected_pid: u32,
    expected_vm: VmId,
}

impl LinuxOwnershipObservation {
    pub fn open(
        runtime_root: &Path,
        expected_pid: u32,
        expected_vm: VmId,
    ) -> Result<Self, LinuxObservationError> {
        Self::open_with_proc(runtime_root, Path::new("/proc"), expected_pid, expected_vm)
    }

    fn open_with_proc(
        runtime_root: &Path,
        proc_root: &Path,
        expected_pid: u32,
        expected_vm: VmId,
    ) -> Result<Self, LinuxObservationError> {
        if expected_pid == 0 {
            return Err(LinuxObservationError::InvalidProcess);
        }
        Ok(Self {
            proc_root: open_directory(proc_root)?,
            runtime_root: open_directory(runtime_root)?,
            pidfd: Mutex::new(None),
            state: Mutex::new(SessionState::Initial),
            expected_pid,
            expected_vm,
        })
    }

    fn process(&self, pid: u32) -> Result<Option<ProcessIdentity>, LinuxObservationError> {
        let name = CString::new(pid.to_string()).map_err(|_| LinuxObservationError::UnsafePath)?;
        let dir = match openat_directory(self.proc_root.as_raw_fd(), &name) {
            Ok(file) => file,
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let stat = read_at(dir.as_raw_fd(), c"stat", MAX_PROC_BYTES)?;
        let stat = std::str::from_utf8(&stat).map_err(|_| LinuxObservationError::InvalidProcess)?;
        let start_ticks = cellhv_core_runtime_ownership::parse_proc_start_ticks(stat)
            .map_err(|_| LinuxObservationError::InvalidProcess)?;
        let (uid, gid) = parse_status(&read_at(dir.as_raw_fd(), c"status", MAX_PROC_BYTES)?)?;
        let cgroup_fingerprint =
            parse_cgroup(&read_at(dir.as_raw_fd(), c"cgroup", MAX_PROC_BYTES)?)?;
        // Following procfs's kernel-owned exe link is safe; starttime and pidfd fence reuse.
        let executable = openat(dir.as_raw_fd(), c"exe", libc::O_PATH, false)?;
        let executable = file_identity(&executable.metadata()?);
        let sys = openat_directory(self.proc_root.as_raw_fd(), c"sys")?;
        let kernel = openat_directory(sys.as_raw_fd(), c"kernel")?;
        let random = openat_directory(kernel.as_raw_fd(), c"random")?;
        let boot_id = read_at(random.as_raw_fd(), c"boot_id", 128)?;
        let boot_id = std::str::from_utf8(&boot_id)
            .map_err(|_| LinuxObservationError::InvalidProcess)?
            .trim()
            .to_owned();
        Ok(Some(ProcessIdentity {
            pid,
            start_ticks,
            boot_id,
            executable,
            uid,
            gid,
            cgroup_fingerprint,
        }))
    }

    fn socket_evidence(&self, vm: &VmId) -> Result<Option<SocketIdentity>, LinuxObservationError> {
        if vm != &self.expected_vm {
            return Err(LinuxObservationError::IdentityChanged);
        }
        let vm_name = safe_component(vm.as_str())?;
        let vm_dir = match openat_directory(self.runtime_root.as_raw_fd(), &vm_name) {
            Ok(file) => file,
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let vm_metadata = vm_dir.metadata()?;
        require_owned_not_writable(&vm_metadata)?;
        let runtime_directory = file_identity(&vm_metadata);
        let before = match socket_stat(vm_dir.as_raw_fd()) {
            Ok(value) => value,
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut stream = connect_anchored(vm_dir.as_raw_fd(), CONNECT_TIMEOUT)?;
        stream.set_read_timeout(Some(Duration::from_secs(1)))?;
        stream.set_write_timeout(Some(Duration::from_secs(1)))?;
        let credentials = peer_credentials(&stream)?;
        if credentials.pid <= 0 {
            return Err(LinuxObservationError::IdentityChanged);
        }
        // Probe over this exact connected fd, then revalidate the anchored pathname.
        let api_live = crate::ch_api::probe_vmm_ping_connected(
            &mut stream,
            Duration::from_secs(1),
            MAX_API_BYTES,
        )
        .is_ok();
        let after = socket_stat(vm_dir.as_raw_fd())?;
        if before != after {
            return Err(LinuxObservationError::IdentityChanged);
        }
        Ok(Some(SocketIdentity {
            runtime_directory,
            socket: before,
            peer_pid: credentials.pid as u32,
            peer_uid: credentials.uid,
            api_live,
        }))
    }

    fn open_pidfd(&self, pid: u32) -> Result<(), LinuxObservationError> {
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) as i32 };
        if fd < 0 {
            let error = std::io::Error::last_os_error();
            if matches!(
                error.raw_os_error(),
                Some(libc::ENOSYS) | Some(libc::EINVAL)
            ) {
                return Err(LinuxObservationError::CapabilityUnavailable);
            }
            return Err(error.into());
        }
        let mut pidfd = self
            .pidfd
            .lock()
            .map_err(|_| LinuxObservationError::Poisoned)?;
        if pidfd.is_some() {
            unsafe { libc::close(fd) };
            return Err(LinuxObservationError::IdentityChanged);
        }
        *pidfd = Some(unsafe { OwnedFd::from_raw_fd(fd) });
        Ok(())
    }
}

impl Observation for LinuxOwnershipObservation {
    type Error = LinuxObservationError;

    fn process_before(&self, pid: u32) -> Result<Option<ProcessIdentity>, Self::Error> {
        if pid != self.expected_pid {
            return Err(LinuxObservationError::IdentityChanged);
        }
        self.transition(SessionState::DuplicateChecked, SessionState::ProcessBefore)?;
        self.open_pidfd(pid)?;
        self.process(pid)
    }

    fn socket(&self, vm: &VmId) -> Result<Option<SocketIdentity>, Self::Error> {
        self.transition(SessionState::ProcessBefore, SessionState::Socket)?;
        self.socket_evidence(vm)
    }

    fn process_after(&self, pid: u32) -> Result<Option<ProcessIdentity>, Self::Error> {
        self.transition(SessionState::Socket, SessionState::ProcessAfter)?;
        if pid != self.expected_pid
            || self
                .pidfd
                .lock()
                .map_err(|_| LinuxObservationError::Poisoned)?
                .is_none()
        {
            return Err(LinuxObservationError::CapabilityUnavailable);
        }
        self.process(pid)
    }

    fn pidfd_alive(&self, pid: u32) -> Result<bool, Self::Error> {
        if pid != self.expected_pid {
            return Err(LinuxObservationError::IdentityChanged);
        }
        self.transition(SessionState::ProcessAfter, SessionState::Complete)?;
        let guard = self
            .pidfd
            .lock()
            .map_err(|_| LinuxObservationError::Poisoned)?;
        let fd = guard
            .as_ref()
            .ok_or(LinuxObservationError::CapabilityUnavailable)?;
        let mut pollfd = libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut pollfd, 1, 0) };
        if result < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(result == 0)
    }

    fn duplicate_evidence(&self, vm: &VmId) -> Result<DuplicateEvidence, Self::Error> {
        if vm != &self.expected_vm {
            return Err(LinuxObservationError::IdentityChanged);
        }
        self.transition(SessionState::Initial, SessionState::DuplicateChecked)?;

        let mut count = 0;
        let start = Instant::now();
        let vm_sock_suffix = format!("/{}/vm.sock", vm.as_str());
        let vm_sock_suffix_bytes = vm_sock_suffix.as_bytes();

        let proc_fd = self.proc_root.as_raw_fd();
        let path = format!("/proc/self/fd/{}", proc_fd);
        let Ok(entries) = std::fs::read_dir(&path) else {
            return Ok(DuplicateEvidence::Indeterminate);
        };

        for entry in entries {
            count += 1;
            if count > 131072 || start.elapsed() > Duration::from_secs(2) {
                return Ok(DuplicateEvidence::Indeterminate);
            }
            let Ok(entry) = entry else {
                continue;
            };
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            if !file_name_str.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let Ok(pid) = file_name_str.parse::<u32>() else {
                continue;
            };
            if pid == self.expected_pid {
                continue;
            }

            let pid_cstr = match CString::new(file_name_str.as_bytes()) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let pid_dir = match openat_directory(proc_fd, &pid_cstr) {
                Ok(dir) => dir,
                Err(_) => continue,
            };
            let cmdline = match read_at(pid_dir.as_raw_fd(), c"cmdline", MAX_PROC_BYTES) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let args: Vec<&[u8]> = cmdline.split(|&b| b == 0).collect();
            for i in 0..args.len() {
                if args[i] == b"--api-socket" && i + 1 < args.len() {
                    let socket_path = args[i + 1];
                    if socket_path.ends_with(vm_sock_suffix_bytes) {
                        return Ok(DuplicateEvidence::Conflict);
                    }
                }
            }
        }

        Ok(DuplicateEvidence::Indeterminate)
    }
}

impl LinuxOwnershipObservation {
    fn transition(
        &self,
        expected: SessionState,
        next: SessionState,
    ) -> Result<(), LinuxObservationError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| LinuxObservationError::Poisoned)?;
        if *state != expected {
            *state = SessionState::Terminal;
            return Err(LinuxObservationError::IdentityChanged);
        }
        *state = next;
        Ok(())
    }
}

fn open_directory(path: &Path) -> Result<File, LinuxObservationError> {
    use std::path::Component;
    if path.as_os_str().is_empty() {
        return Err(LinuxObservationError::UnsafePath);
    }
    let base = if path.is_absolute() { c"/" } else { c"." };
    let fd = unsafe {
        libc::open(
            base.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut current = unsafe { File::from_raw_fd(fd) };
    require_trusted_directory(&current.metadata()?)?;
    for component in path.components() {
        let Component::Normal(component) = component else {
            if matches!(component, Component::RootDir | Component::CurDir) {
                continue;
            }
            return Err(LinuxObservationError::UnsafePath);
        };
        let component =
            CString::new(component.as_bytes()).map_err(|_| LinuxObservationError::UnsafePath)?;
        current = openat_directory(current.as_raw_fd(), &component)?;
        require_trusted_directory(&current.metadata()?)?;
    }
    Ok(current)
}

fn openat_directory(parent: RawFd, name: &CStr) -> std::io::Result<File> {
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn openat(parent: RawFd, name: &CStr, flags: i32, nofollow: bool) -> std::io::Result<File> {
    let flags = flags | libc::O_CLOEXEC | if nofollow { libc::O_NOFOLLOW } else { 0 };
    let fd = unsafe { libc::openat(parent, name.as_ptr(), flags) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn read_at(parent: RawFd, name: &CStr, max: usize) -> Result<Vec<u8>, LinuxObservationError> {
    let file = openat(parent, name, libc::O_RDONLY, true)?;
    let mut bytes = Vec::new();
    file.take((max + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > max {
        return Err(LinuxObservationError::TooLarge);
    }
    Ok(bytes)
}

fn safe_component(value: &str) -> Result<CString, LinuxObservationError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(LinuxObservationError::UnsafePath);
    }
    CString::new(value).map_err(|_| LinuxObservationError::UnsafePath)
}

fn file_identity(metadata: &std::fs::Metadata) -> FileIdentity {
    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

fn socket_stat(parent: RawFd) -> std::io::Result<FileIdentity> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            parent,
            c"vm.sock".as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } < 0
    {
        return Err(std::io::Error::last_os_error());
    }
    let stat = unsafe { stat.assume_init() };
    if stat.st_mode & libc::S_IFMT != libc::S_IFSOCK {
        return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
    }
    if stat.st_uid != unsafe { libc::geteuid() } || stat.st_mode & 0o022 != 0 {
        return Err(std::io::Error::from_raw_os_error(libc::EACCES));
    }
    Ok(FileIdentity {
        device: stat.st_dev,
        inode: stat.st_ino,
    })
}

fn peer_credentials(stream: &UnixStream) -> std::io::Result<libc::ucred> {
    let mut value = std::mem::MaybeUninit::<libc::ucred>::uninit();
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            value.as_mut_ptr().cast(),
            &mut length,
        )
    } < 0
    {
        return Err(std::io::Error::last_os_error());
    }
    if length as usize != std::mem::size_of::<libc::ucred>() {
        return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
    }
    Ok(unsafe { value.assume_init() })
}

fn connect_anchored(parent: RawFd, timeout: Duration) -> Result<UnixStream, LinuxObservationError> {
    let path = CString::new(format!("/proc/self/fd/{parent}/vm.sock"))
        .map_err(|_| LinuxObservationError::UnsafePath)?;
    if path.as_bytes().len() >= std::mem::size_of::<libc::sockaddr_un>() - 2 {
        return Err(LinuxObservationError::UnsafePath);
    }
    let fd = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            0,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    unsafe {
        std::ptr::copy_nonoverlapping(
            path.as_ptr(),
            address.sun_path.as_mut_ptr(),
            path.as_bytes_with_nul().len(),
        )
    };
    let length = (std::mem::size_of::<libc::sa_family_t>() + path.as_bytes_with_nul().len())
        as libc::socklen_t;
    let result =
        unsafe { libc::connect(fd, (&address as *const libc::sockaddr_un).cast(), length) };
    if result < 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EINPROGRESS) {
            return Err(error.into());
        }
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Unix socket connect timed out",
                )
                .into());
            }
            let mut pollfd = libc::pollfd {
                fd,
                events: libc::POLLOUT,
                revents: 0,
            };
            let milliseconds = remaining.as_millis().clamp(1, i32::MAX as u128) as i32;
            let polled = unsafe { libc::poll(&mut pollfd, 1, milliseconds) };
            if polled < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error.into());
            }
            if polled == 0 {
                continue;
            }
            let mut socket_error = 0i32;
            let mut size = std::mem::size_of::<i32>() as libc::socklen_t;
            if unsafe {
                libc::getsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_ERROR,
                    (&mut socket_error as *mut i32).cast(),
                    &mut size,
                )
            } < 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            if socket_error != 0 {
                return Err(std::io::Error::from_raw_os_error(socket_error).into());
            }
            break;
        }
    }
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(UnixStream::from(owned))
}

fn parse_status(bytes: &[u8]) -> Result<(u32, u32), LinuxObservationError> {
    let text = std::str::from_utf8(bytes).map_err(|_| LinuxObservationError::InvalidProcess)?;
    let first = |label: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(label))
            .and_then(|v| v.split_ascii_whitespace().next())
            .and_then(|v| v.parse::<u32>().ok())
            .ok_or(LinuxObservationError::InvalidProcess)
    };
    Ok((first("Uid:")?, first("Gid:")?))
}

fn parse_cgroup(bytes: &[u8]) -> Result<String, LinuxObservationError> {
    let text = std::str::from_utf8(bytes).map_err(|_| LinuxObservationError::InvalidProcess)?;
    let mut paths = text
        .lines()
        .map(|line| line.rsplit(':').next().unwrap_or_default())
        .filter(|v| !v.is_empty());
    let path = paths.next().ok_or(LinuxObservationError::InvalidProcess)?;
    if paths.any(|other| other != path) {
        return Err(LinuxObservationError::InvalidProcess);
    }
    Ok(path.to_owned())
}

fn require_owned_not_writable(metadata: &std::fs::Metadata) -> Result<(), LinuxObservationError> {
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o022 != 0
    {
        return Err(LinuxObservationError::UnsafePath);
    }
    Ok(())
}

fn require_trusted_directory(metadata: &std::fs::Metadata) -> Result<(), LinuxObservationError> {
    let effective_uid = unsafe { libc::geteuid() };
    if !metadata.is_dir()
        || (metadata.uid() != 0 && metadata.uid() != effective_uid)
        || metadata.mode() & 0o022 != 0
    {
        return Err(LinuxObservationError::UnsafePath);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cellhv_core_runtime_ownership::{
        inspect, OwnerMarkerV1, RequestedOwner, MARKER_SCHEMA_VERSION,
    };
    use cellhv_core_types::{HostId, OperationId};
    use std::fs;
    use std::io::Write as _;
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;
    use std::process::{Command, Stdio};
    use std::thread;
    use tempfile::TempDir;

    fn trusted_tempdir() -> TempDir {
        TempDir::new_in(std::env::current_dir().unwrap()).unwrap()
    }

    #[test]
    fn observes_process_pidfd_socket_peer_and_liveness() {
        let temp = trusted_tempdir();
        let vm_dir = temp.path().join("vm-1");
        fs::create_dir(&vm_dir).unwrap();
        let listener = UnixListener::bind(vm_dir.join("vm.sock")).unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 128];
            let _ = stream.read(&mut request).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
        });
        let pid = std::process::id();
        let observer =
            LinuxOwnershipObservation::open(temp.path(), pid, VmId::new("vm-1").unwrap()).unwrap();
        observer.open_pidfd(pid).unwrap();
        let before = observer.process(pid).unwrap().unwrap();
        let socket = observer
            .socket_evidence(&VmId::new("vm-1").unwrap())
            .unwrap()
            .unwrap();
        let after = observer.process(pid).unwrap().unwrap();
        assert_eq!(before, after);
        assert_eq!(socket.peer_pid, pid);
        assert!(socket.api_live);
        assert!(observer.pidfd.lock().unwrap().is_some());
        server.join().unwrap();
    }

    #[test]
    fn refuses_symlinked_vm_directory() {
        let temp = trusted_tempdir();
        let target = trusted_tempdir();
        symlink(target.path(), temp.path().join("vm-1")).unwrap();
        let observer = LinuxOwnershipObservation::open(
            temp.path(),
            std::process::id(),
            VmId::new("vm-1").unwrap(),
        )
        .unwrap();
        assert!(observer
            .socket_evidence(&VmId::new("vm-1").unwrap())
            .is_err());
    }

    #[test]
    fn refuses_writable_socket_and_mismatched_session_identity() {
        let temp = trusted_tempdir();
        let vm_dir = temp.path().join("vm-1");
        fs::create_dir(&vm_dir).unwrap();
        let _listener = UnixListener::bind(vm_dir.join("vm.sock")).unwrap();
        let mut permissions = fs::metadata(vm_dir.join("vm.sock")).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o777);
        fs::set_permissions(vm_dir.join("vm.sock"), permissions).unwrap();
        let observer = LinuxOwnershipObservation::open(
            temp.path(),
            std::process::id(),
            VmId::new("vm-1").unwrap(),
        )
        .unwrap();
        assert!(observer
            .socket_evidence(&VmId::new("vm-1").unwrap())
            .is_err());
        assert!(observer
            .socket_evidence(&VmId::new("vm-2").unwrap())
            .is_err());
        assert!(observer.process_before(std::process::id() + 1).is_err());
    }

    #[test]
    fn duplicate_step_is_indeterminate_but_preserves_future_inspection_order() {
        let temp = trusted_tempdir();
        let vm = VmId::new("vm-1").unwrap();
        let observer =
            LinuxOwnershipObservation::open(temp.path(), std::process::id(), vm.clone()).unwrap();
        assert!(matches!(
            observer.duplicate_evidence(&vm),
            Ok(DuplicateEvidence::Indeterminate)
        ));
        assert!(observer
            .process_before(std::process::id())
            .unwrap()
            .is_some());

        let repeated =
            LinuxOwnershipObservation::open(temp.path(), std::process::id(), vm.clone()).unwrap();
        assert!(matches!(
            repeated.duplicate_evidence(&vm),
            Ok(DuplicateEvidence::Indeterminate) | Ok(DuplicateEvidence::Conflict)
        ));
        assert!(matches!(
            repeated.duplicate_evidence(&vm),
            Err(LinuxObservationError::IdentityChanged)
        ));
    }

    #[test]
    fn observes_duplicate_candidate_conflict() {
        let temp = trusted_tempdir();
        let vm = VmId::new("vm-duplicate-test").unwrap();
        let proc = trusted_tempdir();
        let pid1 = 1000;
        let pid2 = 1001;

        fs::create_dir(proc.path().join(pid1.to_string())).unwrap();
        fs::write(
            proc.path().join(pid1.to_string()).join("cmdline"),
            b"cloud-hypervisor\0--api-socket\0/run/vm-duplicate-test/vm.sock\0",
        )
        .unwrap();

        fs::create_dir(proc.path().join(pid2.to_string())).unwrap();
        fs::write(
            proc.path().join(pid2.to_string()).join("cmdline"),
            b"cloud-hypervisor\0--api-socket\0/run/other/vm.sock\0",
        )
        .unwrap();

        let observer =
            LinuxOwnershipObservation::open_with_proc(temp.path(), proc.path(), pid2, vm.clone())
                .unwrap();

        let duplicate = observer.duplicate_evidence(&vm).unwrap();
        assert_eq!(duplicate, DuplicateEvidence::Conflict);
    }

    #[test]
    fn inspect_preserves_ambiguity_without_duplicate_proof() {
        let temp = trusted_tempdir();
        let vm = VmId::new("vm-1").unwrap();
        let host = HostId::new("host-1").unwrap();
        let operation = OperationId::new("op-1").unwrap();
        let marker = OwnerMarkerV1 {
            schema_version: MARKER_SCHEMA_VERSION,
            host_id: host.clone(),
            vm_id: vm.clone(),
            operation_id: operation.clone(),
            runtime_generation: "00000000-0000-4000-8000-000000000001".into(),
            active_attempt_token: "attempt-1".into(),
            config_fingerprint: "a".repeat(64),
            publication_nonce: "publication-0001".into(),
            pid: std::process::id(),
            proc_start_ticks: 1,
            boot_id: "00000000-0000-4000-8000-000000000002".into(),
            executable: FileIdentity {
                device: 1,
                inode: 1,
            },
            uid: unsafe { libc::geteuid() },
            gid: unsafe { libc::getegid() },
            cgroup_fingerprint: "/test".into(),
            runtime_directory_name: "vm-1".into(),
            api_socket_name: "vm.sock".into(),
            runtime_directory: FileIdentity {
                device: 1,
                inode: 2,
            },
            api_socket: FileIdentity {
                device: 1,
                inode: 3,
            },
        };
        let requested = RequestedOwner {
            host_id: host,
            vm_id: vm.clone(),
            operation_id: operation,
            runtime_generation: marker.runtime_generation.clone(),
            active_attempt_token: marker.active_attempt_token.clone(),
            config_fingerprint: marker.config_fingerprint.clone(),
        };
        let observer = LinuxOwnershipObservation::open(temp.path(), marker.pid, vm).unwrap();
        assert_eq!(
            inspect(&requested, Ok(marker), &observer),
            cellhv_core_runtime_ownership::Classification::AmbiguousPreserve
        );
    }

    #[test]
    fn missing_and_non_socket_paths_fail_closed() {
        let temp = trusted_tempdir();
        let vm_dir = temp.path().join("vm-1");
        fs::create_dir(&vm_dir).unwrap();
        let observer = LinuxOwnershipObservation::open(
            temp.path(),
            std::process::id(),
            VmId::new("vm-1").unwrap(),
        )
        .unwrap();
        assert!(observer
            .socket_evidence(&VmId::new("vm-1").unwrap())
            .unwrap()
            .is_none());
        fs::write(vm_dir.join("vm.sock"), b"not a socket").unwrap();
        assert!(observer
            .socket_evidence(&VmId::new("vm-1").unwrap())
            .is_err());
    }

    #[test]
    fn api_failure_is_not_live() {
        let temp = trusted_tempdir();
        let vm_dir = temp.path().join("vm-1");
        fs::create_dir(&vm_dir).unwrap();
        let listener = UnixListener::bind(vm_dir.join("vm.sock")).unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 128];
            let _ = stream.read(&mut request).unwrap();
            stream
                .write_all(b"HTTP/1.1 500 Error\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
        });
        let observer = LinuxOwnershipObservation::open(
            temp.path(),
            std::process::id(),
            VmId::new("vm-1").unwrap(),
        )
        .unwrap();
        assert!(
            !observer
                .socket_evidence(&VmId::new("vm-1").unwrap())
                .unwrap()
                .unwrap()
                .api_live
        );
        server.join().unwrap();
    }

    #[test]
    fn api_timeout_is_not_live() {
        let temp = trusted_tempdir();
        let vm_dir = temp.path().join("vm-1");
        fs::create_dir(&vm_dir).unwrap();
        let listener = UnixListener::bind(vm_dir.join("vm.sock")).unwrap();
        let server = thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(1100));
        });
        let observer = LinuxOwnershipObservation::open(
            temp.path(),
            std::process::id(),
            VmId::new("vm-1").unwrap(),
        )
        .unwrap();
        assert!(
            !observer
                .socket_evidence(&VmId::new("vm-1").unwrap())
                .unwrap()
                .unwrap()
                .api_live
        );
        server.join().unwrap();
    }

    #[test]
    fn malformed_proc_stat_fails_closed() {
        let runtime = trusted_tempdir();
        let proc = trusted_tempdir();
        fs::create_dir(proc.path().join("123")).unwrap();
        fs::write(proc.path().join("123/stat"), b"malformed").unwrap();
        let observer = LinuxOwnershipObservation::open_with_proc(
            runtime.path(),
            proc.path(),
            123,
            VmId::new("vm-1").unwrap(),
        )
        .unwrap();
        assert!(matches!(
            observer.process(123),
            Err(LinuxObservationError::InvalidProcess)
        ));
    }

    #[test]
    fn oversized_proc_stat_fails_before_parsing() {
        let runtime = trusted_tempdir();
        let proc = trusted_tempdir();
        fs::create_dir(proc.path().join("123")).unwrap();
        fs::write(proc.path().join("123/stat"), vec![b'x'; MAX_PROC_BYTES + 1]).unwrap();
        let observer = LinuxOwnershipObservation::open_with_proc(
            runtime.path(),
            proc.path(),
            123,
            VmId::new("vm-1").unwrap(),
        )
        .unwrap();
        assert!(matches!(
            observer.process(123),
            Err(LinuxObservationError::TooLarge)
        ));
    }

    #[test]
    fn pidfd_reports_subprocess_exit_during_ordered_observation() {
        let temp = trusted_tempdir();
        let vm_dir = temp.path().join("vm-1");
        fs::create_dir(&vm_dir).unwrap();
        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        let observer =
            LinuxOwnershipObservation::open(temp.path(), pid, VmId::new("vm-1").unwrap()).unwrap();

        assert_eq!(
            observer
                .duplicate_evidence(&VmId::new("vm-1").unwrap())
                .unwrap(),
            DuplicateEvidence::Indeterminate
        );
        assert!(observer.process_before(pid).unwrap().is_some());
        assert!(observer
            .socket(&VmId::new("vm-1").unwrap())
            .unwrap()
            .is_none());

        child.kill().unwrap();
        child.wait().unwrap();
        assert!(observer.process_after(pid).unwrap().is_none());
        assert!(!observer.pidfd_alive(pid).unwrap());
    }

    #[test]
    fn symlinked_runtime_root_ancestor_is_rejected() {
        let parent = trusted_tempdir();
        let target = trusted_tempdir();
        fs::create_dir(target.path().join("runtime")).unwrap();
        symlink(target.path(), parent.path().join("redirect")).unwrap();
        assert!(LinuxOwnershipObservation::open(
            &parent.path().join("redirect/runtime"),
            std::process::id(),
            VmId::new("vm-1").unwrap(),
        )
        .is_err());
    }

    #[test]
    fn writable_runtime_root_ancestor_is_rejected() {
        let parent = trusted_tempdir();
        let runtime = parent.path().join("runtime");
        fs::create_dir(&runtime).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(parent.path()).unwrap().permissions();
        permissions.set_mode(0o770);
        fs::set_permissions(parent.path(), permissions).unwrap();
        assert!(LinuxOwnershipObservation::open(
            &runtime,
            std::process::id(),
            VmId::new("vm-1").unwrap(),
        )
        .is_err());
    }

    #[test]
    fn writable_runtime_root_is_rejected() {
        let temp = trusted_tempdir();
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(temp.path()).unwrap().permissions();
        permissions.set_mode(0o770);
        fs::set_permissions(temp.path(), permissions).unwrap();
        assert!(LinuxOwnershipObservation::open(
            temp.path(),
            std::process::id(),
            VmId::new("vm-1").unwrap(),
        )
        .is_err());
    }
}
