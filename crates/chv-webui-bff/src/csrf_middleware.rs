use axum::{extract::Request, http::StatusCode, middleware::Next, response::IntoResponse};
use serde_json::json;

pub async fn csrf_protection(req: Request, next: Next) -> impl IntoResponse {
    if req.method() == axum::http::Method::GET || req.method() == axum::http::Method::OPTIONS {
        return next.run(req).await;
    }

    let content_type = req
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !content_type.starts_with("application/json") {
        let body = axum::Json(json!({
            "message": "Content-Type must be application/json",
            "code": "CSRF_REJECTED",
        }));
        return (StatusCode::UNSUPPORTED_MEDIA_TYPE, body).into_response();
    }

    next.run(req).await
}
