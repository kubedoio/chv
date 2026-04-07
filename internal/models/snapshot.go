package models

import (
	"time"

	"github.com/google/uuid"
)

// SnapshotStatus represents the status of a snapshot
type SnapshotStatus string

const (
	SnapshotStatusCreating SnapshotStatus = "creating"
	SnapshotStatusReady    SnapshotStatus = "ready"
	SnapshotStatusError    SnapshotStatus = "error"
	SnapshotStatusDeleting SnapshotStatus = "deleting"
)

// Snapshot represents a VM volume snapshot (external qcow2 with backing file)
type Snapshot struct {
	ID          uuid.UUID      `json:"id" db:"id"`
	VMID        uuid.UUID      `json:"vm_id" db:"vm_id"`
	VolumeID    uuid.UUID      `json:"volume_id" db:"volume_id"`
	Name        string         `json:"name" db:"name"`
	Description string         `json:"description" db:"description"`
	Path        string         `json:"path" db:"path"`
	Status      SnapshotStatus `json:"status" db:"status"`
	SizeBytes   int64          `json:"size_bytes" db:"size_bytes"`
	CreatedAt   time.Time      `json:"created_at" db:"created_at"`
}

// IsReady returns true if the snapshot is ready for use
func (s *Snapshot) IsReady() bool {
	return s.Status == SnapshotStatusReady
}
