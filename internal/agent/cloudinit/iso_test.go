package cloudinit

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestNewISOGenerator(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	if gen == nil {
		t.Fatal("NewISOGenerator returned nil")
	}

	if gen.vmDataDir != tmpDir {
		t.Errorf("vmDataDir = %s, want %s", gen.vmDataDir, tmpDir)
	}

	// Verify cloudinit subdirectory is set up correctly
	isoDir := filepath.Join(tmpDir, "cloudinit")
	if gen.core == nil {
		t.Error("core generator is nil")
	}

	// The directory should be created on first use, not on initialization
	_, err := os.Stat(isoDir)
	if !os.IsNotExist(err) {
		t.Error("cloudinit directory should not exist until first use")
	}
}

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

	// Verify ISO was created in the cloudinit directory
	expectedDir := filepath.Join(tmpDir, "cloudinit")
	if filepath.Dir(isoPath) != expectedDir {
		t.Errorf("ISO directory mismatch: got %s, want %s", filepath.Dir(isoPath), expectedDir)
	}

	// Verify file exists
	if _, err := os.Stat(isoPath); os.IsNotExist(err) {
		t.Fatal("ISO file was not created")
	}

	// Verify it's a valid ISO
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

func TestISOGenerator_GenerateISO_InvalidVMID(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)
	config := &Config{UserData: "#cloud-config\n"}

	// Test empty VM ID
	_, err := gen.GenerateISO("", config)
	if err == nil {
		t.Error("GenerateISO should fail with empty VM ID")
	}

	// Test VM ID with path traversal
	_, err = gen.GenerateISO("../etc/passwd", config)
	if err == nil {
		t.Error("GenerateISO should fail with path traversal in VM ID")
	}

	// Test absolute path VM ID
	_, err = gen.GenerateISO("/etc/passwd", config)
	if err == nil {
		t.Error("GenerateISO should fail with absolute path VM ID")
	}
}

func TestISOGenerator_GenerateISOForVM(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	config := &Config{
		UserData: "#cloud-config\nusers:\n  - name: test\n",
		MetaData: "instance-id: test-vm\nlocal-hostname: test-vm\n",
	}

	isoPath, err := gen.GenerateISOForVM("test-vm-456", config)
	if err != nil {
		if strings.Contains(err.Error(), "no ISO creation tool found") {
			t.Skip("Skipping: no ISO creation tool installed")
		}
		t.Fatalf("GenerateISOForVM failed: %v", err)
	}

	// Verify ISO was created in the VM directory
	expectedPath := filepath.Join(tmpDir, "instances", "test-vm-456", "cloudinit.iso")
	if isoPath != expectedPath {
		t.Errorf("ISO path mismatch: got %s, want %s", isoPath, expectedPath)
	}

	// Verify file exists
	if _, err := os.Stat(isoPath); os.IsNotExist(err) {
		t.Fatal("ISO file was not created")
	}
}

func TestISOGenerator_DeleteISO(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	// Create ISO in cloudinit directory
	isoDir := filepath.Join(tmpDir, "cloudinit")
	if err := os.MkdirAll(isoDir, 0750); err != nil {
		t.Fatalf("Failed to create ISO directory: %v", err)
	}

	isoPath := filepath.Join(isoDir, "test-delete-cloudinit.iso")
	if err := os.WriteFile(isoPath, []byte("dummy iso content"), 0644); err != nil {
		t.Fatalf("Failed to create test file: %v", err)
	}

	// Delete it
	if err := gen.DeleteISO("test-delete"); err != nil {
		t.Fatalf("DeleteISO failed: %v", err)
	}

	// Verify it's gone from cloudinit directory
	if _, err := os.Stat(isoPath); !os.IsNotExist(err) {
		t.Error("ISO file should be deleted from cloudinit directory")
	}

	// Deleting non-existent should not error
	if err := gen.DeleteISO("non-existent"); err != nil {
		t.Errorf("DeleteISO non-existent should not error: %v", err)
	}
}

func TestISOGenerator_DeleteISOByPath(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	// Create a test ISO
	isoPath := filepath.Join(tmpDir, "test-iso.iso")
	if err := os.WriteFile(isoPath, []byte("dummy iso content"), 0644); err != nil {
		t.Fatalf("Failed to create test file: %v", err)
	}

	// Delete by path
	if err := gen.DeleteISOByPath(isoPath); err != nil {
		t.Fatalf("DeleteISOByPath failed: %v", err)
	}

	// Verify it's gone
	if _, err := os.Stat(isoPath); !os.IsNotExist(err) {
		t.Error("ISO file should be deleted")
	}

	// Deleting empty path should not error
	if err := gen.DeleteISOByPath(""); err != nil {
		t.Errorf("DeleteISOByPath empty should not error: %v", err)
	}

	// Deleting non-existent should not error
	if err := gen.DeleteISOByPath("/non/existent/path.iso"); err != nil {
		t.Errorf("DeleteISOByPath non-existent should not error: %v", err)
	}
}

func TestISOGenerator_DeleteISO_BothLocations(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	// Create ISO in cloudinit directory
	isoDir := filepath.Join(tmpDir, "cloudinit")
	if err := os.MkdirAll(isoDir, 0750); err != nil {
		t.Fatalf("Failed to create ISO directory: %v", err)
	}
	cloudinitISO := filepath.Join(isoDir, "test-dual-cloudinit.iso")
	if err := os.WriteFile(cloudinitISO, []byte("dummy"), 0644); err != nil {
		t.Fatalf("Failed to create test file: %v", err)
	}

	// Create ISO in VM directory
	vmDir := filepath.Join(tmpDir, "instances", "test-dual")
	if err := os.MkdirAll(vmDir, 0750); err != nil {
		t.Fatalf("Failed to create VM directory: %v", err)
	}
	vmISO := filepath.Join(vmDir, "cloudinit.iso")
	if err := os.WriteFile(vmISO, []byte("dummy"), 0644); err != nil {
		t.Fatalf("Failed to create test file: %v", err)
	}

	// Delete should remove both
	if err := gen.DeleteISO("test-dual"); err != nil {
		t.Fatalf("DeleteISO failed: %v", err)
	}

	if _, err := os.Stat(cloudinitISO); !os.IsNotExist(err) {
		t.Error("ISO should be deleted from cloudinit directory")
	}
	if _, err := os.Stat(vmISO); !os.IsNotExist(err) {
		t.Error("ISO should be deleted from VM directory")
	}
}

func TestISOGenerator_GetISOPath(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	vmID := "test-path"
	expected := filepath.Join(tmpDir, "cloudinit", "test-path-cloudinit.iso")

	got := gen.GetISOPath(vmID)
	if got != expected {
		t.Errorf("GetISOPath() = %s, want %s", got, expected)
	}
}

func TestISOGenerator_GetVMISOPath(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	vmID := "test-vm-path"
	expected := filepath.Join(tmpDir, "instances", "test-vm-path", "cloudinit.iso")

	got := gen.GetVMISOPath(vmID)
	if got != expected {
		t.Errorf("GetVMISOPath() = %s, want %s", got, expected)
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
	isoDir := filepath.Join(tmpDir, "cloudinit")
	if err := os.MkdirAll(isoDir, 0750); err != nil {
		t.Fatalf("Failed to create ISO directory: %v", err)
	}
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

func TestISOGenerator_ISOExistsAtPath(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	isoPath := filepath.Join(tmpDir, "test-at-path.iso")

	// Initially should not exist
	if gen.ISOExistsAtPath(isoPath) {
		t.Error("ISO should not exist initially")
	}

	// Create file
	if err := os.WriteFile(isoPath, []byte("dummy content"), 0644); err != nil {
		t.Fatalf("Failed to create test file: %v", err)
	}

	// Now should exist
	if !gen.ISOExistsAtPath(isoPath) {
		t.Error("ISO should exist after creation")
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

func TestISOGenerator_GetISOSize(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	// Test non-existent file
	_, err := gen.GetISOSize(filepath.Join(tmpDir, "non-existent.iso"))
	if err == nil {
		t.Error("GetISOSize should error for non-existent file")
	}

	// Create test file
	testPath := filepath.Join(tmpDir, "test.iso")
	content := []byte("test content for ISO")
	if err := os.WriteFile(testPath, content, 0644); err != nil {
		t.Fatalf("Failed to create test file: %v", err)
	}

	size, err := gen.GetISOSize(testPath)
	if err != nil {
		t.Fatalf("GetISOSize failed: %v", err)
	}
	if size != int64(len(content)) {
		t.Errorf("GetISOSize = %d, want %d", size, len(content))
	}
}

func TestISOGenerator_CleanupOrphanedISOs(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	// Create ISO directory
	isoDir := filepath.Join(tmpDir, "cloudinit")
	if err := os.MkdirAll(isoDir, 0750); err != nil {
		t.Fatalf("Failed to create ISO directory: %v", err)
	}

	// Create ISOs for different VMs
	vms := []string{"vm-active", "vm-orphaned", "vm-also-orphaned"}
	for _, vm := range vms {
		isoPath := filepath.Join(isoDir, vm+"-cloudinit.iso")
		// Create file larger than 32KB to pass validation
		if err := os.WriteFile(isoPath, make([]byte, 35000), 0644); err != nil {
			t.Fatalf("Failed to create test file: %v", err)
		}
	}

	// Clean up with only vm-active being active
	if err := gen.CleanupOrphanedISOs([]string{"vm-active"}); err != nil {
		t.Fatalf("CleanupOrphanedISOs failed: %v", err)
	}

	// vm-active should still exist
	if _, err := os.Stat(filepath.Join(isoDir, "vm-active-cloudinit.iso")); os.IsNotExist(err) {
		t.Error("Active VM ISO should not be deleted")
	}

	// Orphaned ISOs should be deleted
	if _, err := os.Stat(filepath.Join(isoDir, "vm-orphaned-cloudinit.iso")); !os.IsNotExist(err) {
		t.Error("Orphaned VM ISO should be deleted")
	}
	if _, err := os.Stat(filepath.Join(isoDir, "vm-also-orphaned-cloudinit.iso")); !os.IsNotExist(err) {
		t.Error("Orphaned VM ISO should be deleted")
	}
}

func TestISOGenerator_InspectISO(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	// Create test file
	isoPath := filepath.Join(tmpDir, "test-inspect-cloudinit.iso")
	if err := os.WriteFile(isoPath, make([]byte, 35000), 0644); err != nil {
		t.Fatalf("Failed to create test file: %v", err)
	}

	info, err := gen.InspectISO(isoPath)
	if err != nil {
		t.Fatalf("InspectISO failed: %v", err)
	}

	if info.Path != isoPath {
		t.Errorf("Path = %s, want %s", info.Path, isoPath)
	}
	if info.Size != 35000 {
		t.Errorf("Size = %d, want 35000", info.Size)
	}
	if info.VMID != "test-inspect" {
		t.Errorf("VMID = %s, want test-inspect", info.VMID)
	}
	if !info.IsValid {
		t.Error("IsValid should be true")
	}
	if info.IsReadOnly {
		t.Error("IsReadOnly should be false (we created it writable)")
	}
}

func TestISOGenerator_InspectISO_NotExist(t *testing.T) {
	tmpDir := t.TempDir()
	gen := NewISOGenerator(tmpDir)

	_, err := gen.InspectISO(filepath.Join(tmpDir, "non-existent.iso"))
	if err == nil {
		t.Error("InspectISO should error for non-existent file")
	}
}

func TestValidateVMID(t *testing.T) {
	tests := []struct {
		name    string
		vmID    string
		wantErr bool
	}{
		{"valid simple", "test-vm", false},
		{"valid with dashes", "my-test-vm-123", false},
		{"valid with numbers", "vm123", false},
		{"empty", "", true},
		{"path traversal", "../etc/passwd", true},
		{"absolute path", "/etc/passwd", true},
		{"current dir", ".", true},
		{"parent dir", "..", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := validateVMID(tt.vmID)
			if (err != nil) != tt.wantErr {
				t.Errorf("validateVMID(%q) error = %v, wantErr %v", tt.vmID, err, tt.wantErr)
			}
		})
	}
}

func TestExtractVMIDFromISOPath(t *testing.T) {
	tests := []struct {
		isoPath  string
		expected string
	}{
		{"/data/cloudinit/myvm-cloudinit.iso", "myvm"},
		{"/data/cloudinit/test-vm-123-cloudinit.iso", "test-vm-123"},
		{"/data/instances/myvm/cloudinit.iso", "myvm"},
		{"/data/instances/test-vm/cloudinit.iso", "test-vm"},
		{"/data/random.iso", ""},
		{"/data/cloudinit.iso", "data"},
	}

	for _, tt := range tests {
		t.Run(tt.isoPath, func(t *testing.T) {
			got := extractVMIDFromISOPath(tt.isoPath)
			if got != tt.expected {
				t.Errorf("extractVMIDFromISOPath(%q) = %q, want %q", tt.isoPath, got, tt.expected)
			}
		})
	}
}

func TestWriteCloudInitFiles(t *testing.T) {
	tmpDir := t.TempDir()

	config := &Config{
		UserData:      "#cloud-config\nusers:\n  - name: test\n",
		MetaData:      "instance-id: test-vm\nlocal-hostname: test-vm\n",
		NetworkConfig: "version: 2\nethernets:\n  eth0:\n    dhcp4: true\n",
	}

	gen := &ISOGenerator{}
	if err := gen.writeCloudInitFiles(tmpDir, "test-vm", config); err != nil {
		t.Fatalf("writeCloudInitFiles failed: %v", err)
	}

	// Verify user-data
	userData, err := os.ReadFile(filepath.Join(tmpDir, "user-data"))
	if err != nil {
		t.Fatalf("Failed to read user-data: %v", err)
	}
	if string(userData) != config.UserData {
		t.Errorf("user-data mismatch: got %q, want %q", string(userData), config.UserData)
	}

	// Verify meta-data
	metaData, err := os.ReadFile(filepath.Join(tmpDir, "meta-data"))
	if err != nil {
		t.Fatalf("Failed to read meta-data: %v", err)
	}
	if string(metaData) != config.MetaData {
		t.Errorf("meta-data mismatch: got %q, want %q", string(metaData), config.MetaData)
	}

	// Verify network-config
	networkConfig, err := os.ReadFile(filepath.Join(tmpDir, "network-config"))
	if err != nil {
		t.Fatalf("Failed to read network-config: %v", err)
	}
	if string(networkConfig) != config.NetworkConfig {
		t.Errorf("network-config mismatch: got %q, want %q", string(networkConfig), config.NetworkConfig)
	}
}

func TestWriteCloudInitFiles_Defaults(t *testing.T) {
	tmpDir := t.TempDir()

	// Empty config should get defaults
	config := &Config{}

	gen := &ISOGenerator{}
	if err := gen.writeCloudInitFiles(tmpDir, "test-vm-default", config); err != nil {
		t.Fatalf("writeCloudInitFiles failed: %v", err)
	}

	// Verify default user-data
	userData, err := os.ReadFile(filepath.Join(tmpDir, "user-data"))
	if err != nil {
		t.Fatalf("Failed to read user-data: %v", err)
	}
	if string(userData) != "#cloud-config\n" {
		t.Errorf("user-data default mismatch: got %q", string(userData))
	}

	// Verify default meta-data
	metaData, err := os.ReadFile(filepath.Join(tmpDir, "meta-data"))
	if err != nil {
		t.Fatalf("Failed to read meta-data: %v", err)
	}
	expectedMetaData := "instance-id: test-vm-default\nlocal-hostname: test-vm-default\n"
	if string(metaData) != expectedMetaData {
		t.Errorf("meta-data default mismatch: got %q, want %q", string(metaData), expectedMetaData)
	}

	// network-config should not exist when not provided
	_, err = os.Stat(filepath.Join(tmpDir, "network-config"))
	if !os.IsNotExist(err) {
		t.Error("network-config should not exist when not provided")
	}
}
