package models

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

// ImageFormat represents the image format.
type ImageFormat string

const (
	ImageFormatQCOW2 ImageFormat = "qcow2"
	ImageFormatRaw   ImageFormat = "raw"
)

// ImageStatus represents the image import status.
type ImageStatus string

const (
	ImageStatusImporting ImageStatus = "importing"
	ImageStatusReady     ImageStatus = "ready"
	ImageStatusError     ImageStatus = "error"
	ImageStatusFailed    ImageStatus = "failed"
)

// Image represents an imported cloud image template.
type Image struct {
	ID                 uuid.UUID       `json:"id" db:"id"`
	Name               string          `json:"name" db:"name"`
	OSFamily           string          `json:"os_family" db:"os_family"`
	SourceFormat       ImageFormat     `json:"source_format" db:"source_format"`
	NormalizedFormat   ImageFormat     `json:"normalized_format" db:"normalized_format"`
	Architecture       string          `json:"architecture" db:"architecture"`
	CloudInitSupported bool            `json:"cloud_init_supported" db:"cloud_init_supported"`
	DefaultUsername    string          `json:"default_username" db:"default_username"`
	Checksum           string          `json:"checksum" db:"checksum"`
	Status             ImageStatus     `json:"status" db:"status"`
	SizeBytes          uint64          `json:"size_bytes" db:"size_bytes"`
	Metadata           json.RawMessage `json:"metadata" db:"metadata"`
	CreatedAt          time.Time       `json:"created_at" db:"created_at"`
	ImportedAt         *time.Time      `json:"imported_at,omitempty" db:"imported_at"`
}

// IsReady returns true if the image is ready for use.
func (i *Image) IsReady() bool {
	return i.Status == ImageStatusReady
}

// GetMetadata parses and returns image metadata.
func (i *Image) GetMetadata() map[string]interface{} {
	var meta map[string]interface{}
	if i.Metadata != nil {
		json.Unmarshal(i.Metadata, &meta)
	}
	return meta
}
