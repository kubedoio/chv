package api

import (
	"context"
	"fmt"
	"sync"
	"time"

	"github.com/chv/chv/internal/agentapi"
	"github.com/chv/chv/internal/agentclient"
	"github.com/chv/chv/internal/logger"
	"github.com/chv/chv/internal/models"
)

// VM status constants for local use
const (
	statusRunning = "running"
	statusStopped = "stopped"
)

// ReconciliationLoop periodically checks VM states and reconciles any inconsistencies
// between the controller's database and the actual state reported by the agent.
type ReconciliationLoop struct {
	handler   *Handler
	interval  time.Duration
	stopChan  chan struct{}
	mu        sync.RWMutex
	isRunning bool
}

// NewReconciliationLoop creates a new reconciliation loop instance
func NewReconciliationLoop(handler *Handler) *ReconciliationLoop {
	return &ReconciliationLoop{
		handler:  handler,
		interval: 30 * time.Second,
		stopChan: make(chan struct{}),
	}
}

// Start begins the reconciliation loop in a background goroutine
func (rl *ReconciliationLoop) Start(ctx context.Context) {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	if rl.isRunning {
		logger.L().Warn("Reconciliation loop is already running")
		return
	}

	rl.isRunning = true
	// Recreate stopChan in case Stop() was called before
	rl.stopChan = make(chan struct{})
	logger.L().Info("Starting VM state reconciliation loop", logger.F("interval", rl.interval))

	// Run initial reconciliation immediately
	go rl.runReconciliation(ctx)

	// Start the ticker for periodic reconciliation
	go rl.loop(ctx)
}

// Stop gracefully stops the reconciliation loop
func (rl *ReconciliationLoop) Stop() {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	if !rl.isRunning {
		return
	}

	logger.L().Info("Stopping VM state reconciliation loop")
	close(rl.stopChan)
	rl.isRunning = false
}

// IsRunning returns whether the reconciliation loop is currently running
func (rl *ReconciliationLoop) IsRunning() bool {
	rl.mu.RLock()
	defer rl.mu.RUnlock()
	return rl.isRunning
}

// loop runs the periodic reconciliation with a ticker
func (rl *ReconciliationLoop) loop(ctx context.Context) {
	ticker := time.NewTicker(rl.interval)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			logger.L().Info("Reconciliation loop stopping due to context cancellation")
			rl.mu.Lock()
			rl.isRunning = false
			rl.mu.Unlock()
			return
		case <-rl.stopChan:
			logger.L().Info("Reconciliation loop stopped")
			return
		case <-ticker.C:
			rl.runReconciliation(ctx)
		}
	}
}

// runReconciliation performs a single reconciliation pass
func (rl *ReconciliationLoop) runReconciliation(ctx context.Context) {
	logger.L().Info("Running VM state reconciliation")

	// Skip if VM service is not available
	if rl.handler.vmService == nil {
		logger.L().Warn("VM service not available, skipping reconciliation")
		return
	}

	// Get all VMs that should be running
	vms, err := rl.handler.repo.ListVMsByDesiredState(ctx, "running")
	if err != nil {
		logger.L().Error("Failed to list VMs for reconciliation", logger.ErrorField(err))
		return
	}

	logger.L().Info("Reconciling VM states", logger.F("vm_count", len(vms)))

	// Get agent client once for all checks
	agentClient := rl.handler.vmService.GetAgentClient()
	if agentClient == nil {
		logger.L().Warn("Agent client not available, skipping reconciliation")
		return
	}

	for _, vm := range vms {
		// Check if the VM is actually running according to the agent
		isRunning, pid, err := rl.checkVMActualStatusWithPID(ctx, agentClient, &vm)
		if err != nil {
			logger.L().Warn("Failed to check VM actual status",
				logger.F("vm_id", vm.ID),
				logger.F("vm_name", vm.Name),
				logger.ErrorField(err))
			continue
		}

		// Case 1: DB says running, but agent says stopped -> Update DB to stopped
		if !isRunning && vm.ActualState == statusRunning {
			logger.L().Warn("VM state inconsistency detected: DB shows running but agent reports stopped",
				logger.F("vm_id", vm.ID),
				logger.F("vm_name", vm.Name),
				logger.F("db_state", vm.ActualState),
				logger.F("agent_state", "stopped"))

			// Update the VM state in database
			vm.ActualState = statusStopped
			vm.CloudHypervisorPID = 0
			vm.LastError = "VM stopped unexpectedly (detected by reconciliation)"
			vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)

			if err := rl.handler.repo.UpdateVM(ctx, &vm); err != nil {
				logger.L().Error("Failed to update VM state during reconciliation",
					logger.F("vm_id", vm.ID),
					logger.ErrorField(err))
				continue
			}

			logger.L().Info("VM state reconciled: updated to stopped",
				logger.F("vm_id", vm.ID),
				logger.F("vm_name", vm.Name))
		}

		// Case 2: DB says stopped, but agent says running -> Update DB to running
		if isRunning && vm.ActualState == statusStopped {
			logger.L().Info("VM state inconsistency detected: DB shows stopped but agent reports running",
				logger.F("vm_id", vm.ID),
				logger.F("vm_name", vm.Name),
				logger.F("db_state", vm.ActualState),
				logger.F("agent_state", "running"),
				logger.F("pid", pid))

			// Update the VM state in database
			vm.ActualState = statusRunning
			vm.CloudHypervisorPID = pid
			vm.LastError = ""
			vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)

			if err := rl.handler.repo.UpdateVM(ctx, &vm); err != nil {
				logger.L().Error("Failed to update VM state during reconciliation",
					logger.F("vm_id", vm.ID),
					logger.ErrorField(err))
				continue
			}

			logger.L().Info("VM state reconciled: updated to running",
				logger.F("vm_id", vm.ID),
				logger.F("vm_name", vm.Name),
				logger.F("pid", pid))
		}
	}
}

// checkVMActualStatusWithPID checks with the agent whether a VM is actually running and returns the PID
func (rl *ReconciliationLoop) checkVMActualStatusWithPID(ctx context.Context, agentClient *agentclient.Client, vm *models.VirtualMachine) (bool, int, error) {
	// Check with the agent
	req := &agentapi.VMStatusRequest{
		VMID: vm.ID,
		PID:  vm.CloudHypervisorPID,
	}

	resp, err := agentClient.GetVMStatus(ctx, req)
	if err != nil {
		// If we can't reach the agent, assume the VM state is unknown
		// but don't change it to avoid flapping
		return false, 0, fmt.Errorf("failed to get VM status from agent: %w", err)
	}

	return resp.Running, resp.PID, nil
}
