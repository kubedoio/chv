//! Read-only `ImageRepository` used by the Architecture Designer fleet
//! checks. `IMAGE_NOT_FOUND` matches against `display_name`, so the row
//! exposes exactly the three columns the validator needs.
//!
//! Read-only by design — image lifecycle is owned by the existing
//! `images` handler in the BFF (import / delete).

use crate::{StoreError, StorePool};
use sqlx::Row;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRow {
    pub image_id: String,
    pub display_name: String,
    pub format: String,
}

#[derive(Clone)]
pub struct ImageRepository {
    pool: StorePool,
}

impl ImageRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn list(&self) -> Result<Vec<ImageRow>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT image_id, display_name, format
            FROM images
            ORDER BY image_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(|row| {
                Ok(ImageRow {
                    image_id: row.try_get("image_id")?,
                    display_name: row.try_get("display_name")?,
                    format: row.try_get("format")?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::create_test_pool;

    #[tokio::test]
    async fn list_returns_images() {
        let pool = create_test_pool().await;

        sqlx::query(
            r#"INSERT INTO images (image_id, display_name, format)
               VALUES ('img-1', 'ubuntu-24.04', 'qcow2')"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let repo = ImageRepository::new(pool);
        let rows = repo.list().await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].image_id, "img-1");
        assert_eq!(rows[0].display_name, "ubuntu-24.04");
        assert_eq!(rows[0].format, "qcow2");
    }
}
