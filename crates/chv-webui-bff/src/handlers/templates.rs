use axum::{
    extract::{Path, State},
    response::Json,
};
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

pub async fn list_vm_templates(State(state): State<AppState>) -> Result<Json<Value>, BffError> {
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

    let vcpu = payload.get("vcpu").and_then(|v| v.as_i64()).unwrap_or(2);

    let memory_mb = payload
        .get("memory_mb")
        .and_then(|v| v.as_i64())
        .unwrap_or(2048);

    let memory_bytes = memory_mb * 1024 * 1024;

    let image_id = payload.get("image_id").and_then(|v| v.as_str());

    let network_id = payload.get("network_id").and_then(|v| v.as_str());

    let cloud_init_config = payload.get("cloud_init_config").and_then(|v| v.as_str());

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
           WHERE t.template_id = ?"#,
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
// VM Template Clone / Preview
// ---------------------------------------------------------------------------

pub async fn preview_vm_template(
    crate::auth::BearerToken(_claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    Path(template_id): Path<String>,
) -> Result<Json<Value>, BffError> {
    let row = sqlx::query_as::<_, VMTemplateRow>(
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
        WHERE template_id = ?
        "#,
    )
    .bind(&template_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to fetch vm_template: {}", e)))?;

    let row =
        row.ok_or_else(|| BffError::NotFound(format!("vm template {} not found", template_id)))?;

    Ok(Json(json!({
        "id": row.template_id,
        "name": row.name,
        "description": row.description.unwrap_or_default(),
        "vcpu": row.cpu_count,
        "memory_mb": row.memory_bytes / (1024 * 1024),
        "image_id": row.image_id.unwrap_or_default(),
        "network_id": row.network_id.unwrap_or_default(),
        "cloud_init_config": row.cloud_init_userdata,
        "created_at": row.created_at,
    })))
}

pub async fn clone_vm_template(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    Path(template_id): Path<String>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;

    let display_name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing name".into()))?
        .to_string();

    // Fetch template
    let template = sqlx::query_as::<_, VMTemplateRow>(
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
        WHERE template_id = ?
        "#,
    )
    .bind(&template_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to fetch vm_template: {}", e)))?
    .ok_or_else(|| BffError::NotFound(format!("vm template {} not found", template_id)))?;

    // Pick default node
    let node_id = sqlx::query_scalar::<_, String>(
        "SELECT node_id FROM nodes ORDER BY enrolled_at DESC LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to query nodes: {}", e)))?
    .ok_or_else(|| BffError::BadRequest("no nodes enrolled".into()))?;

    // Resolve image_ref
    let mut image_ref = template.image_id.clone().unwrap_or_default();
    if !image_ref.is_empty() && image_ref != "default" {
        if let Some(source_url) = sqlx::query_scalar::<_, Option<String>>(
            "SELECT source_url FROM images WHERE image_id = ?",
        )
        .bind(&image_ref)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to look up image: {}", e)))?
        .flatten()
        {
            if source_url.starts_with('/') {
                image_ref = source_url;
            }
        }
    }
    if image_ref.is_empty() {
        image_ref = "default".to_string();
    }

    // Cloud-init: custom_user_data > rendered template variables > template default
    let cloud_init_userdata =
        if let Some(custom) = payload.get("custom_user_data").and_then(|v| v.as_str()) {
            Some(custom.to_string())
        } else {
            let vars = payload.get("variables").and_then(|v| v.as_object());
            if let (Some(base), Some(vars)) = (template.cloud_init_userdata.as_ref(), vars) {
                let mut rendered = base.clone();
                for (key, value) in vars {
                    let placeholder = format!("{{{{{}}}}}", key);
                    rendered = rendered.replace(&placeholder, value.as_str().unwrap_or(""));
                }
                Some(rendered)
            } else {
                template.cloud_init_userdata.clone()
            }
        };

    let network_id = template
        .network_id
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let volume_size_bytes = payload
        .get("volume_size_gb")
        .and_then(|v| v.as_i64())
        .unwrap_or(10)
        * 1024
        * 1024
        * 1024;

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| BffError::Internal(format!("failed to begin transaction: {}", e)))?;

    // Enforce quota inside the transaction to avoid races
    crate::handlers::vms::enforce_user_quota(
        &mut tx,
        &claims.username,
        template.cpu_count,
        template.memory_bytes,
        volume_size_bytes,
        1, // vm_count_delta: creating 1 new VM from template
    )
    .await?;

    let vm_id = chv_common::gen_short_id();
    let volume_id = chv_common::gen_short_id();
    let operation_id = chv_common::gen_short_id();

    // Insert VM
    sqlx::query(
        r#"
        INSERT INTO vms (vm_id, node_id, display_name, created_at, updated_at)
        VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&vm_id)
    .bind(&node_id)
    .bind(&display_name)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert vm: {}", e)))?;

    // Insert VM desired state
    sqlx::query(
        r#"
        INSERT INTO vm_desired_state (vm_id, desired_generation, desired_status, desired_power_state, requested_by, target_node_id, cpu_count, memory_bytes, image_ref, cloud_init_userdata, requested_at, updated_at)
        VALUES (?, 1, 'Pending', 'Running', ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&vm_id)
    .bind(&claims.username)
    .bind(&node_id)
    .bind(template.cpu_count)
    .bind(template.memory_bytes)
    .bind(&image_ref)
    .bind(&cloud_init_userdata)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert vm_desired_state: {}", e)))?;

    // Insert volume
    sqlx::query(
        r#"
        INSERT INTO volumes (volume_id, node_id, display_name, capacity_bytes, updated_at)
        VALUES (?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&volume_id)
    .bind(&node_id)
    .bind(format!("{}-disk", display_name))
    .bind(volume_size_bytes)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert volume: {}", e)))?;

    // Insert volume desired state
    sqlx::query(
        r#"
        INSERT INTO volume_desired_state (volume_id, desired_generation, desired_status, requested_by, attached_vm_id, device_name, read_only, requested_at, updated_at)
        VALUES (?, 1, 'Pending', ?, ?, 'vda', 0, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&volume_id)
    .bind(&claims.username)
    .bind(&vm_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert volume_desired_state: {}", e)))?;

    // Ensure network exists
    let network_exists: Option<String> =
        sqlx::query_scalar("SELECT network_id FROM networks WHERE network_id = ?")
            .bind(&network_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| BffError::Internal(format!("failed to check network: {}", e)))?;

    if network_exists.is_none() {
        sqlx::query(
            r#"
            INSERT INTO networks (network_id, node_id, display_name, updated_at)
            VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'))
            "#,
        )
        .bind(&network_id)
        .bind(&node_id)
        .bind(format!("network-{}", network_id))
        .execute(&mut *tx)
        .await
        .map_err(|e| BffError::Internal(format!("failed to insert network: {}", e)))?;

        sqlx::query(
            r#"
            INSERT INTO network_desired_state (
                network_id, desired_generation, desired_status,
                cidr, gateway, dhcp_enabled, ipam_mode, is_default,
                requested_by, requested_at, updated_at
            )
            VALUES (?, 1, 'Pending', '10.200.0.0/24', '10.200.0.1', 1, 'internal', 0, ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
            "#,
        )
        .bind(&network_id)
        .bind(&claims.username)
        .execute(&mut *tx)
        .await
        .map_err(|e| BffError::Internal(format!("failed to insert network_desired_state: {}", e)))?;
    }

    // Fetch network CIDR for IP generation
    let network_cidr: String = sqlx::query_scalar(
        "SELECT COALESCE(cidr, '') FROM network_desired_state WHERE network_id = ?",
    )
    .bind(&network_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to fetch network cidr: {}", e)))?;

    // Insert NIC
    let nic_id = format!("{}-{}", vm_id, network_id);
    let mac_address = crate::handlers::vms::generate_mac(&vm_id, &network_id);
    let ip_address = crate::handlers::vms::generate_ip(&vm_id, &network_id, &network_cidr);

    sqlx::query(
        r#"
        INSERT INTO vm_nic_desired_state (nic_id, vm_id, network_id, mac_address, ip_address, nic_model, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, 'virtio', strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&nic_id)
    .bind(&vm_id)
    .bind(&network_id)
    .bind(&mac_address)
    .bind(&ip_address)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert vm_nic_desired_state: {}", e)))?;

    // Insert operation
    let idempotency_key = format!("clone-vm-{}", vm_id);
    sqlx::query(
        r#"
        INSERT INTO operations (operation_id, idempotency_key, resource_kind, resource_id, operation_type, status, requested_by, desired_generation, requested_at, created_at, updated_at)
        VALUES (?, ?, 'vm', ?, 'CreateVm', 'Accepted', ?, 1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&operation_id)
    .bind(&idempotency_key)
    .bind(&vm_id)
    .bind(&claims.username)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert operation: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| BffError::Internal(format!("failed to commit transaction: {}", e)))?;

    Ok(Json(json!({
        "vm_id": vm_id,
        "name": display_name,
        "operation_id": operation_id,
        "status": "Accepted",
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

pub async fn render_cloud_init_template(
    crate::auth::BearerToken(_claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    Path(template_id): Path<String>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let row = sqlx::query_as::<_, CloudInitTemplateRow>(
        r#"
        SELECT
            template_id,
            name,
            description,
            content,
            created_at
        FROM cloud_init_templates
        WHERE template_id = ?
        "#,
    )
    .bind(&template_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to fetch cloud_init_template: {}", e)))?
    .ok_or_else(|| BffError::NotFound(format!("cloud-init template {} not found", template_id)))?;

    let mut rendered = row.content;
    if let Some(vars) = payload.get("variables").and_then(|v| v.as_object()) {
        for (key, value) in vars {
            let placeholder = format!("{{{{{}}}}}", key);
            rendered = rendered.replace(&placeholder, value.as_str().unwrap_or(""));
        }
    }

    Ok(Json(json!({
        "rendered": rendered,
        "template_id": template_id,
        "template_name": row.name,
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
