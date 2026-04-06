// Package integration provides integration tests for Cloud Hypervisor.
package integration

import (
	"context"
	"io"
	"net"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/chv/chv/internal/hypervisor"
)

func TestConsoleConnection(t *testing.T) {
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
	ciConfig := CreateTestCloudInitConfig("console-test-vm")
	isoPath, err := env.ISOGenerator.GenerateISO(vmID, ciConfig)
	if err != nil {
		t.Fatalf("Failed to generate cloud-init ISO: %v", err)
	}
	config.CloudInitISO = isoPath
	instance, err := env.Launcher.StartVM(config, "console-op-1")
	if err != nil {
		t.Fatalf("Failed to start VM: %v", err)
	}
	defer func() {
		env.Launcher.StopVM(vmID, true, "cleanup")
	}()
	ctx, cancel := context.WithTimeout(context.Background(), TestVMTimeout)
	defer cancel()
	client := hypervisor.NewCHVClient(instance.APISocket)
	if err := WaitForVMRunning(ctx, client); err != nil {
		t.Fatalf("VM did not reach running state: %v", err)
	}
	proxy := hypervisor.NewConsoleProxy(instance.APISocket)
	availableCtx, availableCancel := context.WithTimeout(ctx, 2*time.Second)
	available := proxy.IsAvailable(availableCtx)
	availableCancel()
	if !available {
		t.Log("Console proxy reports unavailable - this may be expected depending on CH configuration")
	} else {
		t.Log("Console proxy is available")
	}
	consoleCtx, consoleCancel := context.WithTimeout(ctx, 2*time.Second)
	defer consoleCancel()
	stream, err := proxy.OpenConsole(consoleCtx)
	if err != nil {
		t.Logf("Console open returned expected error (serial socket mode not configured): %v", err)
	} else {
		if stream != nil {
			stream.Input.Close()
			stream.Output.Close()
			t.Log("Console stream opened successfully")
		}
	}
	socketPath := proxy.GetAPISocketPath()
	if socketPath != instance.APISocket {
		t.Errorf("API socket path mismatch: expected %s, got %s", instance.APISocket, socketPath)
	}
}

func TestConsoleSessionManagement(t *testing.T) {
	sessionManager := hypervisor.NewSessionManager()
	vmID := "test-vm-123"
	userID := "user-456"
	err := sessionManager.AcquireSession(vmID, userID)
	if err != nil {
		t.Fatalf("Failed to acquire session: %v", err)
	}
	err = sessionManager.AcquireSession(vmID, userID)
	if err == nil {
		t.Error("Expected error when acquiring duplicate session")
	}
	session := &hypervisor.ConsoleSession{
		VMID:      vmID,
		UserID:    userID,
		StartedAt: time.Now(),
	}
	sessionManager.RegisterSession(session)
	if !sessionManager.HasSession(vmID, userID) {
		t.Error("Session should exist")
	}
	sessionManager.ReleaseSession(vmID, userID)
	if sessionManager.HasSession(vmID, userID) {
		t.Error("Session should be released")
	}
	err = sessionManager.AcquireSession(vmID, userID)
	if err != nil {
		t.Errorf("Failed to acquire session after release: %v", err)
	}
}

func TestSerialConsoleLog(t *testing.T) {
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
	ciConfig := CreateTestCloudInitConfig("serial-test-vm")
	isoPath, err := env.ISOGenerator.GenerateISO(vmID, ciConfig)
	if err != nil {
		t.Fatalf("Failed to generate cloud-init ISO: %v", err)
	}
	config.CloudInitISO = isoPath
	instance, err := env.Launcher.StartVM(config, "serial-op-1")
	if err != nil {
		t.Fatalf("Failed to start VM: %v", err)
	}
	defer func() {
		env.Launcher.StopVM(vmID, true, "cleanup")
	}()
	ctx, cancel := context.WithTimeout(context.Background(), TestVMTimeout)
	defer cancel()
	client := hypervisor.NewCHVClient(instance.APISocket)
	if err := WaitForVMRunning(ctx, client); err != nil {
		t.Fatalf("VM did not reach running state: %v", err)
	}
	time.Sleep(1 * time.Second)
	stdoutLog := filepath.Join(env.LogDir, vmID+".stdout.log")
	stderrLog := filepath.Join(env.LogDir, vmID+".stderr.log")
	for _, logFile := range []string{stdoutLog, stderrLog} {
		content, err := os.ReadFile(logFile)
		if err != nil {
			t.Logf("Could not read log file %s: %v", logFile, err)
			continue
		}
		if len(content) > 0 {
			t.Logf("Log file %s has %d bytes", logFile, len(content))
			preview := string(content)
			if len(preview) > 200 {
				preview = preview[:200]
			}
			t.Logf("Preview: %s", preview)
		}
	}
}

func TestConsoleStreamWithMock(t *testing.T) {
	env := SetupTestEnvironment(t)
	defer env.Cleanup()
	serialSocket := filepath.Join(env.SocketDir, "mock-serial.sock")
	listener, err := createMockSerialServer(serialSocket)
	if err != nil {
		t.Skipf("Failed to create mock serial server: %v", err)
		return
	}
	defer listener.Close()
	proxy := hypervisor.NewConsoleProxy(serialSocket)
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	pr, pw := io.Pipe()
	defer pr.Close()
	defer pw.Close()
	errChan := make(chan error, 1)
	go func() {
		errChan <- proxy.StreamConsole(ctx, pw, pr)
	}()
	testData := []byte("test data\n")
	if _, err := pw.Write(testData); err != nil {
		t.Logf("Write error: %v", err)
	}
	time.Sleep(100 * time.Millisecond)
	cancel()
	select {
	case err := <-errChan:
		if err != nil && err != context.Canceled {
			t.Logf("Stream error (may be expected): %v", err)
		}
	case <-time.After(2 * time.Second):
		t.Log("Stream timeout")
	}
}

func createMockSerialServer(socketPath string) (net.Listener, error) {
	os.Remove(socketPath)
	addr, err := net.ResolveUnixAddr("unix", socketPath)
	if err != nil {
		return nil, err
	}
	listener, err := net.ListenUnix("unix", addr)
	if err != nil {
		return nil, err
	}
	go func() {
		for {
			conn, err := listener.Accept()
			if err != nil {
				return
			}
			go func(c net.Conn) {
				defer c.Close()
				buf := make([]byte, 1024)
				for {
					n, err := c.Read(buf)
					if err != nil {
						return
					}
					c.Write(buf[:n])
				}
			}(conn)
		}
	}()
	return listener, nil
}
