use axum::Router;
use cellhv_core_operations::AuthorityHandle;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::{conn::auto::Builder, graceful::GracefulShutdown},
    service::TowerToHyperService,
};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::task::{JoinHandle, JoinSet};

use crate::{bind_private_owned, router, BindError, ExistingSocketPolicy};

#[derive(Debug, Error)]
pub enum ListenerError {
    #[error("failed to bind the Core API listener: {0}")]
    Bind(#[from] BindError),
    #[error("Core API listener accept failed: {0}")]
    Accept(#[source] std::io::Error),
    #[error("Core API connection failed: {0}")]
    Connection(String),
    #[error("Core API connection task panicked: {0}")]
    ConnectionTask(#[source] tokio::task::JoinError),
    #[error("Core API listener task panicked: {0}")]
    Join(#[source] tokio::task::JoinError),
    #[error("Core API graceful drain exceeded {0:?}")]
    DrainTimeout(std::time::Duration),
}

/// Exclusive owner of the native Core HTTP listener task and socket inode.
///
/// This is transport lifecycle only. It routes to the supplied process-wide
/// authority handle and creates no operation service or serialization actor.
pub struct CoreApiListener {
    socket: PathBuf,
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<JoinHandle<Result<(), ListenerError>>>,
}

impl CoreApiListener {
    /// Starts a listener for a caller that already owns the process-wide
    /// authority lease. A socket left by an unclean exit may be recovered,
    /// but live listeners and foreign filesystem objects are never replaced.
    pub async fn start_authority_owned_with_drain_timeout(
        socket: &Path,
        authority: AuthorityHandle,
        drain_timeout: std::time::Duration,
    ) -> Result<Self, ListenerError> {
        Self::start_router_with_policy(
            socket,
            router(authority),
            drain_timeout,
            ExistingSocketPolicy::RecoverStale,
        )
        .await
    }

    async fn start_router_with_policy(
        socket: &Path,
        app: Router,
        drain_timeout: std::time::Duration,
        existing_socket_policy: ExistingSocketPolicy,
    ) -> Result<Self, ListenerError> {
        if drain_timeout.is_zero() {
            return Err(ListenerError::DrainTimeout(drain_timeout));
        }
        let (listener, identity) = bind_private_owned(socket, existing_socket_policy).await?;
        let socket = socket.to_path_buf();
        let cleanup = SocketCleanup {
            path: socket.clone(),
            identity,
        };
        let (shutdown, stopped) = oneshot::channel();
        let join = tokio::spawn(run(listener, app, stopped, cleanup, drain_timeout));
        Ok(Self {
            socket,
            shutdown: Some(shutdown),
            join: Some(join),
        })
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket
    }

    /// Stops accepting, gracefully drains in-flight requests, and propagates
    /// listener or task panics. The owned socket is removed only if its inode
    /// still matches the one created during startup.
    pub async fn shutdown(mut self) -> Result<(), ListenerError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.join
            .take()
            .expect("listener join handle is owned until shutdown")
            .await
            .map_err(ListenerError::Join)?
    }
}

impl Drop for CoreApiListener {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SocketIdentity {
    pub(crate) device: u64,
    pub(crate) inode: u64,
}

pub(crate) fn open_socket_path(path: &Path) -> std::io::Result<(std::fs::File, SocketIdentity)> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_PATH | nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_socket() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Core API path fd is not a Unix socket",
        ));
    }
    Ok((
        file,
        SocketIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        },
    ))
}

pub(crate) fn set_socket_mode(
    path: &Path,
    path_file: &std::fs::File,
    identity: SocketIdentity,
) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    set_socket_mode_with(path, identity, || {
        if unsafe {
            nix::libc::syscall(
                nix::libc::SYS_fchmodat2,
                path_file.as_raw_fd(),
                c"".as_ptr(),
                0o600,
                nix::libc::AT_EMPTY_PATH,
            )
        } == 0
        {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    })
}

fn set_socket_mode_with(
    path: &Path,
    identity: SocketIdentity,
    fchmodat2: impl FnOnce() -> std::io::Result<()>,
) -> std::io::Result<()> {
    match fchmodat2() {
        Ok(()) => return Ok(()),
        Err(error)
            if matches!(
                error.raw_os_error(),
                Some(nix::libc::ENOSYS | nix::libc::EINVAL | nix::libc::EOPNOTSUPP)
            ) => {}
        Err(error) => return Err(error),
    }
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    let (_, verified) = open_socket_path(path)?;
    let metadata = std::fs::symlink_metadata(path)?;
    if verified.device != identity.device
        || verified.inode != identity.inode
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Core API socket changed during chmod fallback",
        ));
    }
    Ok(())
}

pub(crate) fn remove_matching_socket(path: &Path, identity: SocketIdentity) {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_socket()
        && metadata.dev() == identity.device
        && metadata.ino() == identity.inode
    {
        let _ = std::fs::remove_file(path);
    }
}

struct SocketCleanup {
    path: PathBuf,
    identity: SocketIdentity,
}

impl Drop for SocketCleanup {
    fn drop(&mut self) {
        remove_matching_socket(&self.path, self.identity);
    }
}

async fn run(
    listener: tokio::net::UnixListener,
    app: Router,
    mut shutdown: oneshot::Receiver<()>,
    _cleanup: SocketCleanup,
    drain_timeout: std::time::Duration,
) -> Result<(), ListenerError> {
    let graceful = GracefulShutdown::new();
    let mut connections = JoinSet::new();
    let accept_result = loop {
        tokio::select! {
            _ = &mut shutdown => break Ok(()),
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    let service = TowerToHyperService::new(app.clone());
                    let watcher = graceful.watcher();
                    connections.spawn(async move {
                        let builder = Builder::new(TokioExecutor::new());
                        watcher
                            .watch(builder.serve_connection_with_upgrades(TokioIo::new(stream), service))
                            .await
                            .map_err(|error| ListenerError::Connection(error.to_string()))
                    });
                }
                Err(error) => break Err(ListenerError::Accept(error)),
            },
            Some(completed) = connections.join_next(), if !connections.is_empty() => {
                match completed {
                    Ok(Ok(())) => {}
                    Ok(Err(ListenerError::Connection(error))) => {
                        tracing::warn!(error = %error, "Core API client connection ended with a protocol error");
                    }
                    Ok(Err(error)) => break Err(error),
                    Err(error) => break Err(ListenerError::ConnectionTask(error)),
                }
            },
        }
    };
    drop(listener);
    if tokio::time::timeout(drain_timeout, graceful.shutdown())
        .await
        .is_err()
    {
        connections.abort_all();
        while connections.join_next().await.is_some() {}
        return Err(ListenerError::DrainTimeout(drain_timeout));
    }
    while let Some(result) = connections.join_next().await {
        match result.map_err(ListenerError::ConnectionTask)? {
            Ok(()) => {}
            Err(ListenerError::Connection(error)) => {
                tracing::warn!(error = %error, "Core API client connection ended during drain");
            }
            Err(error) => return Err(error),
        }
    }
    accept_result
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use std::os::unix::fs::PermissionsExt;

    const DEFAULT_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn socket_path() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().join("core.sock");
        (directory, path)
    }

    async fn request(path: &Path, target: &str) -> String {
        let mut stream = tokio::net::UnixStream::connect(path).await.unwrap();
        stream
            .write_all(
                format!("GET {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        response
    }

    #[tokio::test]
    async fn serves_http_and_removes_owned_socket() {
        let (_directory, path) = socket_path();
        let owner = CoreApiListener::start_router_with_policy(
            &path,
            Router::new().route("/ready", get(|| async { "ready" })),
            DEFAULT_DRAIN_TIMEOUT,
            ExistingSocketPolicy::Refuse,
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::symlink_metadata(&path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let response = request(&path, "/ready").await;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("ready"));
        owner.shutdown().await.unwrap();
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn chmod_fallback_is_injected_and_revalidates_original_inode() {
        let (_directory, path) = socket_path();
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let (_path_file, identity) = open_socket_path(&path).unwrap();
        set_socket_mode_with(&path, identity, || {
            Err(std::io::Error::from_raw_os_error(nix::libc::ENOSYS))
        })
        .unwrap();
        let (_, verified) = open_socket_path(&path).unwrap();
        assert_eq!(
            (verified.device, verified.inode),
            (identity.device, identity.inode)
        );
        assert_eq!(
            std::fs::symlink_metadata(&path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        drop(listener);
        remove_matching_socket(&path, identity);
    }

    #[tokio::test]
    async fn malformed_client_does_not_stop_listener() {
        let (_directory, path) = socket_path();
        let owner = CoreApiListener::start_router_with_policy(
            &path,
            Router::new().route("/ready", get(|| async { "ready" })),
            DEFAULT_DRAIN_TIMEOUT,
            ExistingSocketPolicy::Refuse,
        )
        .await
        .unwrap();
        let mut malformed = tokio::net::UnixStream::connect(&path).await.unwrap();
        malformed
            .write_all(b"GET /ready HTTP/1.1\r\nBad")
            .await
            .unwrap();
        drop(malformed);
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(request(&path, "/ready").await.starts_with("HTTP/1.1 200"));
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn concurrent_bind_refuses_to_replace_live_socket() {
        let (_directory, path) = socket_path();
        let owner = CoreApiListener::start_router_with_policy(
            &path,
            Router::new(),
            DEFAULT_DRAIN_TIMEOUT,
            ExistingSocketPolicy::Refuse,
        )
        .await
        .unwrap();
        assert!(matches!(
            CoreApiListener::start_router_with_policy(
                &path,
                Router::new(),
                DEFAULT_DRAIN_TIMEOUT,
                ExistingSocketPolicy::Refuse,
            )
            .await,
            Err(ListenerError::Bind(BindError::ExistingPath(_)))
        ));
        owner.shutdown().await.unwrap();

        std::fs::write(&path, b"foreign").unwrap();
        assert!(matches!(
            CoreApiListener::start_router_with_policy(
                &path,
                Router::new(),
                DEFAULT_DRAIN_TIMEOUT,
                ExistingSocketPolicy::Refuse,
            )
            .await,
            Err(ListenerError::Bind(BindError::ExistingPath(_)))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"foreign");
    }

    #[tokio::test]
    async fn authority_owned_start_recovers_stale_socket_inode() {
        let (_directory, path) = socket_path();
        let stale = tokio::net::UnixListener::bind(&path).unwrap();
        drop(stale);

        let owner = CoreApiListener::start_router_with_policy(
            &path,
            Router::new().route("/ready", get(|| async { "ready" })),
            DEFAULT_DRAIN_TIMEOUT,
            ExistingSocketPolicy::RecoverStale,
        )
        .await
        .unwrap();
        assert!(request(&path, "/ready").await.starts_with("HTTP/1.1 200"));
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn authority_owned_start_refuses_live_socket() {
        let (_directory, path) = socket_path();
        let live = tokio::net::UnixListener::bind(&path).unwrap();
        let identity = std::fs::symlink_metadata(&path).unwrap().ino();
        assert!(matches!(
            CoreApiListener::start_router_with_policy(
                &path,
                Router::new(),
                DEFAULT_DRAIN_TIMEOUT,
                ExistingSocketPolicy::RecoverStale,
            )
            .await,
            Err(ListenerError::Bind(BindError::ExistingPath(_)))
        ));
        assert_eq!(std::fs::symlink_metadata(&path).unwrap().ino(), identity);
        drop(live);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn authority_owned_start_preserves_regular_file_and_symlink() {
        let (directory, path) = socket_path();
        std::fs::write(&path, b"foreign").unwrap();
        assert!(matches!(
            CoreApiListener::start_router_with_policy(
                &path,
                Router::new(),
                DEFAULT_DRAIN_TIMEOUT,
                ExistingSocketPolicy::RecoverStale,
            )
            .await,
            Err(ListenerError::Bind(BindError::ExistingPath(_)))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"foreign");

        std::fs::remove_file(&path).unwrap();
        let target = directory.path().join("target");
        std::fs::write(&target, b"target").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(matches!(
            CoreApiListener::start_router_with_policy(
                &path,
                Router::new(),
                DEFAULT_DRAIN_TIMEOUT,
                ExistingSocketPolicy::RecoverStale,
            )
            .await,
            Err(ListenerError::Bind(BindError::ExistingPath(_)))
        ));
        assert_eq!(std::fs::read_link(&path).unwrap(), target);
    }

    #[tokio::test]
    async fn shutdown_drains_in_flight_request() {
        let (_directory, path) = socket_path();
        let entered = Arc::new(AtomicBool::new(false));
        let release = Arc::new(tokio::sync::Notify::new());
        let app = Router::new().route(
            "/slow",
            get({
                let entered = Arc::clone(&entered);
                let release = Arc::clone(&release);
                move || {
                    let entered = Arc::clone(&entered);
                    let release = Arc::clone(&release);
                    async move {
                        entered.store(true, Ordering::SeqCst);
                        release.notified().await;
                        "done"
                    }
                }
            }),
        );
        let owner = CoreApiListener::start_router_with_policy(
            &path,
            app,
            DEFAULT_DRAIN_TIMEOUT,
            ExistingSocketPolicy::Refuse,
        )
        .await
        .unwrap();
        let request_path = path.clone();
        let request_task = tokio::spawn(async move { request(&request_path, "/slow").await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !entered.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let shutdown = tokio::spawn(owner.shutdown());
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!shutdown.is_finished());
        release.notify_one();
        assert!(request_task.await.unwrap().ends_with("done"));
        shutdown.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn foreign_replacement_is_not_removed_on_shutdown() {
        let (_directory, path) = socket_path();
        let owner = CoreApiListener::start_router_with_policy(
            &path,
            Router::new(),
            DEFAULT_DRAIN_TIMEOUT,
            ExistingSocketPolicy::Refuse,
        )
        .await
        .unwrap();
        std::fs::remove_file(&path).unwrap();
        let foreign = tokio::net::UnixListener::bind(&path).unwrap();
        let foreign_identity = std::fs::symlink_metadata(&path).unwrap().ino();
        owner.shutdown().await.unwrap();
        assert_eq!(
            std::fs::symlink_metadata(&path).unwrap().ino(),
            foreign_identity
        );
        drop(foreign);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn listener_task_panic_is_propagated_by_explicit_join() {
        let (_directory, path) = socket_path();
        let (shutdown, _stopped) = oneshot::channel();
        let owner = CoreApiListener {
            socket: path,
            shutdown: Some(shutdown),
            join: Some(tokio::spawn(async {
                panic!("injected listener panic");
                #[allow(unreachable_code)]
                Ok(())
            })),
        };
        assert!(
            matches!(owner.shutdown().await, Err(ListenerError::Join(error)) if error.is_panic())
        );
    }

    #[tokio::test]
    async fn drain_timeout_aborts_remaining_connection_and_is_structured() {
        let (_directory, path) = socket_path();
        let entered = Arc::new(AtomicBool::new(false));
        let app = Router::new().route(
            "/blocked",
            get({
                let entered = Arc::clone(&entered);
                move || {
                    let entered = Arc::clone(&entered);
                    async move {
                        entered.store(true, Ordering::SeqCst);
                        std::future::pending::<&'static str>().await
                    }
                }
            }),
        );
        let timeout = Duration::from_millis(20);
        let owner = CoreApiListener::start_router_with_policy(
            &path,
            app,
            timeout,
            ExistingSocketPolicy::Refuse,
        )
        .await
        .unwrap();
        let request_path = path.clone();
        let request = tokio::spawn(async move { request(&request_path, "/blocked").await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !entered.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(matches!(
            owner.shutdown().await,
            Err(ListenerError::DrainTimeout(value)) if value == timeout
        ));
        request.abort();
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn dropping_owner_aborts_blocked_handler_and_removes_socket() {
        struct DropSignal(Arc<AtomicBool>);
        impl Drop for DropSignal {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let (_directory, path) = socket_path();
        let entered = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicBool::new(false));
        let app = Router::new().route(
            "/blocked",
            get({
                let entered = Arc::clone(&entered);
                let dropped = Arc::clone(&dropped);
                move || {
                    let entered = Arc::clone(&entered);
                    let dropped = Arc::clone(&dropped);
                    async move {
                        let _signal = DropSignal(dropped);
                        entered.store(true, Ordering::SeqCst);
                        std::future::pending::<&'static str>().await
                    }
                }
            }),
        );
        let owner = CoreApiListener::start_router_with_policy(
            &path,
            app,
            DEFAULT_DRAIN_TIMEOUT,
            ExistingSocketPolicy::Refuse,
        )
        .await
        .unwrap();
        let request_path = path.clone();
        let request = tokio::spawn(async move { request(&request_path, "/blocked").await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !entered.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(owner);
        tokio::time::timeout(Duration::from_secs(2), async {
            while path.exists() || !dropped.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        request.abort();
    }
}
