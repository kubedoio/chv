use crate::{StoreError, StorePool};

const LIST_JOBS_SQL: &str = r#"
SELECT
    job_id,
    vm_id,
    name,
    schedule,
    destination,
    retention_days,
    enabled,
    last_run_at,
    next_run_at,
    created_at,
    updated_at
FROM backup_jobs
ORDER BY created_at DESC
LIMIT ? OFFSET ?
"#;

const COUNT_JOBS_SQL: &str = "SELECT COUNT(*) FROM backup_jobs";

const GET_JOB_SQL: &str = r#"
SELECT
    job_id,
    vm_id,
    name,
    schedule,
    destination,
    retention_days,
    enabled,
    last_run_at,
    next_run_at,
    created_at,
    updated_at
FROM backup_jobs
WHERE job_id = ?
"#;

const INSERT_JOB_SQL: &str = r#"
INSERT INTO backup_jobs (
    job_id,
    vm_id,
    name,
    schedule,
    destination,
    retention_days,
    enabled,
    last_run_at,
    next_run_at,
    created_at,
    updated_at
)
VALUES (
    ?,
    ?,
    ?,
    ?,
    ?,
    ?,
    ?,
    ?,
    ?,
    strftime('%Y-%m-%dT%H:%M:%SZ','now'),
    strftime('%Y-%m-%dT%H:%M:%SZ','now')
)
"#;

const UPDATE_JOB_SQL: &str = r#"
UPDATE backup_jobs SET
    vm_id = ?,
    name = ?,
    schedule = ?,
    destination = ?,
    retention_days = ?,
    enabled = ?,
    last_run_at = ?,
    next_run_at = ?,
    updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
WHERE job_id = ?
"#;

const DELETE_JOB_SQL: &str = "DELETE FROM backup_jobs WHERE job_id = ?";

const LIST_HISTORY_FOR_JOB_SQL: &str = r#"
SELECT
    history_id,
    job_id,
    vm_id,
    started_at,
    completed_at,
    status,
    size_bytes,
    error_message,
    created_at
FROM backup_history
WHERE job_id = ?
ORDER BY started_at DESC
LIMIT ? OFFSET ?
"#;

const COUNT_HISTORY_FOR_JOB_SQL: &str = "SELECT COUNT(*) FROM backup_history WHERE job_id = ?";

const LIST_RECENT_HISTORY_SQL: &str = r#"
SELECT
    history_id,
    job_id,
    vm_id,
    started_at,
    completed_at,
    status,
    size_bytes,
    error_message,
    created_at
FROM backup_history
ORDER BY started_at DESC
LIMIT ? OFFSET ?
"#;

const COUNT_RECENT_HISTORY_SQL: &str = "SELECT COUNT(*) FROM backup_history";

#[derive(Clone)]
pub struct BackupJobRepository {
    pool: StorePool,
}

impl BackupJobRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn list_jobs(
        &self,
        page_size: i64,
        offset: i64,
    ) -> Result<(Vec<BackupJobRow>, i64), StoreError> {
        let rows = sqlx::query_as::<_, BackupJobRow>(LIST_JOBS_SQL)
            .bind(page_size)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let count: i64 = sqlx::query_scalar(COUNT_JOBS_SQL)
            .fetch_one(&self.pool)
            .await?;

        Ok((rows, count))
    }

    pub async fn get_job(&self, job_id: &str) -> Result<Option<BackupJobRow>, StoreError> {
        let row = sqlx::query_as::<_, BackupJobRow>(GET_JOB_SQL)
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn create_job(&self, input: &BackupJobCreateInput) -> Result<String, StoreError> {
        let job_id = chv_common::gen_short_id();
        sqlx::query(INSERT_JOB_SQL)
            .bind(&job_id)
            .bind(&input.vm_id)
            .bind(&input.name)
            .bind(&input.schedule)
            .bind(&input.destination)
            .bind(input.retention_days)
            .bind(input.enabled)
            .bind(&input.last_run_at)
            .bind(&input.next_run_at)
            .execute(&self.pool)
            .await?;
        Ok(job_id)
    }

    pub async fn update_job(&self, input: &BackupJobUpdateInput) -> Result<(), StoreError> {
        sqlx::query(UPDATE_JOB_SQL)
            .bind(&input.vm_id)
            .bind(&input.name)
            .bind(&input.schedule)
            .bind(&input.destination)
            .bind(input.retention_days)
            .bind(input.enabled)
            .bind(&input.last_run_at)
            .bind(&input.next_run_at)
            .bind(&input.job_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_job(&self, job_id: &str) -> Result<(), StoreError> {
        sqlx::query(DELETE_JOB_SQL)
            .bind(job_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_history_for_job(
        &self,
        job_id: &str,
        page_size: i64,
        offset: i64,
    ) -> Result<(Vec<BackupHistoryRow>, i64), StoreError> {
        let rows = sqlx::query_as::<_, BackupHistoryRow>(LIST_HISTORY_FOR_JOB_SQL)
            .bind(job_id)
            .bind(page_size)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let count: i64 = sqlx::query_scalar(COUNT_HISTORY_FOR_JOB_SQL)
            .bind(job_id)
            .fetch_one(&self.pool)
            .await?;

        Ok((rows, count))
    }

    pub async fn list_recent_history(
        &self,
        page_size: i64,
        offset: i64,
    ) -> Result<(Vec<BackupHistoryRow>, i64), StoreError> {
        let rows = sqlx::query_as::<_, BackupHistoryRow>(LIST_RECENT_HISTORY_SQL)
            .bind(page_size)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let count: i64 = sqlx::query_scalar(COUNT_RECENT_HISTORY_SQL)
            .fetch_one(&self.pool)
            .await?;

        Ok((rows, count))
    }
}

#[derive(sqlx::FromRow)]
pub struct BackupJobRow {
    pub job_id: String,
    pub vm_id: String,
    pub name: String,
    pub schedule: String,
    pub destination: String,
    pub retention_days: i64,
    pub enabled: bool,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct BackupJobCreateInput {
    pub vm_id: String,
    pub name: String,
    pub schedule: String,
    pub destination: String,
    pub retention_days: i64,
    pub enabled: bool,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
}

#[derive(Clone)]
pub struct BackupJobUpdateInput {
    pub job_id: String,
    pub vm_id: String,
    pub name: String,
    pub schedule: String,
    pub destination: String,
    pub retention_days: i64,
    pub enabled: bool,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
}

#[derive(sqlx::FromRow)]
pub struct BackupHistoryRow {
    pub history_id: String,
    pub job_id: String,
    pub vm_id: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: String,
    pub size_bytes: Option<i64>,
    pub error_message: Option<String>,
    pub created_at: String,
}
