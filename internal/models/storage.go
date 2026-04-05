package models

import (
	"time"

	"github.com/google/uuid"
)

// StoragePoolType represents the storage pool type.
type StoragePoolType string

const (
	StoragePoolTypeLocal StoragePoolType = "local"
	StoragePoolTypeNFS   StoragePoolType = "nfs"
)

// StoragePoolStatus represents the storage pool status.
type StoragePoolStatus string

const (
	StoragePoolStatusActive   StoragePoolStatus = "active"
	StoragePoolStatusDegraded StoragePoolStatus = "degraded"
	StoragePoolStatusOffline  StoragePoolStatus = "offline"
)

// StoragePool represents a storage target.
type StoragePool struct {
	ID                   uuid.UUID         `json:"id" db:"id"`
	NodeID               *uuid.UUID        `json:"node_id" db:"node_id"`
	Name                 string            `json:"name" db:"name"`
	PoolType             StoragePoolType   `json:"pool_type" db:"pool_type"`
	PathOrExport         string            `json:"path_or_export" db:"path_or_export"`
	CapacityBytes        *int64            `json:"capacity_bytes" db:"capacity_bytes"`
	AllocatableBytes     *int64            `json:"allocatable_bytes" db:"allocatable_bytes"`
	Status               StoragePoolStatus `json:"status" db:"status"`
	SupportsOnlineResize bool              `json:"supports_online_resize" db:"supports_online_resize"`
	SupportsClone        bool              `json:"supports_clone" db:"supports_clone"`
	SupportsSnapshot     bool              `json:"supports_snapshot" db:"supports_snapshot"`
	CreatedAt            time.Time         `json:"created_at" db:"created_at"`
}

// IsAvailable returns true if the pool is available for use.
func (p *StoragePool) IsAvailable() bool {
	return p.Status == StoragePoolStatusActive
}

// HasSpace checks if the pool has sufficient space.
func (p *StoragePool) HasSpace(bytes int64) bool {
	if p.AllocatableBytes == nil {
		return true
	}
	return *p.AllocatableBytes >= bytes
}
