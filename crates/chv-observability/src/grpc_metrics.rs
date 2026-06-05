//! Tonic-compatible tower [`Layer`] that records RED metrics for every
//! incoming gRPC request.
//!
//! The layer wraps the inner service and observes:
//!
//! - `chv_grpc_server_requests_total{service,method,grpc_status}` — counter
//! - `chv_grpc_server_duration_seconds{service,method,grpc_status}` — histogram
//!
//! ### Status classification
//!
//! tonic returns gRPC errors as HTTP 200 with a `grpc-status` header (sometimes
//! delivered as a trailer for streaming responses). This layer inspects the
//! response headers for `grpc-status` and falls back to mapping the HTTP status
//! code (non-200 → `"unknown"`). For unary handlers — which dominate CHV's
//! gRPC surface — tonic emits `grpc-status` in the leading headers because the
//! body is buffered, so the inspection is reliable. For streaming handlers
//! that only set `grpc-status` in trailers, the recorded label degrades to
//! `"unknown"`; this is documented and acceptable for a v1 RED layer.
//!
//! ### Service / method extraction
//!
//! tonic routes requests under `/<package>.<service>/<method>`. The path is
//! parsed by splitting on `/`. Anything else (health checks, malformed paths)
//! is bucketed as `service="unknown"` / `method="unknown"`.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use http::{HeaderMap, Request, Response};
use pin_project_lite::pin_project;
use tower_layer::Layer;
use tower_service::Service;

use crate::{CHV_GRPC_SERVER_DURATION_SECONDS, CHV_GRPC_SERVER_REQUESTS_TOTAL};

/// Tower layer that wraps a tonic service stack with RED-style gRPC metrics.
///
/// Apply via `Server::builder().layer(GrpcMetricsLayer::new()).add_service(..)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct GrpcMetricsLayer;

impl GrpcMetricsLayer {
    /// Construct a new layer. Equivalent to [`Default::default`].
    pub const fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for GrpcMetricsLayer {
    type Service = GrpcMetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GrpcMetricsService { inner }
    }
}

/// Service produced by [`GrpcMetricsLayer`]. Records duration on every
/// response — both successful and error — without altering the response.
#[derive(Clone, Copy, Debug)]
pub struct GrpcMetricsService<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for GrpcMetricsService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = GrpcMetricsFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let (service_label, method_label) = parse_service_method(req.uri().path());
        let started = Instant::now();
        GrpcMetricsFuture {
            inner: self.inner.call(req),
            started,
            service_label: Some(service_label),
            method_label: Some(method_label),
        }
    }
}

pin_project! {
    /// Future returned by [`GrpcMetricsService`]. Records metrics exactly once
    /// — when the inner future resolves — and forwards the response untouched.
    pub struct GrpcMetricsFuture<F> {
        #[pin]
        inner: F,
        started: Instant,
        service_label: Option<String>,
        method_label: Option<String>,
    }
}

impl<F, ResBody, E> Future for GrpcMetricsFuture<F>
where
    F: Future<Output = Result<Response<ResBody>, E>>,
{
    type Output = Result<Response<ResBody>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let result = std::task::ready!(this.inner.poll(cx));
        // Only record metrics once; the labels are taken so a re-poll is a
        // no-op on the metrics side. Service futures should not be polled
        // after completion in practice.
        let service = this
            .service_label
            .take()
            .unwrap_or_else(|| "unknown".to_string());
        let method = this
            .method_label
            .take()
            .unwrap_or_else(|| "unknown".to_string());
        let elapsed = this.started.elapsed().as_secs_f64();

        let status_label = match &result {
            Ok(resp) => classify_response(resp.headers(), resp.status().as_u16()),
            Err(_) => "unknown".to_string(),
        };

        record(&service, &method, &status_label, elapsed);

        Poll::Ready(result)
    }
}

/// Parse a tonic path of shape `/<package>.<service>/<method>` into
/// `(service, method)` label values. Anything that does not match this shape
/// is bucketed as `("unknown", "unknown")` so cardinality cannot explode.
fn parse_service_method(path: &str) -> (String, String) {
    let trimmed = path.strip_prefix('/').unwrap_or(path);
    let mut parts = trimmed.splitn(2, '/');
    match (parts.next(), parts.next()) {
        (Some(service), Some(method)) if !service.is_empty() && !method.is_empty() => {
            (service.to_string(), method.to_string())
        }
        _ => ("unknown".to_string(), "unknown".to_string()),
    }
}

/// Map the response headers (and HTTP status code as fallback) to a
/// `grpc_status` label value. The value is the numeric gRPC status code as a
/// string (e.g. `"0"` for OK, `"5"` for NotFound) when discoverable, otherwise
/// `"unknown"`.
fn classify_response(headers: &HeaderMap, http_status: u16) -> String {
    if let Some(v) = headers.get("grpc-status") {
        if let Ok(s) = v.to_str() {
            return s.to_string();
        }
    }
    if http_status == 200 {
        // tonic typically sets grpc-status on unary success; absence with
        // HTTP 200 suggests a streaming response whose status lives in the
        // trailers. We cannot read trailers without consuming the body, so
        // bucket as unknown.
        return "unknown".to_string();
    }
    // Non-200 HTTP responses are protocol-level failures (auth, malformed
    // request, etc.). Surface the HTTP status under the same label namespace
    // so operators can spot them.
    format!("http_{}", http_status)
}

fn record(service: &str, method: &str, grpc_status: &str, duration_secs: f64) {
    metrics::counter!(
        CHV_GRPC_SERVER_REQUESTS_TOTAL,
        "service" => service.to_string(),
        "method" => method.to_string(),
        "grpc_status" => grpc_status.to_string()
    )
    .increment(1);
    metrics::histogram!(
        CHV_GRPC_SERVER_DURATION_SECONDS,
        "service" => service.to_string(),
        "method" => method.to_string(),
        "grpc_status" => grpc_status.to_string()
    )
    .record(duration_secs);
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use bytes::Bytes;
    use http::{HeaderValue, Request, Response, StatusCode};
    use http_body_util::Empty;
    use tower_layer::Layer;
    use tower_service::Service;

    use super::*;

    #[test]
    fn parse_service_method_happy_path() {
        let (s, m) = parse_service_method("/chv.controlplane.LifecycleService/CreateVm");
        assert_eq!(s, "chv.controlplane.LifecycleService");
        assert_eq!(m, "CreateVm");
    }

    #[test]
    fn parse_service_method_bad_paths_bucket_to_unknown() {
        for bad in ["", "/", "/no-slash-after", "/svc/", "//method"] {
            let (s, m) = parse_service_method(bad);
            assert_eq!(s, "unknown", "service for {bad:?}");
            assert_eq!(m, "unknown", "method for {bad:?}");
        }
    }

    #[test]
    fn classify_response_reads_grpc_status_header() {
        let mut headers = HeaderMap::new();
        headers.insert("grpc-status", HeaderValue::from_static("5"));
        assert_eq!(classify_response(&headers, 200), "5");
    }

    #[test]
    fn classify_response_falls_back_to_http_status() {
        let headers = HeaderMap::new();
        assert_eq!(classify_response(&headers, 200), "unknown");
        assert_eq!(classify_response(&headers, 503), "http_503");
    }

    /// Echo service that returns a `grpc-status: 0` header — the canonical
    /// "unary OK" tonic shape.
    #[derive(Clone)]
    struct OkService;

    impl Service<Request<Empty<Bytes>>> for OkService {
        type Response = Response<Empty<Bytes>>;
        type Error = Infallible;
        type Future = std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
        >;

        fn poll_ready(
            &mut self,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: Request<Empty<Bytes>>) -> Self::Future {
            Box::pin(async move {
                let mut resp = Response::new(Empty::<Bytes>::new());
                *resp.status_mut() = StatusCode::OK;
                resp.headers_mut()
                    .insert("grpc-status", HeaderValue::from_static("0"));
                Ok(resp)
            })
        }
    }

    #[tokio::test]
    async fn layer_passes_response_through_unchanged() {
        let mut svc = GrpcMetricsLayer::new().layer(OkService);
        let req = Request::builder()
            .uri("/chv.controlplane.LifecycleService/CreateVm")
            .body(Empty::<Bytes>::new())
            .unwrap();

        // poll_ready then call to drive the wrapped service end-to-end.
        futures_util::future::poll_fn(|cx| svc.poll_ready(cx))
            .await
            .unwrap();
        let resp = svc.call(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get("grpc-status")
                .and_then(|v| v.to_str().ok()),
            Some("0")
        );
    }
}
