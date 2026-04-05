// Package cloudinit provides cloud-init ISO generation for CloudHypervisor VMs.
package cloudinit

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/chv/chv/internal/cloudinit"
)

// ISOGenerator wraps the core cloudinit ISO generation for agent use.
// It handles VM-specific paths and cleanup for CloudHypervisor integration.
type ISOGenerator struct {
	core      *cloudinit.ISOGenerator
	vmDataDir string
}

// Config represents cloud-init configuration data.
// This is an alias for convenience.
type Config = cloudinit.Config

// NewISOGenerator creates a new ISO generator for the agent.
// vmDataDir is the base directory where VM data (including ISOs) is stored.
func NewISOGenerator(vmDataDir string) *ISOGenerator {
	isoDir := filepath.Join(vmDataDir, "cloudinit")
	return &ISOGenerator{
		core:      cloudinit.NewISOGenerator(isoDir),
		vmDataDir: vmDataDir,
	}
}

// GenerateISO creates a cloud-init ISO for a VM.
// The ISO is stored in <vmDataDir>/cloudinit/<vmID>-cloudinit.iso
// Returns the absolute path to the generated ISO file.
func (g *ISOGenerator) GenerateISO(vmID string, config *Config) (string, error) {
	if err := validateVMID(vmID); err != nil {
		return "", fmt.Errorf("invalid VM ID: %w", err)
	}

	// Ensure the ISO directory exists
	isoDir := filepath.Join(g.vmDataDir, "cloudinit")
	if err := os.MkdirAll(isoDir, 0750); err != nil {
		return "", fmt.Errorf("failed to create ISO directory: %w", err)
	}

	// Generate the ISO using the core generator
	isoPath, err := g.core.GenerateISO(vmID, config)
	if err != nil {
		return "", fmt.Errorf("failed to generate cloud-init ISO: %w", err)
	}

	// Verify the ISO was created and is valid
	if err := g.ValidateISO(isoPath); err != nil {
		// Clean up on validation failure
		_ = os.Remove(isoPath)
		return "", fmt.Errorf("ISO validation failed: %w", err)
	}

	return isoPath, nil
}

// GenerateISOForVM creates a cloud-init ISO and stores it in the VM's directory.
// This is useful when you want the ISO to be co-located with other VM files.
func (g *ISOGenerator) GenerateISOForVM(vmID string, config *Config) (string, error) {
	if err := validateVMID(vmID); err != nil {
		return "", fmt.Errorf("invalid VM ID: %w", err)
	}

	// Create VM-specific directory
	vmDir := filepath.Join(g.vmDataDir, "instances", vmID)
	if err := os.MkdirAll(vmDir, 0750); err != nil {
		return "", fmt.Errorf("failed to create VM directory: %w", err)
	}

	// Generate ISO path in VM directory
	isoPath := filepath.Join(vmDir, "cloudinit.iso")

	// Create temporary directory for ISO contents
	tempDir, err := os.MkdirTemp("", "cloudinit-"+vmID+"-*")
	if err != nil {
		return "", fmt.Errorf("failed to create temp directory: %w", err)
	}
	defer os.RemoveAll(tempDir)

	// Write cloud-init files
	if err := g.writeCloudInitFiles(tempDir, vmID, config); err != nil {
		return "", err
	}

	// Create ISO using core generator's logic
	if err := g.createISO(tempDir, isoPath); err != nil {
		return "", fmt.Errorf("failed to create ISO: %w", err)
	}

	// Verify the ISO
	if err := g.ValidateISO(isoPath); err != nil {
		_ = os.Remove(isoPath)
		return "", fmt.Errorf("ISO validation failed: %w", err)
	}

	return isoPath, nil
}

// writeCloudInitFiles writes the cloud-init configuration files to the temp directory.
func (g *ISOGenerator) writeCloudInitFiles(tempDir, vmID string, config *Config) error {
	// Write user-data
	userData := config.UserData
	if userData == "" {
		userData = "#cloud-config\n"
	}
	if err := os.WriteFile(filepath.Join(tempDir, "user-data"), []byte(userData), 0644); err != nil {
		return fmt.Errorf("failed to write user-data: %w", err)
	}

	// Write meta-data
	metaData := config.MetaData
	if metaData == "" {
		metaData = fmt.Sprintf("instance-id: %s\nlocal-hostname: %s\n", vmID, vmID)
	}
	if err := os.WriteFile(filepath.Join(tempDir, "meta-data"), []byte(metaData), 0644); err != nil {
		return fmt.Errorf("failed to write meta-data: %w", err)
	}

	// Write network-config if provided
	if config.NetworkConfig != "" {
		if err := os.WriteFile(filepath.Join(tempDir, "network-config"), []byte(config.NetworkConfig), 0644); err != nil {
			return fmt.Errorf("failed to write network-config: %w", err)
		}
	}

	return nil
}

// createISO creates an ISO image from the source directory.
// This implementation reuses the core cloudinit package's generation by
// temporarily creating a generator with the output directory set correctly.
func (g *ISOGenerator) createISO(sourceDir, outputPath string) error {
	// Create a temporary generator with the target directory
	targetDir := filepath.Dir(outputPath)
	tempGen := cloudinit.NewISOGenerator(targetDir)

	// Create a minimal config - the core generator will write its own files
	// We need to work around this by generating to a temp name and renaming
	tempVMID := "temp-" + filepath.Base(outputPath)

	// Read our prepared files and pass their contents to the generator
	userDataBytes, _ := os.ReadFile(filepath.Join(sourceDir, "user-data"))
	metaDataBytes, _ := os.ReadFile(filepath.Join(sourceDir, "meta-data"))
	networkConfigBytes, _ := os.ReadFile(filepath.Join(sourceDir, "network-config"))

	config := &cloudinit.Config{
		UserData:      string(userDataBytes),
		MetaData:      string(metaDataBytes),
		NetworkConfig: string(networkConfigBytes),
	}

	// Generate ISO using the core generator
	tempPath, err := tempGen.GenerateISO(tempVMID, config)
	if err != nil {
		return err
	}

	// Rename to the desired output path
	if err := os.Rename(tempPath, outputPath); err != nil {
		os.Remove(tempPath)
		return fmt.Errorf("failed to rename ISO: %w", err)
	}

	return nil
}

// DeleteISO removes a VM's cloud-init ISO file.
// This should be called when the VM is deleted.
func (g *ISOGenerator) DeleteISO(vmID string) error {
	if err := validateVMID(vmID); err != nil {
		return fmt.Errorf("invalid VM ID: %w", err)
	}

	// Delete from the cloudinit directory
	if err := g.core.DeleteISO(vmID); err != nil {
		// Log but don't fail - might be already deleted
		if !os.IsNotExist(err) {
			return fmt.Errorf("failed to delete ISO from cloudinit dir: %w", err)
		}
	}

	// Also delete from VM directory if it exists there
	vmISOPath := filepath.Join(g.vmDataDir, "instances", vmID, "cloudinit.iso")
	if err := os.Remove(vmISOPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("failed to delete VM ISO: %w", err)
	}

	return nil
}

// DeleteISOByPath removes a cloud-init ISO by its full path.
// Useful when you have the path from VM state.
func (g *ISOGenerator) DeleteISOByPath(isoPath string) error {
	if isoPath == "" {
		return nil // Nothing to delete
	}

	if err := os.Remove(isoPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("failed to delete ISO at %s: %w", isoPath, err)
	}

	return nil
}

// GetISOPath returns the path to a VM's cloud-init ISO in the cloudinit directory.
func (g *ISOGenerator) GetISOPath(vmID string) string {
	return g.core.GetISOPath(vmID)
}

// GetVMISOPath returns the path to a VM's cloud-init ISO in the VM directory.
func (g *ISOGenerator) GetVMISOPath(vmID string) string {
	return filepath.Join(g.vmDataDir, "instances", vmID, "cloudinit.iso")
}

// ISOExists checks if a cloud-init ISO exists.
func (g *ISOGenerator) ISOExists(vmID string) bool {
	return g.core.ISOExists(vmID)
}

// ISOExistsAtPath checks if a cloud-init ISO exists at a specific path.
func (g *ISOGenerator) ISOExistsAtPath(isoPath string) bool {
	info, err := os.Stat(isoPath)
	if err != nil {
		return false
	}
	return info.Size() > 0
}

// ValidateISO validates that an ISO file is properly formatted.
func (g *ISOGenerator) ValidateISO(isoPath string) error {
	return g.core.ValidateISO(isoPath)
}

// GetISOSize returns the size of an ISO file in bytes.
func (g *ISOGenerator) GetISOSize(isoPath string) (int64, error) {
	info, err := os.Stat(isoPath)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, fmt.Errorf("ISO file does not exist: %s", isoPath)
		}
		return 0, fmt.Errorf("failed to stat ISO: %w", err)
	}
	return info.Size(), nil
}

// CleanupOrphanedISOs removes ISO files for VMs that are no longer running.
// This should be called on agent startup to clean up any leftover files.
// activeVMIDs is a list of VM IDs that should be kept.
func (g *ISOGenerator) CleanupOrphanedISOs(activeVMIDs []string) error {
	// Build a set of active VM IDs
	activeSet := make(map[string]bool)
	for _, vmID := range activeVMIDs {
		activeSet[vmID] = true
	}

	// List all ISOs in the cloudinit directory
	isos, err := g.core.ListISOs()
	if err != nil {
		return fmt.Errorf("failed to list ISOs: %w", err)
	}

	// Extract VM IDs from ISO filenames and delete orphaned ones
	for _, isoPath := range isos {
		vmID := extractVMIDFromISOPath(isoPath)
		if vmID == "" {
			continue // Skip files that don't match our naming pattern
		}

		if !activeSet[vmID] {
			// This ISO is orphaned - delete it
			if err := g.core.DeleteISO(vmID); err != nil {
				// Log but continue with other ISOs
				continue
			}
		}
	}

	return nil
}

// GetISOInfo returns detailed information about a cloud-init ISO.
type ISOInfo struct {
	Path      string
	Size      int64
	VMID      string
	IsValid   bool
	IsReadOnly bool
}

// InspectISO returns detailed information about an ISO file.
func (g *ISOGenerator) InspectISO(isoPath string) (*ISOInfo, error) {
	info, err := os.Stat(isoPath)
	if err != nil {
		return nil, fmt.Errorf("failed to stat ISO: %w", err)
	}

	vmID := extractVMIDFromISOPath(isoPath)
	isValid := g.ValidateISO(isoPath) == nil

	return &ISOInfo{
		Path:       isoPath,
		Size:       info.Size(),
		VMID:       vmID,
		IsValid:    isValid,
		IsReadOnly: info.Mode().Perm()&0200 == 0,
	}, nil
}

// validateVMID validates that a VM ID is safe to use in file paths.
func validateVMID(vmID string) error {
	if vmID == "" {
		return fmt.Errorf("VM ID cannot be empty")
	}
	// Check for path traversal attempts
	if filepath.IsAbs(vmID) {
		return fmt.Errorf("VM ID cannot be an absolute path")
	}
	// Check for special path components
	if vmID == "." || vmID == ".." {
		return fmt.Errorf("VM ID cannot be '.' or '..'")
	}
	// Check for any path separators or traversal sequences
	clean := filepath.Clean(vmID)
	if clean != vmID {
		return fmt.Errorf("VM ID contains invalid characters or path traversal")
	}
	// Check that it doesn't contain any path separators
	if strings.ContainsAny(vmID, `/\`) {
		return fmt.Errorf("VM ID cannot contain path separators")
	}
	return nil
}

// extractVMIDFromISOPath extracts the VM ID from an ISO path.
// ISO filenames are expected to be in the format: <vmID>-cloudinit.iso
func extractVMIDFromISOPath(isoPath string) string {
	base := filepath.Base(isoPath)
	// Remove -cloudinit.iso suffix
	if len(base) > 14 && base[len(base)-14:] == "-cloudinit.iso" {
		return base[:len(base)-14]
	}
	// Also handle cloudinit.iso (VM directory format)
	if base == "cloudinit.iso" {
		// Extract VM ID from parent directory
		dir := filepath.Dir(isoPath)
		vmID := filepath.Base(dir)
		// Validate that it's not a special directory name
		if vmID == "." || vmID == ".." || vmID == "/" || vmID == string(filepath.Separator) {
			return ""
		}
		return vmID
	}
	return ""
}
