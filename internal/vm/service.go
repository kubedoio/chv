package vm

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"sync"

	"github.com/chv/chv/internal/agent/services"
	"github.com/chv/chv/internal/agentapi"
	"github.com/chv/chv/internal/agentclient"
	"github.com/chv/chv/internal/cloudinit"
	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
)

// Status constants
const (
	StatusProvisioning = "provisioning"
	StatusPrepared     = "prepared"
	StatusStarting     = "starting"
	StatusRunning      = "running"
	StatusStopping     = "stopping"
	StatusStopped      = "stopped"
	StatusDeleting     = "deleting"
	StatusError        = "error"
)

// Service handles VM lifecycle operations
type Service struct {
	repo         *db.Repository
	dataRoot     string
	cloudinitRdr *cloudinit.Renderer
	seedSvc      *services.SeedISOService
	agentClient  *agentclient.Client
	agentURL     string

	// Metrics history
	metricsMu sync.RWMutex
	history   map[string][]agentapi.VMMetricsResponse
}

// NewService creates a new VM service
func NewService(repo *db.Repository, dataRoot string) *Service {
	s := &Service{
		repo:         repo,
		dataRoot:     dataRoot,
		cloudinitRdr: cloudinit.NewRenderer(dataRoot),
		seedSvc:      services.NewSeedISOService(),
		history:      make(map[string][]agentapi.VMMetricsResponse),
	}

	// Start background poller
	go s.startMetricsPoller()

	return s
}

// SetAgentClient sets the agent client for VM lifecycle operations
func (s *Service) SetAgentClient(agentURL string) {
	s.agentURL = agentURL
	if agentURL != "" {
		s.agentClient = agentclient.NewClient(agentURL)
	}
}

// GetAgentClient returns the agent client for external use
func (s *Service) GetAgentClient() *agentclient.Client {
	return s.agentClient
}

// CreateVMInput holds parameters for creating a VM
type CreateVMInput struct {
	Name              string
	ImageID           string
	StoragePoolID     string
	NetworkID         string
	VCPU              int
	MemoryMB          int
	UserData          string
	Username          string
	Password          string
	SSHAuthorizedKeys []string
}

// CreateVM creates a new VM with provisioning workflow
func (s *Service) CreateVM(ctx context.Context, input CreateVMInput) (*models.VirtualMachine, error) {
	// Validate inputs
	if input.Name == "" {
		return nil, fmt.Errorf("VM name is required")
	}
	if input.ImageID == "" {
		return nil, fmt.Errorf("image ID is required")
	}
	if input.StoragePoolID == "" {
		return nil, fmt.Errorf("storage pool ID is required")
	}
	if input.NetworkID == "" {
		return nil, fmt.Errorf("network ID is required")
	}

	// Get image
	image, err := s.repo.GetImageByID(ctx, input.ImageID)
	if err != nil {
		return nil, fmt.Errorf("failed to get image: %w", err)
	}
	if image == nil {
		return nil, fmt.Errorf("image not found: %s", input.ImageID)
	}

	// Get storage pool
	pool, err := s.repo.GetStoragePoolByID(ctx, input.StoragePoolID)
	if err != nil {
		return nil, fmt.Errorf("failed to get storage pool: %w", err)
	}
	if pool == nil {
		return nil, fmt.Errorf("storage pool not found: %s", input.StoragePoolID)
	}

	// Get local node ID
	localNode, err := s.repo.GetLocalNode(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to get local node: %w", err)
	}
	if localNode == nil {
		return nil, fmt.Errorf("local node not found")
	}

	// Create VM record
	vmID := uuid.NewString()
	now := time.Now().UTC().Format(time.RFC3339)
	workspacePath := filepath.Join(s.dataRoot, "vms", vmID)

	vm := &models.VirtualMachine{
		ID:            vmID,
		NodeID:        localNode.ID,
		Name:          input.Name,
		ImageID:       input.ImageID,
		StoragePoolID: input.StoragePoolID,
		NetworkID:     input.NetworkID,
		VCPU:          input.VCPU,
		MemoryMB:      input.MemoryMB,
		DesiredState:  "stopped",
		ActualState:   StatusProvisioning,
		WorkspacePath: workspacePath,
		DiskPath:      filepath.Join(workspacePath, "disk.qcow2"),
		CreatedAt:     now,
		UpdatedAt:     now,
	}

	// Create VM record in DB
	if err := s.repo.CreateVM(ctx, vm); err != nil {
		return nil, fmt.Errorf("failed to create VM record: %w", err)
	}

	// Provision VM (async in production, sync for MVP)
	if err := s.provisionVM(ctx, vm, image, input); err != nil {
		// Update status to error
		vm.ActualState = StatusError
		vm.LastError = err.Error()
		vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
		_ = s.repo.UpdateVM(ctx, vm)
		return nil, fmt.Errorf("failed to provision VM: %w", err)
	}

	// Update to prepared
	vm.ActualState = StatusPrepared
	vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	if err := s.repo.UpdateVM(ctx, vm); err != nil {
		return nil, fmt.Errorf("failed to update VM status: %w", err)
	}

	return vm, nil
}

// provisionVM creates workspace, cloud-init files, clones disk, writes config
func (s *Service) provisionVM(ctx context.Context, vm *models.VirtualMachine, image *models.Image, input CreateVMInput) error {
	if s.agentClient == nil {
		return fmt.Errorf("agent client not available")
	}

	// Delegate provisioning to the agent (Phase 4.1 Distributed Infrastructure)
	provisionReq := &agentapi.VMProvisionRequest{
		VMID:              vm.ID,
		VMName:            vm.Name,
		ImagePath:         image.LocalPath,
		DiskPath:          vm.DiskPath,
		WorkspacePath:     vm.WorkspacePath,
		Username:          input.Username,
		Password:          input.Password,
		SSHAuthorizedKeys: input.SSHAuthorizedKeys,
		UserData:          input.UserData,
	}

	if _, err := s.agentClient.ProvisionVM(ctx, provisionReq); err != nil {
		return fmt.Errorf("agent failed to provision VM: %w", err)
	}

	// Update seed ISO path (Agent generates it in workspace root)
	vm.SeedISOPath = filepath.Join(vm.WorkspacePath, "seed.iso")

	return nil
}

// cloneDisk clones the base image to the VM disk path
func (s *Service) cloneDisk(source, dest string) error {
	// Try qemu-img first for proper qcow2 handling
	if _, err := exec.LookPath("qemu-img"); err == nil {
		cmd := exec.Command("qemu-img", "convert", "-f", "qcow2", "-O", "qcow2", source, dest)
		if output, err := cmd.CombinedOutput(); err != nil {
			return fmt.Errorf("qemu-img convert failed: %w (output: %s)", err, output)
		}
		return nil
	}

	// Fallback to cp (only works if source is raw)
	input, err := os.ReadFile(source)
	if err != nil {
		return fmt.Errorf("failed to read source disk: %w", err)
	}

	if err := os.WriteFile(dest, input, 0644); err != nil {
		return fmt.Errorf("failed to write dest disk: %w", err)
	}

	return nil
}

// writeConfig writes the VM config to a JSON file
func (s *Service) writeConfig(path string, config VMConfig) error {
	file, err := os.Create(path)
	if err != nil {
		return err
	}
	defer file.Close()

	encoder := json.NewEncoder(file)
	encoder.SetIndent("", "  ")
	return encoder.Encode(config)
}

// VMConfig represents the on-disk VM configuration
type VMConfig struct {
	ID       string `json:"id"`
	Name     string `json:"name"`
	VCPU     int    `json:"vcpu"`
	MemoryMB int    `json:"memory_mb"`
	DiskPath string `json:"disk_path"`
	SeedISO  string `json:"seed_iso"`
}

// StartVM starts a VM by launching Cloud Hypervisor
func (s *Service) StartVM(ctx context.Context, vmID string) error {
	// Get VM
	vm, err := s.repo.GetVMByID(ctx, vmID)
	if err != nil {
		return fmt.Errorf("failed to get VM: %w", err)
	}
	if vm == nil {
		return fmt.Errorf("VM not found: %s", vmID)
	}

	// Check if already running
	if vm.ActualState == StatusRunning {
		return fmt.Errorf("VM is already running")
	}

	// Run boot gates
	gatekeeper := NewGatekeeper(s.repo)
	result := gatekeeper.CheckAll(ctx, vm)
	if !result.Passed {
		return fmt.Errorf("boot gate failed: %v", result.Errors)
	}

	// Update to starting
	vm.DesiredState = "running"
	vm.ActualState = StatusStarting
	vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	if err := s.repo.UpdateVM(ctx, vm); err != nil {
		return fmt.Errorf("failed to update VM status: %w", err)
	}

	// If agent is available, use it to start VM
	if s.agentClient != nil {
		// Get network info for TAP device
		network, err := s.repo.GetNetworkByID(ctx, vm.NetworkID)
		if err != nil {
			return fmt.Errorf("failed to get network: %w", err)
		}

		req := &agentapi.VMStartRequest{
			VMID:        vmID,
			DiskPath:    vm.DiskPath,
			SeedISOPath: vm.SeedISOPath,
			MACAddress:  vm.MACAddress,
			IPAddress:   vm.IPAddress,
			Netmask:     "255.255.255.0", // Default for MVP
			VCPU:        vm.VCPU,
			MemoryMB:    vm.MemoryMB,
			WorkspacePath: vm.WorkspacePath,
			BridgeName:  network.BridgeName,
		}

		resp, err := s.agentClient.StartVM(ctx, req)
		if err != nil {
			// If agent says VM is already running, reconcile state instead of failing
			if strings.Contains(err.Error(), "already running") {
				statusResp, statusErr := s.agentClient.GetVMStatus(ctx, &agentapi.VMStatusRequest{VMID: vmID})
				if statusErr == nil && statusResp.Running {
					vm.ActualState = StatusRunning
					vm.DesiredState = "running"
					vm.CloudHypervisorPID = statusResp.PID
					vm.LastError = ""
					vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
					_ = s.repo.UpdateVM(ctx, vm)
					return nil // State is now consistent
				}
			}
			// Mark as error
			vm.ActualState = StatusError
			vm.LastError = fmt.Sprintf("Failed to start VM: %v", err)
			vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
			_ = s.repo.UpdateVM(ctx, vm)
			return fmt.Errorf("failed to start VM via agent: %w", err)
		}

		vm.CloudHypervisorPID = resp.PID
	} else {
		// No agent - simulate for development/testing
		vm.CloudHypervisorPID = 12345 // Placeholder
	}

	vm.ActualState = StatusRunning
	vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	if err := s.repo.UpdateVM(ctx, vm); err != nil {
		return fmt.Errorf("failed to update VM status: %w", err)
	}

	return nil
}

// StopVM stops a running VM
func (s *Service) StopVM(ctx context.Context, vmID string) error {
	vm, err := s.repo.GetVMByID(ctx, vmID)
	if err != nil {
		return fmt.Errorf("failed to get VM: %w", err)
	}
	if vm == nil {
		return fmt.Errorf("VM not found: %s", vmID)
	}

	if vm.ActualState != StatusRunning {
		return fmt.Errorf("VM is not running")
	}

	// Update to stopping
	vm.DesiredState = "stopped"
	vm.ActualState = StatusStopping
	vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	if err := s.repo.UpdateVM(ctx, vm); err != nil {
		return fmt.Errorf("failed to update VM status: %w", err)
	}

	// If agent is available, use it to stop VM
	if s.agentClient != nil && vm.CloudHypervisorPID > 0 {
		req := &agentapi.VMStopRequest{
			VMID: vmID,
			PID:  vm.CloudHypervisorPID,
		}

		_, err := s.agentClient.StopVM(ctx, req)
		if err != nil {
			// Log error but continue to mark as stopped
			fmt.Printf("Warning: failed to stop VM via agent: %v\n", err)
		}
	}

	vm.ActualState = StatusStopped
	vm.CloudHypervisorPID = 0
	vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	if err := s.repo.UpdateVM(ctx, vm); err != nil {
		return fmt.Errorf("failed to update VM status: %w", err)
	}

	return nil
}

// RestartVM stops and restarts a VM atomically
func (s *Service) RestartVM(ctx context.Context, vmID string) error {
	return s.RestartVMWithOptions(ctx, vmID, false, 60*time.Second)
}

// RestartVMWithOptions restarts a VM with configurable graceful shutdown
func (s *Service) RestartVMWithOptions(ctx context.Context, vmID string, graceful bool, timeout time.Duration) error {
	// Get VM
	vm, err := s.repo.GetVMByID(ctx, vmID)
	if err != nil {
		return fmt.Errorf("failed to get VM: %w", err)
	}
	if vm == nil {
		return fmt.Errorf("VM not found: %s", vmID)
	}

	// Check if in transition
	if vm.ActualState == StatusStarting || vm.ActualState == StatusStopping {
		return fmt.Errorf("VM is in transition state: %s", vm.ActualState)
	}

	// Stop if running
	if vm.ActualState == StatusRunning {
		if graceful {
			if err := s.ShutdownVM(ctx, vmID, timeout); err != nil {
				return fmt.Errorf("failed to shutdown VM for restart: %w", err)
			}
		} else {
			if err := s.StopVM(ctx, vmID); err != nil {
				return fmt.Errorf("failed to stop VM for restart: %w", err)
			}
		}

		// Wait for stop with timeout
		waitTimeout := timeout + 10*time.Second // Add buffer to shutdown timeout
		if !graceful {
			waitTimeout = 30 * time.Second
		}
		if err := s.waitForState(ctx, vmID, StatusStopped, waitTimeout); err != nil {
			return fmt.Errorf("timeout waiting for VM to stop: %w", err)
		}
	}

	// Small delay for cleanup
	time.Sleep(1 * time.Second)

	// Start VM
	return s.StartVM(ctx, vmID)
}

// ShutdownVM gracefully shuts down a VM using ACPI signal
func (s *Service) ShutdownVM(ctx context.Context, vmID string, timeout time.Duration) error {
	vm, err := s.repo.GetVMByID(ctx, vmID)
	if err != nil {
		return fmt.Errorf("failed to get VM: %w", err)
	}
	if vm == nil {
		return fmt.Errorf("VM not found: %s", vmID)
	}

	if vm.ActualState != StatusRunning {
		return fmt.Errorf("VM is not running")
	}

	// Update to stopping state
	vm.DesiredState = "stopped"
	vm.ActualState = StatusStopping
	vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	if err := s.repo.UpdateVM(ctx, vm); err != nil {
		return fmt.Errorf("failed to update VM status: %w", err)
	}

	// If agent is available, use it to send ACPI shutdown
	if s.agentClient != nil && vm.CloudHypervisorPID > 0 {
		req := &agentapi.VMShutdownRequest{
			VMID:    vmID,
			PID:     vm.CloudHypervisorPID,
			Timeout: int(timeout.Seconds()),
		}

		_, err := s.agentClient.ShutdownVM(ctx, req)
		if err != nil {
			// Log error but continue - VM might still shut down
			fmt.Printf("Warning: failed to send shutdown signal via agent: %v\n", err)
		}
	}

	// Wait for VM to stop (with timeout)
	done := make(chan error, 1)
	go func() {
		err := s.waitForState(ctx, vmID, StatusStopped, timeout)
		done <- err
	}()

	select {
	case err := <-done:
		if err != nil {
			// Shutdown timed out, VM may still be running
			return fmt.Errorf("shutdown timed out after %v: %w", timeout, err)
		}
		// VM stopped successfully
	case <-time.After(timeout + 5*time.Second):
		// This shouldn't happen given waitForState has its own timeout
		return fmt.Errorf("shutdown wait exceeded timeout")
	}

	return nil
}

// ForceStopVM immediately kills a VM process
func (s *Service) ForceStopVM(ctx context.Context, vmID string) error {
	vm, err := s.repo.GetVMByID(ctx, vmID)
	if err != nil {
		return fmt.Errorf("failed to get VM: %w", err)
	}
	if vm == nil {
		return fmt.Errorf("VM not found: %s", vmID)
	}

	if vm.ActualState != StatusRunning && vm.ActualState != StatusStarting {
		return fmt.Errorf("VM is not running")
	}

	// Update to stopping state
	vm.DesiredState = "stopped"
	vm.ActualState = StatusStopping
	vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	if err := s.repo.UpdateVM(ctx, vm); err != nil {
		return fmt.Errorf("failed to update VM status: %w", err)
	}

	// If agent is available, use it to force stop
	if s.agentClient != nil && vm.CloudHypervisorPID > 0 {
		req := &agentapi.VMForceStopRequest{
			VMID: vmID,
			PID:  vm.CloudHypervisorPID,
		}

		_, err := s.agentClient.ForceStopVM(ctx, req)
		if err != nil {
			// Log error but continue to update state
			fmt.Printf("Warning: failed to force stop VM via agent: %v\n", err)
		}
	}

	// Update state to stopped
	vm.ActualState = StatusStopped
	vm.CloudHypervisorPID = 0
	vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	if err := s.repo.UpdateVM(ctx, vm); err != nil {
		return fmt.Errorf("failed to update VM status: %w", err)
	}

	return nil
}

// ResetVM resets a VM (power cycle without full shutdown)
func (s *Service) ResetVM(ctx context.Context, vmID string) error {
	vm, err := s.repo.GetVMByID(ctx, vmID)
	if err != nil {
		return fmt.Errorf("failed to get VM: %w", err)
	}
	if vm == nil {
		return fmt.Errorf("VM not found: %s", vmID)
	}

	if vm.ActualState != StatusRunning {
		return fmt.Errorf("VM is not running")
	}

	// If agent is available, use it to reset
	if s.agentClient != nil && vm.CloudHypervisorPID > 0 {
		req := &agentapi.VMResetRequest{
			VMID: vmID,
			PID:  vm.CloudHypervisorPID,
		}

		_, err := s.agentClient.ResetVM(ctx, req)
		if err != nil {
			return fmt.Errorf("failed to reset VM via agent: %w", err)
		}
	}

	// Update timestamp
	vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	if err := s.repo.UpdateVM(ctx, vm); err != nil {
		return fmt.Errorf("failed to update VM status: %w", err)
	}

	return nil
}

// waitForState polls the VM until it reaches the target state or timeout
func (s *Service) waitForState(ctx context.Context, vmID string, targetState string, timeout time.Duration) error {
	deadline := time.After(timeout)
	ticker := time.NewTicker(500 * time.Millisecond)
	defer ticker.Stop()

	for {
		select {
		case <-deadline:
			return fmt.Errorf("timeout")
		case <-ticker.C:
			vm, err := s.repo.GetVMByID(ctx, vmID)
			if err != nil {
				return err
			}
			if vm != nil && vm.ActualState == targetState {
				return nil
			}
		}
	}
}

// GetVMMetrics retrieves VM metrics from the agent
func (s *Service) GetVMMetrics(ctx context.Context, vmID string, pid int, workspacePath string) (*agentapi.VMMetricsResponse, error) {
	if s.agentClient == nil {
		return nil, fmt.Errorf("agent client not available")
	}

	if pid == 0 {
		return nil, fmt.Errorf("VM is not running")
	}

	req := &agentapi.VMMetricsRequest{
		VMID:      vmID,
		PID:       pid,
		APISocket: filepath.Join(workspacePath, "api.sock"),
	}

	return s.agentClient.GetVMMetrics(ctx, req)
}

// GetVMMetricsHistory returns the historical metrics for a VM
func (s *Service) GetVMMetricsHistory(vmID string) []agentapi.VMMetricsResponse {
	s.metricsMu.RLock()
	defer s.metricsMu.RUnlock()

	res := s.history[vmID]
	if res == nil {
		return []agentapi.VMMetricsResponse{}
	}

	// Return a copy to avoid race conditions
	copied := make([]agentapi.VMMetricsResponse, len(res))
	copy(copied, res)
	return copied
}

func (s *Service) startMetricsPoller() {
	ticker := time.NewTicker(30 * time.Second)
	defer ticker.Stop()

	for {
		<-ticker.C
		s.pollAllVMs()
	}
}

func (s *Service) pollAllVMs() {
	ctx := context.Background()
	vms, err := s.repo.ListVMs(ctx)
	if err != nil {
		return
	}

	for _, vm := range vms {
		if vm.ActualState == StatusRunning && vm.CloudHypervisorPID > 0 {
			metrics, err := s.GetVMMetrics(ctx, vm.ID, vm.CloudHypervisorPID, vm.WorkspacePath)
			if err == nil && metrics != nil {
				s.recordMetrics(vm.ID, *metrics)
			}
		}
	}
}

func (s *Service) recordMetrics(vmID string, m agentapi.VMMetricsResponse) {
	s.metricsMu.Lock()
	defer s.metricsMu.Unlock()

	history := s.history[vmID]
	history = append(history, m)

	// Keep last 60 points (30 minutes if polling every 30s)
	if len(history) > 60 {
		history = history[1:]
	}

	s.history[vmID] = history
}

// DeleteVM deletes a VM and cleans up resources
func (s *Service) DeleteVM(ctx context.Context, vmID string) error {
	vm, err := s.repo.GetVMByID(ctx, vmID)
	if err != nil {
		return fmt.Errorf("failed to get VM: %w", err)
	}
	if vm == nil {
		return fmt.Errorf("VM not found: %s", vmID)
	}

	// Stop if running
	if vm.ActualState == StatusRunning {
		if err := s.StopVM(ctx, vmID); err != nil {
			return fmt.Errorf("failed to stop VM before delete: %w", err)
		}
	}

	// Update to deleting
	vm.DesiredState = "deleted"
	vm.ActualState = StatusDeleting
	vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	if err := s.repo.UpdateVM(ctx, vm); err != nil {
		return fmt.Errorf("failed to update VM status: %w", err)
	}

	// Cleanup workspace
	if vm.WorkspacePath != "" {
		if err := os.RemoveAll(vm.WorkspacePath); err != nil {
			// Log but don't fail - DB record is what matters
			fmt.Printf("Warning: failed to cleanup workspace: %v\n", err)
		}
	}

	// Delete from DB (or mark as deleted)
	if err := s.repo.DeleteVM(ctx, vmID); err != nil {
		return fmt.Errorf("failed to delete VM: %w", err)
	}

	return nil
}

// GetVM retrieves a VM by ID
func (s *Service) GetVM(ctx context.Context, vmID string) (*models.VirtualMachine, error) {
	return s.repo.GetVMByID(ctx, vmID)
}

// CreateSnapshot creates a new internal snapshot for a VM
func (s *Service) CreateSnapshot(ctx context.Context, vmID string) (*models.VMSnapshot, error) {
	vm, err := s.repo.GetVMByID(ctx, vmID)
	if err != nil {
		return nil, err
	}
	if vm == nil {
		return nil, fmt.Errorf("VM not found")
	}

	// Enforce non-live (Stage 3 restriction)
	if vm.ActualState != StatusStopped && vm.ActualState != StatusPrepared {
		return nil, fmt.Errorf("VM must be stopped to create a snapshot (current state: %s)", vm.ActualState)
	}

	if s.agentClient == nil {
		return nil, fmt.Errorf("agent client not available")
	}

	snapName := fmt.Sprintf("snap-%s", time.Now().UTC().Format("20060102150405"))
	req := &agentapi.VMSnapshotCreateRequest{
		VMID:     vmID,
		DiskPath: vm.DiskPath,
		Name:     snapName,
	}

	if _, err := s.agentClient.CreateSnapshot(ctx, req); err != nil {
		return nil, fmt.Errorf("agent failed to create snapshot: %w", err)
	}

	snapshot := &models.VMSnapshot{
		ID:        uuid.NewString(),
		VMID:      vmID,
		Name:      snapName,
		CreatedAt: time.Now().UTC().Format(time.RFC3339),
		Status:    "ready",
	}

	if err := s.repo.CreateVMSnapshot(ctx, snapshot); err != nil {
		return nil, fmt.Errorf("failed to save snapshot metadata: %w", err)
	}

	return snapshot, nil
}

// ListSnapshots returns all snapshots for a VM from the local database
func (s *Service) ListSnapshots(ctx context.Context, vmID string) ([]models.VMSnapshot, error) {
	return s.repo.ListVMSnapshots(ctx, vmID)
}

// RestoreSnapshot reverts a VM to a specific internal snapshot
func (s *Service) RestoreSnapshot(ctx context.Context, vmID, snapID string) error {
	vm, err := s.repo.GetVMByID(ctx, vmID)
	if err != nil {
		return err
	}
	if vm == nil {
		return fmt.Errorf("VM not found")
	}

	// Enforce non-live (Stage 3 restriction)
	if vm.ActualState != StatusStopped && vm.ActualState != StatusPrepared {
		return fmt.Errorf("VM must be stopped to restore a snapshot (current state: %s)", vm.ActualState)
	}

	snapshot, err := s.repo.GetVMSnapshot(ctx, snapID)
	if err != nil {
		return err
	}
	if snapshot == nil {
		return fmt.Errorf("snapshot not found")
	}

	if s.agentClient == nil {
		return fmt.Errorf("agent client not available")
	}

	req := &agentapi.VMSnapshotRestoreRequest{
		VMID:     vmID,
		DiskPath: vm.DiskPath,
		Name:     snapshot.Name,
	}

	if _, err := s.agentClient.RestoreSnapshot(ctx, req); err != nil {
		return fmt.Errorf("agent failed to restore snapshot: %w", err)
	}

	// Update VM updated_at timestamp to flag changes
	vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	_ = s.repo.UpdateVM(ctx, vm)

	return nil
}

// DeleteSnapshot deletes a snapshot from both the hypervisor and the database
func (s *Service) DeleteSnapshot(ctx context.Context, vmID, snapID string) error {
	vm, err := s.repo.GetVMByID(ctx, vmID)
	if err != nil {
		return err
	}
	if vm == nil {
		return fmt.Errorf("VM not found")
	}

	snapshot, err := s.repo.GetVMSnapshot(ctx, snapID)
	if err != nil {
		return err
	}
	if snapshot == nil {
		return fmt.Errorf("snapshot not found")
	}

	if s.agentClient == nil {
		return fmt.Errorf("agent client not available")
	}

	req := &agentapi.VMSnapshotDeleteRequest{
		VMID:     vmID,
		DiskPath: vm.DiskPath,
		Name:     snapshot.Name,
	}

	if _, err := s.agentClient.DeleteSnapshot(ctx, req); err != nil {
		return fmt.Errorf("agent failed to delete snapshot: %w", err)
	}

	return s.repo.DeleteVMSnapshot(ctx, snapID)
}
