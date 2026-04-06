//go:build integration
// +build integration

package integration

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/chv/chv/internal/agent/ch"
)

// TestVMLifecycle tests the full VM lifecycle with real CH
func TestVMLifecycle(t *testing.T) {
	chPath := os.Getenv("CHV_CH_PATH")
	if chPath == "" {
		chPath = "../../../bin/cloud-hypervisor"
	}

	if _, err := os.Stat(chPath); os.IsNotExist(err) {
		t.Skip("Cloud Hypervisor binary not found at ", chPath)
	}

	kernelPath := os.Getenv("CHV_TEST_KERNEL")
	if kernelPath == "" {
		t.Skip("Set CHV_TEST_KERNEL to run VM lifecycle test")
	}

	ctx := context.Background()
	helper := NewCHTestHelper(t, chPath)
	defer helper.Cleanup()

	apiSocket := filepath.Join(helper.TempDir, "ch-api.sock")
	if err := helper.StartCHAPIServer(ctx, apiSocket); err != nil {
		t.Fatalf("Failed to start CH API server: %v", err)
	}

	client := ch.NewClient(apiSocket)

	config := &ch.VMConfig{
		Cpus: ch.CpusConfig{
			BootVcpus: 1,
			MaxVcpus:  2,
		},
		Memory: ch.MemoryConfig{
			Size: 512 * 1024 * 1024,
		},
		Kernel: ch.KernelConfig{
			Path: kernelPath,
		},
		Console: ch.ConsoleConfig{
			Mode: "Off",
		},
	}

	// Create VM
	t.Run("Create", func(t *testing.T) {
		if err := client.CreateVM(config); err != nil {
			t.Fatalf("CreateVM failed: %v", err)
		}
		info, _ := client.GetVMInfo()
		if info.State != "Created" {
			t.Errorf("Expected Created, got %s", info.State)
		}
	})

	// Delete VM
	t.Run("Delete", func(t *testing.T) {
		if err := client.DeleteVM(); err != nil {
			t.Fatalf("DeleteVM failed: %v", err)
		}
	})
}
