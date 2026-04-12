package health

import (
	"context"
	"net/http"
	"time"

	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/logger"
	"github.com/chv/chv/internal/models"
)

// HeartbeatService manages node heartbeats and health monitoring
type HeartbeatService struct {
	interval time.Duration
	timeout  time.Duration
	repo     *db.Repository
	stopChan chan struct{}
}

// NewHeartbeatService creates a new heartbeat service
func NewHeartbeatService(repo *db.Repository, interval time.Duration) *HeartbeatService {
	if interval == 0 {
		interval = 30 * time.Second
	}

	return &HeartbeatService{
		interval: interval,
		timeout:  2 * time.Minute, // Nodes marked offline after 2 minutes of no heartbeat
		repo:     repo,
		stopChan: make(chan struct{}),
	}
}

// Start begins the background monitoring goroutine
func (s *HeartbeatService) Start() {
	log := logger.L()
	log.Info("Starting heartbeat service", logger.F("interval", s.interval), logger.F("timeout", s.timeout))

	// Run immediately on start
	s.checkNodes()

	ticker := time.NewTicker(s.interval)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			s.checkNodes()
		case <-s.stopChan:
			log.Info("Heartbeat service stopped")
			return
		}
	}
}

// Stop stops the heartbeat service
func (s *HeartbeatService) Stop() {
	close(s.stopChan)
}

// RecordHeartbeat records a heartbeat from a node and updates its status to online
func (s *HeartbeatService) RecordHeartbeat(ctx context.Context, nodeID string, metrics NodeMetrics) error {
	log := logger.L()

	// Update node's last seen timestamp
	if err := s.repo.UpdateNodeLastSeen(ctx, nodeID); err != nil {
		log.Error("Failed to update node last seen", logger.ErrorField(err), logger.F("node_id", nodeID))
		return err
	}

	// Update node status to online if it was offline
	node, err := s.repo.GetNode(ctx, nodeID)
	if err != nil {
		log.Error("Failed to get node for heartbeat", logger.ErrorField(err), logger.F("node_id", nodeID))
		return err
	}

	if node == nil {
		return nil // Node doesn't exist, ignore
	}

	if node.Status != models.NodeStatusOnline {
		if err := s.repo.UpdateNodeStatus(ctx, nodeID, models.NodeStatusOnline); err != nil {
			log.Error("Failed to update node status to online", logger.ErrorField(err), logger.F("node_id", nodeID))
			return err
		}
		log.Info("Node is now online", logger.F("node_id", nodeID), logger.F("node_name", node.Name))
	}

	// Store metrics if provided
	if metrics.CPUPercent > 0 || metrics.MemoryUsedMB > 0 {
		if err := s.repo.RecordNodeMetrics(ctx, nodeID, metrics); err != nil {
			log.Error("Failed to record node metrics", logger.ErrorField(err), logger.F("node_id", nodeID))
			// Don't return error here, heartbeat is still successful
		}
	}

	return nil
}

// checkNodes checks all nodes and marks stale ones as offline.
// For nodes with an agent_url, it actively probes the agent's /health endpoint.
func (s *HeartbeatService) checkNodes() {
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	log := logger.L()

	nodes, err := s.repo.ListNodes(ctx)
	if err != nil {
		log.Error("Failed to list nodes for health check", logger.ErrorField(err))
		return
	}

	for _, node := range nodes {
		// Skip local node - it's always online if controller is running
		if node.IsLocal {
			continue
		}

		// Skip nodes in maintenance mode
		if node.Status == models.NodeStatusMaintenance {
			continue
		}

		// Active health check: if the node has an agent_url, probe it
		if node.AgentURL != "" {
			reachable := s.probeAgent(node.AgentURL)
			if reachable {
				// Agent is reachable — mark online and record last_seen
				if err := s.repo.UpdateNodeLastSeen(ctx, node.ID); err != nil {
					log.Error("Failed to update node last seen", logger.ErrorField(err), logger.F("node_id", node.ID))
				}
				if node.Status != models.NodeStatusOnline {
					if err := s.repo.UpdateNodeStatus(ctx, node.ID, models.NodeStatusOnline); err != nil {
						log.Error("Failed to mark node as online", logger.ErrorField(err), logger.F("node_id", node.ID))
					} else {
						log.Info("Node is now online (active probe)", logger.F("node_id", node.ID), logger.F("node_name", node.Name))
					}
				}
			} else {
				// Agent is unreachable — mark offline
				if node.Status == models.NodeStatusOnline {
					if err := s.repo.UpdateNodeStatus(ctx, node.ID, models.NodeStatusOffline); err != nil {
						log.Error("Failed to mark node as offline", logger.ErrorField(err), logger.F("node_id", node.ID))
					} else {
						log.Warn("Node marked offline - agent unreachable",
							logger.F("node_id", node.ID),
							logger.F("node_name", node.Name),
							logger.F("agent_url", node.AgentURL))
					}
				}
			}
			continue
		}

		// Passive heartbeat check for nodes without agent_url
		cutoff := time.Now().UTC().Add(-s.timeout)
		if node.LastSeenAt != "" {
			lastSeen, err := time.Parse(time.RFC3339, node.LastSeenAt)
			if err != nil {
				log.Error("Failed to parse last seen time", logger.ErrorField(err), logger.F("node_id", node.ID))
				continue
			}

			if lastSeen.Before(cutoff) && node.Status == models.NodeStatusOnline {
				if err := s.repo.UpdateNodeStatus(ctx, node.ID, models.NodeStatusOffline); err != nil {
					log.Error("Failed to mark node as offline", logger.ErrorField(err), logger.F("node_id", node.ID))
					continue
				}
				log.Warn("Node marked offline due to missed heartbeats",
					logger.F("node_id", node.ID),
					logger.F("node_name", node.Name),
					logger.F("last_seen", node.LastSeenAt))
			}
		} else if node.Status == models.NodeStatusOnline {
			if err := s.repo.UpdateNodeStatus(ctx, node.ID, models.NodeStatusOffline); err != nil {
				log.Error("Failed to mark node as offline", logger.ErrorField(err), logger.F("node_id", node.ID))
				continue
			}
			log.Warn("Node marked offline - no heartbeat received",
				logger.F("node_id", node.ID),
				logger.F("node_name", node.Name))
		}
	}
}

// probeAgent checks if a remote agent is reachable by hitting its /health endpoint.
func (s *HeartbeatService) probeAgent(agentURL string) bool {
	client := &http.Client{Timeout: 5 * time.Second}
	resp, err := client.Get(agentURL + "/health")
	if err != nil {
		return false
	}
	defer resp.Body.Close()
	return resp.StatusCode == http.StatusOK
}

// NodeMetrics represents resource metrics for a node
type NodeMetrics struct {
	CPUPercent      float64 `json:"cpu_percent"`
	MemoryUsedMB    int     `json:"memory_used_mb"`
	MemoryTotalMB   int     `json:"memory_total_mb"`
	DiskUsedGB      int     `json:"disk_used_gb"`
	DiskTotalGB     int     `json:"disk_total_gb"`
	Timestamp       string  `json:"timestamp"`
}

// GetCPUPercent returns CPU percent for interface compatibility
func (m NodeMetrics) GetCPUPercent() float64 { return m.CPUPercent }

// GetMemoryUsedMB returns memory used MB for interface compatibility
func (m NodeMetrics) GetMemoryUsedMB() int { return m.MemoryUsedMB }

// GetMemoryTotalMB returns memory total MB for interface compatibility
func (m NodeMetrics) GetMemoryTotalMB() int { return m.MemoryTotalMB }

// GetDiskUsedGB returns disk used GB for interface compatibility
func (m NodeMetrics) GetDiskUsedGB() int { return m.DiskUsedGB }

// GetDiskTotalGB returns disk total GB for interface compatibility
func (m NodeMetrics) GetDiskTotalGB() int { return m.DiskTotalGB }

// GetTimestamp returns timestamp for interface compatibility
func (m NodeMetrics) GetTimestamp() string { return m.Timestamp }
