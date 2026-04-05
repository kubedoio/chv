package cloudinit

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestISOGenerator_GenerateISO(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	config := &Config{
		UserData: "#cloud-config\nusers:\n  - name: test\n",
		MetaData: "instance-id: test-vm\nlocal-hostname: test-vm\n",
		NetworkConfig: "version: 2\nethernets:\n  eth0:\n    dhcp4: true\n",
	}

	isoPath, err := gen.GenerateISO("test-vm-123", config)
	if err != nil {
		// ISO tools might not be installed, skip test
		if strings.Contains(err.Error(), "no ISO creation tool found") {
			t.Skip("Skipping: no ISO creation tool installed (xorrisofs, mkisofs, or genisoimage)")
		}
		t.Fatalf("GenerateISO failed: %v", err)
	}

	// Verify ISO was created
	if _, err := os.Stat(isoPath); os.IsNotExist(err) {
		t.Fatal("ISO file was not created")
	}

	// Verify path is correct
	expectedPath := filepath.Join(tmpDir, "test-vm-123-cloudinit.iso")
	if isoPath != expectedPath {
		t.Errorf("ISO path mismatch: got %s, want %s", isoPath, expectedPath)
	}

	// Verify ISO is valid (basic check)
	if err := gen.ValidateISO(isoPath); err != nil {
		t.Errorf("ISO validation failed: %v", err)
	}
}

func TestISOGenerator_GenerateISO_Defaults(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	// Test with empty config (should use defaults)
	config := &Config{}

	isoPath, err := gen.GenerateISO("test-vm-defaults", config)
	if err != nil {
		if strings.Contains(err.Error(), "no ISO creation tool found") {
			t.Skip("Skipping: no ISO creation tool installed")
		}
		t.Fatalf("GenerateISO failed: %v", err)
	}

	// Verify ISO was created
	if _, err := os.Stat(isoPath); os.IsNotExist(err) {
		t.Fatal("ISO file was not created")
	}
}

func TestISOGenerator_GenerateISO_NoNetworkConfig(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	config := &Config{
		UserData: "#cloud-config\n",
		MetaData: "instance-id: test\n",
		// No NetworkConfig
	}

	isoPath, err := gen.GenerateISO("test-vm-nonet", config)
	if err != nil {
		if strings.Contains(err.Error(), "no ISO creation tool found") {
			t.Skip("Skipping: no ISO creation tool installed")
		}
		t.Fatalf("GenerateISO failed: %v", err)
	}

	// Verify ISO was created
	if _, err := os.Stat(isoPath); os.IsNotExist(err) {
		t.Fatal("ISO file was not created")
	}
}

func TestISOGenerator_DeleteISO(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	// Create a dummy ISO file
	isoPath := filepath.Join(tmpDir, "test-delete-cloudinit.iso")
	if err := os.WriteFile(isoPath, []byte("dummy iso content"), 0644); err != nil {
		t.Fatalf("Failed to create test file: %v", err)
	}

	// Delete it
	if err := gen.DeleteISO("test-delete"); err != nil {
		t.Fatalf("DeleteISO failed: %v", err)
	}

	// Verify it's gone
	if _, err := os.Stat(isoPath); !os.IsNotExist(err) {
		t.Error("ISO file should be deleted")
	}

	// Deleting non-existent should not error
	if err := gen.DeleteISO("non-existent"); err != nil {
		t.Errorf("DeleteISO non-existent should not error: %v", err)
	}
}

func TestISOGenerator_ISOExists(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	vmID := "test-exists"

	// Initially should not exist
	if gen.ISOExists(vmID) {
		t.Error("ISO should not exist initially")
	}

	// Create dummy file
	isoPath := gen.GetISOPath(vmID)
	if err := os.WriteFile(isoPath, []byte("dummy content"), 0644); err != nil {
		t.Fatalf("Failed to create test file: %v", err)
	}

	// Now should exist
	if !gen.ISOExists(vmID) {
		t.Error("ISO should exist after creation")
	}

	// Empty file should not count as existing
	if err := os.WriteFile(isoPath, []byte{}, 0644); err != nil {
		t.Fatalf("Failed to create empty file: %v", err)
	}
	if gen.ISOExists(vmID) {
		t.Error("Empty ISO should not count as existing")
	}
}

func TestISOGenerator_GetISOPath(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	vmID := "test-path"
	expected := filepath.Join(tmpDir, "test-path-cloudinit.iso")
	
	got := gen.GetISOPath(vmID)
	if got != expected {
		t.Errorf("GetISOPath() = %s, want %s", got, expected)
	}
}

func TestISOGenerator_ValidateISO(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	// Test non-existent file
	err := gen.ValidateISO(filepath.Join(tmpDir, "non-existent.iso"))
	if err == nil {
		t.Error("ValidateISO should error for non-existent file")
	}

	// Test empty file
	emptyPath := filepath.Join(tmpDir, "empty.iso")
	if err := os.WriteFile(emptyPath, []byte{}, 0644); err != nil {
		t.Fatalf("Failed to create empty file: %v", err)
	}
	
	err = gen.ValidateISO(emptyPath)
	if err == nil {
		t.Error("ValidateISO should error for empty file")
	}

	// Test small file (< 32KB)
	smallPath := filepath.Join(tmpDir, "small.iso")
	if err := os.WriteFile(smallPath, make([]byte, 1000), 0644); err != nil {
		t.Fatalf("Failed to create small file: %v", err)
	}
	
	err = gen.ValidateISO(smallPath)
	if err == nil {
		t.Error("ValidateISO should error for small file")
	}

	// Test valid-sized file
	validPath := filepath.Join(tmpDir, "valid.iso")
	if err := os.WriteFile(validPath, make([]byte, 35000), 0644); err != nil {
		t.Fatalf("Failed to create valid file: %v", err)
	}
	
	err = gen.ValidateISO(validPath)
	if err != nil {
		t.Errorf("ValidateISO should succeed for valid file: %v", err)
	}
}

func TestISOGenerator_ListISOs(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	// Initially empty
	isos, err := gen.ListISOs()
	if err != nil {
		t.Fatalf("ListISOs failed: %v", err)
	}
	if len(isos) != 0 {
		t.Errorf("Expected 0 ISOs, got %d", len(isos))
	}

	// Create some ISO files
	for _, name := range []string{"vm1-cloudinit.iso", "vm2-cloudinit.iso", "vm3-cloudinit.iso"} {
		path := filepath.Join(tmpDir, name)
		if err := os.WriteFile(path, make([]byte, 35000), 0644); err != nil {
			t.Fatalf("Failed to create test file: %v", err)
		}
	}

	// Create a non-ISO file
	if err := os.WriteFile(filepath.Join(tmpDir, "not-an-iso.txt"), []byte("text"), 0644); err != nil {
		t.Fatalf("Failed to create test file: %v", err)
	}

	// Create a subdirectory
	if err := os.Mkdir(filepath.Join(tmpDir, "subdir"), 0755); err != nil {
		t.Fatalf("Failed to create subdir: %v", err)
	}

	// List should return only ISOs
	isos, err = gen.ListISOs()
	if err != nil {
		t.Fatalf("ListISOs failed: %v", err)
	}
	if len(isos) != 3 {
		t.Errorf("Expected 3 ISOs, got %d", len(isos))
	}
}

func TestISOGenerator_hasCommand(t *testing.T) {
	gen := NewISOGenerator("/tmp")

	// "ls" should exist on any Unix system
	if !gen.hasCommand("ls") {
		t.Error("hasCommand should return true for 'ls'")
	}

	// "nonexistent-command-12345" should not exist
	if gen.hasCommand("nonexistent-command-12345") {
		t.Error("hasCommand should return false for non-existent command")
	}
}

func TestISOGenerator_hasCommand_NotFound(t *testing.T) {
	gen := NewISOGenerator("/tmp")

	// Test with command that definitely doesn't exist
	result := gen.hasCommand("xyz-not-a-real-command-abc123")
	if result {
		t.Error("hasCommand should return false for fake command")
	}
}
