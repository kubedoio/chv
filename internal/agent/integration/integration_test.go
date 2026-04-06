// Package integration provides integration tests for Cloud Hypervisor.
package integration

import (
	"context"
	"fmt"
	"os"
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

func TestCHClientConnectionFailure(t *testing.T) {
	env := SetupTestEnvironment(t)
	defer env.Cleanup()
	client := hypervisor.NewCHVClient(filepath.Join(env.SocketDir, "nonexistent.sock"))
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()
	_, err := client.GetVMInfo(ctx)
	if err == nil {
		t.Error("Expected error for non-existent socket")
	}
}

func TestCHClientTimeout(t *testing.T) {
	env := SetupTestEnvironment(t)
	defer env.Cleanup()
	socketPath := filepath.Join(env.SocketDir, "timeout.sock")
	listener, err := syscall.Socket(syscall.AF_UNIX, syscall.SOCK_STREAM, 0)
	if err != nil {
		t.Skipf("Failed to create test socket: %v", err)
	}
	defer syscall.Close(listener)
	addr := &syscall.SockaddrUnix{Name: socketPath}
	if err := syscall.Bind(listener, addr); err != nil {
		t.Skipf("Failed to bind test socket: %v", err)
	}
	if err := syscall.Listen(listener, 1); err != nil {
		t.Skipf("Failed to listen on test socket: %v", err)
	}
	go func() {
		for {
			fd, _, err := syscall.Accept(listener)
			if err != nil {
				return
			}
			time.Sleep(5 * time.Second)
			syscall.Close(fd)
		}
	}()
	time.Sleep(50 * time.Millisecond)
	client := hypervisor.NewCHVClient(socketPath)
	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()
	_, err = client.GetVMInfo(ctx)
	if err == nil {
		t.Error("Expected timeout error")
	}
}

func TestMultipleVMs(t *testing.T) {
	SkipIfCHNotAvailable(t)
	SkipIfNoKVM(t)
	env := SetupTestEnvironment(t)
	defer env.Cleanup()
	numVMs := 2
	instances := make([]*hypervisor.VMInstance, 0, numVMs)
	for i := 0; i < numVMs; i++ {
		vmID := fmt.Sprintf("%s-vm%d", GenerateTestVMID(t), i)
		diskPath, err := env.CreateTestDisk(vmID, DefaultTestDiskSize)
		if err != nil {
			t.Fatalf("Failed to create test disk for VM %d: %v", i, err)
		}
		config := env.CreateTestVMConfig(vmID)
		config.VolumePath = diskPath
		ciConfig := CreateTestCloudInitConfig(fmt.Sprintf("test-vm-%d", i))
		isoPath, err := env.ISOGenerator.GenerateISO(vmID, ciConfig)
		if err != nil {
			t.Fatalf("Failed to generate cloud-init ISO for VM %d: %v", i, err)
		}
		config.CloudInitISO = isoPath
		instance, err := env.Launcher.StartVM(config, fmt.Sprintf("test-op-%d", i))
		if err != nil {
			for _, inst := range instances {
				_ = env.Launcher.StopVM(inst.VMID, true, "cleanup")
			}
			t.Fatalf("Failed to start VM %d: %v", i, err)
		}
		instances = append(instances, instance)
	}
	defer func() {
		for _, inst := range instances {
			_ = env.Launcher.StopVM(inst.VMID, true, "cleanup")
		}
	}()
	for i, instance := range instances {
		client := hypervisor.NewCHVClient(instance.APISocket)
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		running, err := client.IsRunning(ctx)
		cancel()
		if err != nil {
			t.Errorf("VM %d: Failed to check status: %v", i, err)
			continue
		}
		if !running {
			t.Errorf("VM %d: Expected VM to be running", i)
		}
		t.Logf("VM %d (ID: %s) is running", i, instance.VMID)
	}
}

func TestVMAPIsocketPath(t *testing.T) {
	SkipIfCHNotAvailable(t)
	SkipIfNoKVM(t)
	env := SetupTestEnvironment(t)
	defer env.Cleanup()
	vmID := GenerateTestVMID(t)
	diskPath, err := env.CreateTestDisk(vmID, DefaultTestDiskSize)
	if err != nil {
		t.Fatalf("Failed to create test disk: %v", err)
	}
	customSocket := filepath.Join(env.SocketDir, "custom-"+vmID+".sock")
	config := env.CreateTestVMConfig(vmID)
	config.VolumePath = diskPath
	config.APIsocket = customSocket
	ciConfig := CreateTestCloudInitConfig("test-vm")
	isoPath, err := env.ISOGenerator.GenerateISO(vmID, ciConfig)
	if err != nil {
		t.Fatalf("Failed to generate cloud-init ISO: %v", err)
	}
	config.CloudInitISO = isoPath
	instance, err := env.Launcher.StartVM(config, "test-socket")
	if err != nil {
		t.Fatalf("Failed to start VM: %v", err)
	}
	defer func() {
		_ = env.Launcher.StopVM(vmID, true, "cleanup")
	}()
	if instance.APISocket != customSocket {
		t.Errorf("Expected API socket %s, got %s", customSocket, instance.APISocket)
	}
	if _, err := os.Stat(customSocket); os.IsNotExist(err) {
		t.Errorf("API socket file does not exist at %s", customSocket)
	}
	client := hypervisor.NewCHVClient(customSocket)
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	if err := client.Ping(ctx); err != nil {
		t.Errorf("Failed to ping VM via custom socket: %v", err)
	}
}

func TestCHProcessLifecycle(t *testing.T) {
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
	instance, err := env.Launcher.StartVM(config, "test-lifecycle")
	if err != nil {
		t.Fatalf("Failed to start VM: %v", err)
	}
	if instance.PID <= 0 {
		t.Errorf("Invalid PID: %d", instance.PID)
	}
	process, err := os.FindProcess(instance.PID)
	if err != nil {
		t.Errorf("Failed to find process %d: %v", instance.PID, err)
	}
	if err := process.Signal(syscall.Signal(0)); err != nil {
		t.Errorf("Process %d does not appear to be alive: %v", instance.PID, err)
	}
	if err := env.Launcher.StopVM(vmID, true, "test-stop"); err != nil {
		t.Errorf("Failed to stop VM: %v", err)
	}
	time.Sleep(100 * time.Millisecond)
	err = process.Signal(syscall.Signal(0))
	if err == nil {
		t.Error("Process should be gone after stop")
	}
}

func TestLogFiles(t *testing.T) {
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
	_, err = env.Launcher.StartVM(config, "test-logs")
	if err != nil {
		t.Fatalf("Failed to start VM: %v", err)
	}
	defer func() {
		_ = env.Launcher.StopVM(vmID, true, "cleanup")
	}()
	time.Sleep(500 * time.Millisecond)
	stdoutLog := filepath.Join(env.LogDir, vmID+".stdout.log")
	stderrLog := filepath.Join(env.LogDir, vmID+".stderr.log")
	for _, logFile := range []string{stdoutLog, stderrLog} {
		info, err := os.Stat(logFile)
		if err != nil {
			t.Logf("Log file %s not created: %v", logFile, err)
			continue
		}
		if info.Size() == 0 {
			t.Logf("Log file %s is empty", logFile)
		} else {
			t.Logf("Log file %s exists, size: %d bytes", logFile, info.Size())
		}
		content, err := os.ReadFile(logFile)
		if err == nil && len(content) > 0 {
			preview := string(content)
			if len(preview) > 200 {
				preview = preview[:200] + "..."
			}
			t.Logf("Log preview: %s", preview)
		}
	}
}
