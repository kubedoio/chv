use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use std::os::fd::{AsRawFd, FromRawFd};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct ConsoleServer {
    vm_runtime: crate::vm_runtime::VmRuntime,
    jwt_secret: String,
}

#[derive(Clone)]
struct ConsoleState {
    vm_runtime: crate::vm_runtime::VmRuntime,
    jwt_secret: String,
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
        Self { vm_runtime, jwt_secret }
    }

    pub async fn run(self, bind: &str) -> Result<(), chv_errors::ChvError> {
        let state = ConsoleState {
            vm_runtime: self.vm_runtime.clone(),
            jwt_secret: self.jwt_secret.clone(),
        };
        let app = Router::new()
            .route("/vms/:vm_id/console", get(Self::ws_handler))
            .with_state(state);

        let listener = TcpListener::bind(bind).await.map_err(|e| chv_errors::ChvError::Io {
            path: bind.to_string(),
            source: e,
        })?;

        axum::serve(listener, app).await.map_err(|e| chv_errors::ChvError::Internal {
            reason: format!("console server error: {}", e),
        })?;
        Ok(())
    }

    async fn ws_handler(
        State(state): State<ConsoleState>,
        ws: WebSocketUpgrade,
        Path(vm_id): Path<String>,
        Query(params): Query<ConsoleParams>,
    ) -> Response {
        // Decode and validate JWT
        match validate_console_token(&params.token, &state.jwt_secret) {
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "console token validation failed");
                return StatusCode::UNAUTHORIZED.into_response();
            }
        }
        let vm_runtime = state.vm_runtime.clone();
        ws.on_upgrade(move |socket| Self::handle_socket(socket, vm_id, vm_runtime))
    }

    async fn handle_socket(
        socket: axum::extract::ws::WebSocket,
        vm_id: String,
        vm_runtime: crate::vm_runtime::VmRuntime,
    ) {
        let pty_fd = match vm_runtime.pty_master(&vm_id) {
            Some(fd) => fd,
            None => {
                tracing::warn!(vm_id = %vm_id, "no pty master for vm");
                return;
            }
        };

        let raw_fd = pty_fd.as_raw_fd();

        // Dup fd for tokio async file (keep original for ioctl)
        let dup_fd = unsafe { libc::dup(raw_fd) };
        if dup_fd < 0 {
            tracing::warn!(error = %std::io::Error::last_os_error(), "failed to dup pty fd");
            return;
        }

        let std_file = unsafe { std::fs::File::from_raw_fd(dup_fd) };
        let tokio_file = tokio::fs::File::from_std(std_file);
        let (mut pty_reader, mut pty_writer) = tokio::io::split(tokio_file);

        let (mut ws_tx, mut ws_rx) = socket.split();

        // PTY → WebSocket
        let mut read_task = tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match pty_reader.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let msg = axum::extract::ws::Message::Binary(buf[..n].to_vec());
                        if ws_tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "pty read error");
                        break;
                    }
                }
            }
        });

        // WebSocket → PTY
        let mut write_task = tokio::spawn(async move {
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
    let ws = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe {
        if libc::ioctl(fd, libc::TIOCSWINSZ, &ws) < 0 {
            tracing::warn!(error = %std::io::Error::last_os_error(), "failed to set pty size");
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
}
