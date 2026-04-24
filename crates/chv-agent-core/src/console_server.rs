use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::os::fd::{AsRawFd, FromRawFd};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

nix::ioctl_write_ptr_bad!(set_winsize, nix::libc::TIOCSWINSZ, nix::libc::winsize);

#[derive(Clone)]
pub struct ConsoleServer {
    vm_runtime: crate::vm_runtime::VmRuntime,
    jwt_secret: String,
    rate_limiter: Arc<Mutex<HashMap<String, Instant>>>,
    consumed_tokens: Arc<Mutex<HashMap<String, Instant>>>,
}

#[derive(Clone)]
struct ConsoleState {
    vm_runtime: crate::vm_runtime::VmRuntime,
    jwt_secret: String,
    rate_limiter: Arc<Mutex<HashMap<String, Instant>>>,
    consumed_tokens: Arc<Mutex<HashMap<String, Instant>>>,
}

#[derive(serde::Deserialize)]
struct ConsoleParams {
    token: String,
}

#[derive(serde::Deserialize)]
struct ResizeMsg {
    cols: u16,
    rows: u16,
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct Claims {
    sub: String,
    username: String,
    exp: u64,
}

impl ConsoleServer {
    pub fn new(vm_runtime: crate::vm_runtime::VmRuntime, jwt_secret: String) -> Self {
        Self {
            vm_runtime,
            jwt_secret,
            rate_limiter: Arc::new(Mutex::new(HashMap::new())),
            consumed_tokens: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn try_bind(bind: &str) -> Result<TcpListener, chv_errors::ChvError> {
        TcpListener::bind(bind).await.map_err(|e| chv_errors::ChvError::Io {
            path: bind.to_string(),
            source: e,
        })
    }

    fn router(self) -> Router {
        let state = ConsoleState {
            vm_runtime: self.vm_runtime.clone(),
            jwt_secret: self.jwt_secret.clone(),
            rate_limiter: self.rate_limiter.clone(),
            consumed_tokens: self.consumed_tokens.clone(),
        };

        // Spawn periodic cleanup of rate limiter and consumed token cache
        let rate_limiter = self.rate_limiter.clone();
        let consumed_tokens = self.consumed_tokens.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                let now = Instant::now();
                let cutoff = Duration::from_secs(120);
                if let Ok(mut limits) = rate_limiter.lock() {
                    limits.retain(|_, last| now.duration_since(*last) < cutoff);
                }
                if let Ok(mut tokens) = consumed_tokens.lock() {
                    tokens.retain(|_, last| now.duration_since(*last) < cutoff);
                }
            }
        });

        Router::new()
            .route("/vms/:vm_id/console", get(Self::ws_handler))
            .with_state(state)
    }

    pub async fn run(self, listener: TcpListener) -> Result<(), chv_errors::ChvError> {
        let app = self.router();
        axum::serve(listener, app).await.map_err(|e| chv_errors::ChvError::Internal {
            reason: format!("console server error: {}", e),
        })?;
        Ok(())
    }

    fn check_rate_limit(
        vm_id: &str,
        rate_limiter: &Arc<Mutex<HashMap<String, Instant>>>,
    ) -> Option<Response> {
        const RATE_LIMIT_SECS: u64 = 2;
        let mut limits = rate_limiter.lock().unwrap();
        let now = Instant::now();
        if let Some(last) = limits.get(vm_id) {
            if now.duration_since(*last) < Duration::from_secs(RATE_LIMIT_SECS) {
                tracing::warn!(vm_id = %vm_id, "console connection rate limited");
                return Some(StatusCode::TOO_MANY_REQUESTS.into_response());
            }
        }
        limits.insert(vm_id.to_string(), now);
        None
    }

    fn check_replay(
        token: &str,
        consumed_tokens: &Arc<Mutex<HashMap<String, Instant>>>,
    ) -> Option<Response> {
        let mut tokens = consumed_tokens.lock().unwrap();
        if tokens.contains_key(token) {
            return Some(StatusCode::UNAUTHORIZED.into_response());
        }
        tokens.insert(token.to_string(), Instant::now());
        None
    }

    async fn ws_handler(
        State(state): State<ConsoleState>,
        Path(vm_id): Path<String>,
        Query(params): Query<ConsoleParams>,
        ws: WebSocketUpgrade,
    ) -> Response {
        if let Some(response) = Self::check_rate_limit(&vm_id, &state.rate_limiter) {
            return response;
        }

        if let Err(e) = validate_console_token(&params.token, &state.jwt_secret) {
            tracing::warn!(error = %e, "console token validation failed");
            return StatusCode::UNAUTHORIZED.into_response();
        }

        if let Some(response) = Self::check_replay(&params.token, &state.consumed_tokens) {
            tracing::warn!(vm_id = %vm_id, "console token replay detected");
            return response;
        }

        let vm_runtime = state.vm_runtime.clone();
        ws.on_upgrade(move |socket| Self::handle_socket(socket, vm_id, vm_runtime))
    }

    async fn handle_socket(
        socket: axum::extract::ws::WebSocket,
        vm_id: String,
        vm_runtime: crate::vm_runtime::VmRuntime,
    ) {
        let pty_fd = {
            let mut attempts = 0;
            loop {
                match vm_runtime.pty_master(&vm_id) {
                    Some(fd) => break fd,
                    None => {
                        attempts += 1;
                        if attempts >= 10 {
                            tracing::warn!(vm_id = %vm_id, "no pty master for vm after 10 retries");
                            return;
                        }
                        tracing::debug!(vm_id = %vm_id, attempt = attempts, "pty not ready, retrying");
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                }
            }
        };

        let raw_fd = pty_fd.as_raw_fd();

        // Obtain broadcast channel for PTY output
        let mut pty_rx = {
            let mut attempts = 0;
            loop {
                match vm_runtime.pty_output_rx(&vm_id) {
                    Some(rx) => break rx,
                    None => {
                        attempts += 1;
                        if attempts >= 10 {
                            tracing::warn!(vm_id = %vm_id, "no pty broadcast channel for vm after 10 retries");
                            return;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                }
            }
        };

        let (mut ws_tx, mut ws_rx) = socket.split();

        // Send scrollback history before subscribing to live feed so the
        // client sees previous console output immediately on connect.
        if let Some(scrollback) = vm_runtime.pty_scrollback(&vm_id) {
            const CHUNK_SIZE: usize = 32 * 1024;
            for chunk in scrollback.chunks(CHUNK_SIZE) {
                let msg = axum::extract::ws::Message::Binary(chunk.to_vec());
                if ws_tx.send(msg).await.is_err() {
                    drop(pty_fd);
                    return;
                }
            }
        }

        // PTY broadcast → WebSocket
        let mut read_task = tokio::spawn(async move {
            loop {
                match pty_rx.recv().await {
                    Ok(data) => {
                        let msg = axum::extract::ws::Message::Binary(data);
                        if ws_tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // If lagged, continue reading latest data
                    }
                }
            }
        });

        // WebSocket → PTY
        let mut write_task = tokio::spawn(async move {
            // Dup fd for tokio async write
            let dup_fd = unsafe { libc::dup(raw_fd) };
            if dup_fd < 0 {
                tracing::warn!(error = %std::io::Error::last_os_error(), "failed to dup pty fd for write");
                return;
            }
            let std_file = unsafe { std::fs::File::from_raw_fd(dup_fd) };
            let tokio_file = tokio::fs::File::from_std(std_file);
            let mut pty_writer = tokio_file;

            while let Some(result) = ws_rx.next().await {
                match result {
                    Ok(axum::extract::ws::Message::Text(text)) => {
                        if let Ok(resize) = serde_json::from_str::<ResizeMsg>(&text) {
                            set_pty_size(raw_fd, resize.cols, resize.rows);
                        } else if pty_writer.write_all(text.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                    Ok(axum::extract::ws::Message::Binary(data)) => {
                        if pty_writer.write_all(&data).await.is_err() {
                            break;
                        }
                    }
                    Ok(axum::extract::ws::Message::Close(_)) => break,
                    Err(e) => {
                        tracing::debug!(error = %e, "websocket receive error");
                        break;
                    }
                    _ => {}
                }
            }
        });

        tokio::select! {
            _ = &mut read_task => {
                write_task.abort();
            },
            _ = &mut write_task => {
                read_task.abort();
            },
        }

        drop(pty_fd);
    }
}

fn set_pty_size(fd: std::os::fd::RawFd, cols: u16, rows: u16) {
    let ws = nix::libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe {
        if let Err(e) = set_winsize(fd, &ws) {
            tracing::warn!(error = %e, "failed to set pty size");
        }
    }
}

/// Validate a JWT token against the given secret using HS256.
/// Returns Ok(Claims) if the token is valid and not expired, Err otherwise.
fn validate_console_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let decoding_key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.validate_aud = false;
    jsonwebtoken::decode::<Claims>(token, &decoding_key, &validation).map(|d| d.claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_secret() -> String {
        "test-secret-do-not-use-in-production".to_string()
    }

    fn encode_claims(sub: &str, username: &str, exp: u64, secret: &str) -> String {
        #[derive(serde::Serialize)]
        struct TestClaims<'a> {
            sub: &'a str,
            username: &'a str,
            exp: u64,
        }
        let claims = TestClaims { sub, username, exp };
        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
        jsonwebtoken::encode(
            &header,
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("encoding should succeed in tests")
    }

    fn future_exp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600
    }

    #[test]
    fn valid_jwt_is_accepted() {
        let token = encode_claims("user-1", "admin", future_exp(), &test_secret());
        let result = validate_console_token(&token, &test_secret());
        assert!(result.is_ok(), "valid JWT should be accepted, got: {:?}", result.err());
    }

    #[test]
    fn expired_jwt_is_rejected() {
        // exp = 1 means epoch 1 second — long expired
        let token = encode_claims("user-1", "admin", 1, &test_secret());
        let result = validate_console_token(&token, &test_secret());
        assert!(result.is_err(), "expired JWT should be rejected");
    }

    #[test]
    fn empty_token_is_rejected() {
        let result = validate_console_token("", &test_secret());
        assert!(result.is_err(), "empty token should be rejected");
    }

    #[test]
    fn malformed_token_is_rejected() {
        let result = validate_console_token("not-a-valid-jwt", &test_secret());
        assert!(result.is_err(), "malformed token should be rejected");
    }

    #[test]
    fn wrong_secret_is_rejected() {
        let token = encode_claims("user-1", "admin", future_exp(), "wrong-secret");
        let result = validate_console_token(&token, &test_secret());
        assert!(result.is_err(), "token signed with wrong secret should be rejected");
    }

    #[tokio::test]
    async fn try_bind_fails_when_port_in_use() {
        // Bind a temporary listener to occupy the port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // try_bind should fail because the port is already occupied
        let result = ConsoleServer::try_bind(&addr.to_string()).await;
        assert!(
            result.is_err(),
            "try_bind should fail when port is already in use"
        );
        let err = result.unwrap_err();
        let err_str = format!("{}", err);
        assert!(
            err_str.contains("Address already in use") || err_str.contains("io error"),
            "error should indicate address in use, got: {}",
            err_str
        );
    }

    fn test_console_server() -> ConsoleServer {
        let adapter: Arc<dyn chv_agent_runtime_ch::CloudHypervisorAdapter> =
            Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default());
        let vm_runtime = crate::vm_runtime::VmRuntime::new(adapter);
        ConsoleServer::new(vm_runtime, test_secret())
    }

    fn ws_upgrade_request(uri: &str) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::get(uri)
            .header("upgrade", "websocket")
            .header("connection", "upgrade")
            .header("sec-websocket-version", "13")
            .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
            .body(axum::body::Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn missing_token_returns_bad_request() {
        use tower::ServiceExt;
        let app = test_console_server().router();
        let response = app
            .oneshot(ws_upgrade_request("/vms/test-vm/console"))
            .await
            .unwrap();
        // Axum's Query<ConsoleParams> fails to deserialize when token is missing
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn valid_token_without_ws_headers_returns_bad_request() {
        use tower::ServiceExt;
        let app = test_console_server().router();
        let token = encode_claims("test-vm", "admin", future_exp(), &test_secret());
        let response = app
            .oneshot(
                axum::http::Request::get(&format!("/vms/test-vm/console?token={}", token))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // WebSocketUpgrade extractor requires upgrade headers
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    }

    #[test]
    fn rate_limit_blocks_rapid_requests() {
        let rate_limiter = Arc::new(Mutex::new(HashMap::new()));
        // First request should pass
        assert!(ConsoleServer::check_rate_limit("vm-1", &rate_limiter).is_none());
        // Immediate second request should be blocked
        assert!(
            ConsoleServer::check_rate_limit("vm-1", &rate_limiter).is_some(),
            "rapid request should be rate limited"
        );
        // Different VM should pass
        assert!(ConsoleServer::check_rate_limit("vm-2", &rate_limiter).is_none());
    }

    #[test]
    fn rate_limit_allows_after_cooldown() {
        let rate_limiter = Arc::new(Mutex::new(HashMap::new()));
        assert!(ConsoleServer::check_rate_limit("vm-1", &rate_limiter).is_none());
        assert!(ConsoleServer::check_rate_limit("vm-1", &rate_limiter).is_some());
        // Manually expire the entry
        rate_limiter.lock().unwrap().insert("vm-1".to_string(), Instant::now() - Duration::from_secs(10));
        assert!(ConsoleServer::check_rate_limit("vm-1", &rate_limiter).is_none());
    }

    #[test]
    fn replay_prevention_blocks_reused_token() {
        let consumed = Arc::new(Mutex::new(HashMap::new()));
        let token = "token-abc";
        // First use should pass
        assert!(ConsoleServer::check_replay(token, &consumed).is_none());
        // Reuse should be blocked
        assert!(
            ConsoleServer::check_replay(token, &consumed).is_some(),
            "reused token should be blocked"
        );
        // Different token should pass
        assert!(ConsoleServer::check_replay("token-def", &consumed).is_none());
    }
}
