// Package hypervisor provides VM lifecycle management for Cloud Hypervisor.
package hypervisor

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sync"
	"syscall"
	"time"

	"github.com/chv/chv/internal/agent/cloudinit"
	"github.com/chv/chv/internal/network"
	"github.com/chv/chv/pkg/uuidx"
	"go.uber.org/zap"
)

// Launcher manages Cloud Hypervisor VM processes.
type Launcher struct {
	chvBinary     string
	stateDir      string
	logDir        string
	socketDir     string
	stateManager  *StateManager
	tapManager    *network.TAPManager
	isoGenerator  *cloudinit.ISOGenerator
	dataDir       string // Base data directory for ISO generation
	logger        *zap.Logger
	
	// In-memory tracking of running VMs (supplemented by stateManager)
	instances map[string]*VMInstance
	mu        sync.RWMutex // Protects instances map
}

// VMConfig contains configuration for starting a VM.
type VMConfig struct {
	VMID            string
	Name            string
	VCPU            int
	MemoryMB        int
	VolumePath      string
	VolumeFormat    string    // Disk format: "raw", "qcow2" (default: "raw")
	BackingImageID  string
	BridgeName      string
	CloudInit       *cloudinit.Config
	CloudInitISO    string // Path to generated ISO (optional, overrides CloudInit)
	APIsocket       string // Optional: override socket path
}

// VMInstance represents a running VM instance.
type VMInstance struct {
	VMID        string
	PID         int
	APISocket   string
	TAPDevice   *network.TAPDevice
	ISOPath     string
	Process     *os.Process
	chvClient   *CHVClient
	cleanupOnce sync.Once // Ensures cleanup happens exactly once
}

// NewLauncher creates a new VM launcher.
func NewLauncher(
	chvBinary string,
	stateDir string,
	logDir string,
	socketDir string,
	stateManager *StateManager,
	tapManager *network.TAPManager,
	isoGenerator *cloudinit.ISOGenerator,
	logger *zap.Logger,
) *Launcher {
	// Determine dataDir from stateDir (stateDir is typically <dataDir>/instances)
	dataDir := filepath.Dir(stateDir)

	if logger == nil {
		logger = zap.NewNop()
	}

	return &Launcher{
		chvBinary:    chvBinary,
		stateDir:     stateDir,
		logDir:       logDir,
		socketDir:    socketDir,
		stateManager: stateManager,
		tapManager:   tapManager,
		isoGenerator: isoGenerator,
		dataDir:      dataDir,
		logger:       logger,
		instances:    make(map[string]*VMInstance),
	}
}

// Initialize prepares the launcher directories.
func (l *Launcher) Initialize() error {
	for _, dir := range []string{l.stateDir, l.logDir, l.socketDir} {
		if err := os.MkdirAll(dir, 0750); err != nil {
			return fmt.Errorf("failed to create directory %s: %w", dir, err)
		}
	}
	return nil
}

// Recover rebuilds the instance map from persisted state.
// Call this on agent startup.
func (l *Launcher) Recover() error {
	states, err := l.stateManager.Recover()
	if err != nil {
		return fmt.Errorf("failed to recover state: %w", err)
	}

	l.mu.Lock()
	defer l.mu.Unlock()

	for vmID, state := range states {
		if state.State == "running" && state.PID > 0 {
			// Verify process is actually running
			process, err := os.FindProcess(state.PID)
			if err != nil {
				// Process not found, mark as stopped
				state.State = "stopped"
				state.PID = 0
				l.stateManager.Save(state)
				continue
			}

			// Try to signal process to verify it's alive
			if err := process.Signal(syscall.Signal(0)); err != nil {
				// Process is dead
				state.State = "stopped"
				state.PID = 0
				l.stateManager.Save(state)
				continue
			}

			// Process exists, create CHV client
			chvClient := NewCHVClient(state.APISocket)

			// Try to ping the VM
			ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
			defer cancel()

			if err := chvClient.Ping(ctx); err != nil {
				// VM API not responding, process might be zombie
				process.Kill()
				state.State = "stopped"
				state.PID = 0
				l.stateManager.Save(state)
				continue
			}

			// VM is actually running
			l.instances[vmID] = &VMInstance{
				VMID:      vmID,
				PID:       state.PID,
				APISocket: state.APISocket,
				Process:   process,
				chvClient: chvClient,
				TAPDevice: &network.TAPDevice{
					Name:       state.TAPDevice,
					Bridge:     "", // Will be filled in if needed
					MACAddress: "",
				},
				ISOPath: state.CloudInitISO,
			}
		}
	}

	return nil
}

// getInstanceRLocked returns an instance while holding a read lock.
// Caller must hold l.mu.RLock() or l.mu.Lock().
func (l *Launcher) getInstanceRLocked(vmID string) *VMInstance {
	return l.instances[vmID]
}

// StartVM starts a new VM.
func (l *Launcher) StartVM(config *VMConfig, operationID string) (*VMInstance, error) {
	// Validate VM ID is safe for path usage
	if err := uuidx.ValidateSafeForPath(config.VMID); err != nil {
		return nil, fmt.Errorf("invalid VM ID: %w", err)
	}

	// Check idempotency
	if l.stateManager.WasOperationPerformed(config.VMID, operationID) {
		// Operation already performed, return existing instance
		l.mu.RLock()
		instance, ok := l.instances[config.VMID]
		l.mu.RUnlock()
		if ok {
			return instance, nil
		}
		// Load from state
		state, err := l.stateManager.Load(config.VMID)
		if err != nil {
			return nil, err
		}
		if state != nil {
			// Recreate instance from state
			return l.recreateInstance(state)
		}
	}

	// Check if VM already running
	l.mu.RLock()
	_, ok := l.instances[config.VMID]
	l.mu.RUnlock()
	if ok {
		return nil, fmt.Errorf("VM %s is already running", config.VMID)
	}

	// Create volume from backing image if needed
	if config.BackingImageID != "" {
		// For MVP-1, we assume backing image is already converted to raw
		// In production, would copy-on-write or rebase here
	}

	// Create TAP device
	tapDevice, err := l.tapManager.CreateTAP(config.VMID, config.BridgeName)
	if err != nil {
		return nil, fmt.Errorf("failed to create TAP device: %w", err)
	}

	// Create or use existing cloud-init ISO
	var isoPath string
	if config.CloudInitISO != "" {
		// Use pre-generated ISO from provisioning
		isoPath = config.CloudInitISO
	} else if config.CloudInit != nil {
		// Generate ISO from config
		var err error
		isoPath, err = l.isoGenerator.GenerateISO(config.VMID, config.CloudInit)
		if err != nil {
			l.tapManager.DeleteTAPByVMID(config.VMID)
			return nil, fmt.Errorf("failed to generate cloud-init ISO: %w", err)
		}
	}

	// Determine API socket path
	apiSocket := config.APIsocket
	if apiSocket == "" {
		apiSocket = filepath.Join(l.socketDir, config.VMID+".sock")
	}

	// Build cloud-hypervisor command
	cmd, err := l.buildCommand(config, tapDevice, isoPath, apiSocket)
	if err != nil {
		l.cleanupOnFailure(config.VMID, tapDevice, isoPath)
		return nil, fmt.Errorf("failed to build command: %w", err)
	}

	// Setup log files
	stdoutLog := filepath.Join(l.logDir, config.VMID+".stdout.log")
	stderrLog := filepath.Join(l.logDir, config.VMID+".stderr.log")

	stdoutFile, err := os.OpenFile(stdoutLog, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0640)
	if err != nil {
		l.cleanupOnFailure(config.VMID, tapDevice, isoPath)
		return nil, fmt.Errorf("failed to open stdout log: %w", err)
	}
	defer stdoutFile.Close()

	stderrFile, err := os.OpenFile(stderrLog, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0640)
	if err != nil {
		l.cleanupOnFailure(config.VMID, tapDevice, isoPath)
		return nil, fmt.Errorf("failed to open stderr log: %w", err)
	}
	defer stderrFile.Close()

	cmd.Stdout = stdoutFile
	cmd.Stderr = stderrFile

	// Start the process
	if err := cmd.Start(); err != nil {
		l.cleanupOnFailure(config.VMID, tapDevice, isoPath)
		return nil, fmt.Errorf("failed to start cloud-hypervisor: %w", err)
	}

	// Create instance
	instance := &VMInstance{
		VMID:      config.VMID,
		PID:       cmd.Process.Pid,
		APISocket: apiSocket,
		TAPDevice: tapDevice,
		ISOPath:   isoPath,
		Process:   cmd.Process,
	}

	// Wait for API socket to appear
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	if err := l.waitForAPISocket(ctx, apiSocket); err != nil {
		// Kill process on failure
		cmd.Process.Kill()
		l.cleanupOnFailure(config.VMID, tapDevice, isoPath)
		return nil, fmt.Errorf("API socket did not appear: %w", err)
	}

	// Create CHV client
	instance.chvClient = NewCHVClient(apiSocket)

	// Wait for VM to reach Running state
	ctx, cancel = context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()

	if err := instance.chvClient.WaitForRunning(ctx, 60*time.Second); err != nil {
		// VM didn't start properly
		cmd.Process.Kill()
		l.cleanupOnFailure(config.VMID, tapDevice, isoPath)
		return nil, fmt.Errorf("VM did not reach running state: %w", err)
	}

	// Save state
	state := &VMInstanceState{
		VMID:            config.VMID,
		PID:             instance.PID,
		APISocket:       apiSocket,
		TAPDevice:       tapDevice.Name,
		VolumePaths:     []string{config.VolumePath},
		CloudInitISO:    isoPath,
		CreatedAt:       time.Now(),
		LastOperationID: operationID,
		State:           "running",
	}

	if err := l.stateManager.Save(state); err != nil {
		// Log but don't fail - VM is running
		l.logger.Warn("Failed to save VM state after start",
			zap.String("vm_id", config.VMID),
			zap.String("operation", "start_vm"),
			zap.Int("pid", instance.PID),
			zap.Error(err))
	}

	// Track in memory
	l.mu.Lock()
	l.instances[config.VMID] = instance
	l.mu.Unlock()

	// Start a goroutine to wait for process exit
	go l.waitForProcessExit(instance)

	return instance, nil
}

// RebootVM reboots a running VM.
func (l *Launcher) RebootVM(vmID string, operationID string) error {
	// Validate VM ID is safe for path usage
	if err := uuidx.ValidateSafeForPath(vmID); err != nil {
		return fmt.Errorf("invalid VM ID: %w", err)
	}

	// Check idempotency
	if l.stateManager.WasOperationPerformed(vmID, operationID) {
		return nil
	}

	l.mu.RLock()
	instance, ok := l.instances[vmID]
	l.mu.RUnlock()
	if !ok {
		// Check if running in persisted state
		state, err := l.stateManager.Load(vmID)
		if err != nil {
			return err
		}
		if state == nil || state.State != "running" {
			return fmt.Errorf("VM %s is not running", vmID)
		}
		var errRecreate error
		instance, errRecreate = l.recreateInstance(state)
		if errRecreate != nil || instance == nil {
			return fmt.Errorf("VM %s is not running", vmID)
		}
		// Re-add to instances map for tracking
		l.mu.Lock()
		l.instances[vmID] = instance
		l.mu.Unlock()
	}

	// Send reboot command via API
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	if err := instance.chvClient.Reboot(ctx); err != nil {
		return fmt.Errorf("failed to reboot VM: %w", err)
	}

	// Update operation ID
	l.stateManager.UpdateLastOperation(vmID, operationID)

	return nil
}

// StopVM stops a running VM.
func (l *Launcher) StopVM(vmID string, force bool, operationID string) error {
	// Validate VM ID is safe for path usage
	if err := uuidx.ValidateSafeForPath(vmID); err != nil {
		return fmt.Errorf("invalid VM ID: %w", err)
	}

	// Check idempotency
	if l.stateManager.WasOperationPerformed(vmID, operationID) {
		return nil // Already stopped
	}

	l.mu.RLock()
	instance, ok := l.instances[vmID]
	l.mu.RUnlock()
	if !ok {
		// Check if stopped but state not cleaned up
		state, err := l.stateManager.Load(vmID)
		if err != nil {
			return err
		}
		if state == nil || state.State == "stopped" {
			return nil // Already stopped
		}
		// Try to load running instance
		var errRecreate error
		instance, errRecreate = l.recreateInstance(state)
		if errRecreate != nil || instance == nil {
			return fmt.Errorf("VM %s is not running", vmID)
		}
		// Re-add to instances map for tracking
		l.mu.Lock()
		l.instances[vmID] = instance
		l.mu.Unlock()
	}

	var err error

	if force {
		// Immediate kill
		err = instance.Process.Kill()
	} else {
		// Graceful shutdown via API
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer cancel()

		err = instance.chvClient.Shutdown(ctx)
		if err != nil {
			// Fallback to SIGTERM
			err = instance.Process.Signal(syscall.SIGTERM)
			if err != nil {
				// Last resort: SIGKILL
				err = instance.Process.Kill()
			}
		}

		// Wait for process to exit
		if err == nil {
			ctx, cancel = context.WithTimeout(context.Background(), 60*time.Second)
			defer cancel()
			instance.chvClient.WaitForStopped(ctx, 60*time.Second)
		}
	}

	// Cleanup resources (using sync.Once to ensure it happens exactly once)
	l.cleanupAfterStop(vmID, instance)

	// Update state
	state, _ := l.stateManager.Load(vmID)
	if state != nil {
		state.State = "stopped"
		state.PID = 0
		state.LastOperationID = operationID
		l.stateManager.Save(state)
	}

	// Remove from tracking
	l.mu.Lock()
	delete(l.instances, vmID)
	l.mu.Unlock()

	return nil
}

// GetVMState returns the current state of a VM.
func (l *Launcher) GetVMState(vmID string) (string, error) {
	// Validate VM ID is safe for path usage
	if err := uuidx.ValidateSafeForPath(vmID); err != nil {
		return "", fmt.Errorf("invalid VM ID: %w", err)
	}

	l.mu.RLock()
	instance, ok := l.instances[vmID]
	l.mu.RUnlock()
	if !ok {
		// Check persisted state
		state, err := l.stateManager.Load(vmID)
		if err != nil {
			return "", err
		}
		if state == nil {
			return "", fmt.Errorf("VM %s not found", vmID)
		}
		return state.State, nil
	}

	// Query via API
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	running, err := instance.chvClient.IsRunning(ctx)
	if err != nil {
		// API error, VM might be in bad state
		return "error", nil
	}

	if running {
		return "running", nil
	}
	return "stopped", nil
}

// GetInstance returns the instance info for a running VM.
func (l *Launcher) GetInstance(vmID string) *VMInstance {
	// Validate VM ID is safe for path usage
	if err := uuidx.ValidateSafeForPath(vmID); err != nil {
		return nil
	}

	l.mu.RLock()
	defer l.mu.RUnlock()
	return l.instances[vmID]
}

// ListInstances returns all running instances.
func (l *Launcher) ListInstances() []*VMInstance {
	l.mu.RLock()
	defer l.mu.RUnlock()

	instances := make([]*VMInstance, 0, len(l.instances))
	for _, instance := range l.instances {
		instances = append(instances, instance)
	}
	return instances
}

// buildCommand builds the cloud-hypervisor command.
func (l *Launcher) buildCommand(config *VMConfig, tapDevice *network.TAPDevice, isoPath string, apiSocket string) (*exec.Cmd, error) {
	args := []string{
		"--cpus", fmt.Sprintf("boot=%d", config.VCPU),
		"--memory", fmt.Sprintf("size=%dM", config.MemoryMB),
	}

	// Build disk list - only include boot volume for now
	// Note: When both boot volume and ISO are attached, the firmware sometimes
	// tries to boot from the ISO instead of the boot volume. For MVP, we only
	// attach the boot volume. Cloud-init can be added later via metadata service.
	if config.VolumePath != "" {
		args = append(args, "--disk", fmt.Sprintf("path=%s", config.VolumePath))
	}

	// Network
	if tapDevice != nil && tapDevice.Name != "" {
		args = append(args, "--net", fmt.Sprintf("tap=%s,mac=%s", tapDevice.Name, tapDevice.MACAddress))
	}

	// For cloud images with a boot disk, use hypervisor-fw firmware
	if config.VolumePath != "" {
		args = append(args, "--firmware", "/usr/local/bin/hypervisor-fw")
	}

	// API socket and console
	args = append(args,
		"--api-socket", apiSocket,
		"--console", "off",
		"--serial", "tty",
	)

	cmd := exec.Command(l.chvBinary, args...)
	return cmd, nil
}

// waitForAPISocket waits for the API socket file to appear.
func (l *Launcher) waitForAPISocket(ctx context.Context, socketPath string) error {
	ticker := time.NewTicker(100 * time.Millisecond)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return fmt.Errorf("timeout waiting for API socket")
		case <-ticker.C:
			if _, err := os.Stat(socketPath); err == nil {
				return nil
			}
		}
	}
}

// waitForProcessExit waits for a VM process to exit and cleans up.
func (l *Launcher) waitForProcessExit(instance *VMInstance) {
	// Wait for process to exit
	_, _ = instance.Process.Wait()

	// Cleanup (using sync.Once to ensure it happens exactly once)
	l.cleanupAfterStop(instance.VMID, instance)

	// Update state
	state, _ := l.stateManager.Load(instance.VMID)
	if state != nil {
		state.State = "stopped"
		state.PID = 0
		l.stateManager.Save(state)
	}

	// Remove from tracking
	l.mu.Lock()
	delete(l.instances, instance.VMID)
	l.mu.Unlock()
}

// cleanupOnFailure cleans up resources when VM start fails.
func (l *Launcher) cleanupOnFailure(vmID string, tapDevice *network.TAPDevice, isoPath string) {
	if tapDevice != nil {
		l.tapManager.DeleteTAP(tapDevice.Name)
	}
	if isoPath != "" {
		l.isoGenerator.DeleteISOByPath(isoPath)
	}
}

// cleanupAfterStop cleans up resources after VM stops.
// Uses sync.Once to ensure cleanup happens exactly once per instance.
func (l *Launcher) cleanupAfterStop(vmID string, instance *VMInstance) {
	instance.cleanupOnce.Do(func() {
		// Delete TAP
		if instance.TAPDevice != nil {
			l.tapManager.DeleteTAP(instance.TAPDevice.Name)
		}

		// Delete cloud-init ISO by path (handles both locations)
		if instance.ISOPath != "" {
			l.isoGenerator.DeleteISOByPath(instance.ISOPath)
		}

		// Delete API socket
		if instance.APISocket != "" {
			os.Remove(instance.APISocket)
		}

		// Delete state file
		l.stateManager.Delete(vmID)
	})
}

// recreateInstance recreates a VMInstance from persisted state.
func (l *Launcher) recreateInstance(state *VMInstanceState) (*VMInstance, error) {
	if state.PID == 0 {
		return nil, fmt.Errorf("VM is not running")
	}

	process, err := os.FindProcess(state.PID)
	if err != nil {
		return nil, fmt.Errorf("process not found: %w", err)
	}

	chvClient := NewCHVClient(state.APISocket)

	return &VMInstance{
		VMID:      state.VMID,
		PID:       state.PID,
		APISocket: state.APISocket,
		TAPDevice: &network.TAPDevice{
			Name: state.TAPDevice,
		},
		ISOPath:   state.CloudInitISO,
		Process:   process,
		chvClient: chvClient,
	}, nil
}
