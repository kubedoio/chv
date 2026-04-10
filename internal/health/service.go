package health

import (
	"context"
	"time"

	"github.com/chv/chv/internal/db"
)

// MetricsRecorder interface for recording node metrics
type MetricsRecorder interface {
	GetCPUPercent() float64
	GetMemoryUsedMB() int
	GetMemoryTotalMB() int
	GetDiskUsedGB() int
	GetDiskTotalGB() int
	GetTimestamp() string
}

// Service provides high-level health monitoring functionality
type Service struct {
	heartbeat *HeartbeatService
	repo      *db.Repository
}

// NewService creates a new health service
func NewService(repo *db.Repository) *Service {
	return &Service{
		repo: repo,
	}
}

// StartHeartbeatService starts the heartbeat monitoring service
func (s *Service) StartHeartbeatService(interval time.Duration) *HeartbeatService {
	s.heartbeat = NewHeartbeatService(s.repo, interval)
	go s.heartbeat.Start()
	return s.heartbeat
}

// Stop stops all health monitoring services
func (s *Service) Stop() {
	if s.heartbeat != nil {
		s.heartbeat.Stop()
	}
}

// GetNodeHealth returns the health status of a specific node
func (s *Service) GetNodeHealth(ctx context.Context, nodeID string) (*NodeHealth, error) {
	node, err := s.repo.GetNode(ctx, nodeID)
	if err != nil {
		return nil, err
	}

	if node == nil {
		return nil, nil
	}

	// Get latest metrics
	metrics, err := s.repo.GetLatestNodeMetrics(ctx, nodeID)
	if err != nil {
		return nil, err
	}

	health := &NodeHealth{
		NodeID:     node.ID,
		NodeName:   node.Name,
		Status:     node.Status,
		LastSeenAt: node.LastSeenAt,
	}

	if metrics != nil {
		health.Metrics = &NodeMetrics{
			CPUPercent:    metrics.CPUPercent,
			MemoryUsedMB:  metrics.MemoryUsedMB,
			MemoryTotalMB: metrics.MemoryTotalMB,
			DiskUsedGB:    metrics.DiskUsedGB,
			DiskTotalGB:   metrics.DiskTotalGB,
			Timestamp:     metrics.Timestamp,
		}
	}

	return health, nil
}

// GetAllNodesHealth returns health status for all nodes
func (s *Service) GetAllNodesHealth(ctx context.Context) ([]*NodeHealth, error) {
	nodes, err := s.repo.ListNodes(ctx)
	if err != nil {
		return nil, err
	}

	var healths []*NodeHealth
	for _, node := range nodes {
		health, err := s.GetNodeHealth(ctx, node.ID)
		if err != nil {
			continue
		}
		if health != nil {
			healths = append(healths, health)
		}
	}

	return healths, nil
}

// NodeHealth represents the health status of a node
type NodeHealth struct {
	NodeID     string       `json:"node_id"`
	NodeName   string       `json:"node_name"`
	Status     string       `json:"status"`
	LastSeenAt string       `json:"last_seen_at,omitempty"`
	Metrics    *NodeMetrics `json:"metrics,omitempty"`
}


