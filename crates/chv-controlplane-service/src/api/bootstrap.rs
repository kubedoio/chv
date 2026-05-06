use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    Json,
};
use chv_webui_bff::AppState;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Deserialize)]
pub struct SeedTokenRequest {
    pub token: String,
    #[serde(default)]
    pub description: String,
    /// Whether this token should expire after one use. Defaults to true.
    #[serde(default = "default_one_time_use")]
    pub one_time_use: bool,
}

fn default_one_time_use() -> bool {
    true
}

#[derive(Serialize)]
pub struct SeedTokenResponse {
    pub status: String,
}

/// Seed a bootstrap token via HTTP API.
///
/// This endpoint only accepts connections from localhost (127.0.0.1 or ::1).
/// It hashes the provided token with SHA-256 and inserts or replaces it in
/// the `bootstrap_tokens` table, avoiding the need to stop the controlplane
/// and use the sqlite3 CLI directly.
pub async fn seed_bootstrap_token(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    Json(body): Json<SeedTokenRequest>,
) -> Result<Json<SeedTokenResponse>, StatusCode> {
    // Only allow from localhost
    if !addr.ip().is_loopback() {
        tracing::warn!(
            remote_addr = %addr,
            "rejected bootstrap token seed request from non-loopback address"
        );
        return Err(StatusCode::FORBIDDEN);
    }

    if body.token.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let token_hash = chv_common::sha256_hex(&body.token);

    sqlx::query(
        r#"INSERT OR REPLACE INTO bootstrap_tokens
           (token_hash, description, one_time_use, used_at, expires_at, created_at, updated_at)
           VALUES ($1, $2, $3, NULL, NULL,
                   strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                   strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))"#,
    )
    .bind(&token_hash)
    .bind(&body.description)
    .bind(body.one_time_use)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to seed bootstrap token");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::info!(
        description = %body.description,
        one_time_use = body.one_time_use,
        "bootstrap token seeded successfully"
    );

    Ok(Json(SeedTokenResponse {
        status: "ok".to_string(),
    }))
}
