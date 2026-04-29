use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

/// Validate that a CIDR string is in IPv4 or IPv6 CIDR notation.
/// Accepts "X.X.X.X/N" (N 0-32) or "hex:…/N" (N 0-128).
fn is_valid_cidr(cidr: &str) -> bool {
    if let Some((addr, prefix)) = cidr.split_once('/') {
        if let Ok(n) = prefix.parse::<u32>() {
            if addr.contains(':') {
                // IPv6: simple check for hex+colons structure, prefix 0-128
                return n <= 128 && addr.split(':').count() <= 8;
            } else {
                // IPv4: four dotted octets, prefix 0-32
                let octets: Vec<&str> = addr.split('.').collect();
                if octets.len() == 4 && n <= 32 {
                    return octets.iter().all(|o| o.parse::<u8>().is_ok());
                }
            }
        }
    }
    false
}

#[derive(sqlx::FromRow)]
struct FirewallRuleRow {
    rule_id: String,
    network_id: String,
    direction: String,
    action: String,
    protocol: String,
    port_range: Option<String>,
    source_cidr: Option<String>,
    description: Option<String>,
    priority: i64,
    created_at: String,
}

pub async fn list_firewall_rules(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let network_id = payload
        .get("network_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing network_id".into()))?;

    let rows = sqlx::query_as::<_, FirewallRuleRow>(
        r#"
        SELECT
            rule_id,
            network_id,
            direction,
            action,
            protocol,
            port_range,
            source_cidr,
            description,
            priority,
            created_at
        FROM firewall_rules
        WHERE network_id = ?
        ORDER BY priority ASC
        "#,
    )
    .bind(network_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list firewall_rules: {}", e)))?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.rule_id,
                "network_id": r.network_id,
                "direction": r.direction,
                "action": r.action,
                "protocol": r.protocol,
                "port_range": r.port_range.unwrap_or_default(),
                "source_cidr": r.source_cidr.unwrap_or_else(|| "0.0.0.0/0".to_string()),
                "description": r.description.unwrap_or_default(),
                "priority": r.priority,
                "created_at": r.created_at,
            })
        })
        .collect();

    Ok(Json(json!(items)))
}

pub async fn create_firewall_rule(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_admin(&claims)?;
    let network_id = payload
        .get("network_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing network_id".into()))?;

    let direction = payload
        .get("direction")
        .and_then(|v| v.as_str())
        .unwrap_or("inbound");

    let action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("allow");

    let protocol = payload
        .get("protocol")
        .and_then(|v| v.as_str())
        .unwrap_or("tcp");

    let port_range = payload
        .get("port_range")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let source_cidr = payload
        .get("source_cidr")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0.0/0");

    if !is_valid_cidr(source_cidr) {
        return Err(BffError::BadRequest(format!(
            "invalid CIDR format: {}",
            source_cidr
        )));
    }

    let description = payload
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let priority = payload
        .get("priority")
        .and_then(|v| v.as_i64())
        .unwrap_or(100);

    let rule_id = chv_common::gen_short_id();

    sqlx::query(
        r#"INSERT INTO firewall_rules
           (rule_id, network_id, direction, action, protocol, port_range, source_cidr, description, priority)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&rule_id)
    .bind(network_id)
    .bind(direction)
    .bind(action)
    .bind(protocol)
    .bind(port_range)
    .bind(source_cidr)
    .bind(description)
    .bind(priority)
    .execute(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to create firewall_rule: {}", e)))?;
    state.cache.invalidate("networks:").await;
    state.cache.invalidate("overview").await;


    Ok(Json(json!({
        "id": rule_id,
        "network_id": network_id,
        "direction": direction,
        "action": action,
        "protocol": protocol,
        "port_range": port_range,
        "source_cidr": source_cidr,
        "description": description,
        "priority": priority,
        "created_at": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    })))
}

pub async fn delete_firewall_rule(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_admin(&claims)?;
    let rule_id = payload
        .get("rule_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing rule_id".into()))?;

    let exists: bool =
        sqlx::query_scalar("SELECT COUNT(*) > 0 FROM firewall_rules WHERE rule_id = ?")
            .bind(rule_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|e| BffError::Internal(format!("db error: {}", e)))?;

    if !exists {
        return Err(BffError::NotFound(format!(
            "firewall rule {} not found",
            rule_id
        )));
    }

    sqlx::query("DELETE FROM firewall_rules WHERE rule_id = ?")
        .bind(rule_id)
        .execute(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to delete firewall_rule: {}", e)))?;
    state.cache.invalidate("networks:").await;
    state.cache.invalidate("overview").await;


    Ok(Json(json!({
        "deleted": true,
        "id": rule_id,
    })))
}
