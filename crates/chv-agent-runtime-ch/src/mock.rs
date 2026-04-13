use async_trait::async_trait;
use chv_errors::ChvError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::adapter::{CloudHypervisorAdapter, VmConfig};

#[derive(Debug, Clone, Default)]
pub struct MockCloudHypervisorAdapter {
    pub vms: Arc<Mutex<HashMap<String, VmConfig>>>,
}

#[async_trait]
impl CloudHypervisorAdapter for MockCloudHypervisorAdapter {
    async fn create_vm(
        &self,
        config: &VmConfig,
        _operation_id: Option<&str>,
    ) -> Result<String, ChvError> {
        self.vms.lock().unwrap().insert(config.vm_id.clone(), config.clone());
        Ok(config.vm_id.clone())
    }

    async fn start_vm(&self, _vm_id: &str, _operation_id: Option<&str>) -> Result<(), ChvError> {
        Ok(())
    }

    async fn stop_vm(
        &self,
        _vm_id: &str,
        _force: bool,
        _operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn delete_vm(&self, vm_id: &str, _operation_id: Option<&str>) -> Result<(), ChvError> {
        self.vms.lock().unwrap().remove(vm_id);
        Ok(())
    }
}
