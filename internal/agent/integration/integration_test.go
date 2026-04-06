// Package integration provides integration tests for Cloud Hypervisor.
package integration

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
	"time"

	"github.com/chv/chv/internal/hypervisor"
)

func TestCHBinaryAvailability(t *testing.T) {
	SkipIfCHNotAvailable(t)
	version := VerifyCHVersion(t)
	if version != "" {
		t.Logf("Cloud Hypervisor version: %s", strings.TrimSpace(version))
	}
}

func TestCHClientOperations(t *testing.T) {
	SkipIfCHNotAvailable(t)
	SkipIfNoKVM(t)
	env := SetupTestEnvironment(t)
	defer env.Cleanup()
	vmID := GenerateTestVMID(t)
	diskPath, err := env.CreateTestDisk(vmID, DefaultTestDiskSize)
	if err != nil {
		t.Fatalf("Failed to create test disk: %v", err)
	}
	config := env.CreateTestVMConfig(vmID)
	config.VolumePath = diskPath
	ciConfig := CreateTestCloudInitConfig("test-vm")
	isoPath, err := env.ISOGenerator.GenerateISO(vmID, ciConfig)
	if err != nil {
		t.Fatalf("Failed to generate cloud-init ISO: %v", err)
	}
	config.CloudInitISO = isoPath
	instance, err := env.Launcher.StartVM(config, "test-operation-1")
	if err != nil {
		t.Fatalf("Failed to start VM: %v", err)
	}
	defer func() {
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer cancel()
		_ = env.Launcher.StopVM(vmID, true, "cleanup")
		_ = WaitForVMStopped(ctx, instance)
	}()
	client := hypervisor.NewCHVClient(instance.APISocket)
	t.Run("Ping", func(t *testing.T) {
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		if err := client.Ping(ctx); err != nil {
			t.Errorf("Ping failed: %v", err)
		}
	})
	t.Run("GetVMInfo", func(t *testing.T) {
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		info, err := client.GetVMInfo(ctx)
		if err != nil {
			t.Errorf("GetVMInfo failed: %v", err)
			return
		}
		if info.State != "Running" {
			t.Errorf("Expected state Running, got %s", info.State)
		}
		if info.Config.Cpus.BootVcpus != config.VCPU {
			t.Errorf("Expected %d vCPUs, got %d", config.VCPU, info.Config.Cpus.BootVcpus)
		}
		t.Logf("VM Info: State=%s, vCPUs=%d, Memory=%d", info.State, info.Config.Cpus.BootVcpus, info.Config.Memory.Size)
	})
	t.Run("IsRunning", func(t *testing.T) {
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		running, err := client.IsRunning(ctx)
		if err != nil {
			t.Errorf("IsRunning failed: %v", err)
			return
		}
		if !running {
			t.Error("Expected VM to be running")
		}
	})
	t.Run("PauseResume", func(t *testing.T) {
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		if err := client.Pause(ctx); err != nil {
			t.Logf("Pause failed (may not be supported): %v", err)
		} else {
			info, _ := client.GetVMInfo(ctx)
			if info.State == "Paused" {
				t.Log("VM is paused")
				if err := client.Resume(ctx); err != nil {
					t.Errorf("Resume failed: %v", err)
				}
				if err := WaitForVMRunning(ctx, client); err != nil {
					t.Errorf("VM did not resume: %v", err)
				}
				t.Log("VM resumed")
			}
		}
	})
	t.Run("GetVMCounters", func(t *testing.T) {
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		counters, err := client.GetVMCounters(ctx)
		if err != nil {
			t.Logf("GetVMCounters not available: %v", err)
			return
		}
		t.Logf("VM Counters: %+v", counters)
	})
	t.Run("Shutdown", func(t *testing.T) {
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer cancel()
		if err := client.Shutdown(ctx); err != nil {
			t.Logf("Shutdown via API failed (expected for VMs without ACPI): %v", err)
			if err := env.Launcher.StopVM(vmID, true, "test-shutdown"); err != nil {
				t.Errorf("Force stop failed: %v", err)
			}
		}
		if err := WaitForVMStopped(ctx, instance); err != nil {
			t.Errorf("WaitForStopped failed: %v", err)
		}
		running, _ := client.IsRunning(ctx)
		if running {
			t.Error("VM should not be running after shutdown")
		}
	})
}
