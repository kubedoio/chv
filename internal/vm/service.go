package vm

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"time"

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
}

// NewService creates a new VM service
func NewService(repo *db.Repository, dataRoot string) *Service {
	return &Service{
		repo:         repo,
		dataRoot:     dataRoot,
		cloudinitRdr: cloudinit.NewRenderer(dataRoot),
		seedSvc:      services.NewSeedISOService(),
	}
}

// SetAgentClient sets the agent client for VM lifecycle operations
func (s *Service) SetAgentClient(agentURL string) {
	s.agentURL = agentURL
	if agentURL != "" {
		s.agentClient = agentclient.NewClient(agentURL)
	}
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

	// Create VM record
	vmID := uuid.NewString()
	now := time.Now().UTC().Format(time.RFC3339)
	workspacePath := filepath.Join(s.dataRoot, "vms", vmID)

	vm := &models.VirtualMachine{
		ID:            vmID,
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
	// Create workspace
	if err := os.MkdirAll(vm.WorkspacePath, 0755); err != nil {
		return fmt.Errorf("failed to create workspace: %w", err)
	}

	// Create cloud-init files
	cloudinitCfg := cloudinit.Config{
		VMID:              vm.ID,
		VMName:            vm.Name,
		Hostname:          vm.Name,
		Username:          input.Username,
		Password:          input.Password,
		SSHAuthorizedKeys: input.SSHAuthorizedKeys,
		UserData:          input.UserData,
	}

	renderResult, err := s.cloudinitRdr.Render(ctx, vm.ID, cloudinitCfg)
	if err != nil {
		return fmt.Errorf("failed to render cloud-init: %w", err)
	}

	// Generate seed ISO
	seedResult, err := s.seedSvc.Generate(ctx, services.GenerateRequest{
		VMID:         vm.ID,
		CloudinitDir: renderResult.CloudinitDir,
		OutputDir:    vm.WorkspacePath,
	})
	if err != nil {
		return fmt.Errorf("failed to generate seed ISO: %w", err)
	}

	// Store seed ISO path
	vm.SeedISOPath = seedResult.ISOPath

	// Clone disk from image (via agent in future, copy for MVP)
	if err := s.cloneDisk(image.LocalPath, vm.DiskPath); err != nil {
		return fmt.Errorf("failed to clone disk: %w", err)
	}

	// Write config.json
	config := VMConfig{
		ID:       vm.ID,
		Name:     vm.Name,
		VCPU:     vm.VCPU,
		MemoryMB: vm.MemoryMB,
		DiskPath: vm.DiskPath,
		SeedISO:  vm.SeedISOPath,
	}

	configPath := filepath.Join(vm.WorkspacePath, "config.json")
	if err := s.writeConfig(configPath, config); err != nil {
		return fmt.Errorf("failed to write config: %w", err)
	}

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
		if err := s.StopVM(ctx, vmID); err != nil {
			return fmt.Errorf("failed to stop VM for restart: %w", err)
		}

		// Wait for stop with timeout
		if err := s.waitForState(ctx, vmID, StatusStopped, 30*time.Second); err != nil {
			return fmt.Errorf("timeout waiting for VM to stop: %w", err)
		}
	}

	// Small delay for cleanup
	time.Sleep(1 * time.Second)

	// Start VM
	return s.StartVM(ctx, vmID)
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
