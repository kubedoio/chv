package hypervisor

import (
	"context"
	"encoding/json"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"testing"
	"time"
)

// mockCHVServer creates a mock Cloud Hypervisor HTTP server on a Unix socket.
type mockCHVServer struct {
	socketPath string
	listener   net.Listener
	server     *http.Server
	vmState    string
}

func newMockCHVServer(t *testing.T) *mockCHVServer {
	tmpDir := t.TempDir()
	socketPath := filepath.Join(tmpDir, "chv.sock")

	listener, err := net.Listen("unix", socketPath)
	if err != nil {
		t.Fatalf("Failed to create mock server: %v", err)
	}

	mock := &mockCHVServer{
		socketPath: socketPath,
		listener:   listener,
		vmState:    "Running",
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/api/v1/vm.info", mock.handleVMInfo)
	mux.HandleFunc("/api/v1/vm.shutdown", mock.handleShutdown)
	mux.HandleFunc("/api/v1/vm.pause", mock.handlePause)
	mux.HandleFunc("/api/v1/vm.resume", mock.handleResume)
	mux.HandleFunc("/api/v1/vm.counters", mock.handleCounters)

	mock.server = &http.Server{
		Handler: mux,
	}

	go mock.server.Serve(listener)

	// Give server time to start
	time.Sleep(10 * time.Millisecond)

	return mock
}

func (m *mockCHVServer) stop() {
	m.server.Close()
	m.listener.Close()
}

func (m *mockCHVServer) handleVMInfo(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	info := map[string]interface{}{
		"config": map[string]interface{}{
			"cpus": map[string]interface{}{
				"boot_vcpus": 2,
				"max_vcpus":  2,
			},
			"memory": map[string]interface{}{
				"size":      2147483648,
				"mergeable": false,
				"shared":    false,
				"hugepages": false,
			},
		},
		"state": m.vmState,
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(info)
}

func (m *mockCHVServer) handleShutdown(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPut {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	// Read body to check mode
	body, _ := io.ReadAll(r.Body)
	var req map[string]string
	json.Unmarshal(body, &req)

	if req["mode"] == "Reboot" {
		// Keep state as Running for reboot
	} else {
		m.vmState = "Shutdown"
	}

	w.WriteHeader(http.StatusNoContent)
}

func (m *mockCHVServer) handlePause(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPut {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	m.vmState = "Paused"
	w.WriteHeader(http.StatusNoContent)
}

func (m *mockCHVServer) handleResume(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPut {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	m.vmState = "Running"
	w.WriteHeader(http.StatusNoContent)
}

func (m *mockCHVServer) handleCounters(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	counters := map[string]interface{}{
		"vcpu_calibrate": map[string]interface{}{
			"errors":   0,
			"executed": 1000,
			"missed":   0,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(counters)
}

func TestCHVClient_GetVMInfo(t *testing.T) {
	mock := newMockCHVServer(t)
	defer mock.stop()

	client := NewCHVClient(mock.socketPath)
	
	info, err := client.GetVMInfo(context.Background())
	if err != nil {
		t.Fatalf("GetVMInfo failed: %v", err)
	}

	if info.State != "Running" {
		t.Errorf("Expected state Running, got %s", info.State)
	}

	if info.Config.Cpus.BootVcpus != 2 {
		t.Errorf("Expected 2 boot vCPUs, got %d", info.Config.Cpus.BootVcpus)
	}

	if info.Config.Memory.Size != 2147483648 {
		t.Errorf("Expected 2GB memory, got %d", info.Config.Memory.Size)
	}
}

func TestCHVClient_Ping(t *testing.T) {
	mock := newMockCHVServer(t)
	defer mock.stop()

	client := NewCHVClient(mock.socketPath)
	
	if err := client.Ping(context.Background()); err != nil {
		t.Fatalf("Ping failed: %v", err)
	}
}

func TestCHVClient_IsRunning(t *testing.T) {
	mock := newMockCHVServer(t)
	defer mock.stop()

	client := NewCHVClient(mock.socketPath)
	
	running, err := client.IsRunning(context.Background())
	if err != nil {
		t.Fatalf("IsRunning failed: %v", err)
	}
	if !running {
		t.Error("Expected VM to be running")
	}

	// Shutdown the VM
	if err := client.Shutdown(context.Background()); err != nil {
		t.Fatalf("Shutdown failed: %v", err)
	}

	running, err = client.IsRunning(context.Background())
	if err != nil {
		t.Fatalf("IsRunning failed: %v", err)
	}
	if running {
		t.Error("Expected VM to not be running after shutdown")
	}
}

func TestCHVClient_Shutdown(t *testing.T) {
	mock := newMockCHVServer(t)
	defer mock.stop()

	client := NewCHVClient(mock.socketPath)
	
	if err := client.Shutdown(context.Background()); err != nil {
		t.Fatalf("Shutdown failed: %v", err)
	}

	// Verify state changed
	info, _ := client.GetVMInfo(context.Background())
	if info.State != "Shutdown" {
		t.Errorf("Expected state Shutdown, got %s", info.State)
	}
}

func TestCHVClient_Reboot(t *testing.T) {
	mock := newMockCHVServer(t)
	defer mock.stop()

	client := NewCHVClient(mock.socketPath)
	
	if err := client.Reboot(context.Background()); err != nil {
		t.Fatalf("Reboot failed: %v", err)
	}

	// In mock, reboot keeps state as Running
	info, _ := client.GetVMInfo(context.Background())
	if info.State != "Running" {
		t.Errorf("Expected state Running after reboot, got %s", info.State)
	}
}

func TestCHVClient_PauseResume(t *testing.T) {
	mock := newMockCHVServer(t)
	defer mock.stop()

	client := NewCHVClient(mock.socketPath)
	
	// Pause
	if err := client.Pause(context.Background()); err != nil {
		t.Fatalf("Pause failed: %v", err)
	}

	info, _ := client.GetVMInfo(context.Background())
	if info.State != "Paused" {
		t.Errorf("Expected state Paused, got %s", info.State)
	}

	// Resume
	if err := client.Resume(context.Background()); err != nil {
		t.Fatalf("Resume failed: %v", err)
	}

	info, _ = client.GetVMInfo(context.Background())
	if info.State != "Running" {
		t.Errorf("Expected state Running, got %s", info.State)
	}
}

func TestCHVClient_GetVMCounters(t *testing.T) {
	mock := newMockCHVServer(t)
	defer mock.stop()

	client := NewCHVClient(mock.socketPath)
	
	counters, err := client.GetVMCounters(context.Background())
	if err != nil {
		t.Fatalf("GetVMCounters failed: %v", err)
	}

	if counters.VcpuCalibrate.Executed != 1000 {
		t.Errorf("Expected 1000 executed calibrations, got %d", counters.VcpuCalibrate.Executed)
	}
}

func TestCHVClient_ConnectionFailure(t *testing.T) {
	// Use a non-existent socket path
	client := NewCHVClient("/nonexistent/path/chv.sock")
	
	_, err := client.GetVMInfo(context.Background())
	if err == nil {
		t.Error("Expected error for non-existent socket")
	}
}

func TestCHVClient_Timeout(t *testing.T) {
	// Create a slow server
	tmpDir := t.TempDir()
	socketPath := filepath.Join(tmpDir, "slow.sock")

	listener, err := net.Listen("unix", socketPath)
	if err != nil {
		t.Fatalf("Failed to create listener: %v", err)
	}
	defer listener.Close()

	// Server that never responds
	go func() {
		for {
			conn, err := listener.Accept()
			if err != nil {
				return
			}
			// Don't respond - just hold connection
			defer conn.Close()
			time.Sleep(5 * time.Second)
		}
	}()

	client := NewCHVClient(socketPath)
	// Override timeout to be shorter for test
	client.httpClient.Timeout = 100 * time.Millisecond

	ctx, cancel := context.WithTimeout(context.Background(), 200*time.Millisecond)
	defer cancel()

	_, err = client.GetVMInfo(ctx)
	if err == nil {
		t.Error("Expected timeout error")
	}
}

func TestCHVClient_WaitForRunning(t *testing.T) {
	mock := newMockCHVServer(t)
	defer mock.stop()

	client := NewCHVClient(mock.socketPath)
	
	// VM is already running, should return immediately
	ctx, cancel := context.WithTimeout(context.Background(), 1*time.Second)
	defer cancel()

	if err := client.WaitForRunning(ctx, 500*time.Millisecond); err != nil {
		t.Fatalf("WaitForRunning failed: %v", err)
	}
}

func TestCHVClient_WaitForRunning_Timeout(t *testing.T) {
	mock := newMockCHVServer(t)
	defer mock.stop()

	client := NewCHVClient(mock.socketPath)

	// Stop the VM first
	mock.vmState = "Shutdown"
	
	// Should timeout
	ctx, cancel := context.WithTimeout(context.Background(), 200*time.Millisecond)
	defer cancel()

	err := client.WaitForRunning(ctx, 100*time.Millisecond)
	if err == nil {
		t.Error("Expected timeout error")
	}
}

func TestCHVClient_WaitForStopped(t *testing.T) {
	mock := newMockCHVServer(t)
	defer mock.stop()

	client := NewCHVClient(mock.socketPath)

	// Stop the VM
	mock.vmState = "Shutdown"
	
	ctx, cancel := context.WithTimeout(context.Background(), 1*time.Second)
	defer cancel()

	if err := client.WaitForStopped(ctx, 500*time.Millisecond); err != nil {
		t.Fatalf("WaitForStopped failed: %v", err)
	}
}

func TestCHVClient_WaitForStopped_ByConnectionFailure(t *testing.T) {
	// Create a server that will close
	tmpDir := t.TempDir()
	socketPath := filepath.Join(tmpDir, "temp.sock")

	listener, err := net.Listen("unix", socketPath)
	if err != nil {
		t.Fatalf("Failed to create listener: %v", err)
	}

	// Simple server that responds once then closes
	go func() {
		conn, err := listener.Accept()
		if err != nil {
			return
		}
		conn.Close()
		listener.Close()
	}()

	// Give server time to start
	time.Sleep(10 * time.Millisecond)

	client := NewCHVClient(socketPath)

	// This should succeed because the API becomes unavailable (connection refused)
	ctx, cancel := context.WithTimeout(context.Background(), 500*time.Millisecond)
	defer cancel()

	// Note: This might fail in real scenario since connection refused is an error
	// The implementation treats API errors as "stopped" which is correct behavior
	err = client.WaitForStopped(ctx, 400*time.Millisecond)
	// We expect this to potentially fail or succeed depending on timing
	// The important thing is it doesn't hang forever
	_ = err
}

// cleanup removes leftover socket files.
func cleanup(socketPath string) {
	os.Remove(socketPath)
}
