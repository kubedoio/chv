//! Architecture version repository — append-only history.

use crate::architectures::parse_ts;
use crate::{StoreError, StorePool};
use chv_controlplane_types::architecture::{
    ArchitectureId, ArchitectureVersion, ArchitectureVersionId,
};
use sqlx::Row;

const ENTITY: &str = "architecture_version";

#[derive(Clone, Debug)]
pub struct VersionCreateInput {
    pub id: ArchitectureVersionId,
    pub architecture_id: ArchitectureId,
    pub version_number: i64,
    pub yaml_content: String,
    pub design_graph_json: Option<String>,
    pub normalized_model_json: Option<String>,
    pub change_summary: Option<String>,
    pub created_by: Option<String>,
}

#[derive(Clone)]
pub struct VersionRepository {
    pool: StorePool,
}

impl VersionRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn create(
        &self,
        input: VersionCreateInput,
    ) -> Result<ArchitectureVersion, StoreError> {
        let row = sqlx::query(
            r#"
            INSERT INTO architecture_versions (
                id,
                architecture_id,
                version_number,
                yaml_content,
                design_graph_json,
                normalized_model_json,
                change_summary,
                created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
        )
        .bind(input.id.as_str())
        .bind(input.architecture_id.as_str())
        .bind(input.version_number)
        .bind(&input.yaml_content)
        .bind(&input.design_graph_json)
        .bind(&input.normalized_model_json)
        .bind(&input.change_summary)
        .bind(&input.created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| match &err {
            sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => {
                StoreError::NotFound {
                    entity: "architecture_topology",
                    id: input.architecture_id.to_string(),
                }
            }
            _ => StoreError::from(err),
        })?;

        row_to_version(&row)
    }

    pub async fn get(
        &self,
        id: &ArchitectureVersionId,
    ) -> Result<ArchitectureVersion, StoreError> {
        let row = sqlx::query(r#"SELECT * FROM architecture_versions WHERE id = $1"#)
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StoreError::NotFound {
                entity: ENTITY,
                id: id.to_string(),
            })?;
        row_to_version(&row)
    }

    pub async fn list_for_architecture(
        &self,
        architecture_id: &ArchitectureId,
    ) -> Result<Vec<ArchitectureVersion>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM architecture_versions
            WHERE architecture_id = $1
            ORDER BY version_number DESC, created_at DESC
            "#,
        )
        .bind(architecture_id.as_str())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_version).collect()
    }
}

fn row_to_version(row: &sqlx::sqlite::SqliteRow) -> Result<ArchitectureVersion, StoreError> {
    let id_str: String = row.try_get("id")?;
    let arch_id_str: String = row.try_get("architecture_id")?;
    let id = ArchitectureVersionId::new(id_str).map_err(|err| {
        StoreError::InvalidConfiguration {
            reason: format!("invalid id in version row: {err}"),
        }
    })?;
    let architecture_id =
        ArchitectureId::new(arch_id_str).map_err(|err| StoreError::InvalidConfiguration {
            reason: format!("invalid architecture_id in version row: {err}"),
        })?;
    let created_at: String = row.try_get("created_at")?;

    Ok(ArchitectureVersion {
        id,
        architecture_id,
        version_number: row.try_get("version_number")?,
        yaml_content: row.try_get("yaml_content")?,
        design_graph_json: row.try_get("design_graph_json")?,
        normalized_model_json: row.try_get("normalized_model_json")?,
        change_summary: row.try_get("change_summary")?,
        created_by: row.try_get("created_by")?,
        created_at: parse_ts(&created_at, "created_at")?,
    })
}
