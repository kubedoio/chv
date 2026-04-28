use crate::{StoreError, StorePool};

// ── Backup Jobs ────────────────────────────────────────────────────────────

const LIST_JOBS_SQL: &str = r#"
SELECT
    job_id,
    vm_id,
    volume_id,
    status,
    backup_type,
    target_path,
    storage_backend,
    created_at,
    started_at,
    completed_at,
    error_message,
    size_bytes
FROM backup_jobs
ORDER BY created_at DESC
LIMIT ? OFFSET ?
"#;

const COUNT_JOBS_SQL: &str = "SELECT COUNT(*) FROM backup_jobs";

const GET_JOB_SQL: &str = r#"
SELECT
    job_id,
    vm_id,
    volume_id,
    status,
    backup_type,
    target_path,
    storage_backend,
    created_at,
    started_at,
    completed_at,
    error_message,
    size_bytes
FROM backup_jobs
WHERE job_id = ?
"#;

const INSERT_JOB_SQL: &str = r#"
INSERT INTO backup_jobs (
    job_id,
    vm_id,
    volume_id,
    status,
    backup_type,
    target_path,
    storage_backend,
    created_at,
    started_at,
    completed_at,
    error_message,
    size_bytes
)
VALUES (
    ?, ?, ?, ?, ?, ?, ?,
    strftime('%Y-%m-%dT%H:%M:%SZ','now'),
    ?, ?, ?, ?
)
"#;

const UPDATE_JOB_STATUS_SQL: &str = r#"
UPDATE backup_jobs SET
    status = ?,
    started_at = ?,
    completed_at = ?,
    error_message = ?,
    size_bytes = ?
WHERE job_id = ?
"#;

const UPDATE_JOB_SQL: &str = r#"
UPDATE backup_jobs SET
    volume_id = ?,
    status = ?,
    backup_type = ?,
    target_path = ?,
    storage_backend = ?,
    started_at = ?,
    completed_at = ?,
    error_message = ?,
    size_bytes = ?
WHERE job_id = ?
"#;

const DELETE_JOB_SQL: &str = "DELETE FROM backup_jobs WHERE job_id = ?";

const LIST_JOBS_FOR_VM_SQL: &str = r#"
SELECT
    job_id,
    vm_id,
    volume_id,
    status,
    backup_type,
    target_path,
    storage_backend,
    created_at,
    started_at,
    completed_at,
    error_message,
    size_bytes
FROM backup_jobs
WHERE vm_id = ?
ORDER BY created_at DESC
LIMIT ? OFFSET ?
"#;

const COUNT_JOBS_FOR_VM_SQL: &str = "SELECT COUNT(*) FROM backup_jobs WHERE vm_id = ?";

// ── Backup Schedules ───────────────────────────────────────────────────────

const LIST_SCHEDULES_SQL: &str = r#"
SELECT
    schedule_id,
    vm_id,
    volume_id,
    name,
    cron_expression,
    retention_count,
    destination,
    enabled,
    created_at,
    updated_at
FROM backup_schedules
ORDER BY created_at DESC
LIMIT ? OFFSET ?
"#;

const COUNT_SCHEDULES_SQL: &str = "SELECT COUNT(*) FROM backup_schedules";

const GET_SCHEDULE_SQL: &str = r#"
SELECT
    schedule_id,
    vm_id,
    volume_id,
    name,
    cron_expression,
    retention_count,
    destination,
    enabled,
    created_at,
    updated_at
FROM backup_schedules
WHERE schedule_id = ?
"#;

const INSERT_SCHEDULE_SQL: &str = r#"
INSERT INTO backup_schedules (
    schedule_id,
    vm_id,
    volume_id,
    name,
    cron_expression,
    retention_count,
    destination,
    enabled,
    created_at,
    updated_at
)
VALUES (
    ?, ?, ?, ?, ?, ?, ?, ?,
    strftime('%Y-%m-%dT%H:%M:%SZ','now'),
    strftime('%Y-%m-%dT%H:%M:%SZ','now')
)
"#;

const UPDATE_SCHEDULE_SQL: &str = r#"
UPDATE backup_schedules SET
    vm_id = ?,
    volume_id = ?,
    name = ?,
    cron_expression = ?,
    retention_count = ?,
    destination = ?,
    enabled = ?,
    updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
WHERE schedule_id = ?
"#;

const DELETE_SCHEDULE_SQL: &str = "DELETE FROM backup_schedules WHERE schedule_id = ?";

// ── Backup Restores ────────────────────────────────────────────────────────

const LIST_RESTORES_SQL: &str = r#"
SELECT
    restore_id,
    backup_job_id,
    target_vm_id,
    target_volume_id,
    status,
    created_at,
    started_at,
    completed_at,
    error_message
FROM backup_restores
ORDER BY created_at DESC
LIMIT ? OFFSET ?
"#;

const COUNT_RESTORES_SQL: &str = "SELECT COUNT(*) FROM backup_restores";

const GET_RESTORE_SQL: &str = r#"
SELECT
    restore_id,
    backup_job_id,
    target_vm_id,
    target_volume_id,
    status,
    created_at,
    started_at,
    completed_at,
    error_message
FROM backup_restores
WHERE restore_id = ?
"#;

const INSERT_RESTORE_SQL: &str = r#"
INSERT INTO backup_restores (
    restore_id,
    backup_job_id,
    target_vm_id,
    target_volume_id,
    status,
    created_at,
    started_at,
    completed_at,
    error_message
)
VALUES (
    ?, ?, ?, ?, ?,
    strftime('%Y-%m-%dT%H:%M:%SZ','now'),
    ?, ?, ?
)
"#;

const UPDATE_RESTORE_STATUS_SQL: &str = r#"
UPDATE backup_restores SET
    status = ?,
    started_at = ?,
    completed_at = ?,
    error_message = ?
WHERE restore_id = ?
"#;

// ── Repository ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct BackupRepository {
    pool: StorePool,
}

impl BackupRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    // ── Jobs ─────────────────────────────────────────────────────────────────

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
            .bind(&input.volume_id)
            .bind(&input.status)
            .bind(&input.backup_type)
            .bind(&input.target_path)
            .bind(&input.storage_backend)
            .bind(&input.started_at)
            .bind(&input.completed_at)
            .bind(&input.error_message)
            .bind(input.size_bytes)
            .execute(&self.pool)
            .await?;
        Ok(job_id)
    }

    pub async fn update_job_status(
        &self,
        input: &BackupJobStatusUpdateInput,
    ) -> Result<(), StoreError> {
        sqlx::query(UPDATE_JOB_STATUS_SQL)
            .bind(&input.status)
            .bind(&input.started_at)
            .bind(&input.completed_at)
            .bind(&input.error_message)
            .bind(input.size_bytes)
            .bind(&input.job_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_job(&self, input: &BackupJobUpdateInput) -> Result<(), StoreError> {
        sqlx::query(UPDATE_JOB_SQL)
            .bind(&input.volume_id)
            .bind(&input.status)
            .bind(&input.backup_type)
            .bind(&input.target_path)
            .bind(&input.storage_backend)
            .bind(&input.started_at)
            .bind(&input.completed_at)
            .bind(&input.error_message)
            .bind(input.size_bytes)
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

    pub async fn list_jobs_for_vm(
        &self,
        vm_id: &str,
        page_size: i64,
        offset: i64,
    ) -> Result<(Vec<BackupJobRow>, i64), StoreError> {
        let rows = sqlx::query_as::<_, BackupJobRow>(LIST_JOBS_FOR_VM_SQL)
            .bind(vm_id)
            .bind(page_size)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let count: i64 = sqlx::query_scalar(COUNT_JOBS_FOR_VM_SQL)
            .bind(vm_id)
            .fetch_one(&self.pool)
            .await?;

        Ok((rows, count))
    }

    // ── Schedules ────────────────────────────────────────────────────────────

    pub async fn list_schedules(
        &self,
        page_size: i64,
        offset: i64,
    ) -> Result<(Vec<BackupScheduleRow>, i64), StoreError> {
        let rows = sqlx::query_as::<_, BackupScheduleRow>(LIST_SCHEDULES_SQL)
            .bind(page_size)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let count: i64 = sqlx::query_scalar(COUNT_SCHEDULES_SQL)
            .fetch_one(&self.pool)
            .await?;

        Ok((rows, count))
    }

    pub async fn get_schedule(
        &self,
        schedule_id: &str,
    ) -> Result<Option<BackupScheduleRow>, StoreError> {
        let row = sqlx::query_as::<_, BackupScheduleRow>(GET_SCHEDULE_SQL)
            .bind(schedule_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn create_schedule(
        &self,
        input: &BackupScheduleCreateInput,
    ) -> Result<String, StoreError> {
        let schedule_id = chv_common::gen_short_id();
        sqlx::query(INSERT_SCHEDULE_SQL)
            .bind(&schedule_id)
            .bind(&input.vm_id)
            .bind(&input.volume_id)
            .bind(&input.name)
            .bind(&input.cron_expression)
            .bind(input.retention_count)
            .bind(&input.destination)
            .bind(input.enabled)
            .execute(&self.pool)
            .await?;
        Ok(schedule_id)
    }

    pub async fn update_schedule(
        &self,
        input: &BackupScheduleUpdateInput,
    ) -> Result<(), StoreError> {
        sqlx::query(UPDATE_SCHEDULE_SQL)
            .bind(&input.vm_id)
            .bind(&input.volume_id)
            .bind(&input.name)
            .bind(&input.cron_expression)
            .bind(input.retention_count)
            .bind(&input.destination)
            .bind(input.enabled)
            .bind(&input.schedule_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_schedule(&self, schedule_id: &str) -> Result<(), StoreError> {
        sqlx::query(DELETE_SCHEDULE_SQL)
            .bind(schedule_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Restores ─────────────────────────────────────────────────────────────

    pub async fn list_restores(
        &self,
        page_size: i64,
        offset: i64,
    ) -> Result<(Vec<BackupRestoreRow>, i64), StoreError> {
        let rows = sqlx::query_as::<_, BackupRestoreRow>(LIST_RESTORES_SQL)
            .bind(page_size)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let count: i64 = sqlx::query_scalar(COUNT_RESTORES_SQL)
            .fetch_one(&self.pool)
            .await?;

        Ok((rows, count))
    }

    pub async fn get_restore(
        &self,
        restore_id: &str,
    ) -> Result<Option<BackupRestoreRow>, StoreError> {
        let row = sqlx::query_as::<_, BackupRestoreRow>(GET_RESTORE_SQL)
            .bind(restore_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn create_restore(
        &self,
        input: &BackupRestoreCreateInput,
    ) -> Result<String, StoreError> {
        let restore_id = chv_common::gen_short_id();
        sqlx::query(INSERT_RESTORE_SQL)
            .bind(&restore_id)
            .bind(&input.backup_job_id)
            .bind(&input.target_vm_id)
            .bind(&input.target_volume_id)
            .bind(&input.status)
            .bind(&input.started_at)
            .bind(&input.completed_at)
            .bind(&input.error_message)
            .execute(&self.pool)
            .await?;
        Ok(restore_id)
    }

    pub async fn update_restore_status(
        &self,
        input: &BackupRestoreStatusUpdateInput,
    ) -> Result<(), StoreError> {
        sqlx::query(UPDATE_RESTORE_STATUS_SQL)
            .bind(&input.status)
            .bind(&input.started_at)
            .bind(&input.completed_at)
            .bind(&input.error_message)
            .bind(&input.restore_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ── Row Types ──────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
pub struct BackupJobRow {
    pub job_id: String,
    pub vm_id: String,
    pub volume_id: Option<String>,
    pub status: String,
    pub backup_type: String,
    pub target_path: Option<String>,
    pub storage_backend: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
    pub size_bytes: Option<i64>,
}

#[derive(sqlx::FromRow)]
pub struct BackupScheduleRow {
    pub schedule_id: String,
    pub vm_id: String,
    pub volume_id: Option<String>,
    pub name: String,
    pub cron_expression: String,
    pub retention_count: i64,
    pub destination: Option<String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(sqlx::FromRow)]
pub struct BackupRestoreRow {
    pub restore_id: String,
    pub backup_job_id: String,
    pub target_vm_id: Option<String>,
    pub target_volume_id: Option<String>,
    pub status: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
}

// ── Input Types ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct BackupJobCreateInput {
    pub vm_id: String,
    pub volume_id: Option<String>,
    pub status: String,
    pub backup_type: String,
    pub target_path: Option<String>,
    pub storage_backend: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
    pub size_bytes: Option<i64>,
}

#[derive(Clone)]
pub struct BackupJobStatusUpdateInput {
    pub job_id: String,
    pub status: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
    pub size_bytes: Option<i64>,
}

#[derive(Clone)]
pub struct BackupJobUpdateInput {
    pub job_id: String,
    pub volume_id: Option<String>,
    pub status: String,
    pub backup_type: String,
    pub target_path: Option<String>,
    pub storage_backend: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
    pub size_bytes: Option<i64>,
}

#[derive(Clone)]
pub struct BackupScheduleCreateInput {
    pub vm_id: String,
    pub volume_id: Option<String>,
    pub name: String,
    pub cron_expression: String,
    pub retention_count: i64,
    pub destination: Option<String>,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct BackupScheduleUpdateInput {
    pub schedule_id: String,
    pub vm_id: String,
    pub volume_id: Option<String>,
    pub name: String,
    pub cron_expression: String,
    pub retention_count: i64,
    pub destination: Option<String>,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct BackupRestoreCreateInput {
    pub backup_job_id: String,
    pub target_vm_id: Option<String>,
    pub target_volume_id: Option<String>,
    pub status: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone)]
pub struct BackupRestoreStatusUpdateInput {
    pub restore_id: String,
    pub status: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
}
