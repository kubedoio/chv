package manager

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/chv/chv/internal/agent/cloudinit"
	"github.com/chv/chv/internal/hypervisor"
	"github.com/chv/chv/internal/network"
	"github.com/chv/chv/internal/storage"
	"go.uber.org/zap"
)

// setupTestManager creates a VM manager for testing.
func setupTestManager(t *testing.T) (*VMManager, string, func()) {
	// Create temp directories
	tempDir, err := os.MkdirTemp("", "vmmanager-test-*")
	if err != nil {
		t.Fatalf("failed to create temp dir: %v", err)
	}

	stateDir := filepath.Join(tempDir, "state")
	vmDataDir := filepath.Join(tempDir, "vms")
	imagesDir := filepath.Join(tempDir, "images")
	socketDir := filepath.Join(tempDir, "sockets")
	logDir := filepath.Join(tempDir, "logs")
	cloudInitDir := filepath.Join(tempDir, "cloudinit")

	for _, dir := range []string{stateDir, vmDataDir, imagesDir, socketDir, logDir, cloudInitDir} {
		if err := os.MkdirAll(dir, 0755); err != nil {
			os.RemoveAll(tempDir)
			t.Fatalf("failed to create directory %s: %v", dir, err)
		}
	}

	// Create mock/stub dependencies
	storageMgr := storage.NewManager(vmDataDir)
	isoGenerator := cloudinit.NewISOGenerator(cloudInitDir)
	tapManager := network.NewTAPManager("br0", "", "")
	stateManager := hypervisor.NewStateManager(stateDir)

	// Create logger for testing
	logger := zap.NewNop()

	// Create launcher with mock binary (won't actually run VMs)
	launcher := hypervisor.NewLauncher(
		"/usr/bin/cloud-hypervisor", // This won't be executed in unit tests
		stateDir,
		logDir,
		socketDir,
		stateManager,
		tapManager,
		isoGenerator,
		logger.Named("launcher"),
	)

	manager := NewVMManager(
		launcher,
		storageMgr,
		isoGenerator,
		stateDir,
		vmDataDir,
		imagesDir,
		"br0",
		logger.Named("vm_manager"),
	)

	cleanup := func() {
		os.RemoveAll(tempDir)
	}

	return manager, tempDir, cleanup
}

func TestVMStateIsValid(t *testing.T) {
	tests := []struct {
		state VMState
		want  bool
	}{
		{VMStateCreating, true},
		{VMStateRunning, true},
		{VMStateStopped, true},
		{VMStateDeleting, true},
		{VMStateError, true},
		{VMState("invalid"), false},
		{VMState(""), false},
	}

	for _, tt := range tests {
		t.Run(string(tt.state), func(t *testing.T) {
			if got := tt.state.IsValid(); got != tt.want {
				t.Errorf("IsValid() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestVMStateCanTransitionTo(t *testing.T) {
	tests := []struct {
		from   VMState
		to     VMState
		want   bool
	}{
		// From Creating
		{VMStateCreating, VMStateRunning, true},
		{VMStateCreating, VMStateError, true},
		{VMStateCreating, VMStateDeleting, true},
		{VMStateCreating, VMStateStopped, false},

		// From Running
		{VMStateRunning, VMStateStopped, true},
		{VMStateRunning, VMStateError, true},
		{VMStateRunning, VMStateDeleting, true},
		{VMStateRunning, VMStateCreating, false},

		// From Stopped
		{VMStateStopped, VMStateRunning, true},
		{VMStateStopped, VMStateDeleting, true},
		{VMStateStopped, VMStateError, true},
		{VMStateStopped, VMStateCreating, false},

		// From Error
		{VMStateError, VMStateCreating, true},
		{VMStateError, VMStateDeleting, true},
		{VMStateError, VMStateStopped, true},
		{VMStateError, VMStateRunning, false},

		// From Deleting (terminal)
		{VMStateDeleting, VMStateRunning, false},
		{VMStateDeleting, VMStateStopped, false},
		{VMStateDeleting, VMStateError, false},
		{VMStateDeleting, VMStateCreating, false},
	}

	for _, tt := range tests {
		t.Run(string(tt.from)+"->"+string(tt.to), func(t *testing.T) {
			if got := tt.from.CanTransitionTo(tt.to); got != tt.want {
				t.Errorf("CanTransitionTo() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestVMManagerInitialize(t *testing.T) {
	manager, _, cleanup := setupTestManager(t)
	defer cleanup()

	if err := manager.Initialize(); err != nil {
		t.Fatalf("Initialize() failed: %v", err)
	}
}

func TestVMManagerRecordPersistence(t *testing.T) {
	manager, _, cleanup := setupTestManager(t)
	defer cleanup()

	if err := manager.Initialize(); err != nil {
		t.Fatalf("Initialize() failed: %v", err)
	}

	// Create a test record
	record := &VMRecord{
		VMID:      "550e8400-e29b-41d4-a716-446655440000",
		Name:      "test-vm",
		State:     VMStateRunning,
		VCPU:      2,
		MemoryMB:  2048,
		VolumePath: "/tmp/test.raw",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	// Save record
	if err := manager.saveRecord(record); err != nil {
		t.Fatalf("saveRecord() failed: %v", err)
	}

	// Load record
	loaded, err := manager.loadRecordFromDisk(record.VMID)
	if err != nil {
		t.Fatalf("loadRecordFromDisk() failed: %v", err)
	}

	// Verify
	if loaded.VMID != record.VMID {
		t.Errorf("VMID mismatch: got %s, want %s", loaded.VMID, record.VMID)
	}
	if loaded.Name != record.Name {
		t.Errorf("Name mismatch: got %s, want %s", loaded.Name, record.Name)
	}
	if loaded.State != record.State {
		t.Errorf("State mismatch: got %s, want %s", loaded.State, record.State)
	}

	// Delete record
	if err := manager.deleteRecord(record.VMID); err != nil {
		t.Fatalf("deleteRecord() failed: %v", err)
	}

	// Verify deletion
	_, err = manager.loadRecordFromDisk(record.VMID)
	if err == nil {
		t.Error("Expected error after deletion, got nil")
	}
}

func TestVMManagerValidateCreateRequest(t *testing.T) {
	manager, _, cleanup := setupTestManager(t)
	defer cleanup()

	tests := []struct {
		name    string
		req     *CreateVMRequest
		wantErr bool
	}{
		{
			name: "valid with image",
			req: &CreateVMRequest{
				VMID:           "550e8400-e29b-41d4-a716-446655440000",
				Name:           "test-vm",
				VCPU:           2,
				MemoryMB:       2048,
				BackingImageID: "img-123",
			},
			wantErr: false,
		},
		{
			name: "valid with disk size",
			req: &CreateVMRequest{
				VMID:          "550e8400-e29b-41d4-a716-446655440001",
				Name:          "test-vm",
				VCPU:          2,
				MemoryMB:      2048,
				DiskSizeBytes: 10737418240, // 10GB
			},
			wantErr: false,
		},
		{
			name: "missing VMID",
			req: &CreateVMRequest{
				Name:     "test-vm",
				VCPU:     2,
				MemoryMB: 2048,
			},
			wantErr: true,
		},
		{
			name: "missing name",
			req: &CreateVMRequest{
				VMID:     "550e8400-e29b-41d4-a716-446655440002",
				VCPU:     2,
				MemoryMB: 2048,
			},
			wantErr: true,
		},
		{
			name: "invalid VCPU",
			req: &CreateVMRequest{
				VMID:     "550e8400-e29b-41d4-a716-446655440003",
				Name:     "test-vm",
				VCPU:     0,
				MemoryMB: 2048,
			},
			wantErr: true,
		},
		{
			name: "invalid MemoryMB",
			req: &CreateVMRequest{
				VMID:     "550e8400-e29b-41d4-a716-446655440004",
				Name:     "test-vm",
				VCPU:     2,
				MemoryMB: 0,
			},
			wantErr: true,
		},
		{
			name: "no disk or image",
			req: &CreateVMRequest{
				VMID:     "550e8400-e29b-41d4-a716-446655440005",
				Name:     "test-vm",
				VCPU:     2,
				MemoryMB: 2048,
			},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := manager.validateCreateRequest(tt.req)
			if (err != nil) != tt.wantErr {
				t.Errorf("validateCreateRequest() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestVMManagerListVMs(t *testing.T) {
	manager, _, cleanup := setupTestManager(t)
	defer cleanup()

	if err := manager.Initialize(); err != nil {
		t.Fatalf("Initialize() failed: %v", err)
	}

	// Create some test records
	vmIDs := []string{
		"550e8400-e29b-41d4-a716-446655440000",
		"550e8400-e29b-41d4-a716-446655440001",
		"550e8400-e29b-41d4-a716-446655440002",
	}

	for i, vmID := range vmIDs {
		record := &VMRecord{
			VMID:      vmID,
			Name:      "test-vm-" + string(rune('a'+i)),
			State:     VMStateRunning,
			CreatedAt: time.Now(),
			UpdatedAt: time.Now(),
		}
		if err := manager.saveRecord(record); err != nil {
			t.Fatalf("saveRecord() failed: %v", err)
		}
	}

	// List VMs
	records, err := manager.ListVMs()
	if err != nil {
		t.Fatalf("ListVMs() failed: %v", err)
	}

	if len(records) != len(vmIDs) {
		t.Errorf("ListVMs() returned %d records, want %d", len(records), len(vmIDs))
	}
}

func TestVMManagerGetVM(t *testing.T) {
	manager, _, cleanup := setupTestManager(t)
	defer cleanup()

	if err := manager.Initialize(); err != nil {
		t.Fatalf("Initialize() failed: %v", err)
	}

	vmID := "550e8400-e29b-41d4-a716-446655440000"

	// Get non-existent VM
	_, err := manager.GetVM(vmID)
	if err == nil {
		t.Error("Expected error for non-existent VM, got nil")
	}

	// Create a record
	record := &VMRecord{
		VMID:      vmID,
		Name:      "test-vm",
		State:     VMStateRunning,
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	if err := manager.saveRecord(record); err != nil {
		t.Fatalf("saveRecord() failed: %v", err)
	}

	// Get existing VM
	loaded, err := manager.GetVM(vmID)
	if err != nil {
		t.Fatalf("GetVM() failed: %v", err)
	}

	if loaded.VMID != vmID {
		t.Errorf("VMID mismatch: got %s, want %s", loaded.VMID, vmID)
	}
}

func TestVMManagerWasOperationPerformed(t *testing.T) {
	manager, _, cleanup := setupTestManager(t)
	defer cleanup()

	if err := manager.Initialize(); err != nil {
		t.Fatalf("Initialize() failed: %v", err)
	}

	vmID := "550e8400-e29b-41d4-a716-446655440000"
	operationID := "op-12345"

	// No record exists
	if manager.wasOperationPerformed(vmID, operationID) {
		t.Error("wasOperationPerformed() should return false for non-existent VM")
	}

	// Create record
	record := &VMRecord{
		VMID:            vmID,
		Name:            "test-vm",
		State:           VMStateRunning,
		LastOperationID: operationID,
		CreatedAt:       time.Now(),
		UpdatedAt:       time.Now(),
	}
	manager.setRecord(record)

	// Same operation
	if !manager.wasOperationPerformed(vmID, operationID) {
		t.Error("wasOperationPerformed() should return true for same operation")
	}

	// Different operation
	if manager.wasOperationPerformed(vmID, "op-different") {
		t.Error("wasOperationPerformed() should return false for different operation")
	}

	// Empty operation ID
	if manager.wasOperationPerformed(vmID, "") {
		t.Error("wasOperationPerformed() should return false for empty operation ID")
	}
}

func TestVMManagerRecoverState(t *testing.T) {
	manager, tempDir, cleanup := setupTestManager(t)
	defer cleanup()

	if err := manager.Initialize(); err != nil {
		t.Fatalf("Initialize() failed: %v", err)
	}

	// Create a test record with stopped state (simulating a VM that was stopped)
	vmID := "550e8400-e29b-41d4-a716-446655440000"
	record := &VMRecord{
		VMID:      vmID,
		Name:      "test-vm",
		State:     VMStateStopped,
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	if err := manager.saveRecord(record); err != nil {
		t.Fatalf("saveRecord() failed: %v", err)
	}

	// Create new manager instance to test recovery
	stateDir := filepath.Join(tempDir, "state")
	vmDataDir := filepath.Join(tempDir, "vms")
	imagesDir := filepath.Join(tempDir, "images")
	socketDir := filepath.Join(tempDir, "sockets")
	logDir := filepath.Join(tempDir, "logs")
	cloudInitDir := filepath.Join(tempDir, "cloudinit")

	storageMgr := storage.NewManager(vmDataDir)
	isoGenerator := cloudinit.NewISOGenerator(cloudInitDir)
	tapManager := network.NewTAPManager("br0", "", "")
	stateManager := hypervisor.NewStateManager(stateDir)
	testLogger := zap.NewNop()

	launcher := hypervisor.NewLauncher(
		"/usr/bin/cloud-hypervisor",
		stateDir,
		logDir,
		socketDir,
		stateManager,
		tapManager,
		isoGenerator,
		testLogger.Named("launcher"),
	)

	newManager := NewVMManager(
		launcher,
		storageMgr,
		isoGenerator,
		stateDir,
		vmDataDir,
		imagesDir,
		"br0",
		testLogger.Named("vm_manager"),
	)

	// Recover state
	if err := newManager.Initialize(); err != nil {
		t.Fatalf("Initialize() with recovery failed: %v", err)
	}

	// Verify record was recovered
	recovered := newManager.getRecord(vmID)
	if recovered == nil {
		t.Fatal("Record was not recovered")
	}

	if recovered.VMID != vmID {
		t.Errorf("Recovered VMID mismatch: got %s, want %s", recovered.VMID, vmID)
	}

	// State should remain Stopped
	if recovered.State != VMStateStopped {
		t.Errorf("Recovered state mismatch: got %s, want %s", recovered.State, VMStateStopped)
	}
}

func TestVMRecordSerialization(t *testing.T) {
	record := &VMRecord{
		VMID:            "550e8400-e29b-41d4-a716-446655440000",
		Name:            "test-vm",
		State:           VMStateRunning,
		VCPU:            4,
		MemoryMB:        8192,
		VolumePath:      "/data/vms/test.raw",
		CloudInitISO:    "/data/cloudinit/test.iso",
		BackingImageID:  "img-ubuntu-2204",
		BridgeName:      "br0",
		LastError:       "",
		CreatedAt:       time.Date(2024, 1, 1, 0, 0, 0, 0, time.UTC),
		UpdatedAt:       time.Date(2024, 1, 2, 0, 0, 0, 0, time.UTC),
		LastOperationID: "op-create-123",
	}

	// Serialize
	data, err := json.Marshal(record)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	// Deserialize
	var loaded VMRecord
	if err := json.Unmarshal(data, &loaded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}

	// Verify
	if loaded.VMID != record.VMID {
		t.Errorf("VMID mismatch")
	}
	if loaded.Name != record.Name {
		t.Errorf("Name mismatch")
	}
	if loaded.State != record.State {
		t.Errorf("State mismatch")
	}
	if loaded.VCPU != record.VCPU {
		t.Errorf("VCPU mismatch")
	}
	if loaded.MemoryMB != record.MemoryMB {
		t.Errorf("MemoryMB mismatch")
	}
	if loaded.VolumePath != record.VolumePath {
		t.Errorf("VolumePath mismatch")
	}
	if loaded.CloudInitISO != record.CloudInitISO {
		t.Errorf("CloudInitISO mismatch")
	}
	if loaded.BackingImageID != record.BackingImageID {
		t.Errorf("BackingImageID mismatch")
	}
	if loaded.BridgeName != record.BridgeName {
		t.Errorf("BridgeName mismatch")
	}
	if loaded.LastOperationID != record.LastOperationID {
		t.Errorf("LastOperationID mismatch")
	}
}
