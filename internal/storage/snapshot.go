package storage

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/pkg/uuidx"
)

// CreateSnapshot creates an external snapshot using qcow2 with backing file
func (m *Manager) CreateSnapshot(vmID, volumeID, volumePath, snapshotDir, name, description string) (*models.Snapshot, error) {
	snapshotID := uuidx.New()
	timestamp := time.Now().Unix()

	// Snapshot path: {snapshotDir}/{vm_id}/{snapshot_id}-{timestamp}.qcow2
	vmSnapshotDir := filepath.Join(snapshotDir, vmID)
	if err := os.MkdirAll(vmSnapshotDir, 0750); err != nil {
		return nil, fmt.Errorf("failed to create snapshot directory: %w", err)
	}

	snapshotPath := filepath.Join(vmSnapshotDir,
		fmt.Sprintf("%s-%d.qcow2", snapshotID.String(), timestamp))

	// Parse volume ID
	volID, err := uuidx.Parse(volumeID)
	if err != nil {
		return nil, fmt.Errorf("invalid volume ID: %w", err)
	}

	// Parse VM ID
	vmUUID, err := uuidx.Parse(vmID)
	if err != nil {
		return nil, fmt.Errorf("invalid VM ID: %w", err)
	}

	// Create qcow2 snapshot with backing file
	cmd := exec.Command("qemu-img", "create", "-f", "qcow2",
		"-b", volumePath, "-F", "raw", snapshotPath)

	output, err := cmd.CombinedOutput()
	if err != nil {
		return nil, fmt.Errorf("failed to create snapshot: %w (output: %s)",
			err, string(output))
	}

	// Get size
	info, err := os.Stat(snapshotPath)
	if err != nil {
		return nil, fmt.Errorf("failed to stat snapshot file: %w", err)
	}

	return &models.Snapshot{
		ID:          snapshotID,
		VMID:        vmUUID,
		VolumeID:    volID,
		Name:        name,
		Description: description,
		Path:        snapshotPath,
		Status:      models.SnapshotStatusReady,
		SizeBytes:   info.Size(),
		CreatedAt:   time.Now(),
	}, nil
}

// RestoreSnapshot restores a volume from snapshot
func (m *Manager) RestoreSnapshot(snapshotPath, destPath string) error {
	// Ensure destination directory exists
	dir := filepath.Dir(destPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("failed to create destination directory: %w", err)
	}

	// Convert qcow2 back to raw
	cmd := exec.Command("qemu-img", "convert", "-f", "qcow2", "-O", "raw",
		snapshotPath, destPath)

	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("failed to restore snapshot: %w (output: %s)",
			err, string(output))
	}

	return nil
}

// DeleteSnapshot deletes a snapshot file
func (m *Manager) DeleteSnapshot(snapshotPath string) error {
	if err := os.Remove(snapshotPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("failed to delete snapshot: %w", err)
	}
	return nil
}

// GetSnapshotInfo returns information about a snapshot
func (m *Manager) GetSnapshotInfo(snapshotPath string) (map[string]string, error) {
	cmd := exec.Command("qemu-img", "info", "--output=json", snapshotPath)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return nil, fmt.Errorf("failed to get snapshot info: %w", err)
	}

	// Parse JSON output (simplified)
	info := map[string]string{
		"raw": string(output),
	}
	return info, nil
}
