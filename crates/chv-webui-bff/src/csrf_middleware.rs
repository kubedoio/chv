//! Global CSRF middleware for the BFF's `/v1` surface (and the control
//! plane's legacy cookie-authenticated backup routes, which mount the same
//! function).
//!
//! # Threat model
//!
//! The credential this middleware protects is the `chv_session` HttpOnly
//! session cookie (the `Authorization` header cannot be set by a
//! cross-site page at all, so header-authenticated callers are not the
//! CSRF concern). The cookie is `SameSite=Strict`, which already prevents
//! it from riding cross-site requests in compliant browsers; this
//! middleware is the second, request-shape layer of that defense.
//!
//! # Model: un-forgeable request shape, not a shared-secret token
//!
//! There is no CSRF token issuance/verification in this codebase (the
//! `x-csrf-token` name appears only in the CORS allow-list). The
//! documented posture (see `handlers/auth.rs::login` and `auth.rs`'
//! `BearerToken` extractor) is that every non-GET request must have a
//! shape a cross-site HTML form cannot produce:
//!
//! - **`application/json`**: an HTML form's `enctype` can only produce
//!   `application/x-www-form-urlencoded`, `text/plain`, and
//!   `multipart/form-data` — never JSON. A `fetch()` with a JSON content
//!   type is not CORS-safelisted, so it triggers a preflight, which fails
//!   unless the operator allow-listed the origin (`CHV_ALLOWED_ORIGIN`).
//!
//! - **`multipart/form-data` with a non-empty `x-csrf-token` header**
//!   (issue #496): multipart IS a form-native content type, so the
//!   content type alone proves nothing here. The marker is instead a
//!   custom request header: HTML forms cannot set request headers at
//!   all, and `fetch()` only attaches non-safelisted headers after a
//!   successful CORS preflight. `x-csrf-token` is already in the CORS
//!   allow-list, so a same-origin UI can always send it. This is the
//!   standard custom-header CSRF defense, applied only to the one
//!   content type that needs it.
//!
//! Everything else (including form-native `application/x-www-form-
//! urlencoded` and `text/plain`, and a missing content type) is rejected
//! with 415 before routing, exactly as before #496.
//!
//! # Why a content-type lie cannot evade the check
//!
//! - Claiming `multipart/form-data` **without** the header is rejected
//!   here with 403 on every route, before any handler runs.
//! - Claiming `multipart/form-data` **with** the header against a JSON
//!   route still fails in the handler's `Json` extractor (415
//!   `MissingJsonContentType`) — and the request carried the
//!   un-forgeable header anyway.
//! - Claiming `application/json` against the multipart import route
//!   still fails in the handler's `Multipart` extractor (400
//!   `InvalidBoundary`) — and JSON was already an un-forgeable,
//!   protected shape before this change.
//!
//! So no method × content-type combination reaches a handler with less
//! un-forgeability than the pre-#496 posture required.

use axum::{
    extract::Request,
    http::{header, Method, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use serde_json::json;

/// The CSRF marker header required on multipart requests.
///
/// Deliberately the same name the CORS layer already allow-lists
/// (`router.rs::build_cors_layer`), so a same-origin UI never needs a
/// CORS change to send it.
const CSRF_TOKEN_HEADER: &str = "x-csrf-token";

pub async fn csrf_protection(req: Request, next: Next) -> impl IntoResponse {
    if req.method() == Method::GET || req.method() == Method::OPTIONS {
        return next.run(req).await;
    }

    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // JSON path (unchanged): the content type itself is the un-forgeable
    // marker — see the module docs.
    if content_type.starts_with("application/json") {
        return next.run(req).await;
    }

    // Multipart path (#496): the content type is form-native, so a
    // cross-site form CAN produce it. Require the un-forgeable marker
    // instead — a non-empty custom header. The body is never read here:
    // the multipart stream is left intact for the handler to parse.
    if content_type.starts_with("multipart/form-data") {
        let has_csrf_marker = req
            .headers()
            .get(CSRF_TOKEN_HEADER)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| !v.trim().is_empty());
        if has_csrf_marker {
            return next.run(req).await;
        }
        let body = axum::Json(json!({
            "message": "multipart/form-data requests must carry a non-empty x-csrf-token header",
            "code": "CSRF_REJECTED",
        }));
        return (StatusCode::FORBIDDEN, body).into_response();
    }

    let body = axum::Json(json!({
        "message": "Content-Type must be application/json",
        "code": "CSRF_REJECTED",
    }));
    (StatusCode::UNSUPPORTED_MEDIA_TYPE, body).into_response()
}
