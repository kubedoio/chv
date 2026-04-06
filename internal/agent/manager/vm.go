// Package manager provides VM lifecycle management for the agent.
package manager

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/chv/chv/internal/agent/cloudinit"
	"github.com/chv/chv/internal/hypervisor"
	"github.com/chv/chv/internal/storage"
	"github.com/chv/chv/internal/validation"
	"github.com/chv/chv/pkg/uuidx"
)

// VMState represents the state of a VM in the lifecycle state machine.
type VMState string

const (
	// VMStateCreating indicates the VM is being created.
	VMStateCreating VMState = "creating"
	// VMStateRunning indicates the VM is running.
	VMStateRunning VMState = "running"
	// VMStateStopped indicates the VM is stopped.
	VMStateStopped VMState = "stopped"
	// VMStateDeleting indicates the VM is being deleted.
	VMStateDeleting VMState = "deleting"
	// VMStateError indicates the VM is in an error state.
	VMStateError VMState = "error"
)

// IsValid returns true if the state is valid.
func (s VMState) IsValid() bool {
	switch s {
	case VMStateCreating, VMStateRunning, VMStateStopped, VMStateDeleting, VMStateError:
		return true
	}
	return false
}

// CanTransitionTo returns true if the VM can transition to the target state.
func (s VMState) CanTransitionTo(target VMState) bool {
	transitions := map[VMState][]VMState{
		VMStateCreating: {VMStateRunning, VMStateError, VMStateDeleting},
		VMStateRunning:  {VMStateStopped, VMStateError, VMStateDeleting},
		VMStateStopped:  {VMStateRunning, VMStateDeleting, VMStateError},
		VMStateError:    {VMStateCreating, VMStateDeleting, VMStateStopped},
		VMStateDeleting: {}, // Terminal state, no transitions out
	}

	allowed, ok := transitions[s]
	if !ok {
		return false
	}

	for _, state := range allowed {
		if state == target {
			return true
		}
	}
	return false
}

// VMRecord represents a VM's persistent state record.
type VMRecord struct {
	VMID            string    `json:"vm_id"`
	Name            string    `json:"name"`
	State           VMState   `json:"state"`
	VCPU            int       `json:"vcpu"`
	MemoryMB        int       `json:"memory_mb"`
	VolumePath      string    `json:"volume_path"`
	CloudInitISO    string    `json:"cloudinit_iso"`
	BackingImageID  string    `json:"backing_image_id,omitempty"`
	BridgeName      string    `json:"bridge_name"`
	LastError       string    `json:"last_error,omitempty"`
	CreatedAt       time.Time `json:"created_at"`
	UpdatedAt       time.Time `json:"updated_at"`
	LastOperationID string    `json:"last_operation_id,omitempty"`
}

// VMManager orchestrates VM lifecycle operations.
type VMManager struct {
	// Dependencies
	launcher     *hypervisor.Launcher
	storageMgr   *storage.Manager
	isoGenerator *cloudinit.ISOGenerator

	// Configuration
	stateDir      string
	vmDataDir     string
	imagesDir     string
	defaultBridge string

	// State management
	records map[string]*VMRecord
	mu      sync.RWMutex
}

// NewVMManager creates a new VM manager.
func NewVMManager(
	launcher *hypervisor.Launcher,
	storageMgr *storage.Manager,
	isoGenerator *cloudinit.ISOGenerator,
	stateDir string,
	vmDataDir string,
	imagesDir string,
	defaultBridge string,
) *VMManager {
	return &VMManager{
		launcher:      launcher,
		storageMgr:    storageMgr,
		isoGenerator:  isoGenerator,
		stateDir:      stateDir,
		vmDataDir:     vmDataDir,
		imagesDir:     imagesDir,
		defaultBridge: defaultBridge,
		records:       make(map[string]*VMRecord),
	}
}

// Initialize prepares the VM manager directories and recovers state.
func (m *VMManager) Initialize() error {
	// Create necessary directories
	for _, dir := range []string{m.stateDir, m.vmDataDir, m.imagesDir} {
		if err := os.MkdirAll(dir, 0750); err != nil {
			return fmt.Errorf("failed to create directory %s: %w", dir, err)
		}
	}

	// Recover state from disk
	if err := m.recoverState(); err != nil {
		return fmt.Errorf("failed to recover VM state: %w", err)
	}

	return nil
}

// CreateVMRequest contains parameters for creating a VM.
type CreateVMRequest struct {
	VMID           string
	Name           string
	VCPU           int
	MemoryMB       int
	DiskSizeBytes  int64
	BackingImageID string
	BridgeName     string
	CloudInit      *cloudinit.Config
	OperationID    string
}

// CreateVM creates a new VM with full lifecycle orchestration.
// This implements the Create VM Flow:
// 1. Validate VM spec
// 2. Allocate disk space in storage pool
// 3. Copy/convert image to disk (qcow2 → raw)
// 4. Generate cloud-init ISO
// 5. Create CH VM config
// 6. Start CH process (if not running)
// 7. Boot VM
func (m *VMManager) CreateVM(ctx context.Context, req *CreateVMRequest) (*VMRecord, error) {
	// Validate VM ID
	if err := uuidx.ValidateSafeForPath(req.VMID); err != nil {
		return nil, fmt.Errorf("invalid VM ID: %w", err)
	}

	// Check idempotency
	if m.wasOperationPerformed(req.VMID, req.OperationID) {
		record := m.getRecord(req.VMID)
		if record != nil {
			return record, nil
		}
	}

	// Check if VM already exists
	if record := m.getRecord(req.VMID); record != nil {
		if record.State == VMStateRunning || record.State == VMStateStopped {
			return nil, fmt.Errorf("VM %s already exists", req.VMID)
		}
	}

	// Step 1: Validate VM spec
	if err := m.validateCreateRequest(req); err != nil {
		return nil, fmt.Errorf("validation failed: %w", err)
	}

	// Create initial record
	record := &VMRecord{
		VMID:            req.VMID,
		Name:            req.Name,
		State:           VMStateCreating,
		VCPU:            req.VCPU,
		MemoryMB:        req.MemoryMB,
		BackingImageID:  req.BackingImageID,
		BridgeName:      req.BridgeName,
		CreatedAt:       time.Now(),
		UpdatedAt:       time.Now(),
		LastOperationID: req.OperationID,
	}

	// Save initial state
	if err := m.saveRecord(record); err != nil {
		return nil, fmt.Errorf("failed to save initial state: %w", err)
	}
	m.setRecord(record)

	// Determine bridge
	bridgeName := req.BridgeName
	if bridgeName == "" {
		bridgeName = m.defaultBridge
	}

	// Step 2 & 3: Prepare VM disk
	volumePath, err := m.prepareVMDisk(req)
	if err != nil {
		record.State = VMStateError
		record.LastError = fmt.Sprintf("disk preparation failed: %v", err)
		record.UpdatedAt = time.Now()
		m.saveRecord(record)
		return nil, fmt.Errorf("disk preparation failed: %w", err)
	}
	record.VolumePath = volumePath

	// Step 4: Generate cloud-init ISO
	var isoPath string
	if req.CloudInit != nil {
		isoPath, err = m.isoGenerator.GenerateISO(req.VMID, req.CloudInit)
		if err != nil {
			m.cleanupVMDisk(volumePath)
			record.State = VMStateError
			record.LastError = fmt.Sprintf("cloud-init generation failed: %v", err)
			record.UpdatedAt = time.Now()
			m.saveRecord(record)
			return nil, fmt.Errorf("cloud-init generation failed: %w", err)
		}
		record.CloudInitISO = isoPath
	}

	// Step 5 & 6: Create CH VM config and start
	vmConfig := &hypervisor.VMConfig{
		VMID:           req.VMID,
		Name:           req.Name,
		VCPU:           req.VCPU,
		MemoryMB:       req.MemoryMB,
		VolumePath:     volumePath,
		BackingImageID: req.BackingImageID,
		BridgeName:     bridgeName,
		CloudInit:      req.CloudInit,
		CloudInitISO:   isoPath,
	}

	// Step 7: Start the VM (this boots it)
	_, err = m.launcher.StartVM(vmConfig, req.OperationID)
	if err != nil {
		// Cleanup on failure
		m.cleanupVMDisk(volumePath)
		if isoPath != "" {
			m.isoGenerator.DeleteISO(req.VMID)
		}
		record.State = VMStateError
		record.LastError = fmt.Sprintf("VM start failed: %v", err)
		record.UpdatedAt = time.Now()
		m.saveRecord(record)
		return nil, fmt.Errorf("VM start failed: %w", err)
	}

	// Update record to running state
	record.State = VMStateRunning
	record.UpdatedAt = time.Now()
	if err := m.saveRecord(record); err != nil {
		// Log but don't fail - VM is running
		// TODO: Log warning
	}
	m.setRecord(record)

	return record, nil
}

// DeleteVMRequest contains parameters for deleting a VM.
type DeleteVMRequest struct {
	VMID        string
	Force       bool
	OperationID string
}

// DeleteVM deletes a VM with full lifecycle orchestration.
// This implements the Delete VM Flow:
// 1. Graceful shutdown
// 2. Wait for shutdown or force stop
// 3. Delete CH VM
// 4. Delete disk files
// 5. Cleanup cloud-init ISO
func (m *VMManager) DeleteVM(ctx context.Context, req *DeleteVMRequest) error {
	// Validate VM ID
	if err := uuidx.ValidateSafeForPath(req.VMID); err != nil {
		return fmt.Errorf("invalid VM ID: %w", err)
	}

	// Check idempotency
	if m.wasOperationPerformed(req.VMID, req.OperationID) {
		return nil
	}

	// Get VM record
	record := m.getRecord(req.VMID)
	if record == nil {
		// Check if there's a state file on disk
		diskRecord, err := m.loadRecordFromDisk(req.VMID)
		if err != nil {
			return fmt.Errorf("VM not found: %s", req.VMID)
		}
		record = diskRecord
		m.setRecord(record)
	}

	// Check if already deleted
	if record.State == VMStateDeleting {
		// Already being deleted, check if we need to finish cleanup
		return m.finishDeleteVM(req.VMID, record)
	}

	// Validate state transition
	if !record.State.CanTransitionTo(VMStateDeleting) {
		return fmt.Errorf("cannot delete VM in state %s", record.State)
	}

	// Update state to deleting
	record.State = VMStateDeleting
	record.UpdatedAt = time.Now()
	record.LastOperationID = req.OperationID
	if err := m.saveRecord(record); err != nil {
		return fmt.Errorf("failed to save state: %w", err)
	}

	// Step 1-3: Stop the VM (graceful or force)
	if record.State == VMStateRunning || m.isVMRunning(req.VMID) {
		err := m.launcher.StopVM(req.VMID, req.Force, req.OperationID)
		if err != nil {
			// If force is not set and graceful shutdown failed, return error
			if !req.Force {
				record.State = VMStateError
				record.LastError = fmt.Sprintf("shutdown failed: %v", err)
				record.UpdatedAt = time.Now()
				m.saveRecord(record)
				return fmt.Errorf("graceful shutdown failed (use force=true to force stop): %w", err)
			}
			// Force stop failed - continue with cleanup anyway
		}
	}

	// Steps 4-5: Clean up resources
	return m.finishDeleteVM(req.VMID, record)
}

// finishDeleteVM completes the deletion process.
func (m *VMManager) finishDeleteVM(vmID string, record *VMRecord) error {
	// Step 4: Delete disk files
	if record.VolumePath != "" {
		if err := m.cleanupVMDisk(record.VolumePath); err != nil {
			// Log but continue - disk might already be deleted
			// TODO: Log warning
		}
	}

	// Step 5: Cleanup cloud-init ISO
	if record.CloudInitISO != "" {
		if err := m.isoGenerator.DeleteISO(vmID); err != nil {
			// Log but continue
			// TODO: Log warning
		}
	}

	// Remove state file
	if err := m.deleteRecord(vmID); err != nil {
		// Log but don't fail
		// TODO: Log warning
	}

	// Remove from memory
	m.deleteRecordFromMemory(vmID)

	return nil
}

// StopVMRequest contains parameters for stopping a VM.
type StopVMRequest struct {
	VMID        string
	Force       bool
	OperationID string
}

// StopVM stops a running VM.
func (m *VMManager) StopVM(ctx context.Context, req *StopVMRequest) error {
	// Validate VM ID
	if err := uuidx.ValidateSafeForPath(req.VMID); err != nil {
		return fmt.Errorf("invalid VM ID: %w", err)
	}

	// Check idempotency
	if m.wasOperationPerformed(req.VMID, req.OperationID) {
		return nil
	}

	// Get VM record
	record := m.getRecord(req.VMID)
	if record == nil {
		return fmt.Errorf("VM not found: %s", req.VMID)
	}

	// Validate state
	if record.State != VMStateRunning {
		return fmt.Errorf("VM is not running (state: %s)", record.State)
	}

	// Stop the VM
	err := m.launcher.StopVM(req.VMID, req.Force, req.OperationID)
	if err != nil {
		return fmt.Errorf("failed to stop VM: %w", err)
	}

	// Update state
	record.State = VMStateStopped
	record.UpdatedAt = time.Now()
	record.LastOperationID = req.OperationID
	if err := m.saveRecord(record); err != nil {
		// Log but don't fail - VM is stopped
		// TODO: Log warning
	}

	return nil
}

// StartVMRequest contains parameters for starting a VM.
type StartVMRequest struct {
	VMID        string
	OperationID string
}

// StartVM starts a stopped VM.
func (m *VMManager) StartVM(ctx context.Context, req *StartVMRequest) error {
	// Validate VM ID
	if err := uuidx.ValidateSafeForPath(req.VMID); err != nil {
		return fmt.Errorf("invalid VM ID: %w", err)
	}

	// Check idempotency
	if m.wasOperationPerformed(req.VMID, req.OperationID) {
		return nil
	}

	// Get VM record
	record := m.getRecord(req.VMID)
	if record == nil {
		return fmt.Errorf("VM not found: %s", req.VMID)
	}

	// Validate state
	if record.State != VMStateStopped && record.State != VMStateError {
		return fmt.Errorf("VM cannot be started from state %s", record.State)
	}

	// Check if disk exists
	if record.VolumePath == "" {
		return fmt.Errorf("VM disk not found")
	}
	if _, err := os.Stat(record.VolumePath); os.IsNotExist(err) {
		return fmt.Errorf("VM disk not found: %s", record.VolumePath)
	}

	// Determine bridge
	bridgeName := record.BridgeName
	if bridgeName == "" {
		bridgeName = m.defaultBridge
	}

	// Create VM config
	vmConfig := &hypervisor.VMConfig{
		VMID:           req.VMID,
		Name:           record.Name,
		VCPU:           record.VCPU,
		MemoryMB:       record.MemoryMB,
		VolumePath:     record.VolumePath,
		BackingImageID: record.BackingImageID,
		BridgeName:     bridgeName,
		CloudInitISO:   record.CloudInitISO,
	}

	// Start the VM
	_, err := m.launcher.StartVM(vmConfig, req.OperationID)
	if err != nil {
		record.LastError = fmt.Sprintf("start failed: %v", err)
		record.UpdatedAt = time.Now()
		m.saveRecord(record)
		return fmt.Errorf("failed to start VM: %w", err)
	}

	// Update state
	record.State = VMStateRunning
	record.LastError = ""
	record.UpdatedAt = time.Now()
	record.LastOperationID = req.OperationID
	if err := m.saveRecord(record); err != nil {
		// TODO: Log warning
	}

	return nil
}

// GetVM returns the VM record for the given VM ID.
func (m *VMManager) GetVM(vmID string) (*VMRecord, error) {
	if err := uuidx.ValidateSafeForPath(vmID); err != nil {
		return nil, fmt.Errorf("invalid VM ID: %w", err)
	}

	record := m.getRecord(vmID)
	if record == nil {
		// Try loading from disk
		diskRecord, err := m.loadRecordFromDisk(vmID)
		if err != nil {
			return nil, fmt.Errorf("VM not found: %s", vmID)
		}
		m.setRecord(diskRecord)
		return diskRecord, nil
	}

	return record, nil
}

// ListVMs returns all VM records.
func (m *VMManager) ListVMs() ([]*VMRecord, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	// Load all records from disk to ensure we have the latest
	records, err := m.loadAllRecordsFromDisk()
	if err != nil {
		return nil, err
	}

	// Update in-memory map
	for _, record := range records {
		m.records[record.VMID] = record
	}

	result := make([]*VMRecord, 0, len(m.records))
	for _, record := range m.records {
		result = append(result, record)
	}

	return result, nil
}

// GetVMState returns the current state of a VM.
func (m *VMManager) GetVMState(vmID string) (VMState, error) {
	record, err := m.GetVM(vmID)
	if err != nil {
		return "", err
	}

	// If the record says running, verify with launcher
	if record.State == VMStateRunning {
		launcherState, err := m.launcher.GetVMState(vmID)
		if err != nil {
			return record.State, nil // Return recorded state if launcher fails
		}
		if launcherState != "running" {
			// Update record if state has changed
			record.State = VMStateStopped
			record.UpdatedAt = time.Now()
			m.saveRecord(record)
			return VMStateStopped, nil
		}
	}

	return record.State, nil
}

// validateCreateRequest validates a create VM request.
func (m *VMManager) validateCreateRequest(req *CreateVMRequest) error {
	if req.VMID == "" {
		return fmt.Errorf("VM ID is required")
	}
	if req.Name == "" {
		return fmt.Errorf("VM name is required")
	}
	if req.VCPU <= 0 {
		return fmt.Errorf("VCPU must be greater than 0")
	}
	if req.MemoryMB <= 0 {
		return fmt.Errorf("MemoryMB must be greater than 0")
	}
	if req.DiskSizeBytes <= 0 && req.BackingImageID == "" {
		return fmt.Errorf("DiskSizeBytes or BackingImageID is required")
	}
	return nil
}

// prepareVMDisk prepares the VM disk.
// If BackingImageID is provided, it copies/converts the image to the VM disk.
// Otherwise, it creates a new empty raw volume.
func (m *VMManager) prepareVMDisk(req *CreateVMRequest) (string, error) {
	// Validate VM ID to prevent path traversal
	if err := validation.ValidateID(req.VMID); err != nil {
		return "", fmt.Errorf("invalid VM ID: %w", err)
	}
	
	// Determine volume path
	volumePath := filepath.Join(m.vmDataDir, req.VMID+".raw")

	if req.BackingImageID != "" {
		// Validate backing image ID to prevent path traversal
		if err := validation.ValidateID(req.BackingImageID); err != nil {
			return "", fmt.Errorf("invalid backing image ID: %w", err)
		}
		
		// Copy from backing image
		imagePath := filepath.Join(m.imagesDir, req.BackingImageID+".raw")

		// Check if image exists
		if _, err := os.Stat(imagePath); os.IsNotExist(err) {
			// Try with qcow2 extension
			imagePathQCOW2 := filepath.Join(m.imagesDir, req.BackingImageID+".qcow2")
			if _, err := os.Stat(imagePathQCOW2); os.IsNotExist(err) {
				return "", fmt.Errorf("backing image not found: %s", req.BackingImageID)
			}
			// Convert qcow2 to raw
			if err := m.storageMgr.ConvertImage(imagePathQCOW2, volumePath, "qcow2"); err != nil {
				return "", fmt.Errorf("failed to convert image: %w", err)
			}
		} else {
			// Copy the raw image
			if err := m.copyFile(imagePath, volumePath); err != nil {
				return "", fmt.Errorf("failed to copy image: %w", err)
			}
		}

		// Resize if necessary
		if req.DiskSizeBytes > 0 {
			if err := m.storageMgr.ResizeRawVolume(volumePath, req.DiskSizeBytes); err != nil {
				os.Remove(volumePath) // Clean up on failure
				return "", fmt.Errorf("failed to resize volume: %w", err)
			}
		}
	} else {
		// Create new empty volume
		if err := m.storageMgr.CreateRawVolume(volumePath, req.DiskSizeBytes); err != nil {
			return "", fmt.Errorf("failed to create volume: %w", err)
		}
	}

	return volumePath, nil
}

// copyFile copies a file from source to destination using buffered I/O.
// This avoids loading the entire file into memory.
func (m *VMManager) copyFile(source, dest string) error {
	sourceFile, err := os.Open(source)
	if err != nil {
		return err
	}
	defer sourceFile.Close()

	// Ensure destination directory exists
	if err := os.MkdirAll(filepath.Dir(dest), 0755); err != nil {
		return err
	}

	destFile, err := os.Create(dest)
	if err != nil {
		return err
	}
	defer destFile.Close()

	// Use buffered copy to avoid loading entire file into memory
	// ReadFrom already uses buffering internally, but we limit buffer size
	// by using a custom reader if needed for very large files
	_, err = destFile.ReadFrom(sourceFile)
	return err
}

// cleanupVMDisk removes the VM disk file.
func (m *VMManager) cleanupVMDisk(path string) error {
	if path == "" {
		return nil
	}
	return os.Remove(path)
}

// isVMRunning checks if a VM is running via the launcher.
func (m *VMManager) isVMRunning(vmID string) bool {
	state, err := m.launcher.GetVMState(vmID)
	if err != nil {
		return false
	}
	return state == "running"
}

// State persistence methods

// recordPath returns the path to a VM's state file.
func (m *VMManager) recordPath(vmID string) string {
	return filepath.Join(m.stateDir, vmID+".json")
}

// saveRecord persists the VM record to disk.
func (m *VMManager) saveRecord(record *VMRecord) error {
	data, err := json.MarshalIndent(record, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to marshal record: %w", err)
	}

	tempPath := m.recordPath(record.VMID) + ".tmp"
	if err := os.WriteFile(tempPath, data, 0640); err != nil {
		return fmt.Errorf("failed to write state file: %w", err)
	}

	if err := os.Rename(tempPath, m.recordPath(record.VMID)); err != nil {
		os.Remove(tempPath)
		return fmt.Errorf("failed to rename state file: %w", err)
	}

	return nil
}

// loadRecordFromDisk loads a VM record from disk.
func (m *VMManager) loadRecordFromDisk(vmID string) (*VMRecord, error) {
	path := m.recordPath(vmID)
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, fmt.Errorf("VM not found")
		}
		return nil, fmt.Errorf("failed to read state file: %w", err)
	}

	var record VMRecord
	if err := json.Unmarshal(data, &record); err != nil {
		return nil, fmt.Errorf("failed to unmarshal record: %w", err)
	}

	return &record, nil
}

// loadAllRecordsFromDisk loads all VM records from disk.
func (m *VMManager) loadAllRecordsFromDisk() ([]*VMRecord, error) {
	entries, err := os.ReadDir(m.stateDir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("failed to read state directory: %w", err)
	}

	var records []*VMRecord
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		if filepath.Ext(entry.Name()) != ".json" {
			continue
		}

		vmID := entry.Name()[:len(entry.Name())-5] // Remove .json
		record, err := m.loadRecordFromDisk(vmID)
		if err != nil {
			// Log error but continue loading other records
			continue
		}
		records = append(records, record)
	}

	return records, nil
}

// deleteRecord removes a VM record from disk.
func (m *VMManager) deleteRecord(vmID string) error {
	path := m.recordPath(vmID)
	if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
		return err
	}
	return nil
}

// recoverState recovers VM state from disk on startup.
func (m *VMManager) recoverState() error {
	records, err := m.loadAllRecordsFromDisk()
	if err != nil {
		return err
	}

	m.mu.Lock()
	defer m.mu.Unlock()

	for _, record := range records {
		// Verify the VM state matches reality
		if record.State == VMStateRunning {
			// Check if actually running
			if !m.isVMRunning(record.VMID) {
				record.State = VMStateStopped
				record.UpdatedAt = time.Now()
				// Update state file
				_ = m.saveRecord(record)
			}
		}
		m.records[record.VMID] = record
	}

	return nil
}

// In-memory record management

func (m *VMManager) getRecord(vmID string) *VMRecord {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return m.records[vmID]
}

func (m *VMManager) setRecord(record *VMRecord) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.records[record.VMID] = record
}

func (m *VMManager) deleteRecordFromMemory(vmID string) {
	m.mu.Lock()
	defer m.mu.Unlock()
	delete(m.records, vmID)
}

func (m *VMManager) wasOperationPerformed(vmID string, operationID string) bool {
	if operationID == "" {
		return false
	}
	record := m.getRecord(vmID)
	if record == nil {
		return false
	}
	return record.LastOperationID == operationID
}

// ResizeVMDisk resizes a VM's disk.
func (m *VMManager) ResizeVMDisk(vmID string, newSizeBytes int64) error {
	record := m.getRecord(vmID)
	if record == nil {
		return fmt.Errorf("VM not found: %s", vmID)
	}

	if record.VolumePath == "" {
		return fmt.Errorf("VM has no disk")
	}

	// Resize the volume
	if err := m.storageMgr.ResizeRawVolume(record.VolumePath, newSizeBytes); err != nil {
		return fmt.Errorf("failed to resize volume: %w", err)
	}

	return nil
}

// GetVMConfig returns the hypervisor VM config for a VM.
// This is used when the launcher needs to recreate a VM instance.
func (m *VMManager) GetVMConfig(vmID string) (*hypervisor.VMConfig, error) {
	record, err := m.GetVM(vmID)
	if err != nil {
		return nil, err
	}

	bridgeName := record.BridgeName
	if bridgeName == "" {
		bridgeName = m.defaultBridge
	}

	return &hypervisor.VMConfig{
		VMID:           record.VMID,
		Name:           record.Name,
		VCPU:           record.VCPU,
		MemoryMB:       record.MemoryMB,
		VolumePath:     record.VolumePath,
		BackingImageID: record.BackingImageID,
		BridgeName:     bridgeName,
		CloudInitISO:   record.CloudInitISO,
	}, nil
}
