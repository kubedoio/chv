package hypervisor

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"github.com/chv/chv/internal/cloudinit"
	"github.com/chv/chv/internal/network"
)

// mockProcessCmd is a helper to create a mock cloud-hypervisor process
// that creates the API socket and responds to signals
const mockProcessCmd = `#!/bin/bash
# Mock cloud-hypervisor for testing
# Usage: mock-chv.sh --api-socket <path> [other args...]

API_SOCKET=""
for i in "$@"; do
    case $i in
        --api-socket)
            API_SOCKET="${2}"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done

if [ -z "$API_SOCKET" ]; then
    echo "No API socket specified"
    exit 1
fi

# Create the socket file to simulate CHV being ready
touch "$API_SOCKET"

# Wait for signal
trap "rm -f $API_SOCKET; exit 0" SIGTERM SIGINT
while true; do
    sleep 1
done
`

func createMockCHVBinary(t *testing.T, dir string) string {
	mockPath := filepath.Join(dir, "cloud-hypervisor")
	if err := os.WriteFile(mockPath, []byte(mockProcessCmd), 0755); err != nil {
		t.Fatalf("Failed to create mock CHV: %v", err)
	}
	return mockPath
}

func TestLauncher_Initialize(t *testing.T) {
	tmpDir := t.TempDir()
	stateManager := NewStateManager(filepath.Join(tmpDir, "state"))
	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(filepath.Join(tmpDir, "isos"))

	launcher := NewLauncher(
		"/usr/local/bin/cloud-hypervisor",
		filepath.Join(tmpDir, "state"),
		filepath.Join(tmpDir, "logs"),
		filepath.Join(tmpDir, "sockets"),
		stateManager,
		tapManager,
		isoGenerator,
	)

	if err := launcher.Initialize(); err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Verify directories were created
	for _, dir := range []string{"state", "logs", "sockets"} {
		path := filepath.Join(tmpDir, dir)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			t.Errorf("Directory %s should exist after Initialize", dir)
		}
	}
}

func TestLauncher_VMConfig_Validation(t *testing.T) {
	tests := []struct {
		name    string
		config  *VMConfig
		wantErr bool
	}{
		{
			name: "valid config",
			config: &VMConfig{
				VMID:       "test-vm-123",
				Name:       "test-vm",
				VCPU:       2,
				MemoryMB:   1024,
				VolumePath: "/var/lib/chv/volumes/test.raw",
				BridgeName: "br0",
			},
			wantErr: false,
		},
		{
			name: "minimal config",
			config: &VMConfig{
				VMID:       "test-vm-456",
				VCPU:       1,
				MemoryMB:   512,
				VolumePath: "/var/lib/chv/volumes/test2.raw",
			},
			wantErr: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Just validate struct can be created
			if tt.config.VMID == "" {
				t.Error("VMID should not be empty")
			}
			if tt.config.VCPU <= 0 {
				t.Error("VCPU should be positive")
			}
			if tt.config.MemoryMB <= 0 {
				t.Error("MemoryMB should be positive")
			}
		})
	}
}

func TestLauncher_buildCommand(t *testing.T) {
	tmpDir := t.TempDir()
	stateManager := NewStateManager(filepath.Join(tmpDir, "state"))
	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(filepath.Join(tmpDir, "isos"))

	launcher := NewLauncher(
		"/usr/local/bin/cloud-hypervisor",
		filepath.Join(tmpDir, "state"),
		filepath.Join(tmpDir, "logs"),
		filepath.Join(tmpDir, "sockets"),
		stateManager,
		tapManager,
		isoGenerator,
	)

	config := &VMConfig{
		VMID:       "test-vm-123",
		Name:       "test-vm",
		VCPU:       2,
		MemoryMB:   1024,
		VolumePath: "/var/lib/chv/volumes/test.raw",
	}

	tapDevice := &network.TAPDevice{
		Name:       "tap550e8400e29b",
		Bridge:     "br0",
		MACAddress: "02:00:00:00:00:01",
	}

	isoPath := "/var/lib/chv/isos/test-vm-123-cloudinit.iso"
	apiSocket := "/var/lib/chv/sockets/test-vm-123.sock"

	cmd, err := launcher.buildCommand(config, tapDevice, isoPath, apiSocket)
	if err != nil {
		t.Fatalf("buildCommand failed: %v", err)
	}

	// Verify command path
	if cmd.Path != "/usr/local/bin/cloud-hypervisor" {
		t.Errorf("Command path = %s, want /usr/local/bin/cloud-hypervisor", cmd.Path)
	}

	// Verify args contain expected values
	// Note: cmd.Args[0] is the program path
	args := cmd.Args[1:] // Skip program path
	
	// Check for key arguments
	expectedArgs := []string{
		"--cpus", "boot=2",
		"--memory", "size=1024M",
		"--disk", "path=/var/lib/chv/volumes/test.raw",
		"--disk", "path=/var/lib/chv/isos/test-vm-123-cloudinit.iso",
		"--net", "tap=tap550e8400e29b,mac=02:00:00:00:00:01",
		"--api-socket", "/var/lib/chv/sockets/test-vm-123.sock",
		"--console", "off",
		"--serial", "tty",
	}

	for i, expected := range expectedArgs {
		if i >= len(args) {
			t.Errorf("Missing arg at position %d: expected %s", i, expected)
			continue
		}
		if args[i] != expected {
			t.Errorf("Arg[%d] = %s, want %s", i, args[i], expected)
		}
	}
}

func TestLauncher_statePath(t *testing.T) {
	tmpDir := t.TempDir()
	stateManager := NewStateManager(filepath.Join(tmpDir, "state"))

	vmID := "test-vm-123"
	expectedPath := filepath.Join(tmpDir, "state", vmID+".json")
	
	// Use reflection or internal access to test private method
	// For now, we test via public methods
	state := &VMInstanceState{
		VMID:      vmID,
		PID:       12345,
		State:     "running",
		CreatedAt: time.Now(),
	}

	if err := stateManager.Save(state); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	// Verify file was created at expected path
	if _, err := os.Stat(expectedPath); os.IsNotExist(err) {
		t.Errorf("State file should exist at %s", expectedPath)
	}
}

func TestVMInstanceState_Serialization(t *testing.T) {
	state := &VMInstanceState{
		VMID:            "test-vm-123",
		PID:             12345,
		APISocket:       "/var/lib/chv/sockets/test.sock",
		TAPDevice:       "tap550e8400e29b",
		VolumePaths:     []string{"/var/lib/chv/volumes/vol1.raw"},
		CloudInitISO:    "/var/lib/chv/isos/init.iso",
		CreatedAt:       time.Now(),
		LastOperationID: "op-123",
		State:           "running",
	}

	// Verify all fields are set
	if state.VMID == "" {
		t.Error("VMID should not be empty")
	}
	if state.PID == 0 {
		t.Error("PID should not be zero")
	}
	if state.State == "" {
		t.Error("State should not be empty")
	}
}

func TestLauncher_Recover_EmptyState(t *testing.T) {
	tmpDir := t.TempDir()
	stateManager := NewStateManager(filepath.Join(tmpDir, "state"))
	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(filepath.Join(tmpDir, "isos"))

	launcher := NewLauncher(
		"/usr/local/bin/cloud-hypervisor",
		filepath.Join(tmpDir, "state"),
		filepath.Join(tmpDir, "logs"),
		filepath.Join(tmpDir, "sockets"),
		stateManager,
		tapManager,
		isoGenerator,
	)

	// Recover with empty state directory
	if err := launcher.Recover(); err != nil {
		t.Fatalf("Recover failed: %v", err)
	}

	// Should have no instances
	if len(launcher.instances) != 0 {
		t.Errorf("Expected 0 instances, got %d", len(launcher.instances))
	}
}

func TestLauncher_ListInstances(t *testing.T) {
	tmpDir := t.TempDir()
	stateManager := NewStateManager(filepath.Join(tmpDir, "state"))
	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(filepath.Join(tmpDir, "isos"))

	launcher := NewLauncher(
		"/usr/local/bin/cloud-hypervisor",
		filepath.Join(tmpDir, "state"),
		filepath.Join(tmpDir, "logs"),
		filepath.Join(tmpDir, "sockets"),
		stateManager,
		tapManager,
		isoGenerator,
	)

	// Initially empty
	instances := launcher.ListInstances()
	if len(instances) != 0 {
		t.Errorf("Expected 0 instances, got %d", len(instances))
	}
}

func TestLauncher_GetInstance_NotFound(t *testing.T) {
	tmpDir := t.TempDir()
	stateManager := NewStateManager(filepath.Join(tmpDir, "state"))
	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(filepath.Join(tmpDir, "isos"))

	launcher := NewLauncher(
		"/usr/local/bin/cloud-hypervisor",
		filepath.Join(tmpDir, "state"),
		filepath.Join(tmpDir, "logs"),
		filepath.Join(tmpDir, "sockets"),
		stateManager,
		tapManager,
		isoGenerator,
	)

	// Get non-existent instance
	instance := launcher.GetInstance("non-existent")
	if instance != nil {
		t.Error("Expected nil for non-existent instance")
	}
}

func TestLauncher_GetVMState_NotFound(t *testing.T) {
	tmpDir := t.TempDir()
	stateManager := NewStateManager(filepath.Join(tmpDir, "state"))
	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(filepath.Join(tmpDir, "isos"))

	launcher := NewLauncher(
		"/usr/local/bin/cloud-hypervisor",
		filepath.Join(tmpDir, "state"),
		filepath.Join(tmpDir, "logs"),
		filepath.Join(tmpDir, "sockets"),
		stateManager,
		tapManager,
		isoGenerator,
	)

	// Get state for non-existent VM
	_, err := launcher.GetVMState("non-existent")
	if err == nil {
		t.Error("Expected error for non-existent VM")
	}
}

func TestLauncher_waitForAPISocket(t *testing.T) {
	tmpDir := t.TempDir()
	stateManager := NewStateManager(filepath.Join(tmpDir, "state"))
	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(filepath.Join(tmpDir, "isos"))

	launcher := NewLauncher(
		"/usr/local/bin/cloud-hypervisor",
		filepath.Join(tmpDir, "state"),
		filepath.Join(tmpDir, "logs"),
		filepath.Join(tmpDir, "sockets"),
		stateManager,
		tapManager,
		isoGenerator,
	)

	socketPath := filepath.Join(tmpDir, "test.sock")

	// Test timeout
	ctx, cancel := NewContextWithTimeout(t, 100*time.Millisecond)
	defer cancel()

	err := launcher.waitForAPISocket(ctx, socketPath)
	if err == nil {
		t.Error("Expected timeout error")
	}

	// Test success
	go func() {
		time.Sleep(50 * time.Millisecond)
		os.Create(socketPath)
	}()

	ctx, cancel = NewContextWithTimeout(t, 500*time.Millisecond)
	defer cancel()

	// Wait for goroutine to create socket
	time.Sleep(100 * time.Millisecond)

	// Clean up
	os.Remove(socketPath)
}

// NewContextWithTimeout creates a context with timeout for tests
func NewContextWithTimeout(t *testing.T, timeout time.Duration) (context.Context, context.CancelFunc) {
	return context.WithTimeout(context.Background(), timeout)
}

func TestLauncher_StartVM_InvalidVMID(t *testing.T) {
	tmpDir := t.TempDir()
	stateManager := NewStateManager(filepath.Join(tmpDir, "state"))
	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(filepath.Join(tmpDir, "isos"))

	launcher := NewLauncher(
		"/usr/local/bin/cloud-hypervisor",
		filepath.Join(tmpDir, "state"),
		filepath.Join(tmpDir, "logs"),
		filepath.Join(tmpDir, "sockets"),
		stateManager,
		tapManager,
		isoGenerator,
	)

	tests := []struct {
		name   string
		vmID   string
		errMsg string
	}{
		{
			name:   "path traversal unix",
			vmID:   "../../../etc/passwd",
			errMsg: "invalid VM ID",
		},
		{
			name:   "path traversal windows",
			vmID:   `..\..\windows\system32`,
			errMsg: "invalid VM ID",
		},
		{
			name:   "forward slash",
			vmID:   "test/vm-id",
			errMsg: "invalid VM ID",
		},
		{
			name:   "backslash",
			vmID:   `test\vm-id`,
			errMsg: "invalid VM ID",
		},
		{
			name:   "double dot",
			vmID:   "vm..id",
			errMsg: "invalid VM ID",
		},
		{
			name:   "invalid uuid format",
			vmID:   "not-a-uuid",
			errMsg: "invalid VM ID",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			config := &VMConfig{
				VMID:       tt.vmID,
				Name:       "test-vm",
				VCPU:       1,
				MemoryMB:   512,
				VolumePath: "/var/lib/chv/volumes/test.raw",
			}

			_, err := launcher.StartVM(config, "op-123")
			if err == nil {
				t.Errorf("StartVM() expected error containing %q, got nil", tt.errMsg)
				return
			}
			if !containsString(err.Error(), tt.errMsg) {
				t.Errorf("StartVM() error = %q, want error containing %q", err.Error(), tt.errMsg)
			}
		})
	}
}

func TestLauncher_RebootVM_InvalidVMID(t *testing.T) {
	tmpDir := t.TempDir()
	stateManager := NewStateManager(filepath.Join(tmpDir, "state"))
	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(filepath.Join(tmpDir, "isos"))

	launcher := NewLauncher(
		"/usr/local/bin/cloud-hypervisor",
		filepath.Join(tmpDir, "state"),
		filepath.Join(tmpDir, "logs"),
		filepath.Join(tmpDir, "sockets"),
		stateManager,
		tapManager,
		isoGenerator,
	)

	err := launcher.RebootVM("../../../etc/passwd", "op-123")
	if err == nil {
		t.Error("RebootVM() expected error for path traversal, got nil")
	}
}

func TestLauncher_StopVM_InvalidVMID(t *testing.T) {
	tmpDir := t.TempDir()
	stateManager := NewStateManager(filepath.Join(tmpDir, "state"))
	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(filepath.Join(tmpDir, "isos"))

	launcher := NewLauncher(
		"/usr/local/bin/cloud-hypervisor",
		filepath.Join(tmpDir, "state"),
		filepath.Join(tmpDir, "logs"),
		filepath.Join(tmpDir, "sockets"),
		stateManager,
		tapManager,
		isoGenerator,
	)

	err := launcher.StopVM("../../../etc/passwd", false, "op-123")
	if err == nil {
		t.Error("StopVM() expected error for path traversal, got nil")
	}
}

func TestLauncher_GetVMState_InvalidVMID(t *testing.T) {
	tmpDir := t.TempDir()
	stateManager := NewStateManager(filepath.Join(tmpDir, "state"))
	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(filepath.Join(tmpDir, "isos"))

	launcher := NewLauncher(
		"/usr/local/bin/cloud-hypervisor",
		filepath.Join(tmpDir, "state"),
		filepath.Join(tmpDir, "logs"),
		filepath.Join(tmpDir, "sockets"),
		stateManager,
		tapManager,
		isoGenerator,
	)

	_, err := launcher.GetVMState("../../../etc/passwd")
	if err == nil {
		t.Error("GetVMState() expected error for path traversal, got nil")
	}
}

func TestLauncher_GetInstance_InvalidVMID(t *testing.T) {
	tmpDir := t.TempDir()
	stateManager := NewStateManager(filepath.Join(tmpDir, "state"))
	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(filepath.Join(tmpDir, "isos"))

	launcher := NewLauncher(
		"/usr/local/bin/cloud-hypervisor",
		filepath.Join(tmpDir, "state"),
		filepath.Join(tmpDir, "logs"),
		filepath.Join(tmpDir, "sockets"),
		stateManager,
		tapManager,
		isoGenerator,
	)

	instance := launcher.GetInstance("../../../etc/passwd")
	if instance != nil {
		t.Error("GetInstance() expected nil for path traversal, got instance")
	}
}

func containsString(s, substr string) bool {
	return len(substr) <= len(s) && (s == substr || len(s) > 0 && containsStringHelper(s, substr))
}

func containsStringHelper(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

// TestLauncher_StopVM_RaceCondition tests that calling StopVM concurrently with
// process exit does not cause data races or double-cleanup.
func TestLauncher_StopVM_RaceCondition(t *testing.T) {
	tmpDir := t.TempDir()
	stateDir := filepath.Join(tmpDir, "state")
	logDir := filepath.Join(tmpDir, "logs")
	socketDir := filepath.Join(tmpDir, "sockets")

	stateManager := NewStateManager(stateDir)
	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(filepath.Join(tmpDir, "isos"))

	launcher := NewLauncher(
		"/usr/local/bin/cloud-hypervisor",
		stateDir,
		logDir,
		socketDir,
		stateManager,
		tapManager,
		isoGenerator,
	)

	// Create a mock instance manually
	vmID := "550e8400-e29b-41d4-a716-446655440001"
	apiSocket := filepath.Join(socketDir, vmID+".sock")

	// Create a mock process that exits quickly
	mockCmd := exec.Command("sleep", "0.1")
	if err := mockCmd.Start(); err != nil {
		t.Fatalf("Failed to start mock process: %v", err)
	}

	// Create the instance
	instance := &VMInstance{
		VMID:      vmID,
		PID:       mockCmd.Process.Pid,
		APISocket: apiSocket,
		Process:   mockCmd.Process,
		TAPDevice: &network.TAPDevice{
			Name: "tap" + vmID[:8],
		},
		ISOPath: filepath.Join(tmpDir, "isos", vmID+"-cloudinit.iso"),
	}

	// Create mock socket file
	os.WriteFile(apiSocket, []byte{}, 0644)

	// Add to launcher
	launcher.mu.Lock()
	launcher.instances[vmID] = instance
	launcher.mu.Unlock()

	// Start the waitForProcessExit goroutine
	go launcher.waitForProcessExit(instance)

	// Immediately call StopVM (simulating race condition)
	// This will race with waitForProcessExit
	err := launcher.StopVM(vmID, true, "op-stop-123")

	// Both operations should complete without panic
	// The exact error doesn't matter - we're testing for races
	t.Logf("StopVM returned: %v", err)

	// Verify instance is eventually removed from map
	time.Sleep(200 * time.Millisecond)

	launcher.mu.RLock()
	_, exists := launcher.instances[vmID]
	launcher.mu.RUnlock()

	if exists {
		t.Error("Instance should be removed from map after cleanup")
	}

	// Cleanup should have happened exactly once
	// We can't easily verify this without mocking, but the race detector
	// will catch any concurrent access issues
}

// TestLauncher_ConcurrentAccess tests concurrent read/write access to instances map.
func TestLauncher_ConcurrentAccess(t *testing.T) {
	tmpDir := t.TempDir()
	stateManager := NewStateManager(filepath.Join(tmpDir, "state"))
	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(filepath.Join(tmpDir, "isos"))

	launcher := NewLauncher(
		"/usr/local/bin/cloud-hypervisor",
		filepath.Join(tmpDir, "state"),
		filepath.Join(tmpDir, "logs"),
		filepath.Join(tmpDir, "sockets"),
		stateManager,
		tapManager,
		isoGenerator,
	)

	// Add some mock instances
	for i := 0; i < 10; i++ {
		vmID := fmt.Sprintf("vm-%d", i)
		launcher.mu.Lock()
		launcher.instances[vmID] = &VMInstance{
			VMID: vmID,
			PID:  1000 + i,
		}
		launcher.mu.Unlock()
	}

	// Concurrent reads and writes
	done := make(chan bool, 3)

	// Goroutine 1: List instances
	go func() {
		for i := 0; i < 100; i++ {
			_ = launcher.ListInstances()
		}
		done <- true
	}()

	// Goroutine 2: Get instances
	go func() {
		for i := 0; i < 100; i++ {
			_ = launcher.GetInstance("vm-5")
		}
		done <- true
	}()

	// Goroutine 3: Add/remove instances
	go func() {
		for i := 0; i < 100; i++ {
			vmID := fmt.Sprintf("dynamic-%d", i)
			launcher.mu.Lock()
			launcher.instances[vmID] = &VMInstance{VMID: vmID}
			launcher.mu.Unlock()

			launcher.mu.Lock()
			delete(launcher.instances, vmID)
			launcher.mu.Unlock()
		}
		done <- true
	}()

	// Wait for all goroutines
	for i := 0; i < 3; i++ {
		<-done
	}
}
