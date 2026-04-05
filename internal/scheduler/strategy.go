// Package scheduler provides VM scheduling strategies.
package scheduler

import (
	"math"
	"sort"

	"github.com/chv/chv/internal/models"
)

// Strategy defines a scheduling strategy.
type Strategy interface {
	SelectNode(nodes []*models.Node, spec *models.VMSpec) *models.Node
	Name() string
}

// FirstFitStrategy selects the first node with sufficient resources.
type FirstFitStrategy struct{}

// Name returns the strategy name.
func (s *FirstFitStrategy) Name() string {
	return "first-fit"
}

// SelectNode selects the first suitable node.
func (s *FirstFitStrategy) SelectNode(nodes []*models.Node, spec *models.VMSpec) *models.Node {
	for _, node := range nodes {
		if canSchedule(node, spec) {
			return node
		}
	}
	return nil
}

// BestFitStrategy selects the node with the least remaining resources after placement.
type BestFitStrategy struct{}

// Name returns the strategy name.
func (s *BestFitStrategy) Name() string {
	return "best-fit"
}

// SelectNode selects the node that will have the least remaining resources.
func (s *BestFitStrategy) SelectNode(nodes []*models.Node, spec *models.VMSpec) *models.Node {
	var bestNode *models.Node
	bestScore := math.MaxFloat64

	for _, node := range nodes {
		if !canSchedule(node, spec) {
			continue
		}

		// Calculate remaining resources after placement
		remainingCPU := float64(node.AllocatableCPUCores - spec.CPU)
		remainingRAM := float64(node.AllocatableRAMMB - spec.MemoryMB)

		// Score is the sum of remaining resources (lower is better - tight fit)
		score := remainingCPU + remainingRAM/1000 // Normalize RAM to CPU scale

		if score < bestScore {
			bestScore = score
			bestNode = node
		}
	}

	return bestNode
}

// LeastLoadedStrategy selects the node with the most available resources.
type LeastLoadedStrategy struct{}

// Name returns the strategy name.
func (s *LeastLoadedStrategy) Name() string {
	return "least-loaded"
}

// SelectNode selects the node with the most available resources.
func (s *LeastLoadedStrategy) SelectNode(nodes []*models.Node, spec *models.VMSpec) *models.Node {
	var bestNode *models.Node
	bestScore := -1.0

	for _, node := range nodes {
		if !canSchedule(node, spec) {
			continue
		}

		// Calculate available resources
		availableCPU := float64(node.AllocatableCPUCores)
		availableRAM := float64(node.AllocatableRAMMB)

		// Score is the sum of available resources (higher is better - spread load)
		score := availableCPU + availableRAM/1000

		if score > bestScore {
			bestScore = score
			bestNode = node
		}
	}

	return bestNode
}

// RoundRobinStrategy cycles through nodes in order.
type RoundRobinStrategy struct {
	lastIndex int
}

// Name returns the strategy name.
func (s *RoundRobinStrategy) Name() string {
	return "round-robin"
}

// SelectNode selects the next suitable node in rotation.
func (s *RoundRobinStrategy) SelectNode(nodes []*models.Node, spec *models.VMSpec) *models.Node {
	if len(nodes) == 0 {
		return nil
	}

	// Sort nodes by ID for consistent ordering
	sorted := make([]*models.Node, len(nodes))
	copy(sorted, nodes)
	sort.Slice(sorted, func(i, j int) bool {
		return sorted[i].ID.String() < sorted[j].ID.String()
	})

	// Find starting point
	start := s.lastIndex % len(sorted)

	// Look for suitable node starting from last index
	for i := 0; i < len(sorted); i++ {
		idx := (start + i) % len(sorted)
		if canSchedule(sorted[idx], spec) {
			s.lastIndex = idx + 1
			return sorted[idx]
		}
	}

	return nil
}

// canSchedule checks if a node can host a VM.
func canSchedule(node *models.Node, spec *models.VMSpec) bool {
	if node.Status != models.NodeStateOnline {
		return false
	}
	if node.MaintenanceMode {
		return false
	}
	if node.AllocatableCPUCores < spec.CPU {
		return false
	}
	if node.AllocatableRAMMB < spec.MemoryMB {
		return false
	}
	return true
}
