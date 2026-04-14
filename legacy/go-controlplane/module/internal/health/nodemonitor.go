package health

import (
	"context"
	"time"

	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/logger"
	"github.com/chv/chv/internal/models"
)

const (
	// StaleNodeTimeout is how long since last heartbeat before marking a node offline
	StaleNodeTimeout = 2 * time.Minute

	// HealthCheckInterval is how often to check node health
	HealthCheckInterval = 30 * time.Second
)

// NodeMonitor manages node health monitoring and status updates
type NodeMonitor struct {
	repo     *db.Repository
	interval time.Duration
	timeout  time.Duration
	stopCh   chan struct{}
}

// NewNodeMonitor creates a new node monitor
func NewNodeMonitor(repo *db.Repository) *NodeMonitor {
	return &NodeMonitor{
		repo:     repo,
		interval: HealthCheckInterval,
		timeout:  StaleNodeTimeout,
		stopCh:   make(chan struct{}),
	}
}

// Start begins the node monitoring loop
func (m *NodeMonitor) Start(ctx context.Context) {
	logger.Info("Starting node health monitor",
		logger.StringField("interval", m.interval.String()),
		logger.StringField("timeout", m.timeout.String()))

	// Run initial check
	m.checkNodes(ctx)

	// Start ticker for periodic checks
	ticker := time.NewTicker(m.interval)
	go func() {
		defer ticker.Stop()
		for {
			select {
			case <-ticker.C:
				m.checkNodes(ctx)
			case <-m.stopCh:
				logger.Info("Node health monitor stopped")
				return
			case <-ctx.Done():
				logger.Info("Node health monitor context cancelled")
				return
			}
		}
	}()
}

// Stop stops the node monitoring loop
func (m *NodeMonitor) Stop() {
	close(m.stopCh)
}

// checkNodes checks all non-local nodes and updates their status
func (m *NodeMonitor) checkNodes(ctx context.Context) {
	// Get all online nodes (not local, not already offline)
	nodes, err := m.repo.ListNodes(ctx)
	if err != nil {
		logger.Error("Failed to list nodes for health check", logger.ErrorField(err))
		return
	}

	for _, node := range nodes {
		// Skip local nodes - they don't need heartbeat monitoring
		if node.IsLocal {
			continue
		}

		// Check if node has timed out
		if m.isNodeStale(node) {
			if node.Status == models.NodeStatusOnline {
				logger.Warn("Node marked as offline due to missed heartbeats",
					logger.StringField("node_id", node.ID),
					logger.StringField("node_name", node.Name),
					logger.StringField("last_seen", node.LastSeenAt))

				if err := m.repo.UpdateNodeStatus(ctx, node.ID, models.NodeStatusOffline); err != nil {
					logger.Error("Failed to update node status to offline",
						logger.StringField("node_id", node.ID),
						logger.ErrorField(err))
				}
			}
		}
	}
}

// isNodeStale checks if a node has exceeded the stale timeout
func (m *NodeMonitor) isNodeStale(node models.Node) bool {
	// If never seen, it's stale
	if node.LastSeenAt == "" {
		return true
	}

	lastSeen, err := time.Parse(time.RFC3339, node.LastSeenAt)
	if err != nil {
		// If we can't parse the timestamp, treat as stale
		return true
	}

	return time.Since(lastSeen) > m.timeout
}

// RecordHeartbeat updates the last seen timestamp for a node
func (m *NodeMonitor) RecordHeartbeat(ctx context.Context, nodeID string) error {
	return m.repo.UpdateNodeLastSeen(ctx, nodeID)
}

// MarkNodeOnline marks a node as online
func (m *NodeMonitor) MarkNodeOnline(ctx context.Context, nodeID string) error {
	return m.repo.UpdateNodeStatus(ctx, nodeID, models.NodeStatusOnline)
}

// MarkNodeOffline marks a node as offline
func (m *NodeMonitor) MarkNodeOffline(ctx context.Context, nodeID string) error {
	return m.repo.UpdateNodeStatus(ctx, nodeID, models.NodeStatusOffline)
}

// MarkNodeMaintenance marks a node as in maintenance mode
func (m *NodeMonitor) MarkNodeMaintenance(ctx context.Context, nodeID string) error {
	return m.repo.UpdateNodeStatus(ctx, nodeID, models.NodeStatusMaintenance)
}

// GetNodeStatus returns the current status of a node
func (m *NodeMonitor) GetNodeStatus(ctx context.Context, nodeID string) (string, error) {
	node, err := m.repo.GetNode(ctx, nodeID)
	if err != nil {
		return "", err
	}
	if node == nil {
		return "", nil
	}
	return node.Status, nil
}
