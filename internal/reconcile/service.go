package reconcile

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"sync"
	"time"

	"github.com/chv/chv/internal/agent"
	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/scheduler"
	"github.com/chv/chv/internal/store"
	"github.com/chv/chv/pkg/errorsx"
	"github.com/google/uuid"
)

// Service handles reconciliation of desired vs actual state.
type Service struct {
	store      store.Store
	scheduler  *scheduler.Service
	agentClient agent.Client
	ticker     *time.Ticker
	stopCh     chan struct{}
	wg         sync.WaitGroup
	triggerCh  chan uuid.UUID
}

// NewService creates a new reconciliation service.
func NewService(store store.Store, scheduler *scheduler.Service, agentClient agent.Client) *Service {
	if agentClient == nil {
		agentClient = agent.NewClient()
	}
	return &Service{
		store:       store,
		scheduler:   scheduler,
		agentClient: agentClient,
		stopCh:      make(chan struct{}),
		triggerCh:   make(chan uuid.UUID, 100),
	}
}

// Start begins the reconciliation loop.
func (s *Service) Start(ctx context.Context) {
	s.ticker = time.NewTicker(30 * time.Second)

	s.wg.Add(1)
	go s.loop(ctx)
}

// Stop stops the reconciliation loop.
func (s *Service) Stop() {
	s.ticker.Stop()
	close(s.stopCh)
	s.wg.Wait()

	// Close agent connections
	if s.agentClient != nil {
		s.agentClient.Close()
	}
}

// TriggerVM triggers reconciliation for a specific VM.
func (s *Service) TriggerVM(vmID uuid.UUID) {
	select {
	case s.triggerCh <- vmID:
	default:
		// Channel full, will be picked up by periodic sync
	}
}

// loop is the main reconciliation loop.
func (s *Service) loop(ctx context.Context) {
	defer s.wg.Done()

	for {
		select {
		case <-s.stopCh:
			return
		case <-ctx.Done():
			return
		case <-s.ticker.C:
			s.reconcileAll(ctx)
		case vmID := <-s.triggerCh:
			s.reconcileVM(ctx, vmID)
		}
	}
}

// reconcileAll reconciles all VMs that need it.
func (s *Service) reconcileAll(ctx context.Context) {
	vms, err := s.store.ListVMsNeedingReconciliation(ctx)
	if err != nil {
		log.Printf("Failed to list VMs needing reconciliation: %v", err)
		return
	}

	for _, vm := range vms {
		s.reconcileVM(ctx, vm.ID)
	}
}

// reconcileVM reconciles a specific VM.
func (s *Service) reconcileVM(ctx context.Context, vmID uuid.UUID) {
	vm, err := s.store.GetVM(ctx, vmID)
	if err != nil {
		log.Printf("Failed to get VM %s: %v", vmID, err)
		return
	}
	if vm == nil {
		return
	}

	// Check if reconciliation is needed
	if !vm.NeedsReconciliation() {
		return
	}

	log.Printf("Reconciling VM %s: desired=%s, actual=%s", vm.Name, vm.DesiredState, vm.ActualState)

	switch vm.DesiredState {
	case models.VMDesiredStateRunning:
		s.reconcileRunning(ctx, vm)
	case models.VMDesiredStateStopped:
		s.reconcileStopped(ctx, vm)
	case models.VMDesiredStateDeleted:
		s.reconcileDeleted(ctx, vm)
	}
}

// reconcileRunning ensures the VM is running.
func (s *Service) reconcileRunning(ctx context.Context, vm *models.VirtualMachine) {
	switch vm.ActualState {
	case models.VMActualStateProvisioning, models.VMActualStateStopped, models.VMActualStateError, models.VMActualStateUnknown:
		// Need to start the VM
		if vm.NodeID == nil {
			// Schedule first
			if err := s.scheduler.ScheduleVM(ctx, vm.ID); err != nil {
				s.setError(ctx, vm, err)
				return
			}
			// Reload VM to get assigned node
			vm, _ = s.store.GetVM(ctx, vm.ID)
			if vm == nil || vm.NodeID == nil {
				return
			}
		}

		// Get node details
		node, err := s.store.GetNode(ctx, *vm.NodeID)
		if err != nil {
			s.setError(ctx, vm, fmt.Errorf("failed to get node: %w", err))
			return
		}
		if node == nil {
			s.setError(ctx, vm, fmt.Errorf("node %s not found", *vm.NodeID))
			return
		}

		// Update state to starting
		vm.ActualState = models.VMActualStateStarting
		if err := s.store.UpdateVM(ctx, vm); err != nil {
			log.Printf("Failed to update VM state: %v", err)
			return
		}

		// Call agent to start VM
		ctx, cancel := context.WithTimeout(ctx, 120*time.Second)
		defer cancel()

		if err := s.agentClient.StartVM(ctx, node.ManagementIP, vm.ID.String()); err != nil {
			// Try to provision first
			spec, _ := vm.GetSpec()
			if spec != nil {
				provisionReq := &struct {
					VMID     string
					VMName   string
					VCPU     int32
					MemoryMB int64
				}{
					VMID:     vm.ID.String(),
					VMName:   vm.Name,
					VCPU:     spec.CPU,
					MemoryMB: spec.MemoryMB,
				}
				_ = provisionReq // Would send ProvisionVM here
			}

			// Retry start
			if err := s.agentClient.StartVM(ctx, node.ManagementIP, vm.ID.String()); err != nil {
				s.setError(ctx, vm, fmt.Errorf("failed to start VM: %w", err))
				return
			}
		}

		// Update state to running
		vm.ActualState = models.VMActualStateRunning
		if err := s.store.UpdateVM(ctx, vm); err != nil {
			log.Printf("Failed to update VM state: %v", err)
		}

	case models.VMActualStateStopping:
		// Wait for stop to complete before starting
		// Poll for state change
		if vm.NodeID != nil {
			node, _ := s.store.GetNode(ctx, *vm.NodeID)
			if node != nil {
				ctx, cancel := context.WithTimeout(ctx, 60*time.Second)
				defer cancel()
				resp, _ := s.agentClient.GetVMState(ctx, node.ManagementIP, vm.ID.String())
				if resp != nil && resp.State == "stopped" {
					// Now we can start
					s.reconcileVM(ctx, vm.ID)
				}
			}
		}
	}
}

// reconcileStopped ensures the VM is stopped.
func (s *Service) reconcileStopped(ctx context.Context, vm *models.VirtualMachine) {
	switch vm.ActualState {
	case models.VMActualStateRunning, models.VMActualStateStarting:
		if vm.NodeID == nil {
			// VM has no node, just mark as stopped
			vm.ActualState = models.VMActualStateStopped
			s.store.UpdateVM(ctx, vm)
			return
		}

		// Get node
		node, err := s.store.GetNode(ctx, *vm.NodeID)
		if err != nil {
			log.Printf("Failed to get node: %v", err)
			return
		}
		if node == nil {
			// Node gone, mark as stopped
			vm.ActualState = models.VMActualStateStopped
			s.store.UpdateVM(ctx, vm)
			s.scheduler.ReleaseResources(ctx, vm.ID)
			return
		}

		vm.ActualState = models.VMActualStateStopping
		if err := s.store.UpdateVM(ctx, vm); err != nil {
			log.Printf("Failed to update VM state: %v", err)
			return
		}

		// Call agent to stop VM
		ctx, cancel := context.WithTimeout(ctx, 90*time.Second)
		defer cancel()

		if err := s.agentClient.StopVM(ctx, node.ManagementIP, vm.ID.String()); err != nil {
			log.Printf("Failed to stop VM: %v", err)
			// Force stop
			s.agentClient.StopVM(ctx, node.ManagementIP, vm.ID.String())
		}

		vm.ActualState = models.VMActualStateStopped
		if err := s.store.UpdateVM(ctx, vm); err != nil {
			log.Printf("Failed to update VM state: %v", err)
		}

		// Release resources
		s.scheduler.ReleaseResources(ctx, vm.ID)
	}
}

// reconcileDeleted ensures the VM is deleted.
func (s *Service) reconcileDeleted(ctx context.Context, vm *models.VirtualMachine) {
	// Stop first if running
	if vm.ActualState == models.VMActualStateRunning || vm.ActualState == models.VMActualStateStarting {
		vm.ActualState = models.VMActualStateStopping
		if err := s.store.UpdateVM(ctx, vm); err != nil {
			log.Printf("Failed to update VM state: %v", err)
			return
		}

		// Call agent to stop if node exists
		if vm.NodeID != nil {
			node, _ := s.store.GetNode(ctx, *vm.NodeID)
			if node != nil {
				ctx, cancel := context.WithTimeout(ctx, 90*time.Second)
				defer cancel()
				if err := s.agentClient.StopVM(ctx, node.ManagementIP, vm.ID.String()); err != nil {
					log.Printf("Failed to stop VM during delete: %v", err)
					// Continue with delete anyway - force delete
				}
			}
		}

		vm.ActualState = models.VMActualStateStopped
	}

	// Call agent to delete VM resources if node exists
	if vm.NodeID != nil {
		node, _ := s.store.GetNode(ctx, *vm.NodeID)
		if node != nil {
			ctx, cancel := context.WithTimeout(ctx, 60*time.Second)
			defer cancel()
			if err := s.agentClient.DeleteVM(ctx, node.ManagementIP, vm.ID.String()); err != nil {
				log.Printf("Warning: failed to delete VM resources on agent: %v", err)
				// Continue with database deletion anyway
			}
		}
	}

	// Release resources
	if err := s.scheduler.ReleaseResources(ctx, vm.ID); err != nil {
		log.Printf("Warning: failed to release resources: %v", err)
	}

	// Delete VM from database
	if err := s.store.DeleteVM(ctx, vm.ID); err != nil {
		log.Printf("Failed to delete VM: %v", err)
		s.setError(ctx, vm, err)
		return
	}

	log.Printf("Successfully deleted VM %s", vm.ID)
}

// setError sets an error on the VM.
func (s *Service) setError(ctx context.Context, vm *models.VirtualMachine, err error) {
	vm.ActualState = models.VMActualStateError

	var errData map[string]interface{}
	if appErr, ok := err.(*errorsx.Error); ok {
		errData = map[string]interface{}{
			"code":    appErr.Code,
			"message": appErr.Message,
		}
	} else {
		errData = map[string]interface{}{
			"code":    "INTERNAL_ERROR",
			"message": err.Error(),
		}
	}

	errJSON, _ := json.Marshal(errData)
	vm.LastError = errJSON

	if err := s.store.UpdateVM(ctx, vm); err != nil {
		log.Printf("Failed to update VM error state: %v", err)
	}
}


