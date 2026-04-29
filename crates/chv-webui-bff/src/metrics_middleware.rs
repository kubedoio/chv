use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use std::time::Instant;

/// Axum middleware that records HTTP request metrics:
/// - `http_requests_total` counter (method, status)
/// - `http_request_duration_seconds` histogram (method, path)
pub async fn track_metrics(req: Request, next: Next) -> Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(req).await;

    let status = response.status().as_u16().to_string();
    let duration = start.elapsed().as_secs_f64();

    metrics::counter!("http_requests_total", "method" => method.clone(), "status" => status)
        .increment(1);
    metrics::histogram!(
        "http_request_duration_seconds",
        "method" => method,
        "path" => sanitize_path(&path)
    )
    .record(duration);

    response
}

/// Sanitize dynamic path segments so histogram cardinality stays bounded.
/// Replaces UUID-like and hex ID segments with `{id}`.
fn sanitize_path(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if segment.len() >= 8 && segment.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
                "{id}"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}
