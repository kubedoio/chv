use axum::{extract::{Path, State}, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

// ---------------------------------------------------------------------------
// VM Templates
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct VMTemplateRow {
    template_id: String,
    name: String,
    description: Option<String>,
    cpu_count: i64,
    memory_bytes: i64,
    image_id: Option<String>,
    network_id: Option<String>,
    cloud_init_userdata: Option<String>,
    created_at: String,
}

pub async fn list_vm_templates(
    State(state): State<AppState>,
) -> Result<Json<Value>, BffError> {
    let rows = sqlx::query_as::<_, VMTemplateRow>(
        r#"
        SELECT
            template_id,
            name,
            description,
            cpu_count,
            memory_bytes,
            image_id,
            network_id,
            cloud_init_userdata,
            created_at
        FROM vm_templates
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list vm_templates: {}", e)))?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            // Convert memory_bytes -> memory_mb for the UI type
            let memory_mb = r.memory_bytes / (1024 * 1024);
            json!({
                "id": r.template_id,
                "node_id": "",
                "name": r.name,
                "description": r.description.unwrap_or_default(),
                "vcpu": r.cpu_count,
                "memory_mb": memory_mb,
                "image_id": r.image_id.unwrap_or_default(),
                "network_id": r.network_id.unwrap_or_default(),
                "storage_pool_id": "",
                "cloud_init_config": r.cloud_init_userdata,
                "tags": [],
                "created_at": r.created_at,
            })
        })
        .collect();

    Ok(Json(json!(items)))
}

pub async fn create_vm_template(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing name".into()))?;

    let description = payload
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let vcpu = payload
        .get("vcpu")
        .and_then(|v| v.as_i64())
        .unwrap_or(2);

    let memory_mb = payload
        .get("memory_mb")
        .and_then(|v| v.as_i64())
        .unwrap_or(2048);

    let memory_bytes = memory_mb * 1024 * 1024;

    let image_id = payload
        .get("image_id")
        .and_then(|v| v.as_str());

    let network_id = payload
        .get("network_id")
        .and_then(|v| v.as_str());

    let cloud_init_config = payload
        .get("cloud_init_config")
        .and_then(|v| v.as_str());

    let template_id = chv_common::gen_short_id();

    sqlx::query(
        r#"INSERT INTO vm_templates
           (template_id, name, description, cpu_count, memory_bytes, image_id, network_id, cloud_init_userdata,
            created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?,
                   strftime('%Y-%m-%dT%H:%M:%SZ','now'),
                   strftime('%Y-%m-%dT%H:%M:%SZ','now'))"#,
    )
    .bind(&template_id)
    .bind(name)
    .bind(description)
    .bind(vcpu)
    .bind(memory_bytes)
    .bind(image_id)
    .bind(network_id)
    .bind(cloud_init_config)
    .execute(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to create vm_template: {}", e)))?;

    Ok(Json(json!({
        "id": template_id,
        "node_id": "",
        "name": name,
        "description": description,
        "vcpu": vcpu,
        "memory_mb": memory_mb,
        "image_id": image_id.unwrap_or(""),
        "network_id": network_id.unwrap_or(""),
        "storage_pool_id": "",
        "cloud_init_config": cloud_init_config,
        "tags": [],
        "created_at": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    })))
}

pub async fn delete_vm_template(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    Path(template_id): Path<String>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let exists: bool =
        sqlx::query_scalar("SELECT COUNT(*) > 0 FROM vm_templates WHERE template_id = ?")
            .bind(&template_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|e| BffError::Internal(format!("db error: {}", e)))?;

    if !exists {
        return Err(BffError::NotFound(format!(
            "vm template {} not found",
            template_id
        )));
    }

    // Refuse deletion if any VMs reference this template's image
    let vm_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM vm_desired_state vds
           JOIN vm_templates t ON t.image_id IS NOT NULL AND vds.image_ref = t.image_id
           WHERE t.template_id = ?"#
    )
    .bind(&template_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("db error: {}", e)))?;

    if vm_count > 0 {
        return Err(BffError::Conflict(format!(
            "Template is in use by {} VM(s)",
            vm_count
        )));
    }

    sqlx::query("DELETE FROM vm_templates WHERE template_id = ?")
        .bind(&template_id)
        .execute(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to delete vm_template: {}", e)))?;

    Ok(Json(json!({
        "deleted": true,
        "id": template_id,
    })))
}

// ---------------------------------------------------------------------------
// Cloud-init Templates
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct CloudInitTemplateRow {
    template_id: String,
    name: String,
    description: Option<String>,
    content: String,
    created_at: String,
}

pub async fn list_cloud_init_templates(
    State(state): State<AppState>,
) -> Result<Json<Value>, BffError> {
    let rows = sqlx::query_as::<_, CloudInitTemplateRow>(
        r#"
        SELECT
            template_id,
            name,
            description,
            content,
            created_at
        FROM cloud_init_templates
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list cloud_init_templates: {}", e)))?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            // Extract template variables ({{variable_name}} pattern)
            let variables = extract_template_variables(&r.content);
            json!({
                "id": r.template_id,
                "name": r.name,
                "description": r.description.unwrap_or_default(),
                "content": r.content,
                "variables": variables,
                "created_at": r.created_at,
            })
        })
        .collect();

    Ok(Json(json!(items)))
}

pub async fn create_cloud_init_template(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing name".into()))?;

    let description = payload
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let content = payload
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let template_id = chv_common::gen_short_id();

    sqlx::query(
        r#"INSERT INTO cloud_init_templates
           (template_id, name, description, content, created_at, updated_at)
           VALUES (?, ?, ?, ?,
                   strftime('%Y-%m-%dT%H:%M:%SZ','now'),
                   strftime('%Y-%m-%dT%H:%M:%SZ','now'))"#,
    )
    .bind(&template_id)
    .bind(name)
    .bind(description)
    .bind(content)
    .execute(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to create cloud_init_template: {}", e)))?;

    let variables = extract_template_variables(content);

    Ok(Json(json!({
        "id": template_id,
        "name": name,
        "description": description,
        "content": content,
        "variables": variables,
        "created_at": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    })))
}

pub async fn delete_cloud_init_template(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    Path(template_id): Path<String>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let exists: bool =
        sqlx::query_scalar("SELECT COUNT(*) > 0 FROM cloud_init_templates WHERE template_id = ?")
            .bind(&template_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|e| BffError::Internal(format!("db error: {}", e)))?;

    if !exists {
        return Err(BffError::NotFound(format!(
            "cloud-init template {} not found",
            template_id
        )));
    }

    sqlx::query("DELETE FROM cloud_init_templates WHERE template_id = ?")
        .bind(&template_id)
        .execute(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to delete cloud_init_template: {}", e)))?;

    Ok(Json(json!({
        "deleted": true,
        "id": template_id,
    })))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract `{{variable}}` placeholders from a template content string.
fn extract_template_variables(content: &str) -> Vec<String> {
    let mut vars = Vec::new();
    let mut remaining = content;
    while let Some(start) = remaining.find("{{") {
        remaining = &remaining[start + 2..];
        if let Some(end) = remaining.find("}}") {
            let var = remaining[..end].trim().to_string();
            if !var.is_empty() && !vars.contains(&var) {
                vars.push(var);
            }
            remaining = &remaining[end + 2..];
        } else {
            break;
        }
    }
    vars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_no_variables() {
        assert!(extract_template_variables("no vars here").is_empty());
    }

    #[test]
    fn extract_single_variable() {
        let vars = extract_template_variables("hello {{name}}!");
        assert_eq!(vars, vec!["name"]);
    }

    #[test]
    fn extract_multiple_variables() {
        let vars = extract_template_variables("{{a}} and {{b}} and {{a}}");
        assert_eq!(vars, vec!["a", "b"]);
    }
}
