package images

import (
	"fmt"
	"sync"
	"time"
)

// ImportStatus represents the current state of an image import
type ImportStatus string

const (
	StatusImportPending     ImportStatus = "pending"
	StatusImportDownloading ImportStatus = "downloading"
	StatusImportValidating  ImportStatus = "validating"
	StatusImportReady       ImportStatus = "ready"
	StatusImportFailed      ImportStatus = "failed"
)

// ImportProgress tracks the progress of an image import operation
type ImportProgress struct {
	ImageID         string       `json:"image_id"`
	Status          ImportStatus `json:"status"`
	ProgressPercent int          `json:"progress_percent"`
	BytesDownloaded int64        `json:"bytes_downloaded"`
	TotalBytes      int64        `json:"total_bytes"`
	Speed           string       `json:"speed"`
	Error           string       `json:"error,omitempty"`
	UpdatedAt       time.Time    `json:"updated_at"`
}

// ProgressTracker provides thread-safe storage for import progress
type ProgressTracker struct {
	mu       sync.RWMutex
	progress map[string]*ImportProgress
}

// NewProgressTracker creates a new progress tracker
func NewProgressTracker() *ProgressTracker {
	return &ProgressTracker{
		progress: make(map[string]*ImportProgress),
	}
}

// StartTracking begins tracking progress for an image
func (pt *ProgressTracker) StartTracking(imageID string, totalBytes int64) *ImportProgress {
	pt.mu.Lock()
	defer pt.mu.Unlock()

	progress := &ImportProgress{
		ImageID:         imageID,
		Status:          StatusImportPending,
		ProgressPercent: 0,
		BytesDownloaded: 0,
		TotalBytes:      totalBytes,
		Speed:           "0 B/s",
		UpdatedAt:       time.Now(),
	}
	pt.progress[imageID] = progress
	return progress
}

// GetProgress retrieves the current progress for an image
func (pt *ProgressTracker) GetProgress(imageID string) *ImportProgress {
	pt.mu.RLock()
	defer pt.mu.RUnlock()

	if progress, exists := pt.progress[imageID]; exists {
		// Return a copy to avoid race conditions
		progressCopy := *progress
		return &progressCopy
	}
	return nil
}

// UpdateDownloadProgress updates the download progress
func (pt *ProgressTracker) UpdateDownloadProgress(imageID string, bytesDownloaded int64, speed string) {
	pt.mu.Lock()
	defer pt.mu.Unlock()

	if progress, exists := pt.progress[imageID]; exists {
		progress.BytesDownloaded = bytesDownloaded
		if progress.TotalBytes > 0 {
			progress.ProgressPercent = int((float64(bytesDownloaded) / float64(progress.TotalBytes)) * 100)
		}
		progress.Speed = speed
		progress.UpdatedAt = time.Now()
	}
}

// SetStatus updates the status of an import
func (pt *ProgressTracker) SetStatus(imageID string, status ImportStatus) {
	pt.mu.Lock()
	defer pt.mu.Unlock()

	if progress, exists := pt.progress[imageID]; exists {
		progress.Status = status
		progress.UpdatedAt = time.Now()

		// Update progress percent based on status
		switch status {
		case StatusImportReady:
			progress.ProgressPercent = 100
		case StatusImportFailed:
			// Keep current progress on failure
		}
	}
}

// SetError marks the import as failed with an error message
func (pt *ProgressTracker) SetError(imageID string, err error) {
	pt.mu.Lock()
	defer pt.mu.Unlock()

	if progress, exists := pt.progress[imageID]; exists {
		progress.Status = StatusImportFailed
		progress.Error = err.Error()
		progress.UpdatedAt = time.Now()
	}
}

// Complete marks the import as complete
func (pt *ProgressTracker) Complete(imageID string) {
	pt.mu.Lock()
	defer pt.mu.Unlock()

	if progress, exists := pt.progress[imageID]; exists {
		progress.Status = StatusImportReady
		progress.ProgressPercent = 100
		progress.BytesDownloaded = progress.TotalBytes
		progress.Speed = "0 B/s"
		progress.UpdatedAt = time.Now()
	}
}

// Remove deletes the progress tracking for an image
func (pt *ProgressTracker) Remove(imageID string) {
	pt.mu.Lock()
	defer pt.mu.Unlock()

	delete(pt.progress, imageID)
}

// FormatSpeed formats bytes per second as a human-readable string
func FormatSpeed(bytesPerSec float64) string {
	const (
		KB = 1024
		MB = 1024 * KB
		GB = 1024 * MB
	)

	switch {
	case bytesPerSec >= GB:
		return fmt.Sprintf("%.2f GB/s", bytesPerSec/GB)
	case bytesPerSec >= MB:
		return fmt.Sprintf("%.2f MB/s", bytesPerSec/MB)
	case bytesPerSec >= KB:
		return fmt.Sprintf("%.2f KB/s", bytesPerSec/KB)
	default:
		return fmt.Sprintf("%.0f B/s", bytesPerSec)
	}
}

// FormatBytes formats bytes as a human-readable string
func FormatBytes(bytes int64) string {
	const (
		KB = 1024
		MB = 1024 * KB
		GB = 1024 * MB
	)

	switch {
	case bytes >= GB:
		return fmt.Sprintf("%.2f GB", float64(bytes)/GB)
	case bytes >= MB:
		return fmt.Sprintf("%.2f MB", float64(bytes)/MB)
	case bytes >= KB:
		return fmt.Sprintf("%.2f KB", float64(bytes)/KB)
	default:
		return fmt.Sprintf("%d B", bytes)
	}
}
