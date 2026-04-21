use axum::{
    extract::{Path, State},
    response::Json,
};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

#[derive(sqlx::FromRow)]
struct ExportRow {
    filename: String,
    export_path: String,
    status: String,
}

#[derive(sqlx::FromRow)]
struct VmRow {
    display_name: String,
}

pub async fn export_vm(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    Path(vm_id): Path<String>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;

    if !chv_common::validate_id(&vm_id) {
        return Err(BffError::BadRequest("invalid vm_id format".into()));
    }

    // Verify VM exists
    let vm = sqlx::query_as::<_, VmRow>(
        "SELECT display_name FROM vms WHERE vm_id = ?",
    )
    .bind(&vm_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to look up vm: {}", e)))?
    .ok_or_else(|| BffError::NotFound(format!("vm {} not found", vm_id)))?;

    // Find attached volume
    let volume_id: Option<String> = sqlx::query_scalar(
        "SELECT volume_id FROM volume_desired_state WHERE attached_vm_id = ? LIMIT 1",
    )
    .bind(&vm_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to find volume: {}", e)))?;

    let volume_id =
        volume_id.ok_or_else(|| BffError::BadRequest("vm has no attached volumes".into()))?;

    // Construct source disk path
    let vm_dir = state.agent_runtime_dir.join("vms").join(&vm_id);
    let disk_path = vm_dir.join(format!("{}.img", volume_id));

    if !disk_path.exists() {
        return Err(BffError::NotFound("disk image not found".into()));
    }

    // Path traversal check: ensure resolved disk path is under agent_runtime_dir/vms
    let canonical_disk = tokio::fs::canonicalize(&disk_path)
        .await
        .map_err(|e| BffError::Internal(format!("failed to resolve disk path: {}", e)))?;
    let expected_base = tokio::fs::canonicalize(&state.agent_runtime_dir.join("vms"))
        .await
        .unwrap_or_else(|_| state.agent_runtime_dir.join("vms"));

    if !canonical_disk.starts_with(&expected_base) {
        return Err(BffError::BadRequest(
            "disk path is outside expected directory".into(),
        ));
    }

    let export_id = chv_common::gen_short_id();
    let export_dir = state.agent_runtime_dir.join("exports").join(&export_id);
    tokio::fs::create_dir_all(&export_dir)
        .await
        .map_err(|e| BffError::Internal(format!("failed to create export dir: {}", e)))?;

    let filename = format!("{}-export.qcow2", vm.display_name);
    let export_path = export_dir.join(&filename);

    tokio::fs::copy(&canonical_disk, &export_path)
        .await
        .map_err(|e| BffError::Internal(format!("failed to copy disk image: {}", e)))?;

    sqlx::query(
        r#"
        INSERT INTO vm_exports (export_id, vm_id, filename, export_path, status)
        VALUES (?, ?, ?, ?, 'ready')
        "#,
    )
    .bind(&export_id)
    .bind(&vm_id)
    .bind(&filename)
    .bind(export_path.to_string_lossy().to_string())
    .execute(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert export record: {}", e)))?;

    let download_url = format!("/api/v1/exports/{}/download", export_id);

    Ok(Json(json!({
        "export_id": export_id,
        "filename": filename,
        "download_url": download_url,
    })))
}

pub async fn download_export(
    crate::auth::BearerToken(_claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    Path(export_id): Path<String>,
) -> Result<axum::response::Response, BffError> {
    if !chv_common::validate_id(&export_id) {
        return Err(BffError::BadRequest("invalid export_id format".into()));
    }

    let row = sqlx::query_as::<_, ExportRow>(
        "SELECT filename, export_path, status FROM vm_exports WHERE export_id = ?",
    )
    .bind(&export_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to look up export: {}", e)))?
    .ok_or_else(|| BffError::NotFound(format!("export {} not found", export_id)))?;

    if row.status != "ready" {
        return Err(BffError::BadRequest("export is not ready".into()));
    }

    let file = tokio::fs::File::open(&row.export_path)
        .await
        .map_err(|e| BffError::Internal(format!("failed to open export file: {}", e)))?;

    let stream = tokio_util::io::ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);

    let response = axum::response::Response::builder()
        .header("Content-Type", "application/octet-stream")
        .header(
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", row.filename),
        )
        .body(body)
        .map_err(|e| BffError::Internal(format!("failed to build response: {}", e)))?;

    Ok(response)
}
