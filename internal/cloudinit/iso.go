// Package cloudinit generates cloud-init configuration disks.
package cloudinit

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

// ISOGenerator creates cloud-init ISO images.
type ISOGenerator struct {
	outputDir string
}

// NewISOGenerator creates a new ISO generator.
func NewISOGenerator(outputDir string) *ISOGenerator {
	return &ISOGenerator{
		outputDir: outputDir,
	}
}

// GenerateISO creates a cloud-init ISO image.
// Returns the path to the generated ISO file.
func (g *ISOGenerator) GenerateISO(vmID string, config *Config) (string, error) {
	// Create temporary directory for ISO contents
	tempDir, err := os.MkdirTemp("", "cloudinit-"+vmID+"-*")
	if err != nil {
		return "", fmt.Errorf("failed to create temp directory: %w", err)
	}
	defer os.RemoveAll(tempDir)

	// Write user-data
	userData := config.UserData
	if userData == "" {
		userData = "#cloud-config\n"
	}
	if err := os.WriteFile(filepath.Join(tempDir, "user-data"), []byte(userData), 0644); err != nil {
		return "", fmt.Errorf("failed to write user-data: %w", err)
	}

	// Write meta-data
	metaData := config.MetaData
	if metaData == "" {
		metaData = fmt.Sprintf("instance-id: %s\nlocal-hostname: %s\n", vmID, vmID)
	}
	if err := os.WriteFile(filepath.Join(tempDir, "meta-data"), []byte(metaData), 0644); err != nil {
		return "", fmt.Errorf("failed to write meta-data: %w", err)
	}

	// Write network-config if provided
	if config.NetworkConfig != "" {
		if err := os.WriteFile(filepath.Join(tempDir, "network-config"), []byte(config.NetworkConfig), 0644); err != nil {
			return "", fmt.Errorf("failed to write network-config: %w", err)
		}
	}

	// Ensure output directory exists
	if err := os.MkdirAll(g.outputDir, 0755); err != nil {
		return "", fmt.Errorf("failed to create output directory: %w", err)
	}

	// Generate ISO filename
	isoPath := filepath.Join(g.outputDir, vmID+"-cloudinit.iso")

	// Try xorrisofs first (preferred), fall back to mkisofs
	if err := g.createISO(tempDir, isoPath); err != nil {
		return "", fmt.Errorf("failed to create ISO: %w", err)
	}

	// Verify ISO was created
	info, err := os.Stat(isoPath)
	if err != nil {
		return "", fmt.Errorf("ISO file not created: %w", err)
	}
	if info.Size() == 0 {
		return "", fmt.Errorf("ISO file is empty")
	}

	return isoPath, nil
}

// createISO creates an ISO image from the source directory.
// Tries xorrisofs first, then mkisofs.
func (g *ISOGenerator) createISO(sourceDir, outputPath string) error {
	// Try xorrisofs (preferred, more modern)
	if g.hasCommand("xorrisofs") {
		return g.createISOWithXorrisofs(sourceDir, outputPath)
	}

	// Fall back to mkisofs
	if g.hasCommand("mkisofs") {
		return g.createISOWithMkisofs(sourceDir, outputPath)
	}

	// Try genisoimage (Debian/Ubuntu alternative name for mkisofs)
	if g.hasCommand("genisoimage") {
		return g.createISOWithGenisoimage(sourceDir, outputPath)
	}

	return fmt.Errorf("no ISO creation tool found (tried: xorrisofs, mkisofs, genisoimage)")
}

// hasCommand checks if a command is available in PATH.
func (g *ISOGenerator) hasCommand(name string) bool {
	_, err := exec.LookPath(name)
	return err == nil
}

// createISOWithXorrisofs creates ISO using xorrisofs.
func (g *ISOGenerator) createISOWithXorrisofs(sourceDir, outputPath string) error {
	cmd := exec.Command(
		"xorrisofs",
		"-input-charset", "utf-8",
		"-o", outputPath,
		"-V", "cidata",
		"-J", "-R",
		sourceDir,
	)

	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("xorrisofs failed: %v (output: %s)", err, string(output))
	}

	return nil
}

// createISOWithMkisofs creates ISO using mkisofs.
func (g *ISOGenerator) createISOWithMkisofs(sourceDir, outputPath string) error {
	cmd := exec.Command(
		"mkisofs",
		"-input-charset", "utf-8",
		"-o", outputPath,
		"-V", "cidata",
		"-J", "-R",
		sourceDir,
	)

	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("mkisofs failed: %v (output: %s)", err, string(output))
	}

	return nil
}

// createISOWithGenisoimage creates ISO using genisoimage.
func (g *ISOGenerator) createISOWithGenisoimage(sourceDir, outputPath string) error {
	cmd := exec.Command(
		"genisoimage",
		"-input-charset", "utf-8",
		"-o", outputPath,
		"-V", "cidata",
		"-J", "-R",
		sourceDir,
	)

	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("genisoimage failed: %v (output: %s)", err, string(output))
	}

	return nil
}

// DeleteISO removes a cloud-init ISO file.
func (g *ISOGenerator) DeleteISO(vmID string) error {
	isoPath := filepath.Join(g.outputDir, vmID+"-cloudinit.iso")
	
	// Check if file exists
	if _, err := os.Stat(isoPath); os.IsNotExist(err) {
		return nil // Already deleted
	}

	if err := os.Remove(isoPath); err != nil {
		return fmt.Errorf("failed to delete ISO: %w", err)
	}

	return nil
}

// GetISOPath returns the path to a VM's cloud-init ISO.
func (g *ISOGenerator) GetISOPath(vmID string) string {
	return filepath.Join(g.outputDir, vmID+"-cloudinit.iso")
}

// ISOExists checks if a cloud-init ISO exists.
func (g *ISOGenerator) ISOExists(vmID string) bool {
	isoPath := g.GetISOPath(vmID)
	info, err := os.Stat(isoPath)
	if err != nil {
		return false
	}
	return info.Size() > 0
}

// ValidateISO checks if an ISO file is valid.
// This is a basic check - verifies file exists and has content.
// For a more thorough check, we'd use isoinfo or similar.
func (g *ISOGenerator) ValidateISO(isoPath string) error {
	info, err := os.Stat(isoPath)
	if err != nil {
		if os.IsNotExist(err) {
			return fmt.Errorf("ISO file does not exist: %s", isoPath)
		}
		return fmt.Errorf("failed to stat ISO: %w", err)
	}

	if info.Size() == 0 {
		return fmt.Errorf("ISO file is empty: %s", isoPath)
	}

	// ISO9660 files should be at least 32KB (system area + primary volume descriptor)
	if info.Size() < 32768 {
		return fmt.Errorf("ISO file too small (%d bytes), may be corrupted", info.Size())
	}

	return nil
}

// ListISOs returns a list of all cloud-init ISO files in the output directory.
func (g *ISOGenerator) ListISOs() ([]string, error) {
	entries, err := os.ReadDir(g.outputDir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("failed to read output directory: %w", err)
	}

	var isos []string
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		if filepath.Ext(entry.Name()) == ".iso" {
			isos = append(isos, filepath.Join(g.outputDir, entry.Name()))
		}
	}

	return isos, nil
}
