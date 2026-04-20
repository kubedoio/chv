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

    pub fn vm_state_report(
        &self,
        vm_id: &str,
        runtime_status: &str,
        observed_generation: &str,
        health_status: &str,
    ) -> proto::VmStateReport {
        proto::VmStateReport {
            node_id: self.node_id.clone(),
            vm_id: vm_id.to_string(),
            runtime_status: runtime_status.to_string(),
            observed_generation: observed_generation.to_string(),
            health_status: health_status.to_string(),
            last_error: "".to_string(),
            reported_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        }
    }

    pub fn volume_state_report(
        &self,
        volume_id: &str,
        runtime_status: &str,
        observed_generation: &str,
    ) -> proto::VolumeStateReport {
        proto::VolumeStateReport {
            node_id: self.node_id.clone(),
            volume_id: volume_id.to_string(),
            runtime_status: runtime_status.to_string(),
            observed_generation: observed_generation.to_string(),
            health_status: "Healthy".to_string(),
            last_error: "".to_string(),
            reported_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        }
    }

    pub fn network_state_report(
        &self,
        network_id: &str,
        runtime_status: &str,
        observed_generation: &str,
    ) -> proto::NetworkStateReport {
        proto::NetworkStateReport {
            node_id: self.node_id.clone(),
            network_id: network_id.to_string(),
            runtime_status: runtime_status.to_string(),
            observed_generation: observed_generation.to_string(),
            health_status: "Healthy".to_string(),
            last_error: "".to_string(),
            reported_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        }
    }

    pub fn event_report(
        &self,
        meta: proto::RequestMeta,
        severity: &str,
        event_type: &str,
        summary: &str,
    ) -> proto::PublishEventRequest {
        proto::PublishEventRequest {
            meta: Some(meta),
            node_id: self.node_id.clone(),
            severity: severity.to_string(),
            event_type: event_type.to_string(),
            summary: summary.to_string(),
            details_json: vec![],
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

    #[test]
    fn volume_report_has_volume_id() {
        let rep = TelemetryReporter::new("node-1");
        let report = rep.volume_state_report("vol-1", "Attached", "5");
        assert_eq!(report.node_id, "node-1");
        assert_eq!(report.volume_id, "vol-1");
        assert_eq!(report.runtime_status, "Attached");
    }

    #[test]
    fn network_report_has_network_id() {
        let rep = TelemetryReporter::new("node-1");
        let report = rep.network_state_report("net-1", "Ready", "3");
        assert_eq!(report.node_id, "node-1");
        assert_eq!(report.network_id, "net-1");
        assert_eq!(report.runtime_status, "Ready");
    }

    #[test]
    fn event_report_has_event_type() {
        let rep = TelemetryReporter::new("node-1");
        let meta = proto::RequestMeta {
            operation_id: "op-1".to_string(),
            requested_by: "agent".to_string(),
            target_node_id: "node-1".to_string(),
            desired_state_version: "v1".to_string(),
            request_unix_ms: 0,
        };
        let event = rep.event_report(meta, "warning", "NodeStateTransition", "test");
        assert_eq!(event.node_id, "node-1");
        assert_eq!(event.event_type, "NodeStateTransition");
        assert_eq!(event.severity, "warning");
        assert_eq!(event.summary, "test");
    }

    #[test]
    fn event_report_has_severity() {
        let rep = TelemetryReporter::new("node-1");
        let meta = proto::RequestMeta {
            operation_id: "op-2".to_string(),
            requested_by: "agent".to_string(),
            target_node_id: "node-1".to_string(),
            desired_state_version: "v2".to_string(),
            request_unix_ms: 0,
        };
        let event = rep.event_report(meta, "critical", "HealthAlert", "disk full");
        assert_eq!(event.severity, "critical");
        assert_eq!(event.summary, "disk full");
    }
}
