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

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct HypervisorProfileRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub cpu_nested: Option<bool>,
    pub cpu_amx: Option<bool>,
    pub cpu_kvm_hyperv: Option<bool>,
    pub memory_mergeable: Option<bool>,
    pub memory_hugepages: Option<bool>,
    pub memory_shared: Option<bool>,
    pub memory_prefault: Option<bool>,
    pub iommu: Option<bool>,
    pub rng_src: Option<String>,
    pub watchdog: Option<bool>,
    pub landlock_enable: Option<bool>,
    pub serial_mode: Option<String>,
    pub console_mode: Option<String>,
    pub pvpanic: Option<bool>,
    pub tpm_type: Option<String>,
    pub tpm_socket_path: Option<String>,
    pub is_builtin: bool,
    pub created_at: String,
}

#[derive(Clone, Default)]
pub struct HypervisorSettingsPatchInput {
    pub cpu_nested: Option<bool>,
    pub cpu_amx: Option<bool>,
    pub cpu_kvm_hyperv: Option<bool>,
    pub memory_mergeable: Option<bool>,
    pub memory_hugepages: Option<bool>,
    pub memory_shared: Option<bool>,
    pub memory_prefault: Option<bool>,
    pub iommu: Option<bool>,
    pub rng_src: Option<String>,
    pub watchdog: Option<bool>,
    pub landlock_enable: Option<bool>,
    pub serial_mode: Option<String>,
    pub console_mode: Option<String>,
    pub pvpanic: Option<bool>,
    pub tpm_type: Option<String>,
    pub tpm_socket_path: Option<String>,
    pub profile_id: Option<String>,
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

    pub async fn update_settings(
        &self,
        input: &HypervisorSettingsPatchInput,
    ) -> Result<(), StoreError> {
        let mut sets = Vec::new();

        if input.cpu_nested.is_some() {
            sets.push("cpu_nested = ?");
        }
        if input.cpu_amx.is_some() {
            sets.push("cpu_amx = ?");
        }
        if input.cpu_kvm_hyperv.is_some() {
            sets.push("cpu_kvm_hyperv = ?");
        }
        if input.memory_mergeable.is_some() {
            sets.push("memory_mergeable = ?");
        }
        if input.memory_hugepages.is_some() {
            sets.push("memory_hugepages = ?");
        }
        if input.memory_shared.is_some() {
            sets.push("memory_shared = ?");
        }
        if input.memory_prefault.is_some() {
            sets.push("memory_prefault = ?");
        }
        if input.iommu.is_some() {
            sets.push("iommu = ?");
        }
        if input.rng_src.is_some() {
            sets.push("rng_src = ?");
        }
        if input.watchdog.is_some() {
            sets.push("watchdog = ?");
        }
        if input.landlock_enable.is_some() {
            sets.push("landlock_enable = ?");
        }
        if input.serial_mode.is_some() {
            sets.push("serial_mode = ?");
        }
        if input.console_mode.is_some() {
            sets.push("console_mode = ?");
        }
        if input.pvpanic.is_some() {
            sets.push("pvpanic = ?");
        }
        if input.tpm_type.is_some() {
            sets.push("tpm_type = ?");
        }
        if input.tpm_socket_path.is_some() {
            sets.push("tpm_socket_path = ?");
        }
        if input.profile_id.is_some() {
            sets.push("profile_id = ?");
        }

        if sets.is_empty() {
            return Ok(());
        }

        sets.push("updated_at = datetime('now')");

        let sql = format!(
            "UPDATE hypervisor_settings SET {} WHERE id = 1",
            sets.join(", ")
        );
        let mut query = sqlx::query(&sql);

        if let Some(v) = input.cpu_nested {
            query = query.bind(v);
        }
        if let Some(v) = input.cpu_amx {
            query = query.bind(v);
        }
        if let Some(v) = input.cpu_kvm_hyperv {
            query = query.bind(v);
        }
        if let Some(v) = input.memory_mergeable {
            query = query.bind(v);
        }
        if let Some(v) = input.memory_hugepages {
            query = query.bind(v);
        }
        if let Some(v) = input.memory_shared {
            query = query.bind(v);
        }
        if let Some(v) = input.memory_prefault {
            query = query.bind(v);
        }
        if let Some(v) = input.iommu {
            query = query.bind(v);
        }
        if let Some(ref v) = input.rng_src {
            query = query.bind(v);
        }
        if let Some(v) = input.watchdog {
            query = query.bind(v);
        }
        if let Some(v) = input.landlock_enable {
            query = query.bind(v);
        }
        if let Some(ref v) = input.serial_mode {
            query = query.bind(v);
        }
        if let Some(ref v) = input.console_mode {
            query = query.bind(v);
        }
        if let Some(v) = input.pvpanic {
            query = query.bind(v);
        }
        if let Some(ref v) = input.tpm_type {
            query = query.bind(v);
        }
        if let Some(ref v) = input.tpm_socket_path {
            query = query.bind(v);
        }
        if let Some(ref v) = input.profile_id {
            query = query.bind(v);
        }

        query.execute(&self.pool).await?;
        Ok(())
    }

    pub async fn list_profiles(&self) -> Result<Vec<HypervisorProfileRow>, StoreError> {
        let rows = sqlx::query_as::<_, HypervisorProfileRow>(
            "SELECT * FROM hypervisor_profiles ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_profile(&self, id: &str) -> Result<Option<HypervisorProfileRow>, StoreError> {
        let row = sqlx::query_as::<_, HypervisorProfileRow>(
            "SELECT * FROM hypervisor_profiles WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn apply_profile(&self, profile_id: &str) -> Result<(), StoreError> {
        let profile = self.get_profile(profile_id).await?;
        let profile = profile.ok_or_else(|| StoreError::NotFound {
            entity: "hypervisor_profile",
            id: profile_id.to_string(),
        })?;

        let mut sets = vec!["profile_id = ?", "updated_at = datetime('now')"];

        if profile.cpu_nested.is_some() {
            sets.push("cpu_nested = ?");
        }
        if profile.cpu_amx.is_some() {
            sets.push("cpu_amx = ?");
        }
        if profile.cpu_kvm_hyperv.is_some() {
            sets.push("cpu_kvm_hyperv = ?");
        }
        if profile.memory_mergeable.is_some() {
            sets.push("memory_mergeable = ?");
        }
        if profile.memory_hugepages.is_some() {
            sets.push("memory_hugepages = ?");
        }
        if profile.memory_shared.is_some() {
            sets.push("memory_shared = ?");
        }
        if profile.memory_prefault.is_some() {
            sets.push("memory_prefault = ?");
        }
        if profile.iommu.is_some() {
            sets.push("iommu = ?");
        }
        if profile.rng_src.is_some() {
            sets.push("rng_src = ?");
        }
        if profile.watchdog.is_some() {
            sets.push("watchdog = ?");
        }
        if profile.landlock_enable.is_some() {
            sets.push("landlock_enable = ?");
        }
        if profile.serial_mode.is_some() {
            sets.push("serial_mode = ?");
        }
        if profile.console_mode.is_some() {
            sets.push("console_mode = ?");
        }
        if profile.pvpanic.is_some() {
            sets.push("pvpanic = ?");
        }
        if profile.tpm_type.is_some() {
            sets.push("tpm_type = ?");
        }
        if profile.tpm_socket_path.is_some() {
            sets.push("tpm_socket_path = ?");
        }

        let sql = format!(
            "UPDATE hypervisor_settings SET {} WHERE id = 1",
            sets.join(", ")
        );
        let mut query = sqlx::query(&sql);

        query = query.bind(profile_id);

        if profile.cpu_nested.is_some() {
            query = query.bind(profile.cpu_nested);
        }
        if profile.cpu_amx.is_some() {
            query = query.bind(profile.cpu_amx);
        }
        if profile.cpu_kvm_hyperv.is_some() {
            query = query.bind(profile.cpu_kvm_hyperv);
        }
        if profile.memory_mergeable.is_some() {
            query = query.bind(profile.memory_mergeable);
        }
        if profile.memory_hugepages.is_some() {
            query = query.bind(profile.memory_hugepages);
        }
        if profile.memory_shared.is_some() {
            query = query.bind(profile.memory_shared);
        }
        if profile.memory_prefault.is_some() {
            query = query.bind(profile.memory_prefault);
        }
        if profile.iommu.is_some() {
            query = query.bind(profile.iommu);
        }
        if profile.rng_src.is_some() {
            query = query.bind(profile.rng_src);
        }
        if profile.watchdog.is_some() {
            query = query.bind(profile.watchdog);
        }
        if profile.landlock_enable.is_some() {
            query = query.bind(profile.landlock_enable);
        }
        if profile.serial_mode.is_some() {
            query = query.bind(profile.serial_mode);
        }
        if profile.console_mode.is_some() {
            query = query.bind(profile.console_mode);
        }
        if profile.pvpanic.is_some() {
            query = query.bind(profile.pvpanic);
        }
        if profile.tpm_type.is_some() {
            query = query.bind(profile.tpm_type);
        }
        if profile.tpm_socket_path.is_some() {
            query = query.bind(profile.tpm_socket_path);
        }

        query.execute(&self.pool).await?;
        Ok(())
    }
}
