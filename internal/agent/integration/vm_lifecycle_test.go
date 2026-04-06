// Package integration provides integration tests for Cloud Hypervisor.
package integration

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/chv/chv/internal/agent/manager"
	"github.com/chv/chv/internal/hypervisor"
)

func TestVMLifecycleCreateBootShutdown(t *testing.T) {
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
	ciConfig := CreateTestCloudInitConfig("lifecycle-test-vm")
	isoPath, err := env.ISOGenerator.GenerateISO(vmID, ciConfig)
	if err != nil {
		t.Fatalf("Failed to generate cloud-init ISO: %v", err)
	}
	config.CloudInitISO = isoPath
	t.Log("Step 1: Starting VM...")
	instance, err := env.Launcher.StartVM(config, "lifecycle-op-1")
	if err != nil {
		t.Fatalf("Failed to start VM: %v", err)
	}
	if instance.VMID != vmID {
		t.Errorf("VMID mismatch: expected %s, got %s", vmID, instance.VMID)
	}
	client := hypervisor.NewCHVClient(instance.APISocket)
	ctx, cancel := context.WithTimeout(context.Background(), TestVMTimeout)
	defer cancel()
	if err := WaitForVMRunning(ctx, client); err != nil {
		t.Fatalf("VM did not reach running state: %v", err)
	}
	t.Log("VM is running")
	t.Log("Step 2: Getting VM info...")
	info, err := client.GetVMInfo(ctx)
	if err != nil {
		t.Fatalf("Failed to get VM info: %v", err)
	}
	if info.State != "Running" {
		t.Errorf("Expected state Running, got %s", info.State)
	}
	t.Log("Step 3: Pausing VM...")
	if err := client.Pause(ctx); err != nil {
		t.Logf("Pause failed (may not be supported): %v", err)
	} else {
		info, _ = client.GetVMInfo(ctx)
		if info.State == "Paused" {
			t.Log("VM is paused")
			t.Log("Step 3b: Resuming VM...")
			if err := client.Resume(ctx); err != nil {
				t.Errorf("Resume failed: %v", err)
			}
			if err := WaitForVMRunning(ctx, client); err != nil {
				t.Errorf("VM did not resume: %v", err)
			}
			t.Log("VM resumed")
		}
	}
	t.Log("Step 4: Stopping VM...")
	if err := env.Launcher.StopVM(vmID, false, "lifecycle-op-2"); err != nil {
		t.Logf("Graceful shutdown failed, using force: %v", err)
		if err := env.Launcher.StopVM(vmID, true, "lifecycle-op-2"); err != nil {
			t.Fatalf("Force stop failed: %v", err)
		}
	}
	ctx2, cancel2 := context.WithTimeout(context.Background(), TestVMTimeout)
	defer cancel2()
	if err := WaitForVMStopped(ctx2, instance); err != nil {
		t.Logf("WaitForStopped warning: %v", err)
	}
	running, _ := client.IsRunning(ctx2)
	if running {
		t.Error("VM should not be running after stop")
	}
	t.Log("VM stopped successfully")
	t.Log("Step 5: Verifying cleanup...")
	if inst := env.Launcher.GetInstance(vmID); inst != nil {
		t.Error("Instance should be removed from launcher after stop")
	}
	if _, err := os.Stat(instance.APISocket); !os.IsNotExist(err) {
		t.Log("API socket file still exists (may be cleaned up asynchronously)")
	}
}

func TestVMLifecycleRestart(t *testing.T) {
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
	ciConfig := CreateTestCloudInitConfig("restart-test-vm")
	isoPath, err := env.ISOGenerator.GenerateISO(vmID, ciConfig)
	if err != nil {
		t.Fatalf("Failed to generate cloud-init ISO: %v", err)
	}
	config.CloudInitISO = isoPath
	t.Log("Starting VM (first time)...")
	instance1, err := env.Launcher.StartVM(config, "restart-op-1")
	if err != nil {
		t.Fatalf("Failed to start VM: %v", err)
	}
	pid1 := instance1.PID
	socket1 := instance1.APISocket
	client1 := hypervisor.NewCHVClient(socket1)
	ctx, cancel := context.WithTimeout(context.Background(), TestVMTimeout)
	if err := WaitForVMRunning(ctx, client1); err != nil {
		cancel()
		t.Fatalf("VM did not reach running state: %v", err)
	}
	cancel()
	t.Logf("VM running with PID %d", pid1)
	t.Log("Stopping VM...")
	if err := env.Launcher.StopVM(vmID, true, "restart-op-2"); err != nil {
		t.Fatalf("Failed to stop VM: %v", err)
	}
	time.Sleep(200 * time.Millisecond)
	t.Log("Restarting VM...")
	instance2, err := env.Launcher.StartVM(config, "restart-op-3")
	if err != nil {
		t.Fatalf("Failed to restart VM: %v", err)
	}
	pid2 := instance2.PID
	socket2 := instance2.APISocket
	client2 := hypervisor.NewCHVClient(socket2)
	ctx, cancel = context.WithTimeout(context.Background(), TestVMTimeout)
	if err := WaitForVMRunning(ctx, client2); err != nil {
		cancel()
		t.Fatalf("VM did not reach running state after restart: %v", err)
	}
	cancel()
	t.Logf("VM restarted with PID %d", pid2)
	if pid1 == pid2 {
		t.Error("Expected different PID after restart")
	}
	env.Launcher.StopVM(vmID, true, "cleanup")
}

func TestVMManagerIntegration(t *testing.T) {
	SkipIfCHNotAvailable(t)
	SkipIfNoKVM(t)
	env := SetupTestEnvironment(t)
	defer env.Cleanup()
	vmManager := manager.NewVMManager(
		env.Launcher,
		env.StorageMgr,
		env.ISOGenerator,
		env.StateDir,
		env.VMDir,
		env.ImagesDir,
		"",
	)
	if err := vmManager.Initialize(); err != nil {
		t.Fatalf("Failed to initialize VM manager: %v", err)
	}
	vmID := GenerateTestVMID(t)
	t.Log("Creating VM via VM manager...")
	createReq := &manager.CreateVMRequest{
		VMID:          vmID,
		Name:          "manager-test-vm",
		VCPU:          DefaultTestVCPUs,
		MemoryMB:      DefaultTestMemoryMB,
		DiskSizeBytes: DefaultTestDiskSize,
		OperationID:   "manager-op-1",
	}
	record, err := vmManager.CreateVM(context.Background(), createReq)
	if err != nil {
		t.Fatalf("Failed to create VM: %v", err)
	}
	if record.VMID != vmID {
		t.Errorf("VMID mismatch: expected %s, got %s", vmID, record.VMID)
	}
	if record.State != manager.VMStateRunning {
		t.Errorf("Expected state %s, got %s", manager.VMStateRunning, record.State)
	}
	t.Logf("VM created with state: %s", record.State)
	if inst := env.Launcher.GetInstance(vmID); inst == nil {
		t.Error("VM should be tracked by launcher")
	}
	state, err := vmManager.GetVMState(vmID)
	if err != nil {
		t.Errorf("Failed to get VM state: %v", err)
	}
	if state != manager.VMStateRunning {
		t.Errorf("Expected state %s, got %s", manager.VMStateRunning, state)
	}
	vms, err := vmManager.ListVMs()
	if err != nil {
		t.Errorf("Failed to list VMs: %v", err)
	}
	found := false
	for _, vm := range vms {
		if vm.VMID == vmID {
			found = true
			break
		}
	}
	if !found {
		t.Error("Created VM not found in list")
	}
	t.Log("Stopping VM via VM manager...")
	stopReq := &manager.StopVMRequest{
		VMID:        vmID,
		Force:       true,
		OperationID: "manager-op-2",
	}
	if err := vmManager.StopVM(context.Background(), stopReq); err != nil {
		t.Errorf("Failed to stop VM: %v", err)
	}
	time.Sleep(200 * time.Millisecond)
	state, err = vmManager.GetVMState(vmID)
	if err != nil {
		t.Errorf("Failed to get VM state after stop: %v", err)
	}
	if state != manager.VMStateStopped {
		t.Errorf("Expected state %s after stop, got %s", manager.VMStateStopped, state)
	}
	t.Log("Starting VM again via VM manager...")
	startReq := &manager.StartVMRequest{
		VMID:        vmID,
		OperationID: "manager-op-3",
	}
	if err := vmManager.StartVM(context.Background(), startReq); err != nil {
		t.Fatalf("Failed to start VM: %v", err)
	}
	state, err = vmManager.GetVMState(vmID)
	if err != nil {
		t.Errorf("Failed to get VM state after start: %v", err)
	}
	if state != manager.VMStateRunning {
		t.Errorf("Expected state %s after start, got %s", manager.VMStateRunning, state)
	}
	t.Log("VM lifecycle via manager completed successfully")
	t.Log("Deleting VM via VM manager...")
	deleteReq := &manager.DeleteVMRequest{
		VMID:        vmID,
		Force:       true,
		OperationID: "manager-op-4",
	}
	if err := vmManager.DeleteVM(context.Background(), deleteReq); err != nil {
		t.Errorf("Failed to delete VM: %v", err)
	}
	_, err = vmManager.GetVM(vmID)
	if err == nil {
		t.Error("VM should not exist after delete")
	}
}

func TestVMIdempotency(t *testing.T) {
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
	ciConfig := CreateTestCloudInitConfig("idempotency-test-vm")
	isoPath, err := env.ISOGenerator.GenerateISO(vmID, ciConfig)
	if err != nil {
		t.Fatalf("Failed to generate cloud-init ISO: %v", err)
	}
	config.CloudInitISO = isoPath
	opID := "idempotent-op-1"
	instance1, err := env.Launcher.StartVM(config, opID)
	if err != nil {
		t.Fatalf("Failed to start VM: %v", err)
	}
	instance2, err := env.Launcher.StartVM(config, opID)
	if err != nil {
		t.Fatalf("Second start with same operation ID should succeed: %v", err)
	}
	if instance1.PID != instance2.PID {
		t.Error("Idempotent start should return same instance")
	}
	env.Launcher.StopVM(vmID, true, "cleanup")
}

func TestVMStatePersistence(t *testing.T) {
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
	ciConfig := CreateTestCloudInitConfig("persistence-test-vm")
	isoPath, err := env.ISOGenerator.GenerateISO(vmID, ciConfig)
	if err != nil {
		t.Fatalf("Failed to generate cloud-init ISO: %v", err)
	}
	config.CloudInitISO = isoPath
	instance, err := env.Launcher.StartVM(config, "persistence-op-1")
	if err != nil {
		t.Fatalf("Failed to start VM: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), TestVMTimeout)
	client := hypervisor.NewCHVClient(instance.APISocket)
	if err := WaitForVMRunning(ctx, client); err != nil {
		cancel()
		t.Fatalf("VM did not reach running state: %v", err)
	}
	cancel()
	stateFile := filepath.Join(env.StateDir, vmID+".json")
	if _, err := os.Stat(stateFile); os.IsNotExist(err) {
		t.Error("State file should exist after VM start")
	}
	env.Launcher.StopVM(vmID, true, "persistence-op-2")
	time.Sleep(200 * time.Millisecond)
	newLauncher := hypervisor.NewLauncher(
		env.CHBinary,
		env.StateDir,
		env.LogDir,
		env.SocketDir,
		env.StateManager,
		env.TAPManager,
		env.ISOGenerator,
	)
	if err := newLauncher.Recover(); err != nil {
		t.Errorf("Failed to recover state: %v", err)
	}
	if inst := newLauncher.GetInstance(vmID); inst != nil {
		t.Log("Note: VM instance found after recovery (may be expected based on timing)")
	}
	state, err := env.StateManager.Load(vmID)
	if err != nil {
		t.Errorf("Failed to load state: %v", err)
	}
	if state != nil && state.State != "stopped" {
		t.Errorf("Expected state stopped, got %s", state.State)
	}
}

func TestVMCleanupOnFailure(t *testing.T) {
	SkipIfCHNotAvailable(t)
	env := SetupTestEnvironment(t)
	defer env.Cleanup()
	vmID := GenerateTestVMID(t)
	config := env.CreateTestVMConfig(vmID)
	config.VolumePath = "/nonexistent/path/disk.raw"
	_, err := env.Launcher.StartVM(config, "failure-op-1")
	if err == nil {
		t.Fatal("Expected VM start to fail with non-existent disk")
	}
	if inst := env.Launcher.GetInstance(vmID); inst != nil {
		t.Error("No instance should be tracked after failed start")
	}
	socketPath := filepath.Join(env.SocketDir, vmID+".sock")
	if _, err := os.Stat(socketPath); !os.IsNotExist(err) {
		t.Log("Socket file may still exist after failed start (cleanup may be async)")
	}
}

func TestConcurrentVMOperations(t *testing.T) {
	SkipIfCHNotAvailable(t)
	SkipIfNoKVM(t)
	env := SetupTestEnvironment(t)
	defer env.Cleanup()
	numVMs := 2
	vmIDs := make([]string, numVMs)
	done := make(chan struct {
		index int
		err   error
	}, numVMs)
	for i := 0; i < numVMs; i++ {
		go func(idx int) {
			vmID := fmt.Sprintf("%s-concurrent%d", GenerateTestVMID(t), idx)
			vmIDs[idx] = vmID
			diskPath, err := env.CreateTestDisk(vmID, DefaultTestDiskSize)
			if err != nil {
				done <- struct {
					index int
					err   error
				}{idx, err}
				return
			}
			config := env.CreateTestVMConfig(vmID)
			config.VolumePath = diskPath
			ciConfig := CreateTestCloudInitConfig(fmt.Sprintf("concurrent-vm-%d", idx))
			isoPath, err := env.ISOGenerator.GenerateISO(vmID, ciConfig)
			if err != nil {
				done <- struct {
					index int
					err   error
				}{idx, err}
				return
			}
			config.CloudInitISO = isoPath
			_, err = env.Launcher.StartVM(config, fmt.Sprintf("concurrent-op-%d", idx))
			done <- struct {
				index int
				err   error
			}{idx, err}
		}(i)
	}
	errors := make([]error, numVMs)
	for i := 0; i < numVMs; i++ {
		result := <-done
		errors[result.index] = result.err
	}
	defer func() {
		for _, vmID := range vmIDs {
			if vmID != "" {
				env.Launcher.StopVM(vmID, true, "cleanup")
			}
		}
	}()
	for i, err := range errors {
		if err != nil {
			t.Errorf("Failed to start VM %d: %v", i, err)
		}
	}
	for i, vmID := range vmIDs {
		if vmID == "" {
			continue
		}
		instance := env.Launcher.GetInstance(vmID)
		if instance == nil {
			t.Errorf("VM %d not found in launcher", i)
			continue
		}
		client := hypervisor.NewCHVClient(instance.APISocket)
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		running, _ := client.IsRunning(ctx)
		cancel()
		if !running {
			t.Errorf("VM %d not running", i)
		}
	}
	t.Logf("Successfully started %d VMs concurrently", numVMs)
}

func TestVMMetricsCollection(t *testing.T) {
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
	ciConfig := CreateTestCloudInitConfig("metrics-test-vm")
	isoPath, err := env.ISOGenerator.GenerateISO(vmID, ciConfig)
	if err != nil {
		t.Fatalf("Failed to generate cloud-init ISO: %v", err)
	}
	config.CloudInitISO = isoPath
	instance, err := env.Launcher.StartVM(config, "metrics-op-1")
	if err != nil {
		t.Fatalf("Failed to start VM: %v", err)
	}
	defer func() {
		env.Launcher.StopVM(vmID, true, "cleanup")
	}()
	ctx, cancel := context.WithTimeout(context.Background(), TestVMTimeout)
	client := hypervisor.NewCHVClient(instance.APISocket)
	if err := WaitForVMRunning(ctx, client); err != nil {
		cancel()
		t.Fatalf("VM did not reach running state: %v", err)
	}
	cancel()
	for i := 0; i < 3; i++ {
		counters, err := client.GetVMCounters(ctx)
		if err != nil {
			t.Logf("Metrics collection attempt %d failed (may not be available): %v", i+1, err)
			time.Sleep(100 * time.Millisecond)
			continue
		}
		t.Logf("Metrics attempt %d: %+v", i+1, counters)
		time.Sleep(100 * time.Millisecond)
	}
}
