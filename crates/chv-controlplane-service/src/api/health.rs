use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chv_webui_bff::AppState;
use serde_json::json;
use std::time::Instant;
use tokio::time::Duration;

pub async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").fetch_one(&state.pool).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"status": "db_unavailable"})),
        ),
    }
}

pub async fn ready_handler(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").fetch_one(&state.pool).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"status": "not_ready"})),
        ),
    }
}

pub async fn metrics_handler() -> impl IntoResponse {
    match chv_observability::prometheus_handle() {
        Some(handle) => (StatusCode::OK, handle.render()),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "metrics recorder not initialized".to_string(),
        ),
    }
}

pub async fn deep_health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let mut checks = serde_json::Map::new();
    let mut db_pass = false;
    let mut agent_socket_dir_pass = false;
    let mut agent_connectivity_pass = false;
    let mut agent_connectivity_skipped = false;

    // Database check
    let db_start = Instant::now();
    match sqlx::query("SELECT 1").fetch_one(&state.pool).await {
        Ok(_) => {
            let latency_ms = db_start.elapsed().as_millis() as u64;
            checks.insert(
                "database".to_string(),
                json!({ "status": "pass", "latency_ms": latency_ms }),
            );
            db_pass = true;
        }
        Err(e) => {
            let latency_ms = db_start.elapsed().as_millis() as u64;
            checks.insert(
                "database".to_string(),
                json!({ "status": "fail", "latency_ms": latency_ms, "detail": e.to_string() }),
            );
        }
    }

    // Agent socket directory check
    match tokio::fs::metadata(&state.agent_runtime_dir).await {
        Ok(metadata) => {
            if metadata.is_dir() {
                // Try to read the directory to confirm readability
                match tokio::fs::read_dir(&state.agent_runtime_dir).await {
                    Ok(_) => {
                        checks.insert(
                            "agent_socket_dir".to_string(),
                            json!({ "status": "pass", "detail": format!("{} exists and is readable", state.agent_runtime_dir.display()) }),
                        );
                        agent_socket_dir_pass = true;
                    }
                    Err(e) => {
                        checks.insert(
                            "agent_socket_dir".to_string(),
                            json!({ "status": "fail", "detail": format!("directory exists but not readable: {}", e) }),
                        );
                    }
                }
            } else {
                checks.insert(
                    "agent_socket_dir".to_string(),
                    json!({ "status": "fail", "detail": format!("{} exists but is not a directory", state.agent_runtime_dir.display()) }),
                );
            }
        }
        Err(e) => {
            checks.insert(
                "agent_socket_dir".to_string(),
                json!({ "status": "fail", "detail": format!("directory not accessible: {}", e) }),
            );
        }
    }

    // Agent connectivity check
    if agent_socket_dir_pass {
        match find_first_socket(&state.agent_runtime_dir).await {
            Some(socket_path) => {
                match tokio::time::timeout(
                    Duration::from_secs(2),
                    tokio::net::UnixStream::connect(&socket_path),
                )
                .await
                {
                    Ok(Ok(_)) => {
                        checks.insert(
                            "agent_connectivity".to_string(),
                            json!({ "status": "pass", "detail": format!("connected to {}", socket_path.display()) }),
                        );
                        agent_connectivity_pass = true;
                    }
                    Ok(Err(e)) => {
                        checks.insert(
                            "agent_connectivity".to_string(),
                            json!({ "status": "fail", "detail": format!("failed to connect to {}: {}", socket_path.display(), e) }),
                        );
                    }
                    Err(_) => {
                        checks.insert(
                            "agent_connectivity".to_string(),
                            json!({ "status": "fail", "detail": format!("timeout connecting to {}", socket_path.display()) }),
                        );
                    }
                }
            }
            None => {
                checks.insert(
                    "agent_connectivity".to_string(),
                    json!({ "status": "skipped", "detail": "no agent sockets found" }),
                );
                agent_connectivity_skipped = true;
            }
        }
    } else {
        checks.insert(
            "agent_connectivity".to_string(),
            json!({ "status": "skipped", "detail": "agent socket directory not available" }),
        );
        agent_connectivity_skipped = true;
    }

    let overall_status = if !db_pass {
        "unhealthy"
    } else if !agent_socket_dir_pass || (!agent_connectivity_pass && !agent_connectivity_skipped) {
        "degraded"
    } else {
        "healthy"
    };

    let status_code = if overall_status == "healthy" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status_code,
        Json(json!({
            "status": overall_status,
            "checks": checks,
        })),
    )
}

async fn find_first_socket(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(_) => return None,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("sock") {
            return Some(path);
        }
    }

    None
}
