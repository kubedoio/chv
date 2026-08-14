use crate::{StoreError, StorePool};

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct HypervisorSettingsRow {
    pub id: i64,
    pub cpu_nested: bool,
    pub cpu_amx: bool,
    pub cpu_kvm_hyperv: bool,
    pub memory_mergeable: bool,
    pub memory_hugepages: bool,
    pub memory_shared: bool,
    pub memory_prefault: bool,
    pub iommu: bool,
    pub rng_src: String,
    pub watchdog: bool,
    pub landlock_enable: bool,
    pub serial_mode: String,
    pub console_mode: String,
    pub pvpanic: bool,
    pub tpm_type: Option<String>,
    pub tpm_socket_path: Option<String>,
    pub profile_id: Option<String>,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct HypervisorSettingsRepository {
    pool: StorePool,
}

impl HypervisorSettingsRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn get_settings(&self) -> Result<HypervisorSettingsRow, StoreError> {
        let row = sqlx::query_as::<_, HypervisorSettingsRow>(
            "SELECT * FROM hypervisor_settings WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }
}
