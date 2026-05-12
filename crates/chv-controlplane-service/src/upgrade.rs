//! Rolling upgrade orchestration framework.
//!
//! Provides a structured approach to upgrading cluster nodes with rollback
//! capabilities. The actual binary-swap mechanism is platform-dependent and
//! abstracted behind the `NodeUpgrader` trait.

use chv_errors::ChvError;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Strategy for performing a cluster upgrade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeStrategy {
    /// Upgrade nodes one at a time, waiting for health before proceeding.
    Rolling,
    /// Deploy new version alongside old, switch traffic atomically.
    BlueGreen,
    /// Upgrade a subset of nodes first, validate, then proceed.
    Canary,
}

/// Pre-checks to run before upgrading a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreCheck {
    /// Verify the target version is compatible with existing components.
    VersionCompatible,
    /// Ensure the node has sufficient disk space for the upgrade.
    DiskSpace,
    /// Confirm no active VM migrations are in progress on the node.
    NoActiveMigrations,
    /// Verify the node is currently healthy and responsive.
    HealthCheck,
}

/// Current state of an upgrade operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeState {
    /// Upgrade plan is being created.
    Planning,
    /// Running pre-checks before starting the upgrade.
    PreChecking,
    /// Actively upgrading nodes.
    Upgrading {
        completed: Vec<String>,
        in_progress: Option<String>,
        remaining: Vec<String>,
    },
    /// Rolling back due to a failure.
    RollingBack { reason: String },
    /// Upgrade completed successfully on all nodes.
    Completed,
    /// Upgrade failed and could not be completed.
    Failed { reason: String },
}

/// A plan describing how to perform an upgrade.
#[derive(Debug, Clone)]
pub struct UpgradePlan {
    /// Target version to upgrade to.
    pub target_version: String,
    /// Strategy to use for the upgrade.
    pub strategy: UpgradeStrategy,
    /// Nodes ordered by upgrade sequence.
    pub nodes: Vec<String>,
    /// Maximum number of nodes that can be unavailable simultaneously.
    pub max_unavailable: usize,
    /// Pre-checks to run before upgrading each node.
    pub pre_checks: Vec<PreCheck>,
    /// Timeout for draining a node before upgrade.
    pub drain_timeout: Duration,
    /// Timeout for waiting for a node health check after upgrade.
    pub health_check_timeout: Duration,
}

impl Default for UpgradePlan {
    fn default() -> Self {
        Self {
            target_version: String::new(),
            strategy: UpgradeStrategy::Rolling,
            nodes: Vec::new(),
            max_unavailable: 1,
            pre_checks: vec![
                PreCheck::VersionCompatible,
                PreCheck::DiskSpace,
                PreCheck::NoActiveMigrations,
                PreCheck::HealthCheck,
            ],
            drain_timeout: Duration::from_secs(300),
            health_check_timeout: Duration::from_secs(120),
        }
    }
}

/// Trait abstracting the platform-specific node upgrade operations.
///
/// Implementations handle the actual binary swap, service restart, and
/// health verification for a specific platform or deployment model.
#[async_trait::async_trait]
pub trait NodeUpgrader: Send + Sync {
    /// Drain a node: stop scheduling new VMs and live-migrate existing ones off.
    async fn drain_node(&self, node_id: &str) -> Result<(), ChvError>;

    /// Perform the actual upgrade on a node (swap binary, restart service).
    async fn upgrade_node(&self, node_id: &str, target_version: &str) -> Result<(), ChvError>;

    /// Check if a node is healthy after upgrade.
    async fn health_check(&self, node_id: &str) -> Result<bool, ChvError>;

    /// Rollback a node to its previous version.
    async fn rollback_node(&self, node_id: &str) -> Result<(), ChvError>;

    /// Un-drain a node: resume scheduling and mark it available.
    async fn undrain_node(&self, node_id: &str) -> Result<(), ChvError>;

    /// Run a specific pre-check on a node.
    async fn run_pre_check(
        &self,
        node_id: &str,
        check: &PreCheck,
        target_version: &str,
    ) -> Result<bool, ChvError>;
}

/// Orchestrates rolling upgrades across cluster nodes.
pub struct UpgradeOrchestrator {
    upgrader: Box<dyn NodeUpgrader>,
    state: UpgradeState,
}

impl UpgradeOrchestrator {
    /// Create a new orchestrator with the given platform-specific upgrader.
    pub fn new(upgrader: Box<dyn NodeUpgrader>) -> Self {
        Self {
            upgrader,
            state: UpgradeState::Planning,
        }
    }

    /// Create a production orchestrator backed by the `SystemdNodeUpgrader`.
    ///
    /// This is the recommended constructor for production use. Tests should
    /// continue using `new()` with a `DummyUpgrader` or other mock.
    pub fn new_production(
        pool: chv_controlplane_store::StorePool,
        node_client_pool: crate::node_client_pool::NodeClientPool,
    ) -> Self {
        Self::with_systemd_upgrader(pool, node_client_pool)
    }

    /// Get the current state of the upgrade operation.
    pub fn state(&self) -> &UpgradeState {
        &self.state
    }

    /// Create an upgrade plan for the given target version and strategy.
    pub fn plan(
        target_version: String,
        strategy: UpgradeStrategy,
        nodes: Vec<String>,
        max_unavailable: usize,
    ) -> UpgradePlan {
        info!(
            target_version = %target_version,
            strategy = ?strategy,
            node_count = nodes.len(),
            max_unavailable = max_unavailable,
            "creating upgrade plan"
        );

        UpgradePlan {
            target_version,
            strategy,
            nodes,
            max_unavailable: max_unavailable.max(1),
            ..Default::default()
        }
    }

    /// Execute an upgrade plan, upgrading nodes one by one.
    ///
    /// For each node:
    /// 1. Run pre-checks
    /// 2. Drain node (stop scheduling, migrate VMs off)
    /// 3. Upgrade the node binary
    /// 4. Wait for health check
    /// 5. If health check fails, rollback that node
    /// 6. Un-drain and move to next node
    pub async fn execute(&mut self, plan: &UpgradePlan) -> Result<(), ChvError> {
        info!(
            target_version = %plan.target_version,
            strategy = ?plan.strategy,
            node_count = plan.nodes.len(),
            "starting upgrade execution"
        );

        // Pre-check phase
        self.state = UpgradeState::PreChecking;
        for node_id in &plan.nodes {
            for check in &plan.pre_checks {
                debug!(node_id = %node_id, check = ?check, "running pre-check");
                let passed = self
                    .upgrader
                    .run_pre_check(node_id, check, &plan.target_version)
                    .await?;

                if !passed {
                    let reason = format!("pre-check {:?} failed on node {}", check, node_id);
                    warn!(node_id = %node_id, check = ?check, "pre-check failed");
                    self.state = UpgradeState::Failed {
                        reason: reason.clone(),
                    };
                    return Err(ChvError::BadRequest { reason });
                }
            }
        }

        // Upgrade phase
        let mut completed: Vec<String> = Vec::new();
        let remaining: Vec<String> = plan.nodes.clone();

        for (idx, node_id) in plan.nodes.iter().enumerate() {
            let still_remaining: Vec<String> = remaining[idx + 1..].to_vec();
            self.state = UpgradeState::Upgrading {
                completed: completed.clone(),
                in_progress: Some(node_id.clone()),
                remaining: still_remaining,
            };

            info!(node_id = %node_id, "draining node for upgrade");

            // Step 1: Drain node
            if let Err(e) =
                tokio::time::timeout(plan.drain_timeout, self.upgrader.drain_node(node_id)).await
            {
                let reason = format!("drain timed out or failed for node {}: {:?}", node_id, e);
                error!(node_id = %node_id, error = ?e, "drain failed");
                self.state = UpgradeState::Failed {
                    reason: reason.clone(),
                };
                return Err(ChvError::Internal { reason });
            }

            // Step 2: Upgrade the node
            info!(node_id = %node_id, target_version = %plan.target_version, "upgrading node");
            if let Err(e) = self
                .upgrader
                .upgrade_node(node_id, &plan.target_version)
                .await
            {
                warn!(node_id = %node_id, error = %e, "upgrade failed, rolling back node");
                if let Err(rb_err) = self.upgrader.rollback_node(node_id).await {
                    error!(node_id = %node_id, error = %rb_err, "rollback also failed");
                }
                let _ = self.upgrader.undrain_node(node_id).await;
                let reason = format!("upgrade failed on node {}: {}", node_id, e);
                self.state = UpgradeState::Failed {
                    reason: reason.clone(),
                };
                return Err(ChvError::Internal { reason });
            }

            // Step 3: Health check
            info!(node_id = %node_id, "waiting for health check after upgrade");
            let health_ok = match tokio::time::timeout(
                plan.health_check_timeout,
                self.wait_for_health(node_id),
            )
            .await
            {
                Ok(Ok(healthy)) => healthy,
                Ok(Err(e)) => {
                    warn!(node_id = %node_id, error = %e, "health check error");
                    false
                }
                Err(_) => {
                    warn!(node_id = %node_id, "health check timed out");
                    false
                }
            };

            if !health_ok {
                warn!(node_id = %node_id, "health check failed, rolling back node");
                if let Err(rb_err) = self.upgrader.rollback_node(node_id).await {
                    error!(node_id = %node_id, error = %rb_err, "rollback failed after health check failure");
                }
                let _ = self.upgrader.undrain_node(node_id).await;
                let reason = format!(
                    "health check failed on node {} after upgrade, rolled back",
                    node_id
                );
                self.state = UpgradeState::Failed {
                    reason: reason.clone(),
                };
                return Err(ChvError::Internal { reason });
            }

            // Step 4: Un-drain the node
            if let Err(e) = self.upgrader.undrain_node(node_id).await {
                warn!(node_id = %node_id, error = %e, "failed to undrain node after upgrade");
                // Non-fatal: node is upgraded and healthy, just not accepting new workloads yet
            }

            completed.push(node_id.clone());
            info!(
                node_id = %node_id,
                completed_count = completed.len(),
                total = plan.nodes.len(),
                "node upgrade completed"
            );
        }

        self.state = UpgradeState::Completed;
        info!(
            target_version = %plan.target_version,
            node_count = plan.nodes.len(),
            "upgrade completed successfully on all nodes"
        );
        Ok(())
    }

    /// Rollback all upgraded nodes to their previous version.
    pub async fn rollback(&mut self, plan: &UpgradePlan, reason: &str) -> Result<(), ChvError> {
        info!(reason = %reason, "initiating full rollback");

        // Capture nodes to rollback BEFORE overwriting state
        let nodes_to_rollback = match &self.state {
            UpgradeState::Upgrading { completed, .. } => completed.clone(),
            _ => plan.nodes.clone(),
        };

        self.state = UpgradeState::RollingBack {
            reason: reason.to_string(),
        };

        let mut failures = Vec::new();
        for node_id in &nodes_to_rollback {
            info!(node_id = %node_id, "rolling back node");
            if let Err(e) = self.upgrader.rollback_node(node_id).await {
                error!(node_id = %node_id, error = %e, "rollback failed for node");
                failures.push(format!("{}: {}", node_id, e));
            } else {
                let _ = self.upgrader.undrain_node(node_id).await;
            }
        }

        if failures.is_empty() {
            info!("rollback completed successfully");
            self.state = UpgradeState::Failed {
                reason: format!("rolled back: {}", reason),
            };
            Ok(())
        } else {
            let failure_reason = format!("rollback partially failed: {}", failures.join(", "));
            error!(reason = %failure_reason, "partial rollback failure");
            self.state = UpgradeState::Failed {
                reason: failure_reason.clone(),
            };
            Err(ChvError::Internal {
                reason: failure_reason,
            })
        }
    }

    /// Poll health check with retries until success or the caller's timeout fires.
    async fn wait_for_health(&self, node_id: &str) -> Result<bool, ChvError> {
        // Retry health checks every 5 seconds
        loop {
            match self.upgrader.health_check(node_id).await {
                Ok(true) => return Ok(true),
                Ok(false) => {
                    debug!(node_id = %node_id, "health check not yet passing, retrying");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                Err(e) => {
                    debug!(node_id = %node_id, error = %e, "health check error, retrying");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_plan_creation() {
        let plan = UpgradeOrchestrator::plan(
            "1.2.0".to_string(),
            UpgradeStrategy::Rolling,
            vec!["node-1".to_string(), "node-2".to_string()],
            1,
        );

        assert_eq!(plan.target_version, "1.2.0");
        assert_eq!(plan.strategy, UpgradeStrategy::Rolling);
        assert_eq!(plan.nodes.len(), 2);
        assert_eq!(plan.max_unavailable, 1);
        assert_eq!(plan.pre_checks.len(), 4);
    }

    #[test]
    fn test_plan_max_unavailable_minimum() {
        let plan = UpgradeOrchestrator::plan(
            "1.0.0".to_string(),
            UpgradeStrategy::Rolling,
            vec!["node-1".to_string()],
            0, // Should be clamped to 1
        );
        assert_eq!(plan.max_unavailable, 1);
    }

    #[test]
    fn test_upgrade_state_initial() {
        struct DummyUpgrader;

        #[async_trait::async_trait]
        impl NodeUpgrader for DummyUpgrader {
            async fn drain_node(&self, _: &str) -> Result<(), ChvError> {
                Ok(())
            }
            async fn upgrade_node(&self, _: &str, _: &str) -> Result<(), ChvError> {
                Ok(())
            }
            async fn health_check(&self, _: &str) -> Result<bool, ChvError> {
                Ok(true)
            }
            async fn rollback_node(&self, _: &str) -> Result<(), ChvError> {
                Ok(())
            }
            async fn undrain_node(&self, _: &str) -> Result<(), ChvError> {
                Ok(())
            }
            async fn run_pre_check(
                &self,
                _: &str,
                _: &PreCheck,
                _: &str,
            ) -> Result<bool, ChvError> {
                Ok(true)
            }
        }

        let orchestrator = UpgradeOrchestrator::new(Box::new(DummyUpgrader));
        assert_eq!(*orchestrator.state(), UpgradeState::Planning);
    }

    /// Tracks calls made to each method and allows conditional failure behavior.
    #[derive(Debug, Clone, Default)]
    struct MockState {
        drained: Vec<String>,
        upgraded: Vec<String>,
        health_checked: Vec<String>,
        rolled_back: Vec<String>,
        undrained: Vec<String>,
        pre_checked: Vec<(String, String)>, // (node_id, check_debug)
    }

    /// A configurable mock that tracks calls and can fail on specific nodes.
    struct ConfigurableMockUpgrader {
        state: Arc<Mutex<MockState>>,
        /// Node IDs that should fail on `upgrade_node`.
        fail_upgrade_on: Vec<String>,
        /// Node IDs that should return `false` from `health_check`.
        fail_health_on: Vec<String>,
        /// Pre-check types that should fail (as debug strings).
        fail_pre_check: Vec<(String, String)>, // (node_id, check_debug)
    }

    impl ConfigurableMockUpgrader {
        fn new(state: Arc<Mutex<MockState>>) -> Self {
            Self {
                state,
                fail_upgrade_on: Vec::new(),
                fail_health_on: Vec::new(),
                fail_pre_check: Vec::new(),
            }
        }

        fn fail_upgrade_on(mut self, node_ids: Vec<String>) -> Self {
            self.fail_upgrade_on = node_ids;
            self
        }

        fn fail_health_on(mut self, node_ids: Vec<String>) -> Self {
            self.fail_health_on = node_ids;
            self
        }

        fn fail_pre_check_on(mut self, entries: Vec<(String, String)>) -> Self {
            self.fail_pre_check = entries;
            self
        }
    }

    #[async_trait::async_trait]
    impl NodeUpgrader for ConfigurableMockUpgrader {
        async fn drain_node(&self, node_id: &str) -> Result<(), ChvError> {
            self.state.lock().unwrap().drained.push(node_id.to_string());
            Ok(())
        }

        async fn upgrade_node(&self, node_id: &str, _target_version: &str) -> Result<(), ChvError> {
            self.state
                .lock()
                .unwrap()
                .upgraded
                .push(node_id.to_string());
            if self.fail_upgrade_on.contains(&node_id.to_string()) {
                return Err(ChvError::Internal {
                    reason: format!("simulated upgrade failure on {}", node_id),
                });
            }
            Ok(())
        }

        async fn health_check(&self, node_id: &str) -> Result<bool, ChvError> {
            self.state
                .lock()
                .unwrap()
                .health_checked
                .push(node_id.to_string());
            if self.fail_health_on.contains(&node_id.to_string()) {
                return Ok(false);
            }
            Ok(true)
        }

        async fn rollback_node(&self, node_id: &str) -> Result<(), ChvError> {
            self.state
                .lock()
                .unwrap()
                .rolled_back
                .push(node_id.to_string());
            Ok(())
        }

        async fn undrain_node(&self, node_id: &str) -> Result<(), ChvError> {
            self.state
                .lock()
                .unwrap()
                .undrained
                .push(node_id.to_string());
            Ok(())
        }

        async fn run_pre_check(
            &self,
            node_id: &str,
            check: &PreCheck,
            _target_version: &str,
        ) -> Result<bool, ChvError> {
            let check_debug = format!("{:?}", check);
            self.state
                .lock()
                .unwrap()
                .pre_checked
                .push((node_id.to_string(), check_debug.clone()));
            for (fail_node, fail_check) in &self.fail_pre_check {
                if fail_node == node_id && *fail_check == check_debug {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }

    #[tokio::test]
    async fn test_execute_success_with_dummy_upgrader() {
        let state = Arc::new(Mutex::new(MockState::default()));
        let upgrader = ConfigurableMockUpgrader::new(Arc::clone(&state));
        let mut orchestrator = UpgradeOrchestrator::new(Box::new(upgrader));

        let plan = UpgradeOrchestrator::plan(
            "2.0.0".to_string(),
            UpgradeStrategy::Rolling,
            vec!["node-1".to_string(), "node-2".to_string()],
            1,
        );

        let result = orchestrator.execute(&plan).await;
        assert!(result.is_ok(), "execute should succeed: {:?}", result.err());
        assert_eq!(*orchestrator.state(), UpgradeState::Completed);

        let mock_state = state.lock().unwrap();
        // Each node should have been drained, upgraded, health-checked, and undrained
        assert_eq!(mock_state.drained, vec!["node-1", "node-2"]);
        assert_eq!(mock_state.upgraded, vec!["node-1", "node-2"]);
        assert!(mock_state.health_checked.contains(&"node-1".to_string()));
        assert!(mock_state.health_checked.contains(&"node-2".to_string()));
        assert_eq!(mock_state.undrained, vec!["node-1", "node-2"]);
        // No rollbacks should have occurred
        assert!(mock_state.rolled_back.is_empty());
    }

    #[tokio::test]
    async fn test_upgrade_failure_triggers_rollback() {
        let state = Arc::new(Mutex::new(MockState::default()));
        let upgrader = ConfigurableMockUpgrader::new(Arc::clone(&state))
            .fail_upgrade_on(vec!["node-2".to_string()]);
        let mut orchestrator = UpgradeOrchestrator::new(Box::new(upgrader));

        let plan = UpgradeOrchestrator::plan(
            "2.0.0".to_string(),
            UpgradeStrategy::Rolling,
            vec![
                "node-1".to_string(),
                "node-2".to_string(),
                "node-3".to_string(),
            ],
            1,
        );

        let result = orchestrator.execute(&plan).await;
        assert!(result.is_err(), "execute should fail due to node-2 failure");

        // State should be Failed
        match orchestrator.state() {
            UpgradeState::Failed { reason } => {
                assert!(
                    reason.contains("node-2"),
                    "failure reason should mention node-2: {}",
                    reason
                );
            }
            other => panic!("expected Failed state, got {:?}", other),
        }

        let mock_state = state.lock().unwrap();
        // node-1 should have been fully upgraded
        assert!(mock_state.upgraded.contains(&"node-1".to_string()));
        // node-2 upgrade was attempted but failed
        assert!(mock_state.upgraded.contains(&"node-2".to_string()));
        // node-2 should have been rolled back
        assert!(mock_state.rolled_back.contains(&"node-2".to_string()));
        // node-3 should never have been touched
        assert!(!mock_state.drained.contains(&"node-3".to_string()));
    }

    #[tokio::test]
    async fn test_health_check_failure_triggers_rollback() {
        let state = Arc::new(Mutex::new(MockState::default()));
        let upgrader = ConfigurableMockUpgrader::new(Arc::clone(&state))
            .fail_health_on(vec!["node-1".to_string()]);
        let mut orchestrator = UpgradeOrchestrator::new(Box::new(upgrader));

        let plan = UpgradePlan {
            target_version: "2.0.0".to_string(),
            strategy: UpgradeStrategy::Rolling,
            nodes: vec!["node-1".to_string(), "node-2".to_string()],
            max_unavailable: 1,
            pre_checks: vec![],
            drain_timeout: Duration::from_secs(5),
            // Short health check timeout to avoid waiting in tests
            health_check_timeout: Duration::from_millis(100),
        };

        let result = orchestrator.execute(&plan).await;
        assert!(
            result.is_err(),
            "execute should fail due to health check failure"
        );

        match orchestrator.state() {
            UpgradeState::Failed { reason } => {
                assert!(
                    reason.contains("health check failed"),
                    "reason should mention health check: {}",
                    reason
                );
                assert!(
                    reason.contains("node-1"),
                    "reason should mention node-1: {}",
                    reason
                );
            }
            other => panic!("expected Failed state, got {:?}", other),
        }

        let mock_state = state.lock().unwrap();
        // node-1 was drained and upgraded, but health failed
        assert!(mock_state.drained.contains(&"node-1".to_string()));
        assert!(mock_state.upgraded.contains(&"node-1".to_string()));
        // node-1 should have been rolled back after health failure
        assert!(mock_state.rolled_back.contains(&"node-1".to_string()));
        // node-1 should have been undrained after rollback
        assert!(mock_state.undrained.contains(&"node-1".to_string()));
        // node-2 should never have been started
        assert!(!mock_state.drained.contains(&"node-2".to_string()));
    }

    #[tokio::test]
    async fn test_pre_check_failure_prevents_upgrade() {
        let state = Arc::new(Mutex::new(MockState::default()));
        let upgrader = ConfigurableMockUpgrader::new(Arc::clone(&state))
            .fail_pre_check_on(vec![("node-1".to_string(), "DiskSpace".to_string())]);
        let mut orchestrator = UpgradeOrchestrator::new(Box::new(upgrader));

        let plan = UpgradePlan {
            target_version: "2.0.0".to_string(),
            strategy: UpgradeStrategy::Rolling,
            nodes: vec!["node-1".to_string(), "node-2".to_string()],
            max_unavailable: 1,
            pre_checks: vec![PreCheck::VersionCompatible, PreCheck::DiskSpace],
            drain_timeout: Duration::from_secs(5),
            health_check_timeout: Duration::from_secs(5),
        };

        let result = orchestrator.execute(&plan).await;
        assert!(result.is_err(), "execute should fail due to pre-check");

        match orchestrator.state() {
            UpgradeState::Failed { reason } => {
                assert!(
                    reason.contains("pre-check"),
                    "reason should mention pre-check: {}",
                    reason
                );
                assert!(
                    reason.contains("node-1"),
                    "reason should mention node-1: {}",
                    reason
                );
            }
            other => panic!("expected Failed state, got {:?}", other),
        }

        let mock_state = state.lock().unwrap();
        // No nodes should have been drained or upgraded since pre-checks failed
        assert!(
            mock_state.drained.is_empty(),
            "no draining should have happened"
        );
        assert!(
            mock_state.upgraded.is_empty(),
            "no upgrading should have happened"
        );
        assert!(
            mock_state.rolled_back.is_empty(),
            "no rollback should have happened"
        );
    }

    #[tokio::test]
    async fn test_rollback_method() {
        let state = Arc::new(Mutex::new(MockState::default()));
        let upgrader = ConfigurableMockUpgrader::new(Arc::clone(&state));
        let mut orchestrator = UpgradeOrchestrator::new(Box::new(upgrader));

        // Simulate that we're in an upgrading state with some completed nodes
        orchestrator.state = UpgradeState::Upgrading {
            completed: vec!["node-1".to_string(), "node-2".to_string()],
            in_progress: Some("node-3".to_string()),
            remaining: vec!["node-4".to_string()],
        };

        let plan = UpgradeOrchestrator::plan(
            "2.0.0".to_string(),
            UpgradeStrategy::Rolling,
            vec![
                "node-1".to_string(),
                "node-2".to_string(),
                "node-3".to_string(),
                "node-4".to_string(),
            ],
            1,
        );

        let result = orchestrator.rollback(&plan, "test failure").await;
        assert!(
            result.is_ok(),
            "rollback should succeed: {:?}",
            result.err()
        );

        // State should be Failed with rolled-back reason
        match orchestrator.state() {
            UpgradeState::Failed { reason } => {
                assert!(
                    reason.contains("rolled back"),
                    "reason should mention rolled back: {}",
                    reason
                );
                assert!(
                    reason.contains("test failure"),
                    "reason should contain original reason: {}",
                    reason
                );
            }
            other => panic!("expected Failed state, got {:?}", other),
        }

        let mock_state = state.lock().unwrap();
        // Only completed nodes should have been rolled back (node-1, node-2)
        assert_eq!(mock_state.rolled_back.len(), 2);
        assert!(mock_state.rolled_back.contains(&"node-1".to_string()));
        assert!(mock_state.rolled_back.contains(&"node-2".to_string()));
        // Rolled-back nodes should also have been undrained
        assert!(mock_state.undrained.contains(&"node-1".to_string()));
        assert!(mock_state.undrained.contains(&"node-2".to_string()));
    }
}
