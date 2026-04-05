// Package hypervisor provides VM lifecycle management for Cloud Hypervisor.
package hypervisor

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// VMInstanceState represents the persistent state of a VM instance.
// This struct is serialized to disk for crash recovery.
type VMInstanceState struct {
	VMID            string    `json:"vm_id"`
	PID             int       `json:"pid"`
	APISocket       string    `json:"api_socket"`
	TAPDevice       string    `json:"tap_device"`
	VolumePaths     []string  `json:"volume_paths"`
	CloudInitISO    string    `json:"cloudinit_iso"`
	CreatedAt       time.Time `json:"created_at"`
	LastOperationID string    `json:"last_operation_id"`
	State           string    `json:"state"` // running, stopped, error
}

// StateManager handles persistent storage of VM instance state.
type StateManager struct {
	baseDir string
	mu      sync.RWMutex
}

// NewStateManager creates a new state manager.
func NewStateManager(baseDir string) *StateManager {
	return &StateManager{
		baseDir: baseDir,
	}
}

// Initialize creates the base directory if it doesn't exist.
func (sm *StateManager) Initialize() error {
	return os.MkdirAll(sm.baseDir, 0750)
}

// statePath returns the path to a VM's state file.
func (sm *StateManager) statePath(vmID string) string {
	return filepath.Join(sm.baseDir, vmID+".json")
}

// Save persists the VM state to disk.
func (sm *StateManager) Save(state *VMInstanceState) error {
	sm.mu.Lock()
	defer sm.mu.Unlock()
	return sm.saveUnsafe(state)
}

// saveUnsafe persists state without locking (caller must hold lock).
func (sm *StateManager) saveUnsafe(state *VMInstanceState) error {
	// Ensure directory exists
	if err := os.MkdirAll(sm.baseDir, 0750); err != nil {
		return fmt.Errorf("failed to create state directory: %w", err)
	}

	// Marshal with indentation for readability
	data, err := json.MarshalIndent(state, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to marshal state: %w", err)
	}

	// Write atomically using temp file + rename
	tempPath := sm.statePath(state.VMID) + ".tmp"
	if err := os.WriteFile(tempPath, data, 0640); err != nil {
		return fmt.Errorf("failed to write state file: %w", err)
	}

	if err := os.Rename(tempPath, sm.statePath(state.VMID)); err != nil {
		os.Remove(tempPath) // Clean up temp file
		return fmt.Errorf("failed to rename state file: %w", err)
	}

	return nil
}

// Load retrieves the VM state from disk.
func (sm *StateManager) Load(vmID string) (*VMInstanceState, error) {
	sm.mu.RLock()
	defer sm.mu.RUnlock()

	path := sm.statePath(vmID)
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("failed to read state file: %w", err)
	}

	var state VMInstanceState
	if err := json.Unmarshal(data, &state); err != nil {
		return nil, fmt.Errorf("failed to unmarshal state (corrupted file?): %w", err)
	}

	return &state, nil
}

// Delete removes the VM state file.
func (sm *StateManager) Delete(vmID string) error {
	sm.mu.Lock()
	defer sm.mu.Unlock()

	path := sm.statePath(vmID)
	if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("failed to delete state file: %w", err)
	}

	return nil
}

// List returns all VM states from disk.
func (sm *StateManager) List() ([]*VMInstanceState, error) {
	sm.mu.RLock()
	defer sm.mu.RUnlock()

	entries, err := os.ReadDir(sm.baseDir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("failed to read state directory: %w", err)
	}

	var states []*VMInstanceState
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		if filepath.Ext(entry.Name()) != ".json" {
			continue
		}

		vmID := entry.Name()[:len(entry.Name())-5] // Remove .json
		state, err := sm.loadUnsafe(vmID)
		if err != nil {
			// Log error but continue loading other states
			continue
		}
		if state != nil {
			states = append(states, state)
		}
	}

	return states, nil
}

// loadUnsafe loads state without holding lock (caller must hold lock).
func (sm *StateManager) loadUnsafe(vmID string) (*VMInstanceState, error) {
	path := sm.statePath(vmID)
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}

	var state VMInstanceState
	if err := json.Unmarshal(data, &state); err != nil {
		return nil, err
	}

	return &state, nil
}

// Recover rebuilds the in-memory state map from disk.
// Call this on agent startup to recover after a crash.
func (sm *StateManager) Recover() (map[string]*VMInstanceState, error) {
	states, err := sm.List()
	if err != nil {
		return nil, err
	}

	recovered := make(map[string]*VMInstanceState)
	for _, state := range states {
		// Validate: check if PID still exists
		if state.PID > 0 {
			if !processExists(state.PID) {
				// Process is gone, mark as stopped
				state.State = "stopped"
				state.PID = 0
				// Update state file to reflect reality
				_ = sm.Save(state)
			}
		}
		recovered[state.VMID] = state
	}

	return recovered, nil
}

// UpdateLastOperation updates the last operation ID for idempotency tracking.
func (sm *StateManager) UpdateLastOperation(vmID string, operationID string) error {
	sm.mu.Lock()
	defer sm.mu.Unlock()

	state, err := sm.loadUnsafe(vmID)
	if err != nil {
		return err
	}
	if state == nil {
		return fmt.Errorf("VM state not found: %s", vmID)
	}

	state.LastOperationID = operationID
	return sm.saveUnsafe(state)
}

// WasOperationPerformed checks if an operation was recently performed.
// This provides idempotency for retried operations.
func (sm *StateManager) WasOperationPerformed(vmID string, operationID string) bool {
	state, err := sm.Load(vmID)
	if err != nil {
		return false
	}
	if state == nil {
		return false
	}
	return state.LastOperationID == operationID
}

// processExists checks if a process with the given PID exists.
func processExists(pid int) bool {
	// On Unix, we can check if /proc/<pid> exists
	_, err := os.Stat(fmt.Sprintf("/proc/%d", pid))
	return err == nil
}
