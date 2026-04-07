package storage

import (
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
)

// Manager manages storage operations.
type Manager struct {
	basePath string
}

// NewManager creates a new storage manager.
func NewManager(basePath string) *Manager {
	return &Manager{basePath: basePath}
}

// CreateRawVolume creates a new raw volume file.
func (m *Manager) CreateRawVolume(path string, sizeBytes int64) error {
	// Ensure directory exists
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("failed to create directory: %w", err)
	}
	
	// Create sparse file using fallocate or truncate
	cmd := exec.Command("fallocate", "-l", fmt.Sprintf("%d", sizeBytes), path)
	if err := cmd.Run(); err != nil {
		// Fallback to truncate
		cmd = exec.Command("truncate", "-s", fmt.Sprintf("%d", sizeBytes), path)
		if err := cmd.Run(); err != nil {
			return fmt.Errorf("failed to create volume file: %w", err)
		}
	}
	
	return nil
}

// ResizeRawVolume resizes a raw volume file.
func (m *Manager) ResizeRawVolume(path string, newSizeBytes int64) error {
	// Check if file exists
	info, err := os.Stat(path)
	if err != nil {
		return fmt.Errorf("volume not found: %w", err)
	}
	
	if info.Size() > newSizeBytes {
		return fmt.Errorf("shrinking volumes is not supported")
	}
	
	// Try qemu-img resize first (better for disk images)
	cmd := exec.Command("qemu-img", "resize", path, fmt.Sprintf("%d", newSizeBytes))
	if output, err := cmd.CombinedOutput(); err == nil {
		return nil
	} else {
		// Log the qemu-img error and fall back to truncate
		_ = output
	}
	
	// Fallback to truncate for simple sparse files
	cmd = exec.Command("truncate", "-s", fmt.Sprintf("%d", newSizeBytes), path)
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("failed to resize volume: %w", err)
	}
	
	return nil
}

// ResizeQcow2Volume resizes a QCOW2 volume file.
func (m *Manager) ResizeQcow2Volume(path string, newSizeBytes int64) error {
	// Check if file exists
	if _, err := os.Stat(path); err != nil {
		return fmt.Errorf("volume not found: %w", err)
	}
	
	// Use qemu-img resize
	cmd := exec.Command("qemu-img", "resize", path, fmt.Sprintf("%d", newSizeBytes))
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to resize QCOW2 volume: %w (output: %s)", err, string(output))
	}
	
	return nil
}

// DeleteVolume deletes a volume file.
func (m *Manager) DeleteVolume(path string) error {
	return os.Remove(path)
}

// VolumePath returns the full path for a volume.
func (m *Manager) VolumePath(volumeID string) string {
	return filepath.Join(m.basePath, volumeID+".raw")
}

// ImagePath returns the full path for an image.
func (m *Manager) ImagePath(imageID string) string {
	return filepath.Join(m.basePath, "images", imageID+".raw")
}

// ConvertImage converts an image to raw format.
func (m *Manager) ConvertImage(sourcePath, targetPath, sourceFormat string) error {
	// Ensure directory exists
	dir := filepath.Dir(targetPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("failed to create directory: %w", err)
	}
	
	// Use qemu-img to convert
	cmd := exec.Command("qemu-img", "convert", "-f", sourceFormat, "-O", "raw", sourcePath, targetPath)
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to convert image: %w (output: %s)", err, string(out))
	}
	
	return nil
}

// CloneVolume creates a copy of a volume using qemu-img convert
func (m *Manager) CloneVolume(sourcePath, destPath string) error {
	// Ensure source exists
	if _, err := os.Stat(sourcePath); err != nil {
		return fmt.Errorf("source volume not found: %w", err)
	}
	
	// Ensure destination directory exists
	dir := filepath.Dir(destPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("failed to create destination directory: %w", err)
	}
	
	// Use qemu-img convert for efficient copy
	cmd := exec.Command("qemu-img", "convert", "-f", "raw", "-O", "raw",
		sourcePath, destPath)
	
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("failed to clone volume: %w (output: %s)",
			err, string(output))
	}
	
	return nil
}

// CloneVolumeFast creates a copy of a volume using io.Copy (faster for sparse files)
func (m *Manager) CloneVolumeFast(sourcePath, destPath string) error {
	// Ensure source exists
	sourceInfo, err := os.Stat(sourcePath)
	if err != nil {
		return fmt.Errorf("source volume not found: %w", err)
	}
	
	// Ensure destination directory exists
	dir := filepath.Dir(destPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("failed to create destination directory: %w", err)
	}
	
	source, err := os.Open(sourcePath)
	if err != nil {
		return fmt.Errorf("failed to open source volume: %w", err)
	}
	defer source.Close()
	
	dest, err := os.Create(destPath)
	if err != nil {
		return fmt.Errorf("failed to create destination volume: %w", err)
	}
	defer dest.Close()
	
	// Copy data
	_, err = io.Copy(dest, source)
	if err != nil {
		return fmt.Errorf("failed to copy volume data: %w", err)
	}
	
	// Preserve permissions
	if err := os.Chmod(destPath, sourceInfo.Mode()); err != nil {
		return fmt.Errorf("failed to set destination permissions: %w", err)
	}
	
	return nil
}
