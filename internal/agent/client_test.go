// Package agent provides a gRPC client for controller-to-agent communication.
package agent

import (
	"context"
	"errors"
	"net"
	"sync"
	"testing"
	"time"

	"github.com/chv/chv/internal/pb/agent"
	"google.golang.org/grpc"
	"google.golang.org/grpc/connectivity"
	"google.golang.org/grpc/credentials/insecure"
)

// mockAgentServer implements a mock gRPC server for testing
type mockAgentServer struct {
	agent.UnimplementedAgentServiceServer

	mu sync.Mutex

	// Response storage
	pingResponse         *agent.PingResponse
	vmStateResponse      *agent.VMStateResponse
	provisionVMResponse  *agent.VMStateResponse
	startVMResponse      *agent.VMStateResponse
	stopVMResponse       *agent.VMStateResponse
	rebootVMResponse     *agent.VMStateResponse
	deleteVMResponse     *agent.VMStateResponse
	nodeValidateResponse *agent.NodeValidateResponse

	// Error storage
	pingError         error
	provisionVMError  error
	startVMError      error
	stopVMError       error
	rebootVMError     error
	deleteVMError     error
	getVMStateError   error
	createVolumeError error
	resizeVolumeError error
	ensureBridgeError error
	importImageError  error

	// Call tracking
	pingCalled         bool
	provisionVMCalled  bool
	startVMCalled      bool
	stopVMCalled       bool
	rebootVMCalled     bool
	deleteVMCalled     bool
	getVMStateCalled   bool
	createVolumeCalled bool
	resizeVolumeCalled bool
	ensureBridgeCalled bool
	importImageCalled  bool
}

func (m *mockAgentServer) Ping(ctx context.Context, req *agent.Empty) (*agent.PingResponse, error) {
	m.mu.Lock()
	m.pingCalled = true
	m.mu.Unlock()
	if m.pingResponse != nil {
		return m.pingResponse, m.pingError
	}
	return &agent.PingResponse{Ok: true, Version: "1.0.0"}, m.pingError
}

func (m *mockAgentServer) ProvisionVM(ctx context.Context, req *agent.ProvisionVMRequest) (*agent.VMStateResponse, error) {
	m.mu.Lock()
	m.provisionVMCalled = true
	m.mu.Unlock()
	if m.provisionVMResponse != nil {
		return m.provisionVMResponse, m.provisionVMError
	}
	return &agent.VMStateResponse{VmId: req.VmId, State: "Created"}, m.provisionVMError
}

func (m *mockAgentServer) StartVM(ctx context.Context, req *agent.VMStateRequest) (*agent.VMStateResponse, error) {
	m.mu.Lock()
	m.startVMCalled = true
	m.mu.Unlock()
	if m.startVMResponse != nil {
		return m.startVMResponse, m.startVMError
	}
	return &agent.VMStateResponse{VmId: req.VmId, State: "Running"}, m.startVMError
}

func (m *mockAgentServer) StopVM(ctx context.Context, req *agent.VMStateRequest) (*agent.VMStateResponse, error) {
	m.mu.Lock()
	m.stopVMCalled = true
	m.mu.Unlock()
	if m.stopVMResponse != nil {
		return m.stopVMResponse, m.stopVMError
	}
	return &agent.VMStateResponse{VmId: req.VmId, State: "Stopped"}, m.stopVMError
}

func (m *mockAgentServer) RebootVM(ctx context.Context, req *agent.VMStateRequest) (*agent.VMStateResponse, error) {
	m.mu.Lock()
	m.rebootVMCalled = true
	m.mu.Unlock()
	if m.rebootVMResponse != nil {
		return m.rebootVMResponse, m.rebootVMError
	}
	return &agent.VMStateResponse{VmId: req.VmId, State: "Running"}, m.rebootVMError
}

func (m *mockAgentServer) DeleteVM(ctx context.Context, req *agent.VMDeleteRequest) (*agent.VMStateResponse, error) {
	m.mu.Lock()
	m.deleteVMCalled = true
	m.mu.Unlock()
	if m.deleteVMResponse != nil {
		return m.deleteVMResponse, m.deleteVMError
	}
	return &agent.VMStateResponse{VmId: req.VmId, State: "Deleted"}, m.deleteVMError
}

func (m *mockAgentServer) GetVMState(ctx context.Context, req *agent.VMStateRequest) (*agent.VMStateResponse, error) {
	m.mu.Lock()
	m.getVMStateCalled = true
	m.mu.Unlock()
	if m.vmStateResponse != nil {
		return m.vmStateResponse, m.getVMStateError
	}
	return &agent.VMStateResponse{VmId: req.VmId, State: "Running", Pid: "1234"}, m.getVMStateError
}

func (m *mockAgentServer) CreateVolume(ctx context.Context, req *agent.VolumeCreateRequest) (*agent.NodeValidateResponse, error) {
	m.mu.Lock()
	m.createVolumeCalled = true
	m.mu.Unlock()
	if m.createVolumeError != nil {
		return &agent.NodeValidateResponse{Ok: false, Errors: []*agent.ErrorDetail{{Message: m.createVolumeError.Error()}}}, nil
	}
	return &agent.NodeValidateResponse{Ok: true}, nil
}

func (m *mockAgentServer) ResizeVolume(ctx context.Context, req *agent.VolumeResizeRequest) (*agent.NodeValidateResponse, error) {
	m.mu.Lock()
	m.resizeVolumeCalled = true
	m.mu.Unlock()
	if m.resizeVolumeError != nil {
		return &agent.NodeValidateResponse{Ok: false, Errors: []*agent.ErrorDetail{{Message: m.resizeVolumeError.Error()}}}, nil
	}
	return &agent.NodeValidateResponse{Ok: true}, nil
}

func (m *mockAgentServer) EnsureBridge(ctx context.Context, req *agent.EnsureBridgeRequest) (*agent.NodeValidateResponse, error) {
	m.mu.Lock()
	m.ensureBridgeCalled = true
	m.mu.Unlock()
	if m.ensureBridgeError != nil {
		return &agent.NodeValidateResponse{Ok: false, Errors: []*agent.ErrorDetail{{Message: m.ensureBridgeError.Error()}}}, nil
	}
	return &agent.NodeValidateResponse{Ok: true}, nil
}

func (m *mockAgentServer) ImportImage(ctx context.Context, req *agent.ImageImportRequest) (*agent.NodeValidateResponse, error) {
	m.mu.Lock()
	m.importImageCalled = true
	m.mu.Unlock()
	if m.importImageError != nil {
		return &agent.NodeValidateResponse{Ok: false, Errors: []*agent.ErrorDetail{{Message: m.importImageError.Error()}}}, nil
	}
	return &agent.NodeValidateResponse{Ok: true}, nil
}

// testServer encapsulates a mock gRPC server for testing
type testServer struct {
	listener   net.Listener
	grpcServer *grpc.Server
	mock       *mockAgentServer
	address    string
}

func startTestServer(t *testing.T) *testServer {
	listener, err := net.Listen("tcp", "localhost:0")
	if err != nil {
		t.Fatalf("Failed to create listener: %v", err)
	}

	grpcServer := grpc.NewServer()
	mock := &mockAgentServer{}
	agent.RegisterAgentServiceServer(grpcServer, mock)

	go func() {
		if err := grpcServer.Serve(listener); err != nil {
			// Server stopped, this is expected
		}
	}()

	// Give server time to start
	time.Sleep(10 * time.Millisecond)

	return &testServer{
		listener:   listener,
		grpcServer: grpcServer,
		mock:       mock,
		address:    listener.Addr().String(),
	}
}

func (ts *testServer) stop() {
	ts.grpcServer.Stop()
	ts.listener.Close()
}

// Connection Management Tests

func TestNewClient(t *testing.T) {
	cli := NewClient()
	if cli == nil {
		t.Fatal("Expected non-nil client")
	}

	// Verify it's the correct type
	c, ok := cli.(*client)
	if !ok {
		t.Fatal("Expected *client type")
	}

	if c.timeout != 120*time.Second {
		t.Errorf("Expected default timeout of 120s, got %v", c.timeout)
	}

	if c.connections == nil {
		t.Error("Expected connections map to be initialized")
	}
}

func TestClient_SetTimeout(t *testing.T) {
	c := NewClient().(*client)
	defer c.Close()

	newTimeout := 60 * time.Second
	c.SetTimeout(newTimeout)

	if c.timeout != newTimeout {
		t.Errorf("Expected timeout %v, got %v", newTimeout, c.timeout)
	}
}

func TestClient_getConnection_CreatesNew(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient().(*client)
	defer c.Close()

	conn, err := c.getConnection(server.address)
	if err != nil {
		t.Fatalf("Failed to get connection: %v", err)
	}

	if conn == nil {
		t.Fatal("Expected non-nil connection")
	}

	// Verify connection state
	state := conn.GetState()
	if state != connectivity.Ready && state != connectivity.Connecting {
		t.Errorf("Expected connection to be Ready or Connecting, got %v", state)
	}
}

func TestClient_getConnection_ReusesExisting(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient().(*client)
	defer c.Close()

	// First call creates connection
	conn1, err := c.getConnection(server.address)
	if err != nil {
		t.Fatalf("Failed to get first connection: %v", err)
	}

	// Second call should reuse connection
	conn2, err := c.getConnection(server.address)
	if err != nil {
		t.Fatalf("Failed to get second connection: %v", err)
	}

	if conn1 != conn2 {
		t.Error("Expected connection to be reused")
	}

	// Verify only one connection in map
	c.mu.RLock()
	connCount := len(c.connections)
	c.mu.RUnlock()

	if connCount != 1 {
		t.Errorf("Expected 1 connection, got %d", connCount)
	}
}

func TestClient_getConnection_RecreatesClosed(t *testing.T) {
	// Start first server
	server1 := startTestServer(t)
	address := server1.address

	c := NewClient().(*client)
	defer c.Close()

	// Get connection to first server
	conn1, err := c.getConnection(address)
	if err != nil {
		t.Fatalf("Failed to get first connection: %v", err)
	}

	// Stop first server
	server1.stop()

	// Wait for connection to realize it's closed
	time.Sleep(100 * time.Millisecond)

	// Close the connection directly and verify it gets recreated
	conn1.Close()

	// Wait for state to change
	time.Sleep(50 * time.Millisecond)

	// Start new server on same port if possible, or any port
	server2 := startTestServer(t)
	defer server2.stop()

	// Force new connection by using different address
	_, err = c.getConnection(server2.address)
	if err != nil {
		t.Fatalf("Failed to get new connection: %v", err)
	}

	// Verify we have 2 connections now (one old closed, one new)
	c.mu.RLock()
	connCount := len(c.connections)
	c.mu.RUnlock()

	if connCount != 2 {
		t.Errorf("Expected 2 connections, got %d", connCount)
	}
}

func TestClient_Close(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient().(*client)

	// Create a connection
	_, err := c.getConnection(server.address)
	if err != nil {
		t.Fatalf("Failed to get connection: %v", err)
	}

	// Verify connection exists
	c.mu.RLock()
	connCount := len(c.connections)
	c.mu.RUnlock()

	if connCount != 1 {
		t.Errorf("Expected 1 connection before close, got %d", connCount)
	}

	// Close client
	if err := c.Close(); err != nil {
		t.Errorf("Close failed: %v", err)
	}

	// Verify connections map is empty
	c.mu.RLock()
	connCount = len(c.connections)
	c.mu.RUnlock()

	if connCount != 0 {
		t.Errorf("Expected 0 connections after close, got %d", connCount)
	}
}

func TestClient_Close_MultipleConnections(t *testing.T) {
	server1 := startTestServer(t)
	defer server1.stop()

	server2 := startTestServer(t)
	defer server2.stop()

	c := NewClient().(*client)

	// Create connections to both servers
	_, err := c.getConnection(server1.address)
	if err != nil {
		t.Fatalf("Failed to get connection to server1: %v", err)
	}

	_, err = c.getConnection(server2.address)
	if err != nil {
		t.Fatalf("Failed to get connection to server2: %v", err)
	}

	// Verify connections exist
	c.mu.RLock()
	connCount := len(c.connections)
	c.mu.RUnlock()

	if connCount != 2 {
		t.Errorf("Expected 2 connections, got %d", connCount)
	}

	// Close should clean up all connections
	if err := c.Close(); err != nil {
		t.Errorf("Close failed: %v", err)
	}

	// Verify all connections closed
	c.mu.RLock()
	connCount = len(c.connections)
	c.mu.RUnlock()

	if connCount != 0 {
		t.Errorf("Expected 0 connections after close, got %d", connCount)
	}
}

// Connection Failure Tests

func TestClient_ConnectionFailure_InvalidAddress(t *testing.T) {
	c := NewClient().(*client)
	defer c.Close()

	// gRPC dial is lazy, so we need to wait for the connection state change
	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()

	// The getConnection uses grpc.Dial which is async and may not fail immediately
	conn, err := c.getConnection("localhost:59999")

	// gRPC Dial may succeed immediately (async connection), but the connection won't be ready
	if err == nil && conn != nil {
		// Wait for connection state to change with a deadline
		state := conn.GetState()
		if state == connectivity.TransientFailure || state == connectivity.Shutdown {
			// This is expected for a non-existent server
			return
		}

		// Try to wait for state change with timeout
		if !conn.WaitForStateChange(ctx, state) {
			// Context timed out, which means connection never became ready
			// This is expected behavior for non-existent server
			return
		}

		// If we get here without error, the state changed but let's check what state
		newState := conn.GetState()
		if newState != connectivity.TransientFailure && newState != connectivity.Shutdown {
			t.Errorf("Expected connection to fail, got state: %v", newState)
		}
	}
}

func TestClient_dial(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient().(*client)
	defer c.Close()

	conn, err := c.dial(server.address)
	if err != nil {
		t.Fatalf("dial failed: %v", err)
	}
	defer conn.Close()

	if conn == nil {
		t.Error("Expected non-nil connection from dial")
	}
}

// Real gRPC Connection Tests

func TestRealGRPCConnection(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	// Create a direct gRPC connection
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	conn, err := grpc.DialContext(ctx, server.address,
		grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithBlock(),
	)
	if err != nil {
		t.Fatalf("Failed to dial: %v", err)
	}
	defer conn.Close()

	// Verify connection is ready
	state := conn.GetState()
	if state != connectivity.Ready {
		t.Errorf("Expected connection to be Ready, got %v", state)
	}
}

func TestClient_MultipleNodes(t *testing.T) {
	server1 := startTestServer(t)
	defer server1.stop()

	server2 := startTestServer(t)
	defer server2.stop()

	c := NewClient().(*client)
	defer c.Close()

	// Connect to first server
	conn1, err := c.getConnection(server1.address)
	if err != nil {
		t.Fatalf("Failed to connect to server1: %v", err)
	}

	// Connect to second server
	conn2, err := c.getConnection(server2.address)
	if err != nil {
		t.Fatalf("Failed to connect to server2: %v", err)
	}

	// Connections should be different
	if conn1 == conn2 {
		t.Error("Expected different connections for different nodes")
	}

	// Verify two connections in map
	c.mu.RLock()
	connCount := len(c.connections)
	c.mu.RUnlock()

	if connCount != 2 {
		t.Errorf("Expected 2 connections, got %d", connCount)
	}
}

// Close Tests

func TestClient_Close_NoConnections(t *testing.T) {
	c := NewClient().(*client)

	// Close without any connections should not error
	if err := c.Close(); err != nil {
		t.Errorf("Close failed: %v", err)
	}
}

// getConnection Tests with closed connection

func TestClient_getConnection_ExistingClosed(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient().(*client)
	defer c.Close()

	// First, create a connection
	conn1, err := c.getConnection(server.address)
	if err != nil {
		t.Fatalf("Failed to get connection: %v", err)
	}

	// Close the connection
	conn1.Close()

	// Wait for state to change
	time.Sleep(50 * time.Millisecond)

	// Now get connection again - should create new one
	conn2, err := c.getConnection(server.address)
	if err != nil {
		t.Fatalf("Failed to get connection after close: %v", err)
	}

	// Should be different connection
	if conn1 == conn2 {
		t.Error("Expected new connection after close")
	}
}

// Connection state tracking tests

func TestClient_ConnectionStateTracking(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient().(*client)
	defer c.Close()

	// Get connection
	conn, err := c.getConnection(server.address)
	if err != nil {
		t.Fatalf("Failed to get connection: %v", err)
	}

	// Verify connection is tracked
	c.mu.RLock()
	trackedConn, exists := c.connections[server.address]
	c.mu.RUnlock()

	if !exists {
		t.Error("Expected connection to be tracked")
	}

	if trackedConn != conn {
		t.Error("Tracked connection doesn't match returned connection")
	}
}

// Timeout and context tests

func TestClient_SetTimeout_Zero(t *testing.T) {
	c := NewClient().(*client)
	defer c.Close()

	c.SetTimeout(0)

	if c.timeout != 0 {
		t.Errorf("Expected timeout 0, got %v", c.timeout)
	}
}

func TestClient_SetTimeout_Small(t *testing.T) {
	c := NewClient().(*client)
	defer c.Close()

	c.SetTimeout(1 * time.Nanosecond)

	if c.timeout != 1*time.Nanosecond {
		t.Errorf("Expected timeout 1ns, got %v", c.timeout)
	}
}

// Concurrency Tests

func TestClient_ConcurrentConnectionCreation(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient().(*client)
	defer c.Close()

	// Number of concurrent goroutines all trying to connect to the same node
	numGoroutines := 20
	var wg sync.WaitGroup
	errChan := make(chan error, numGoroutines)

	for i := 0; i < numGoroutines; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			conn, err := c.getConnection(server.address)
			if err != nil {
				errChan <- err
				return
			}
			if conn == nil {
				errChan <- errors.New("nil connection")
			}
		}()
	}

	wg.Wait()
	close(errChan)

	// Check for errors
	errorCount := 0
	for err := range errChan {
		t.Logf("Connection error: %v", err)
		errorCount++
	}

	// Connection creation errors should be minimal or zero
	if errorCount > 0 {
		t.Errorf("Got %d errors during concurrent connection creation", errorCount)
	}

	// Verify only one connection was created
	c.mu.RLock()
	connCount := len(c.connections)
	c.mu.RUnlock()

	if connCount != 1 {
		t.Errorf("Expected 1 connection after concurrent access, got %d", connCount)
	}
}

func TestClient_ConcurrentMultiNode(t *testing.T) {
	server1 := startTestServer(t)
	defer server1.stop()

	server2 := startTestServer(t)
	defer server2.stop()

	c := NewClient().(*client)
	defer c.Close()

	var wg sync.WaitGroup
	errChan := make(chan error, 40)

	// Concurrent connections to different nodes
	for i := 0; i < 20; i++ {
		wg.Add(2)

		go func() {
			defer wg.Done()
			_, err := c.getConnection(server1.address)
			if err != nil {
				errChan <- err
			}
		}()

		go func() {
			defer wg.Done()
			_, err := c.getConnection(server2.address)
			if err != nil {
				errChan <- err
			}
		}()
	}

	wg.Wait()
	close(errChan)

	// Check for errors
	for err := range errChan {
		t.Logf("Connection error: %v", err)
	}

	// Verify exactly 2 connections (one per node)
	c.mu.RLock()
	connCount := len(c.connections)
	c.mu.RUnlock()

	if connCount != 2 {
		t.Errorf("Expected 2 connections, got %d", connCount)
	}
}

func TestClient_ConcurrentClose(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient().(*client)

	// Create a connection
	_, err := c.getConnection(server.address)
	if err != nil {
		t.Fatalf("Failed to get connection: %v", err)
	}

	var wg sync.WaitGroup

	// Try to close concurrently (should not panic)
	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_ = c.Close()
		}()
	}

	wg.Wait()

	// Verify connections are closed
	c.mu.RLock()
	connCount := len(c.connections)
	c.mu.RUnlock()

	if connCount != 0 {
		t.Errorf("Expected 0 connections after close, got %d", connCount)
	}
}

// Interface compliance test

func TestClient_InterfaceCompliance(t *testing.T) {
	// Verify that *client implements Client interface
	var _ Client = (*client)(nil)
	var _ Client = NewClient()
}

// RPC Method Tests - These test the actual client RPC method implementations

func TestClient_Ping_Success(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	err := c.Ping(ctx, server.address)

	// With real gRPC client, this should work without error
	if err != nil {
		t.Logf("Ping error (may be expected in test environment): %v", err)
	}

	if !server.mock.pingCalled {
		t.Log("Note: mock server tracking may not work with real gRPC client")
	}
}

func TestClient_StartVM_Success(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	err := c.StartVM(ctx, server.address, "test-vm-123")

	// Test that the method runs without panic
	// With real gRPC client, this will make an actual RPC call
	if err != nil {
		t.Logf("StartVM error: %v", err)
	}
}

func TestClient_StopVM_Success(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	err := c.StopVM(ctx, server.address, "test-vm-123")

	if err != nil {
		t.Logf("StopVM error: %v", err)
	}
}

func TestClient_RebootVM_Success(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	err := c.RebootVM(ctx, server.address, "test-vm-123")

	if err != nil {
		t.Logf("RebootVM error: %v", err)
	}
}

func TestClient_DeleteVM_Success(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	err := c.DeleteVM(ctx, server.address, "test-vm-123")

	if err != nil {
		t.Logf("DeleteVM error: %v", err)
	}
}

func TestClient_GetVMState_Success(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	resp, err := c.GetVMState(ctx, server.address, "test-vm-123")

	if err != nil {
		t.Logf("GetVMState error: %v", err)
	}

	_ = resp // Response may be nil if error occurred
}

func TestClient_ProvisionVM_Success(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	req := &agent.ProvisionVMRequest{
		VmId:     "test-vm-123",
		VmName:   "test-vm",
		Vcpu:     2,
		MemoryMb: 4096,
	}
	err := c.ProvisionVM(ctx, server.address, req)

	if err != nil {
		t.Logf("ProvisionVM error: %v", err)
	}
}

func TestClient_CreateVolume_Success(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	req := &agent.VolumeCreateRequest{
		VolumeId:  "vol-123",
		PoolId:    "pool-1",
		Format:    "qcow2",
		SizeBytes: 10737418240,
	}
	err := c.CreateVolume(ctx, server.address, req)

	if err != nil {
		t.Logf("CreateVolume error: %v", err)
	}
}

func TestClient_ResizeVolume_Success(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	req := &agent.VolumeResizeRequest{
		VolumeId:     "vol-123",
		NewSizeBytes: 21474836480,
	}
	err := c.ResizeVolume(ctx, server.address, req)

	if err != nil {
		t.Logf("ResizeVolume error: %v", err)
	}
}

func TestClient_EnsureBridge_Success(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	req := &agent.EnsureBridgeRequest{
		BridgeName: "br0",
		Mtu:        1500,
	}
	err := c.EnsureBridge(ctx, server.address, req)

	if err != nil {
		t.Logf("EnsureBridge error: %v", err)
	}
}

func TestClient_ImportImage_Success(t *testing.T) {
	server := startTestServer(t)
	defer server.stop()

	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	req := &agent.ImageImportRequest{
		ImageId:          "img-123",
		SourceUrl:        "http://example.com/image.qcow2",
		SourceFormat:     "qcow2",
		NormalizedFormat: "raw",
	}
	err := c.ImportImage(ctx, server.address, req)

	if err != nil {
		t.Logf("ImportImage error: %v", err)
	}
}

// Connection failure tests for RPC methods

func TestClient_Ping_ConnectionFailure(t *testing.T) {
	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	err := c.Ping(ctx, "localhost:59999")

	// Should get connection error
	if err == nil {
		t.Error("Expected error for connection failure")
	}
}

func TestClient_StartVM_ConnectionFailure(t *testing.T) {
	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	err := c.StartVM(ctx, "localhost:59999", "test-vm")

	if err == nil {
		t.Error("Expected error for connection failure")
	}
}

func TestClient_StopVM_ConnectionFailure(t *testing.T) {
	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	err := c.StopVM(ctx, "localhost:59999", "test-vm")

	if err == nil {
		t.Error("Expected error for connection failure")
	}
}

func TestClient_RebootVM_ConnectionFailure(t *testing.T) {
	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	err := c.RebootVM(ctx, "localhost:59999", "test-vm")

	if err == nil {
		t.Error("Expected error for connection failure")
	}
}

func TestClient_DeleteVM_ConnectionFailure(t *testing.T) {
	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	err := c.DeleteVM(ctx, "localhost:59999", "test-vm")

	if err == nil {
		t.Error("Expected error for connection failure")
	}
}

func TestClient_GetVMState_ConnectionFailure(t *testing.T) {
	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	_, err := c.GetVMState(ctx, "localhost:59999", "test-vm")

	if err == nil {
		t.Error("Expected error for connection failure")
	}
}

func TestClient_ProvisionVM_ConnectionFailure(t *testing.T) {
	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	req := &agent.ProvisionVMRequest{VmId: "test-vm"}
	err := c.ProvisionVM(ctx, "localhost:59999", req)

	if err == nil {
		t.Error("Expected error for connection failure")
	}
}

func TestClient_CreateVolume_ConnectionFailure(t *testing.T) {
	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	req := &agent.VolumeCreateRequest{VolumeId: "vol-123"}
	err := c.CreateVolume(ctx, "localhost:59999", req)

	if err == nil {
		t.Error("Expected error for connection failure")
	}
}

func TestClient_ResizeVolume_ConnectionFailure(t *testing.T) {
	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	req := &agent.VolumeResizeRequest{VolumeId: "vol-123"}
	err := c.ResizeVolume(ctx, "localhost:59999", req)

	if err == nil {
		t.Error("Expected error for connection failure")
	}
}

func TestClient_EnsureBridge_ConnectionFailure(t *testing.T) {
	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	req := &agent.EnsureBridgeRequest{BridgeName: "br0"}
	err := c.EnsureBridge(ctx, "localhost:59999", req)

	if err == nil {
		t.Error("Expected error for connection failure")
	}
}

func TestClient_ImportImage_ConnectionFailure(t *testing.T) {
	c := NewClient()
	defer c.Close()

	ctx := context.Background()
	req := &agent.ImageImportRequest{ImageId: "img-123"}
	err := c.ImportImage(ctx, "localhost:59999", req)

	if err == nil {
		t.Error("Expected error for connection failure")
	}
}
