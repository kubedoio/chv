use control_plane_node_api::control_plane_node_api as proto;

pub struct TelemetryReporter {
    node_id: String,
}

impl TelemetryReporter {
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
        }
    }

    pub fn node_state_report(
        &self,
        state: &str,
        observed_generation: &str,
        health_status: &str,
        last_error: Option<String>,
    ) -> proto::NodeStateReport {
        proto::NodeStateReport {
            node_id: self.node_id.clone(),
            state: state.to_string(),
            observed_generation: observed_generation.to_string(),
            health_status: health_status.to_string(),
            last_error: last_error.unwrap_or_default(),
            reported_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_report_has_node_id() {
        let rep = TelemetryReporter::new("node-1");
        let report = rep.node_state_report("TenantReady", "10", "Healthy", None);
        assert_eq!(report.node_id, "node-1");
        assert_eq!(report.state, "TenantReady");
    }
}
