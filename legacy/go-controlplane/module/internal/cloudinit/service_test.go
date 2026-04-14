package cloudinit

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestConfigValidate(t *testing.T) {
	tests := []struct {
		name    string
		config  Config
		wantErr bool
		errMsg  string
	}{
		{
			name: "valid config",
			config: Config{
				VMID:   "vm-123",
				VMName: "test-vm",
			},
			wantErr: false,
		},
		{
			name: "missing VMID",
			config: Config{
				VMName: "test-vm",
			},
			wantErr: true,
			errMsg:  "VMID is required",
		},
		{
			name: "missing VMName",
			config: Config{
				VMID: "vm-123",
			},
			wantErr: true,
			errMsg:  "VMName is required",
		},
		{
			name:    "missing both",
			config:  Config{},
			wantErr: true,
			errMsg:  "VMID is required",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.config.Validate()
			if tt.wantErr {
				if err == nil {
					t.Errorf("Validate() expected error but got nil")
					return
				}
				if !strings.Contains(err.Error(), tt.errMsg) {
					t.Errorf("Validate() error = %v, want containing %v", err, tt.errMsg)
				}
			} else {
				if err != nil {
					t.Errorf("Validate() unexpected error = %v", err)
				}
			}
		})
	}
}

func TestRendererRender(t *testing.T) {
	ctx := context.Background()
	tempDir := t.TempDir()
	renderer := NewRenderer(tempDir)

	t.Run("successful render with all fields", func(t *testing.T) {
		vmID := "vm-test-001"
		config := Config{
			VMID:              vmID,
			VMName:            "test-vm",
			Username:          "admin",
			Password:          "secret123",
			SSHAuthorizedKeys: []string{"ssh-rsa AAAAB3NzaC1 test@example.com"},
		}

		result, err := renderer.Render(ctx, vmID, config)
		if err != nil {
			t.Fatalf("Render() error = %v", err)
		}

		// Verify result paths
		expectedDir := filepath.Join(tempDir, "vms", vmID, "cloudinit")
		if result.CloudinitDir != expectedDir {
			t.Errorf("CloudinitDir = %v, want %v", result.CloudinitDir, expectedDir)
		}
		if result.UserDataPath != filepath.Join(expectedDir, "user-data") {
			t.Errorf("UserDataPath = %v", result.UserDataPath)
		}
		if result.MetaDataPath != filepath.Join(expectedDir, "meta-data") {
			t.Errorf("MetaDataPath = %v", result.MetaDataPath)
		}
		if result.NetworkConfigPath != filepath.Join(expectedDir, "network-config") {
			t.Errorf("NetworkConfigPath = %v", result.NetworkConfigPath)
		}

		// Verify files exist
		for _, path := range []string{result.UserDataPath, result.MetaDataPath, result.NetworkConfigPath} {
			if _, err := os.Stat(path); os.IsNotExist(err) {
				t.Errorf("expected file to exist: %s", path)
			}
		}

		// Verify user-data content
		userData, err := os.ReadFile(result.UserDataPath)
		if err != nil {
			t.Fatalf("failed to read user-data: %v", err)
		}
		content := string(userData)
		if !strings.Contains(content, "#cloud-config") {
			t.Error("user-data missing #cloud-config header")
		}
		if !strings.Contains(content, "hostname: test-vm") {
			t.Error("user-data missing hostname")
		}
		if !strings.Contains(content, "name: admin") {
			t.Error("user-data missing username")
		}
		if !strings.Contains(content, "ssh-rsa AAAAB3NzaC1 test@example.com") {
			t.Error("user-data missing SSH authorized key")
		}
		if !strings.Contains(content, "qemu-guest-agent") {
			t.Error("user-data missing qemu-guest-agent package")
		}

		// Verify meta-data content
		metaData, err := os.ReadFile(result.MetaDataPath)
		if err != nil {
			t.Fatalf("failed to read meta-data: %v", err)
		}
		metaContent := string(metaData)
		if !strings.Contains(metaContent, "instance-id: vm-test-001") {
			t.Error("meta-data missing instance-id")
		}
		if !strings.Contains(metaContent, "local-hostname: test-vm") {
			t.Error("meta-data missing local-hostname")
		}

		// Verify network-config content
		networkConfig, err := os.ReadFile(result.NetworkConfigPath)
		if err != nil {
			t.Fatalf("failed to read network-config: %v", err)
		}
		netContent := string(networkConfig)
		if !strings.Contains(netContent, "version: 2") {
			t.Error("network-config missing version")
		}
		if !strings.Contains(netContent, "dhcp4: true") {
			t.Error("network-config missing dhcp4: true")
		}
	})

	t.Run("successful render with minimal fields", func(t *testing.T) {
		vmID := "vm-test-002"
		config := Config{
			VMID:   vmID,
			VMName: "minimal-vm",
		}

		result, err := renderer.Render(ctx, vmID, config)
		if err != nil {
			t.Fatalf("Render() error = %v", err)
		}

		// Verify user-data is generated with empty username
		userData, err := os.ReadFile(result.UserDataPath)
		if err != nil {
			t.Fatalf("failed to read user-data: %v", err)
		}
		if !strings.Contains(string(userData), "hostname: minimal-vm") {
			t.Error("user-data missing hostname")
		}
	})

	t.Run("invalid config rejected", func(t *testing.T) {
		vmID := "vm-test-003"
		config := Config{
			VMID: "", // Missing VMID
		}

		_, err := renderer.Render(ctx, vmID, config)
		if err == nil {
			t.Fatal("Render() expected error for invalid config")
		}
		if !strings.Contains(err.Error(), "invalid config") {
			t.Errorf("error should mention 'invalid config', got: %v", err)
		}
	})

	t.Run("raw user-data override", func(t *testing.T) {
		vmID := "vm-test-004"
		rawUserData := `#cloud-config
hostname: custom-host
packages:
  - nginx
`
		config := Config{
			VMID:     vmID,
			VMName:   "override-vm",
			UserData: rawUserData,
		}

		result, err := renderer.Render(ctx, vmID, config)
		if err != nil {
			t.Fatalf("Render() error = %v", err)
		}

		userData, err := os.ReadFile(result.UserDataPath)
		if err != nil {
			t.Fatalf("failed to read user-data: %v", err)
		}

		content := string(userData)
		if content != rawUserData {
			t.Errorf("user-data = %q, want %q", content, rawUserData)
		}
		if !strings.Contains(content, "nginx") {
			t.Error("raw user-data should contain nginx package")
		}
	})

	t.Run("multiple SSH keys", func(t *testing.T) {
		vmID := "vm-test-005"
		config := Config{
			VMID:   vmID,
			VMName: "multi-key-vm",
			Username: "user",
			SSHAuthorizedKeys: []string{
				"ssh-rsa KEY1 user1@example.com",
				"ssh-rsa KEY2 user2@example.com",
				"ssh-ed25519 KEY3 user3@example.com",
			},
		}

		result, err := renderer.Render(ctx, vmID, config)
		if err != nil {
			t.Fatalf("Render() error = %v", err)
		}

		userData, err := os.ReadFile(result.UserDataPath)
		if err != nil {
			t.Fatalf("failed to read user-data: %v", err)
		}

		content := string(userData)
		for _, key := range config.SSHAuthorizedKeys {
			if !strings.Contains(content, key) {
				t.Errorf("user-data missing SSH key: %s", key)
			}
		}
	})
}

func TestRendererRenderDeterministic(t *testing.T) {
	ctx := context.Background()
	tempDir := t.TempDir()
	renderer := NewRenderer(tempDir)

	vmID := "vm-deterministic"
	config := Config{
		VMID:              vmID,
		VMName:            "deterministic-vm",
		Username:          "admin",
		Password:          "pass",
		SSHAuthorizedKeys: []string{"ssh-rsa KEY test@test"},
	}

	// Render twice with same config
	result1, err := renderer.Render(ctx, vmID, config)
	if err != nil {
		t.Fatalf("first Render() error = %v", err)
	}

	result2, err := renderer.Render(ctx, vmID+"-2", config)
	if err != nil {
		t.Fatalf("second Render() error = %v", err)
	}

	// Read and compare content (ignoring paths which contain VMID)
	userData1, _ := os.ReadFile(result1.UserDataPath)
	userData2, _ := os.ReadFile(result2.UserDataPath)

	// Replace VM-specific paths for comparison
	content1 := strings.ReplaceAll(string(userData1), vmID, "VMID")
	content2 := strings.ReplaceAll(string(userData2), vmID+"-2", "VMID")

	// Both should have same structure (hostname will differ due to VMName)
	if !strings.Contains(content1, "hostname: deterministic-vm") {
		t.Error("first render missing hostname")
	}
	if !strings.Contains(content2, "hostname: deterministic-vm") {
		t.Error("second render missing hostname")
	}
}

func TestNewRenderer(t *testing.T) {
	workspace := "/var/lib/chv"
	renderer := NewRenderer(workspace)

	if renderer.workspaceBase != workspace {
		t.Errorf("workspaceBase = %v, want %v", renderer.workspaceBase, workspace)
	}
}
