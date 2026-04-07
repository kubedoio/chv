package scheduler

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	"github.com/chv/chv/internal/metrics"
	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/store"
	"github.com/chv/chv/pkg/errorsx"
	"github.com/google/uuid"
)

// Service handles VM scheduling.
type Service struct {
	store    store.Store
	strategy Strategy
}

// NewService creates a new scheduler service.
func NewService(store store.Store) *Service {
	return &Service{
		store:    store,
		strategy: &BestFitStrategy{}, // Default strategy
	}
}

// SetStrategy sets the scheduling strategy.
func (s *Service) SetStrategy(strategy Strategy) {
	s.strategy = strategy
}

// getStrategy returns the current strategy or a default.
func (s *Service) getStrategy() Strategy {
	if s.strategy == nil {
		return &BestFitStrategy{}
	}
	return s.strategy
}

// ScheduleVM attempts to schedule a VM onto a suitable node.
func (s *Service) ScheduleVM(ctx context.Context, vmID uuid.UUID) error {
	start := time.Now()
	vm, err := s.store.GetVM(ctx, vmID)
	if err != nil {
		metrics.SchedulerPlacementFailures.Inc()
		return fmt.Errorf("failed to get VM: %w", err)
	}
	if vm == nil {
		metrics.SchedulerPlacementFailures.Inc()
		return errorsx.New(errorsx.ErrNotFound, "VM not found")
	}
	
	spec, err := vm.GetSpec()
	if err != nil {
		return fmt.Errorf("failed to parse VM spec: %w", err)
	}
	
	// Find suitable nodes
	nodes, err := s.store.ListNodes(ctx)
	if err != nil {
		return fmt.Errorf("failed to list nodes: %w", err)
	}
	
	var candidates []*models.Node
	for _, node := range nodes {
		if canSchedule(node, spec) {
			candidates = append(candidates, node)
		}
	}
	
	if len(candidates) == 0 {
		vm.PlacementStatus = models.PlacementStatusFailed
		errData := map[string]interface{}{
			"code":    "NO_SUITABLE_NODE",
			"message": "No node satisfies VM requirements",
		}
		errJSON, _ := json.Marshal(errData)
		vm.LastError = errJSON
		if err := s.store.UpdateVM(ctx, vm); err != nil {
			return fmt.Errorf("failed to update VM placement status: %w", err)
		}
		metrics.SchedulerPlacementFailures.Inc()
		return errorsx.New(errorsx.ErrPlacementFailed, "No suitable node found")
	}
	
	// Use scheduling strategy to select the best node
	strategy := s.getStrategy()
	selected := strategy.SelectNode(candidates, spec)
	
	vm.NodeID = &selected.ID
	vm.PlacementStatus = models.PlacementStatusScheduled
	vm.ActualState = models.VMActualStateProvisioning
	
	if err := s.store.UpdateVM(ctx, vm); err != nil {
		return fmt.Errorf("failed to assign VM to node: %w", err)
	}
	
	// Update node allocatable resources
	selected.AllocatableCPUCores -= spec.CPU
	selected.AllocatableRAMMB -= spec.MemoryMB
	if err := s.store.UpdateNode(ctx, selected); err != nil {
		return fmt.Errorf("failed to update node resources: %w", err)
	}

	// Record successful placement duration
	metrics.SchedulerPlacementDuration.Observe(time.Since(start).Seconds())

	return nil
}



// ReleaseResources releases resources allocated to a VM on a node.
func (s *Service) ReleaseResources(ctx context.Context, vmID uuid.UUID) error {
	vm, err := s.store.GetVM(ctx, vmID)
	if err != nil {
		return err
	}
	if vm == nil || vm.NodeID == nil {
		return nil
	}
	
	node, err := s.store.GetNode(ctx, *vm.NodeID)
	if err != nil {
		return err
	}
	if node == nil {
		return nil
	}
	
	spec, err := vm.GetSpec()
	if err != nil {
		return err
	}
	
	// Return resources to node
	node.AllocatableCPUCores += spec.CPU
	node.AllocatableRAMMB += spec.MemoryMB
	
	return s.store.UpdateNode(ctx, node)
}
