use axum::{extract::Request, middleware::Next, response::IntoResponse};

pub async fn extract_correlation_id(req: Request, next: Next) -> impl IntoResponse {
    let correlation_id = req
        .headers()
        .get("x-operation-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let mut req = req;
    req.extensions_mut().insert(correlation_id);
    next.run(req).await
}
