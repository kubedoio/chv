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

// TestCHClientLifecycle tests basic CH client operations
func TestCHClientLifecycle(t *testing.T) {
	chPath := os.Getenv("CHV_CH_PATH")
	if chPath == "" {
		chPath = "../../../bin/cloud-hypervisor"
	}

	if _, err := os.Stat(chPath); os.IsNotExist(err) {
		t.Skip("Cloud Hypervisor binary not found at ", chPath)
	}

	ctx := context.Background()
	helper := NewCHTestHelper(t, chPath)
	defer helper.Cleanup()

	// Start CH in API server mode
	apiSocket := filepath.Join(helper.TempDir, "ch-api.sock")
	if err := helper.StartCHAPIServer(ctx, apiSocket); err != nil {
		t.Fatalf("Failed to start CH API server: %v", err)
	}

	// Create client
	client := ch.NewClient(apiSocket)

	// Test VMM ping
	t.Run("Ping", func(t *testing.T) {
		if err := client.PingVMM(); err != nil {
			t.Errorf("PingVMM failed: %v", err)
		}
	})
}

// TestCHClientVMOperations tests VM operations against real CH
func TestCHClientVMOperations(t *testing.T) {
	chPath := os.Getenv("CHV_CH_PATH")
	if chPath == "" {
		chPath = "../../../bin/cloud-hypervisor"
	}

	if _, err := os.Stat(chPath); os.IsNotExist(err) {
		t.Skip("Cloud Hypervisor binary not found at ", chPath)
	}

	// Check for kernel
	kernelPath := os.Getenv("CHV_TEST_KERNEL")
	if kernelPath == "" {
		t.Skip("Set CHV_TEST_KERNEL to run VM operations test")
	}

	ctx := context.Background()
	helper := NewCHTestHelper(t, chPath)
	defer helper.Cleanup()

	apiSocket := filepath.Join(helper.TempDir, "ch-api.sock")
	if err := helper.StartCHAPIServer(ctx, apiSocket); err != nil {
		t.Fatalf("Failed to start CH API server: %v", err)
	}

	client := ch.NewClient(apiSocket)

	// Test VM create
	t.Run("CreateVM", func(t *testing.T) {
		config := &ch.VMConfig{
			Cpus: ch.CpusConfig{
				BootVcpus: 2,
				MaxVcpus:  4,
			},
			Memory: ch.MemoryConfig{
				Size: 1024 * 1024 * 1024, // 1GB
			},
			Kernel: ch.KernelConfig{
				Path: kernelPath,
			},
			Console: ch.ConsoleConfig{
				Mode: "Off",
			},
		}

		if err := client.CreateVM(config); err != nil {
			t.Errorf("CreateVM failed: %v", err)
		}

		// Verify VM was created
		info, err := client.GetVMInfo()
		if err != nil {
			t.Errorf("GetVMInfo failed: %v", err)
			return
		}
		if info.State != "Created" {
			t.Errorf("Expected state 'Created', got '%s'", info.State)
		}
	})

	// Test VM delete
	t.Run("DeleteVM", func(t *testing.T) {
		if err := client.DeleteVM(); err != nil {
			t.Errorf("DeleteVM failed: %v", err)
		}
	})
}

// TestCHClientMetrics tests metrics collection
func TestCHClientMetrics(t *testing.T) {
	chPath := os.Getenv("CHV_CH_PATH")
	if chPath == "" {
		chPath = "../../../bin/cloud-hypervisor"
	}

	if _, err := os.Stat(chPath); os.IsNotExist(err) {
		t.Skip("Cloud Hypervisor binary not found at ", chPath)
	}

	ctx := context.Background()
	helper := NewCHTestHelper(t, chPath)
	defer helper.Cleanup()

	apiSocket := filepath.Join(helper.TempDir, "ch-api.sock")
	if err := helper.StartCHAPIServer(ctx, apiSocket); err != nil {
		t.Fatalf("Failed to start CH API server: %v", err)
	}

	client := ch.NewClient(apiSocket)

	t.Run("Counters", func(t *testing.T) {
		counters, err := client.GetCounters()
		if err != nil {
			t.Errorf("GetCounters failed: %v", err)
			return
		}
		t.Logf("Counters received: %+v", counters)
	})
}
