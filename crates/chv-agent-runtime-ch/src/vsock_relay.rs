//! The node side of the G5 vsock monitoring transport (prompt-06 task 3:
//! host-side ownership of the vsock connection endpoint).
//!
//! Design (ADR-026, D1 — "transparent TLS pipe relay"): the guest's
//! chv-monitor-agent connects to CID 2 (VMADDR_CID_HOST) on the fixed
//! contract port [`VSOCK_MONITOR_PORT`]; cloud-hypervisor's vsock muxer
//! turns that into a host-side Unix connect to
//! `"<device_socket>_<port>"`. This module owns that host endpoint: a
//! per-VM relay that accepts the VMM's connection and pipes RAW BYTES
//! to the control plane's existing mTLS listener (host:port derived
//! from the node's `control_plane_addr`).
//!
//! Invariants (all deliberate, all security-relevant):
//!
//! - The relay NEVER parses, inspects, or authenticates the payload.
//!   The guest runs the same TLS/HTTP stack end-to-end, so enroll,
//!   ingest, and certificate rotation all ride the pipe. All
//!   authentication stays where it already lives: the control plane's
//!   mTLS listener verifies the guest agent's client certificate.
//! - The CID and the Unix socket path are NEVER authentication and are
//!   never used for identity decisions (ADR-026). The relay does not
//!   even know the VM's CID. CID-reuse on the node is therefore
//!   neutralized by construction: whoever holds the guest connection
//!   still has to present a valid agent certificate to the CP.
//! - Bounded: at most [`VsockRelayLimits::max_connections`] concurrent
//!   pipes per VM (a connection above the bound is closed immediately —
//!   the guest transport retries with backoff), and each pipe is closed
//!   after [`VsockRelayLimits::idle_timeout`] without traffic in either
//!   direction (a paused guest eventually loses its pipe; the guest
//!   transport reconnects on resume).
//!
//! Lifecycle: the relay is started (idempotently) whenever a VM with the
//! vsock device is created, (re-)started, re-spawned, or adopted by
//! [`crate::process::ProcessCloudHypervisorAdapter`] — the single
//! choke point both authority paths (Core runtime and legacy reconciler
//! / direct RPC) go through — and stopped when the VM stops or is
//! deleted. The gate is the VM's persisted `vm-config.json`: a create
//! payload without the `vsock` device never gets a relay, so non-vsock
//! VMs are untouched.

use chv_hypervisor_api::resources::vm_config_file;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpStream, UnixListener, UnixStream};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Derive the relay's listening path from the VM's vsock DEVICE socket
/// path. This must byte-match cloud-hypervisor v53.0's muxer convention:
/// a guest connection to host port `P` is mapped onto the Unix socket
/// path `"<device_socket>_<P>"`. The device socket string comes from the
/// VM's persisted create payload — never re-derived — so the two sides
/// can never disagree.
pub fn relay_socket_path(device_socket: &str) -> PathBuf {
    PathBuf::from(format!(
        "{device_socket}_{}",
        chv_hypervisor_api::VSOCK_MONITOR_PORT
    ))
}

/// Bounds for one VM's relay. Defaults are deliberately conservative:
/// two pipes cover one active connection plus reconnect overlap; the
/// idle timeout is far above the guest's sample cadence, so it only
/// fires on a genuinely dead connection (paused guest, wedged CP link).
#[derive(Debug, Clone, Copy)]
pub struct VsockRelayLimits {
    pub max_connections: usize,
    pub idle_timeout: std::time::Duration,
}

impl Default for VsockRelayLimits {
    fn default() -> Self {
        Self {
            max_connections: 2,
            idle_timeout: std::time::Duration::from_secs(15 * 60),
        }
    }
}

/// Extract the vsock device socket path from a VM's persisted
/// `vm-config.json` (the exact cloud-hypervisor REST payload the create
/// path wrote). `None` means the VM has no vsock device — the common
/// case, and the relay's opt-out gate.
pub(crate) fn persisted_vsock_device_socket(vm_dir: &Path) -> Option<String> {
    let raw = match std::fs::read_to_string(vm_config_file(vm_dir)) {
        Ok(raw) => raw,
        Err(e) => {
            debug!(
                vm_dir = %vm_dir.display(),
                error = %e,
                "vsock relay: no readable vm-config.json — no relay for this VM"
            );
            return None;
        }
    };
    let payload: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(e) => {
            warn!(
                vm_dir = %vm_dir.display(),
                error = %e,
                "vsock relay: vm-config.json is malformed — no relay for this VM"
            );
            return None;
        }
    };
    let vsock = payload.get("vsock")?;
    match vsock.get("socket").and_then(|s| s.as_str()) {
        Some(socket) => Some(socket.to_string()),
        None => {
            warn!(
                vm_dir = %vm_dir.display(),
                "vsock relay: vm-config.json carries a vsock device without a usable socket path — no relay for this VM"
            );
            None
        }
    }
}

/// Parse the relay's dial target (host:port) out of the node's
/// `control_plane_addr`. Accepts `https://host:port`, `http://host:port`,
/// and bare `host:port`; a scheme without an explicit port gets the
/// scheme's default. The target string is handed to
/// [`TcpStream::connect`] per connection, so DNS is re-resolved per pipe
/// and a CP address change is picked up without an agent restart.
fn parse_cp_tcp_target(control_plane_addr: &str) -> Result<String, String> {
    let (scheme, rest) = if let Some(rest) = control_plane_addr.strip_prefix("https://") {
        ("https", rest)
    } else if let Some(rest) = control_plane_addr.strip_prefix("http://") {
        ("http", rest)
    } else {
        ("", control_plane_addr)
    };
    let host_port = rest.split('/').next().unwrap_or("").trim();
    if host_port.is_empty() {
        return Err(format!(
            "control plane address {control_plane_addr:?} has no host"
        ));
    }
    if host_port.contains(':') {
        // host:port (v6 literals are not supported as relay targets —
        // fail loudly rather than dial somewhere unexpected).
        if host_port.matches(':').count() > 1 {
            return Err(format!(
                "control plane address {control_plane_addr:?} looks like an IPv6 literal, which the vsock relay does not support"
            ));
        }
        Ok(host_port.to_string())
    } else {
        let port = match scheme {
            "https" => 443,
            "http" => 80,
            _ => {
                return Err(format!(
                    "control plane address {control_plane_addr:?} carries no port and no scheme to default one from"
                ));
            }
        };
        Ok(format!("{host_port}:{port}"))
    }
}

struct RelayEntry {
    relay_socket: PathBuf,
    handle: JoinHandle<()>,
}

/// Per-node manager for the vsock monitoring relays. One relay task per
/// vsock-enabled VM; the manager is interiorly mutable and shared by all
/// of the adapter's lifecycle paths.
pub struct VsockRelayManager {
    cp_target: String,
    limits: VsockRelayLimits,
    relays: Mutex<HashMap<String, RelayEntry>>,
}

impl VsockRelayManager {
    /// Construct a manager with the default bounds.
    pub fn new(control_plane_addr: &str) -> Result<Self, String> {
        Self::with_limits(control_plane_addr, VsockRelayLimits::default())
    }

    /// Construct a manager with explicit bounds (test surface; the
    /// production wiring uses [`VsockRelayManager::new`]).
    pub fn with_limits(control_plane_addr: &str, limits: VsockRelayLimits) -> Result<Self, String> {
        Ok(Self {
            cp_target: parse_cp_tcp_target(control_plane_addr)?,
            limits,
            relays: Mutex::new(HashMap::new()),
        })
    }

    /// Idempotently start this VM's relay, gated on the persisted create
    /// payload carrying the vsock device. Synchronous by design: the
    /// Unix listener is bound before returning, so a caller that just
    /// created or started the VM can rely on the endpoint existing (the
    /// VMM only connects out once the GUEST initiates, which is at least
    /// a boot away). A stale relay socket file (agent died between
    /// bind and cleanup) is unlinked first — Unix sockets need manual
    /// unlinking and a leftover file would make the bind fail.
    pub fn ensure_started(&self, vm_id: &str, vm_dir: &Path) {
        let mut relays = self.relays.lock().expect("vsock relay map lock poisoned");
        if let Some(entry) = relays.get(vm_id) {
            if !entry.handle.is_finished() {
                return;
            }
            // The entry's task died (e.g. the accept loop hit a fatal
            // error); fall through and start a fresh relay.
        }
        let Some(device_socket) = persisted_vsock_device_socket(vm_dir) else {
            return;
        };
        let relay_socket = relay_socket_path(&device_socket);
        let _ = std::fs::remove_file(&relay_socket);
        let listener = match UnixListener::bind(&relay_socket) {
            Ok(listener) => listener,
            Err(e) => {
                warn!(
                    vm_id = %vm_id,
                    socket = %relay_socket.display(),
                    error = %e,
                    "vsock relay: cannot bind the guest endpoint — guest monitoring over vsock will fail until the next VM lifecycle operation"
                );
                return;
            }
        };
        info!(
            vm_id = %vm_id,
            socket = %relay_socket.display(),
            cp_target = %self.cp_target,
            max_connections = self.limits.max_connections,
            "vsock monitoring relay listening"
        );
        let handle = tokio::spawn(accept_loop(
            vm_id.to_string(),
            listener,
            self.cp_target.clone(),
            self.limits,
        ));
        relays.insert(
            vm_id.to_string(),
            RelayEntry {
                relay_socket,
                handle,
            },
        );
    }

    /// Stop this VM's relay: abort the accept loop, drop the listener,
    /// and unlink the socket file (no VMM will connect to it again —
    /// this runs on VM stop and delete). Active pipes are separate
    /// tasks; they end when their peers (a dead VMM / a reconnecting
    /// guest) close, or on the idle timeout.
    pub fn stop(&self, vm_id: &str) {
        let mut relays = self.relays.lock().expect("vsock relay map lock poisoned");
        if let Some(entry) = relays.remove(vm_id) {
            entry.handle.abort();
            let _ = std::fs::remove_file(&entry.relay_socket);
            debug!(vm_id = %vm_id, "vsock monitoring relay stopped");
        }
    }
}

/// The per-VM accept loop: bounded connection admission, one pipe task
/// per accepted connection.
async fn accept_loop(
    vm_id: String,
    listener: UnixListener,
    cp_target: String,
    limits: VsockRelayLimits,
) {
    let active = Arc::new(AtomicUsize::new(0));
    loop {
        match listener.accept().await {
            Ok((conn, _)) => {
                if active.load(Ordering::Relaxed) >= limits.max_connections {
                    debug!(
                        vm_id = %vm_id,
                        active = active.load(Ordering::Relaxed),
                        "vsock relay: connection above the per-VM bound closed — the guest transport retries with backoff"
                    );
                    // Drop closes it immediately; the guest sees
                    // connect-then-EOF and retries.
                    continue;
                }
                active.fetch_add(1, Ordering::Relaxed);
                let active = Arc::clone(&active);
                let cp_target = cp_target.clone();
                let idle_timeout = limits.idle_timeout;
                let vm_id = vm_id.clone();
                tokio::spawn(async move {
                    relay_connection(&vm_id, conn, cp_target, idle_timeout).await;
                    active.fetch_sub(1, Ordering::Relaxed);
                });
            }
            Err(e) => {
                warn!(
                    vm_id = %vm_id,
                    error = %e,
                    "vsock relay: accept failed — relay exiting (the next VM lifecycle operation restarts it)"
                );
                return;
            }
        }
    }
}

/// Pipe one guest connection to the control plane. This is the whole
/// relay: connect, splice bytes both ways, close on idle. No parsing,
/// no inspection, no authentication — the TLS session is between the
/// guest and the control plane and passes through opaquely.
async fn relay_connection(
    vm_id: &str,
    guest: UnixStream,
    cp_target: String,
    idle_timeout: std::time::Duration,
) {
    let cp = match TcpStream::connect(&cp_target).await {
        Ok(cp) => cp,
        Err(e) => {
            warn!(
                vm_id = %vm_id,
                cp_target = %cp_target,
                error = %e,
                "vsock relay: cannot reach the control plane — closing the guest connection (the guest transport retries with backoff)"
            );
            return;
        }
    };
    let last_activity_ms = Arc::new(AtomicU64::new(now_unix_ms()));
    debug!(
        vm_id = %vm_id,
        cp_target = %cp_target,
        "vsock relay connection admitted (piping bytes, no inspection)"
    );
    let mut guest = ActivityTrackingStream::new(guest, Arc::clone(&last_activity_ms));
    let mut cp = ActivityTrackingStream::new(cp, Arc::clone(&last_activity_ms));
    tokio::select! {
        result = tokio::io::copy_bidirectional(&mut guest, &mut cp) => {
            if let Err(e) = result {
                debug!(vm_id = %vm_id, error = %e, "vsock relay connection ended");
            }
        }
        _ = idle_watchdog(last_activity_ms, idle_timeout) => {
            debug!(
                vm_id = %vm_id,
                "vsock relay connection closed after the idle timeout (paused guest or wedged link)"
            );
        }
    }
}

/// A stream wrapper that stamps a shared "last activity" clock on every
/// successful read and write, so the idle watchdog can distinguish a
/// quiet-but-alive pipe from a dead one. Only the data plane is
/// tracked; flush/shutdown are control-plane and do not reset the
/// idle clock.
struct ActivityTrackingStream<S> {
    inner: S,
    last_activity_ms: Arc<AtomicU64>,
}

impl<S> ActivityTrackingStream<S> {
    fn new(inner: S, last_activity_ms: Arc<AtomicU64>) -> Self {
        Self {
            inner,
            last_activity_ms,
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for ActivityTrackingStream<S> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        match std::pin::Pin::new(&mut this.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                if !buf.filled().is_empty() {
                    this.last_activity_ms
                        .store(now_unix_ms(), Ordering::Relaxed);
                }
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for ActivityTrackingStream<S> {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        match std::pin::Pin::new(&mut this.inner).poll_write(cx, buf) {
            Poll::Ready(Ok(n)) => {
                if n > 0 {
                    this.last_activity_ms
                        .store(now_unix_ms(), Ordering::Relaxed);
                }
                Poll::Ready(Ok(n))
            }
            other => other,
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        std::pin::Pin::new(&mut this.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        std::pin::Pin::new(&mut this.inner).poll_shutdown(cx)
    }
}

/// Ends when no data has crossed the pipe for `idle_timeout` (in either
/// direction — see [`ActivityTrackingStream`]). Sleeps until the CURRENT
/// deadline and re-checks, so activity pushed the deadline forward makes
/// it sleep again instead of firing.
async fn idle_watchdog(last_activity_ms: Arc<AtomicU64>, idle_timeout: std::time::Duration) {
    let timeout_ms = idle_timeout.as_millis().min(u64::MAX as u128) as u64;
    loop {
        let deadline_ms = last_activity_ms
            .load(Ordering::Relaxed)
            .saturating_add(timeout_ms);
        let now = now_unix_ms();
        if now >= deadline_ms {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(deadline_ms - now)).await;
    }
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// A fake control plane: accepts TCP connections and echoes every
    /// byte back (so a test client can verify BOTH pipe directions with
    /// one exchange).
    async fn spawn_echo_cp() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut conn, _)) = listener.accept().await else {
                    return;
                };
                tokio::spawn(async move {
                    let (mut read_half, mut write_half) = conn.split();
                    let _ = tokio::io::copy(&mut read_half, &mut write_half).await;
                });
            }
        });
        format!("{}", addr)
    }

    fn vm_dir_with_vsock(tmp: &tempfile::TempDir, device_socket: &str) -> PathBuf {
        let vm_dir = tmp.path().join("vms").join("vm-test");
        std::fs::create_dir_all(&vm_dir).unwrap();
        std::fs::write(
            vm_config_file(&vm_dir),
            serde_json::json!({
                "cpus": 1,
                "vsock": { "cid": 1000, "socket": device_socket }
            })
            .to_string(),
        )
        .unwrap();
        vm_dir
    }

    fn vm_dir_without_vsock(tmp: &tempfile::TempDir) -> PathBuf {
        let vm_dir = tmp.path().join("vms").join("vm-plain");
        std::fs::create_dir_all(&vm_dir).unwrap();
        std::fs::write(
            vm_config_file(&vm_dir),
            serde_json::json!({"cpus": 1}).to_string(),
        )
        .unwrap();
        vm_dir
    }

    #[test]
    fn cp_target_parse_accepts_schemes_and_defaults_ports() {
        assert_eq!(
            parse_cp_tcp_target("https://cp.example:7443").unwrap(),
            "cp.example:7443"
        );
        assert_eq!(
            parse_cp_tcp_target("http://cp.example:7000").unwrap(),
            "cp.example:7000"
        );
        assert_eq!(
            parse_cp_tcp_target("https://cp.example").unwrap(),
            "cp.example:443"
        );
        assert_eq!(
            parse_cp_tcp_target("http://cp.example").unwrap(),
            "cp.example:80"
        );
        assert_eq!(
            parse_cp_tcp_target("cp.example:7443").unwrap(),
            "cp.example:7443"
        );
        // Path suffixes are stripped.
        assert_eq!(
            parse_cp_tcp_target("https://cp.example:7443/api").unwrap(),
            "cp.example:7443"
        );
        assert!(parse_cp_tcp_target("").is_err());
        assert!(parse_cp_tcp_target("https://").is_err());
        assert!(parse_cp_tcp_target("no-scheme-no-port").is_err());
        assert!(parse_cp_tcp_target("https://[::1]:7443").is_err());
    }

    #[test]
    fn relay_socket_path_follows_the_muxer_convention() {
        // Byte-exact contract with cloud-hypervisor v53.0's vsock muxer:
        // guest port P maps onto "<device_socket>_<P>", and the port is
        // the fixed monitoring contract constant.
        assert_eq!(
            relay_socket_path("/var/lib/chv/agent/vms/vm-1/vsock.sock"),
            PathBuf::from(format!(
                "/var/lib/chv/agent/vms/vm-1/vsock.sock_{}",
                chv_hypervisor_api::VSOCK_MONITOR_PORT
            ))
        );
    }

    #[tokio::test]
    async fn relay_pipes_bytes_bidirectionally_to_the_cp() {
        let cp = spawn_echo_cp().await;
        let manager = VsockRelayManager::new(&format!("http://{cp}")).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let device_socket = tmp.path().join("vsock.sock");
        let vm_dir = vm_dir_with_vsock(&tmp, device_socket.to_str().unwrap());
        let relay_socket = relay_socket_path(device_socket.to_str().unwrap());

        manager.ensure_started("vm-test", &vm_dir);
        assert!(relay_socket.exists(), "the relay binds synchronously");

        // A fake "VMM": connect to the relay endpoint and exchange bytes.
        let mut guest = UnixStream::connect(&relay_socket).await.unwrap();
        guest.write_all(b"hello-cp").await.unwrap();
        let mut buf = [0u8; 8];
        guest.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello-cp", "the echo CP answer rode the pipe back");

        // Idempotent: a second ensure does not disturb the endpoint.
        manager.ensure_started("vm-test", &vm_dir);
        guest.write_all(b"again").await.unwrap();
        let mut buf = [0u8; 5];
        guest.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"again");

        manager.stop("vm-test");
        assert!(!relay_socket.exists(), "stop unlinks the endpoint");
        assert!(UnixStream::connect(&relay_socket).await.is_err());
    }

    #[tokio::test]
    async fn relay_is_gated_on_the_persisted_vsock_device() {
        let cp = spawn_echo_cp().await;
        let manager = VsockRelayManager::new(&format!("http://{cp}")).unwrap();
        let tmp = tempfile::tempdir().unwrap();

        // No vsock device in the payload: no relay, no socket, no error.
        let vm_dir = vm_dir_without_vsock(&tmp);
        manager.ensure_started("vm-plain", &vm_dir);
        assert!(
            !relay_socket_path(vm_dir.join("vsock.sock").to_str().unwrap()).exists(),
            "a non-vsock VM must not get a relay endpoint"
        );
        // And an unreadable/missing payload is the same opt-out.
        manager.ensure_started("vm-gone", &tmp.path().join("vms").join("vm-gone"));
    }

    #[tokio::test]
    async fn relay_closes_connections_above_the_per_vm_bound() {
        let cp = spawn_echo_cp().await;
        let manager = VsockRelayManager::with_limits(
            &format!("http://{cp}"),
            VsockRelayLimits {
                max_connections: 1,
                ..VsockRelayLimits::default()
            },
        )
        .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let device_socket = tmp.path().join("vsock.sock");
        let vm_dir = vm_dir_with_vsock(&tmp, device_socket.to_str().unwrap());
        let relay_socket = relay_socket_path(device_socket.to_str().unwrap());
        manager.ensure_started("vm-test", &vm_dir);

        // First connection pipes.
        let mut first = UnixStream::connect(&relay_socket).await.unwrap();
        first.write_all(b"one").await.unwrap();
        let mut buf = [0u8; 3];
        first.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"one");

        // Second connection (above the bound) is closed immediately: a
        // read returns EOF without any data having crossed.
        let mut second = UnixStream::connect(&relay_socket).await.unwrap();
        let n = second.read(&mut buf).await.unwrap();
        assert_eq!(n, 0, "the over-bound connection must close immediately");

        // The first connection is unaffected.
        first.write_all(b"still").await.unwrap();
        let mut buf = [0u8; 5];
        first.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"still");
    }

    #[tokio::test]
    async fn relay_closes_idle_connections_after_the_timeout() {
        let cp = spawn_echo_cp().await;
        let manager = VsockRelayManager::with_limits(
            &format!("http://{cp}"),
            VsockRelayLimits {
                idle_timeout: std::time::Duration::from_millis(150),
                ..VsockRelayLimits::default()
            },
        )
        .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let device_socket = tmp.path().join("vsock.sock");
        let vm_dir = vm_dir_with_vsock(&tmp, device_socket.to_str().unwrap());
        let relay_socket = relay_socket_path(device_socket.to_str().unwrap());
        manager.ensure_started("vm-test", &vm_dir);

        let mut guest = UnixStream::connect(&relay_socket).await.unwrap();
        guest.write_all(b"ping").await.unwrap();
        let mut buf = [0u8; 4];
        guest.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"ping");

        // Idle: the watchdog must close the pipe (EOF for the guest)
        // within a bounded multiple of the timeout.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let n = guest.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the idle connection was not closed within 5 s"
            );
        }
    }

    #[tokio::test]
    async fn relay_replaces_a_stale_socket_file() {
        // An agent crash between bind and cleanup leaves the socket file
        // behind; a fresh ensure_started must unlink it and bind.
        let cp = spawn_echo_cp().await;
        let manager = VsockRelayManager::new(&format!("http://{cp}")).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let device_socket = tmp.path().join("vsock.sock");
        let vm_dir = vm_dir_with_vsock(&tmp, device_socket.to_str().unwrap());
        let relay_socket = relay_socket_path(device_socket.to_str().unwrap());
        std::fs::write(&relay_socket, b"stale").unwrap();

        manager.ensure_started("vm-test", &vm_dir);
        let mut guest = UnixStream::connect(&relay_socket).await.unwrap();
        guest.write_all(b"fresh").await.unwrap();
        let mut buf = [0u8; 5];
        guest.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"fresh");
    }

    #[tokio::test]
    async fn relay_restarts_after_the_accept_loop_dies() {
        // A dead accept task (entry present, handle finished) must be
        // replaced by the next ensure_started, not pinned as "running".
        let cp = spawn_echo_cp().await;
        let manager = VsockRelayManager::new(&format!("http://{cp}")).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let device_socket = tmp.path().join("vsock.sock");
        let vm_dir = vm_dir_with_vsock(&tmp, device_socket.to_str().unwrap());
        let relay_socket = relay_socket_path(device_socket.to_str().unwrap());

        manager.ensure_started("vm-test", &vm_dir);
        // Simulate the accept loop dying: stop() removed the entry, so
        // instead force-finish the task via its handle.
        {
            let mut relays = manager.relays.lock().unwrap();
            let entry = relays.get_mut("vm-test").unwrap();
            entry.handle.abort();
        }
        // Give the abort a beat to land.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        manager.ensure_started("vm-test", &vm_dir);
        let mut guest = UnixStream::connect(&relay_socket).await.unwrap();
        guest.write_all(b"revived").await.unwrap();
        let mut buf = [0u8; 7];
        guest.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"revived");
    }
}
