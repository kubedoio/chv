use axum::{extract::Request, middleware::Next, response::IntoResponse};

pub async fn extract_correlation_id(req: Request, next: Next) -> impl IntoResponse {
    let correlation_id = req
        .headers()
        .get("x-operation-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(chv_common::gen_short_id);
    let mut req = req;
    req.extensions_mut().insert(Some(correlation_id.clone()));
    let mut response = next.run(req).await;
    response.headers_mut().insert(
        "x-correlation-id",
        axum::http::HeaderValue::from_str(&correlation_id).unwrap_or_else(|_| {
            axum::http::HeaderValue::from_static("unknown")
        }),
    );
    response
}
