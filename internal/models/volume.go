package models

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

// VolumeFormat represents the volume format.
type VolumeFormat string

const (
	VolumeFormatRaw VolumeFormat = "raw"
)

// VolumeAttachmentState represents the attachment state.
type VolumeAttachmentState string

const (
	VolumeAttachmentStateDetached  VolumeAttachmentState = "detached"
	VolumeAttachmentStateAttaching VolumeAttachmentState = "attaching"
	VolumeAttachmentStateAttached  VolumeAttachmentState = "attached"
	VolumeAttachmentStateDetaching VolumeAttachmentState = "detaching"
)

// VolumeResizeState represents the resize state.
type VolumeResizeState string

const (
	VolumeResizeStateIdle       VolumeResizeState = "idle"
	VolumeResizeStateResizing   VolumeResizeState = "resizing"
	VolumeResizeStateCompleted  VolumeResizeState = "completed"
	VolumeResizeStateFailed     VolumeResizeState = "failed"
)

// Volume represents a VM-attached runtime disk.
type Volume struct {
	ID               uuid.UUID             `json:"id" db:"id"`
	VMID             *uuid.UUID            `json:"vm_id" db:"vm_id"`
	PoolID           *uuid.UUID            `json:"pool_id" db:"pool_id"`
	BackingImageID   *uuid.UUID            `json:"backing_image_id" db:"backing_image_id"`
	Format           VolumeFormat          `json:"format" db:"format"`
	SizeBytes        int64                 `json:"size_bytes" db:"size_bytes"`
	Path             *string               `json:"path" db:"path"`
	AttachmentState  VolumeAttachmentState `json:"attachment_state" db:"attachment_state"`
	ResizeState      VolumeResizeState     `json:"resize_state" db:"resize_state"`
	Metadata         json.RawMessage       `json:"metadata" db:"metadata"`
	CreatedAt        time.Time             `json:"created_at" db:"created_at"`
}

// GetMetadata parses and returns volume metadata.
func (v *Volume) GetMetadata() map[string]interface{} {
	var meta map[string]interface{}
	if v.Metadata != nil {
		json.Unmarshal(v.Metadata, &meta)
	}
	return meta
}

// IsAttached returns true if the volume is attached.
func (v *Volume) IsAttached() bool {
	return v.AttachmentState == VolumeAttachmentStateAttached
}

// IsResizable returns true if the volume can be resized.
func (v *Volume) IsResizable() bool {
	return v.Format == VolumeFormatRaw && v.ResizeState == VolumeResizeStateIdle
}
