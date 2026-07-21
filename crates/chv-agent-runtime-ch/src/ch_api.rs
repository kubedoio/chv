use chv_errors::ChvError;
use std::fmt::Write as _;
use std::io::{Read as _, Write as _};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_HEADER_BYTES: usize = 64 * 1024;

/// Strict, bounded liveness probe for a pre-connected Cloud Hypervisor API socket.
///
/// Legacy lifecycle requests use a private compatibility transport below and
/// intentionally do not inherit these parsing or response-size guarantees.
#[derive(Debug, Clone, Copy)]
pub struct CloudHypervisorApiClient {
    timeout: Duration,
    max_response_bytes: usize,
}

impl Default for CloudHypervisorApiClient {
    fn default() -> Self {
        Self::with_limits(DEFAULT_TIMEOUT, DEFAULT_MAX_RESPONSE_BYTES)
    }
}

impl CloudHypervisorApiClient {
    pub const fn with_limits(timeout: Duration, max_response_bytes: usize) -> Self {
        Self {
            timeout,
            max_response_bytes,
        }
    }

    /// Probes the fixed VMM ping endpoint on a stream whose peer identity the
    /// caller has already verified. This never reconnects by pathname.
    pub async fn probe_vmm_ping(&self, stream: &mut UnixStream) -> Result<(), ChvError> {
        if self.max_response_bytes == 0 {
            return Err(invalid_label(
                "Cloud Hypervisor API stream",
                "response size limit is zero",
            ));
        }
        let exchange = async {
            stream
                .write_all(b"GET /api/v1/vmm.ping HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n")
                .await
                .map_err(|e| io_label("Cloud Hypervisor API stream", e))?;
            let (status, _) = read_response(stream, self.max_response_bytes).await?;
            if !(200..300).contains(&status) {
                return Err(invalid_label(
                    "Cloud Hypervisor API stream",
                    "probe returned non-success status",
                ));
            }
            Ok(())
        };
        tokio::time::timeout(self.timeout, exchange)
            .await
            .map_err(|_| {
                io_label(
                    "Cloud Hypervisor API stream",
                    std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Cloud Hypervisor API probe timed out",
                    ),
                )
            })?
    }

    pub(crate) async fn request(
        &self,
        socket: &Path,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<u16, ChvError> {
        Ok(self.request_with_body(socket, method, path, body).await?.0)
    }

    pub(crate) async fn request_with_body(
        &self,
        socket: &Path,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<(u16, String), ChvError> {
        legacy_request_with_body(socket, method, path, body).await
    }
}

// Phase C evidence collection is deliberately production-unwired; retain the
// synchronous connected-fd probe for the Linux observer's bounded test surface.
#[allow(dead_code)]
pub(crate) fn probe_vmm_ping_connected(
    stream: &mut std::os::unix::net::UnixStream,
    timeout: Duration,
    limit: usize,
) -> Result<(), ChvError> {
    if limit == 0 {
        return Err(invalid_label(
            "Cloud Hypervisor API stream",
            "response size limit is zero",
        ));
    }
    let deadline = Instant::now() + timeout;
    stream
        .set_write_timeout(Some(remaining(deadline)?))
        .map_err(|e| io_label("Cloud Hypervisor API stream", e))?;
    stream
        .write_all(b"GET /api/v1/vmm.ping HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n")
        .map_err(|e| io_label("Cloud Hypervisor API stream", e))?;

    let mut response = Vec::new();
    let mut chunk = [0_u8; 4096];
    let (header_end, body_len) = loop {
        stream
            .set_read_timeout(Some(remaining(deadline)?))
            .map_err(|e| io_label("Cloud Hypervisor API stream", e))?;
        let count = stream
            .read(&mut chunk)
            .map_err(|e| io_label("Cloud Hypervisor API stream", e))?;
        if count == 0 {
            return Err(invalid_label(
                "Cloud Hypervisor API stream",
                "response ended before complete headers",
            ));
        }
        append(&mut response, &chunk[..count], limit)?;
        if let Some(end) = response.windows(4).position(|w| w == b"\r\n\r\n") {
            if end > MAX_HEADER_BYTES {
                return Err(invalid_label(
                    "Cloud Hypervisor API stream",
                    "response headers exceed size limit",
                ));
            }
            let length = content_length(&response[..end])?;
            if end.saturating_add(4).saturating_add(length) > limit {
                return Err(invalid_label(
                    "Cloud Hypervisor API stream",
                    "response body exceeds size limit",
                ));
            }
            break (end, length);
        }
        if response.len() > MAX_HEADER_BYTES {
            return Err(invalid_label(
                "Cloud Hypervisor API stream",
                "response headers exceed size limit",
            ));
        }
    };
    let required = header_end + 4 + body_len;
    while response.len() < required {
        stream
            .set_read_timeout(Some(remaining(deadline)?))
            .map_err(|e| io_label("Cloud Hypervisor API stream", e))?;
        let count = stream
            .read(&mut chunk)
            .map_err(|e| io_label("Cloud Hypervisor API stream", e))?;
        if count == 0 {
            return Err(invalid_label(
                "Cloud Hypervisor API stream",
                "response body is truncated",
            ));
        }
        append(&mut response, &chunk[..count], limit)?;
    }
    if response.len() != required {
        return Err(invalid_label(
            "Cloud Hypervisor API stream",
            "ping response contains trailing bytes",
        ));
    }
    if body_len != 0 {
        return Err(invalid_label(
            "Cloud Hypervisor API stream",
            "ping response body must be empty",
        ));
    }
    let status = parse_strict_http_status(&response)
        .ok_or_else(|| invalid_label("Cloud Hypervisor API stream", "invalid HTTP status line"))?;
    if !(200..300).contains(&status) {
        return Err(invalid_label(
            "Cloud Hypervisor API stream",
            "probe returned non-success status",
        ));
    }
    Ok(())
}

fn remaining(deadline: Instant) -> Result<Duration, ChvError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(io_label(
            "Cloud Hypervisor API stream",
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Cloud Hypervisor API probe timed out",
            ),
        ));
    }
    Ok(remaining)
}

fn format_request(method: &str, path: &str, body: Option<&str>) -> String {
    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\n");
    if let Some(body) = body {
        let _ = write!(request, "Content-Length: {}\r\n", body.len());
        request.push_str("Content-Type: application/json\r\n\r\n");
        request.push_str(body);
    } else {
        request.push_str("Content-Length: 0\r\n\r\n");
    }
    request
}

async fn legacy_request_with_body(
    socket: &Path,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<(u16, String), ChvError> {
    // Compatibility-only: preserve the historical unbounded response handling
    // and permissive parsing until lifecycle callers migrate under a later ADR.
    let mut stream = UnixStream::connect(socket)
        .await
        .map_err(|e| io(socket, e))?;
    let request = format_request(method, path, body);
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| io(socket, e))?;
    let mut response = Vec::new();
    let mut chunk = [0_u8; 4096];
    let read = async {
        loop {
            let count = stream.read(&mut chunk).await.map_err(|e| io(socket, e))?;
            if count == 0 {
                break;
            }
            response.extend_from_slice(&chunk[..count]);
            if let Some(end) = response.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&response[..end]);
                let length = headers
                    .lines()
                    .find(|line| line.to_ascii_lowercase().starts_with("content-length:"))
                    .and_then(|line| line.split_once(':').map(|pair| pair.1))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if response.len() >= end + 4 + length {
                    break;
                }
            }
        }
        Ok::<(), ChvError>(())
    };
    tokio::time::timeout(DEFAULT_TIMEOUT, read)
        .await
        .map_err(|_| {
            io(
                socket,
                std::io::Error::new(std::io::ErrorKind::TimedOut, "socket read timed out"),
            )
        })??;
    let raw = String::from_utf8_lossy(&response);
    let status = parse_http_status(&response).unwrap_or(0);
    let body = raw
        .find("\r\n\r\n")
        .map_or_else(String::new, |end| raw[end + 4..].to_string());
    Ok((status, body))
}

async fn read_response(stream: &mut UnixStream, limit: usize) -> Result<(u16, String), ChvError> {
    let mut response = Vec::new();
    let mut chunk = [0_u8; 4096];
    let (header_end, body_len) = loop {
        let n = stream
            .read(&mut chunk)
            .await
            .map_err(|e| io_label("Cloud Hypervisor API stream", e))?;
        if n == 0 {
            return Err(invalid_label(
                "Cloud Hypervisor API stream",
                "response ended before complete headers",
            ));
        }
        append(&mut response, &chunk[..n], limit)?;
        if let Some(end) = response.windows(4).position(|w| w == b"\r\n\r\n") {
            if end > MAX_HEADER_BYTES {
                return Err(invalid_label(
                    "Cloud Hypervisor API stream",
                    "response headers exceed size limit",
                ));
            }
            let len = content_length(&response[..end])?;
            if end.saturating_add(4).saturating_add(len) > limit {
                return Err(invalid_label(
                    "Cloud Hypervisor API stream",
                    "response body exceeds size limit",
                ));
            }
            break (end, len);
        }
        if response.len() > MAX_HEADER_BYTES {
            return Err(invalid_label(
                "Cloud Hypervisor API stream",
                "response headers exceed size limit",
            ));
        }
    };
    let required = header_end + 4 + body_len;
    while response.len() < required {
        let n = stream
            .read(&mut chunk)
            .await
            .map_err(|e| io_label("Cloud Hypervisor API stream", e))?;
        if n == 0 {
            return Err(invalid_label(
                "Cloud Hypervisor API stream",
                "response body is truncated",
            ));
        }
        append(&mut response, &chunk[..n], limit)?;
    }
    if response.len() != required {
        return Err(invalid_label(
            "Cloud Hypervisor API stream",
            "ping response contains trailing bytes",
        ));
    }
    if body_len != 0 {
        return Err(invalid_label(
            "Cloud Hypervisor API stream",
            "ping response body must be empty",
        ));
    }
    let status = parse_strict_http_status(&response)
        .ok_or_else(|| invalid_label("Cloud Hypervisor API stream", "invalid HTTP status line"))?;
    let body = std::str::from_utf8(&response[header_end + 4..required])
        .map_err(|_| invalid_label("Cloud Hypervisor API stream", "response body is not UTF-8"))?
        .to_owned();
    Ok((status, body))
}

fn content_length(bytes: &[u8]) -> Result<usize, ChvError> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        invalid_label(
            "Cloud Hypervisor API stream",
            "response headers are not UTF-8",
        )
    })?;
    let mut found = None;
    for line in text.lines().skip(1) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| invalid_label("Cloud Hypervisor API stream", "malformed HTTP header"))?;
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(invalid_label(
                "Cloud Hypervisor API stream",
                "Transfer-Encoding is unsupported",
            ));
        }
        if name.eq_ignore_ascii_case("content-length") {
            if found.is_some() {
                return Err(invalid_label(
                    "Cloud Hypervisor API stream",
                    "duplicate Content-Length header",
                ));
            }
            found = Some(value.trim().parse().map_err(|_| {
                invalid_label(
                    "Cloud Hypervisor API stream",
                    "invalid Content-Length header",
                )
            })?);
        }
    }
    found.ok_or_else(|| {
        invalid_label(
            "Cloud Hypervisor API stream",
            "missing Content-Length header",
        )
    })
}

fn append(target: &mut Vec<u8>, bytes: &[u8], limit: usize) -> Result<(), ChvError> {
    if target.len().saturating_add(bytes.len()) > limit {
        return Err(invalid_label(
            "Cloud Hypervisor API stream",
            "response exceeds size limit",
        ));
    }
    target.extend_from_slice(bytes);
    Ok(())
}

pub(crate) fn parse_http_status(bytes: &[u8]) -> Option<u16> {
    let response = String::from_utf8_lossy(bytes);
    let mut fields = response.lines().next()?.split_whitespace();
    fields.nth(1)?.parse().ok()
}

fn parse_strict_http_status(bytes: &[u8]) -> Option<u16> {
    let end = bytes.windows(2).position(|w| w == b"\r\n")?;
    let line = std::str::from_utf8(&bytes[..end]).ok()?;
    let (version, remainder) = line.split_once(' ')?;
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1") || remainder.starts_with(' ') {
        return None;
    }
    let (status, reason) = remainder.split_once(' ').unwrap_or((remainder, ""));
    if status.len() != 3
        || !status.bytes().all(|byte| byte.is_ascii_digit())
        || reason.starts_with(' ')
        || reason.bytes().any(|byte| byte < b' ' || byte == 0x7f)
    {
        return None;
    }
    status.parse().ok()
}

fn io(socket: &Path, source: std::io::Error) -> ChvError {
    ChvError::Io {
        path: socket.to_string_lossy().into_owned(),
        source,
    }
}

fn io_label(label: &str, source: std::io::Error) -> ChvError {
    ChvError::Io {
        path: label.to_string(),
        source,
    }
}

fn invalid_label(label: &str, reason: &str) -> ChvError {
    io_label(
        label,
        std::io::Error::new(std::io::ErrorKind::InvalidData, reason),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream as StdUnixStream;
    use std::thread;
    use tempfile::TempDir;
    use tokio::net::UnixListener;

    async fn server(response: &'static [u8]) -> (TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("api.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        tokio::spawn(async move {
            let (mut peer, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            let _ = peer.read(&mut request).await.unwrap();
            peer.write_all(response).await.unwrap();
        });
        (dir, socket)
    }

    async fn probe(socket: &Path, client: CloudHypervisorApiClient) -> Result<(), ChvError> {
        let mut stream = UnixStream::connect(socket).await.unwrap();
        client.probe_vmm_ping(&mut stream).await
    }

    #[tokio::test]
    async fn probe_parses_response() {
        let (_dir, socket) = server(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").await;
        probe(
            &socket,
            CloudHypervisorApiClient::with_limits(Duration::from_secs(1), 256),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn probe_rejects_oversize_declaration() {
        let (_dir, socket) = server(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n").await;
        let error = probe(
            &socket,
            CloudHypervisorApiClient::with_limits(Duration::from_secs(1), 128),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("size limit"));
    }

    #[tokio::test]
    async fn probe_times_out_stalled_peer() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("api.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        tokio::spawn(async move {
            let (_peer, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        });
        let error = probe(
            &socket,
            CloudHypervisorApiClient::with_limits(Duration::from_millis(20), 128),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn probe_rejects_truncated_body() {
        let (_dir, socket) = server(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nx").await;
        let error = probe(
            &socket,
            CloudHypervisorApiClient::with_limits(Duration::from_secs(1), 128),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("truncated"));
    }

    #[tokio::test]
    async fn probe_rejects_non_success_status() {
        let (_dir, socket) = server(b"HTTP/1.1 500 Error\r\nContent-Length: 0\r\n\r\n").await;
        let error = probe(
            &socket,
            CloudHypervisorApiClient::with_limits(Duration::from_secs(1), 128),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("non-success"));
    }

    #[tokio::test]
    async fn probe_rejects_malformed_status_and_header() {
        for response in [
            &b"garbage\r\nContent-Length: 0\r\n\r\n"[..],
            &b"HTTP/1.1 200 OK\r\nbroken\r\n\r\n"[..],
        ] {
            let owned = response.to_vec().into_boxed_slice();
            let response: &'static [u8] = Box::leak(owned);
            let (_dir, socket) = server(response).await;
            assert!(probe(
                &socket,
                CloudHypervisorApiClient::with_limits(Duration::from_secs(1), 128)
            )
            .await
            .is_err());
        }
    }

    #[tokio::test]
    async fn probe_rejects_oversized_headers() {
        let response = format!(
            "HTTP/1.1 200 OK\r\nX-Fill: {}\r\nContent-Length: 0\r\n\r\n",
            "x".repeat(MAX_HEADER_BYTES)
        );
        let response: &'static [u8] = Box::leak(response.into_bytes().into_boxed_slice());
        let (_dir, socket) = server(response).await;
        let error = probe(
            &socket,
            CloudHypervisorApiClient::with_limits(Duration::from_secs(1), MAX_HEADER_BYTES + 1024),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("headers exceed"));
    }

    #[tokio::test]
    async fn probe_accepts_only_supported_http_versions_and_three_digit_status() {
        for response in [
            &b"HTTP/2 200 OK\r\nContent-Length: 0\r\n\r\n"[..],
            &b"HTTP/1.2 200 OK\r\nContent-Length: 0\r\n\r\n"[..],
            &b"HTTP/1.1 20 OK\r\nContent-Length: 0\r\n\r\n"[..],
            &b"HTTP/1.1 2000 OK\r\nContent-Length: 0\r\n\r\n"[..],
            &b"HTTP/1.1 200  extra\r\nContent-Length: 0\r\n\r\n"[..],
        ] {
            let response: &'static [u8] = Box::leak(response.to_vec().into_boxed_slice());
            let (_dir, socket) = server(response).await;
            assert!(probe(
                &socket,
                CloudHypervisorApiClient::with_limits(Duration::from_secs(1), 128)
            )
            .await
            .is_err());
        }

        let (_dir, socket) = server(b"HTTP/1.0 204 OK\r\nContent-Length: 0\r\n\r\n").await;
        probe(
            &socket,
            CloudHypervisorApiClient::with_limits(Duration::from_secs(1), 128),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn probe_requires_single_content_length_and_rejects_transfer_encoding() {
        for response in [
            &b"HTTP/1.1 200 OK\r\n\r\n"[..],
            &b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n"[..],
            &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 0\r\n\r\n"[..],
            &b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\ntrailing"[..],
        ] {
            let response: &'static [u8] = Box::leak(response.to_vec().into_boxed_slice());
            let (_dir, socket) = server(response).await;
            assert!(probe(
                &socket,
                CloudHypervisorApiClient::with_limits(Duration::from_secs(1), 256)
            )
            .await
            .is_err());
        }
    }

    #[test]
    fn connected_probe_enforces_total_deadline_against_trickle_peer() {
        let (mut client, mut server) = StdUnixStream::pair().unwrap();
        let peer = thread::spawn(move || {
            let mut request = [0_u8; 128];
            let _ = server.read(&mut request);
            for byte in b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n" {
                thread::sleep(Duration::from_millis(15));
                if server.write_all(&[*byte]).is_err() {
                    break;
                }
            }
        });

        let started = Instant::now();
        let error = probe_vmm_ping_connected(&mut client, Duration::from_millis(60), 256)
            .expect_err("a trickle peer must not reset the total deadline");
        assert!(matches!(
            error,
            ChvError::Io { ref source, .. }
                if matches!(source.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock)
        ));
        assert!(started.elapsed() < Duration::from_millis(250));
        peer.join().unwrap();
    }
}
