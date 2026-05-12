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
}
