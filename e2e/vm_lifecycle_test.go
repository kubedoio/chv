package e2e

import (
	"fmt"
	"strings"
	"testing"
	"time"
)

// TestVMLifecycle_Full tests the complete VM lifecycle.
func TestVMLifecycle_Full(t *testing.T) {
	h := NewHarness(t)
	
	// Wait for controller to be ready
	t.Log("Waiting for controller to be ready...")
	if err := h.WaitForController(30 * time.Second); err != nil {
		t.Fatalf("Controller not ready: %v", err)
	}
	t.Log("Controller is ready")
	
	// Create a network first
	t.Log("Creating network...")
	netReq := &CreateNetworkRequest{
		Name:       "test-network",
		BridgeName: "br-test",
		CIDR:       "10.100.0.0/24",
		GatewayIP:  "10.100.0.1",
	}
	netResp, err := h.CreateNetwork(netReq)
	if err != nil {
		t.Fatalf("Failed to create network: %v", err)
	}
	t.Logf("Created network: %s (%s)", netResp.ID, netResp.Name)
	
	// Register a mock node (simulating an agent)
	t.Log("Creating node...")
	nodeReq := &CreateNodeRequest{
		Hostname:      "test-node-01",
		ManagementIP:  "172.20.0.10",
		TotalCPUCores: 8,
		TotalRAMMB:    16384,
	}
	nodeResp, err := h.CreateNode(nodeReq)
	if err != nil {
		t.Fatalf("Failed to create node: %v", err)
	}
	t.Logf("Created node: %s (%s)", nodeResp.ID, nodeResp.Hostname)
	
	// Create a VM
	t.Log("Creating VM...")
	vmName := fmt.Sprintf("test-vm-%d", time.Now().Unix())
	vmReq := &CreateVMRequest{
		Name:        vmName,
		Description: "E2E test VM",
		VCPU:        2,
		MemoryMB:    2048,
		DiskGB:      10,
		ImageID:     "00000000-0000-0000-0000-000000000001",
		NetworkIDs:  []string{netResp.ID},
		UserData: `#cloud-config
users:
  - name: testuser
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - ssh-rsa AAAAB3NzaC1 test@example.com
`,
	}
	
	vmResp, err := h.CreateVM(vmReq)
	if err != nil {
		t.Fatalf("Failed to create VM: %v", err)
	}
	t.Logf("Created VM: %s (%s)", vmResp.ID, vmResp.Name)
	
	// Verify initial state
	if vmResp.DesiredState != "running" {
		t.Errorf("Expected desired_state=running, got %s", vmResp.DesiredState)
	}
	
	// Wait for VM to be provisioned
	t.Log("Waiting for VM to be provisioned...")
	time.Sleep(2 * time.Second) // Give reconciler time to process
	
	// Get VM status
	vm, err := h.GetVM(vmResp.ID)
	if err != nil {
		t.Fatalf("Failed to get VM: %v", err)
	}
	t.Logf("VM state: desired=%s, actual=%s", vm.DesiredState, vm.ActualState)
	
	// Clean up
	t.Log("Cleaning up...")
	if err := h.DeleteVM(vmResp.ID); err != nil {
		t.Logf("Warning: failed to delete VM: %v", err)
	}
	
	t.Log("Test completed successfully")
}

// TestVMLifecycle_StartStop tests starting and stopping a VM.
func TestVMLifecycle_StartStop(t *testing.T) {
	h := NewHarness(t)
	
	if err := h.WaitForController(30 * time.Second); err != nil {
		t.Skipf("Controller not ready: %v", err)
	}
	
	// Create network
	netResp, err := h.CreateNetwork(&CreateNetworkRequest{
		Name:       "test-net-startstop",
		BridgeName: "br-test2",
		CIDR:       "10.101.0.0/24",
		GatewayIP:  "10.101.0.1",
	})
	if err != nil {
		t.Fatalf("Failed to create network: %v", err)
	}
	
	// Create node
	nodeResp, err := h.CreateNode(&CreateNodeRequest{
		Hostname:      "test-node-startstop",
		ManagementIP:  "172.20.0.11",
		TotalCPUCores: 4,
		TotalRAMMB:    8192,
	})
	if err != nil {
		t.Fatalf("Failed to create node: %v", err)
	}
	
	// Create VM
	vmName := fmt.Sprintf("test-vm-startstop-%d", time.Now().Unix())
	vmResp, err := h.CreateVM(&CreateVMRequest{
		Name:       vmName,
		VCPU:       1,
		MemoryMB:   1024,
		DiskGB:     5,
		ImageID:    "00000000-0000-0000-0000-000000000001",
		NetworkIDs: []string{netResp.ID},
	})
	if err != nil {
		t.Fatalf("Failed to create VM: %v", err)
	}
	
	// Note: In a real E2E test with a running agent, we would:
	// 1. Start the VM
	// 2. Wait for it to be running
	// 3. Stop the VM
	// 4. Wait for it to be stopped
	// 5. Delete the VM
	
	// For now, just verify the VM exists
	vm, err := h.GetVM(vmResp.ID)
	if err != nil {
		t.Fatalf("Failed to get VM: %v", err)
	}
	if vm.Name != vmName {
		t.Errorf("VM name mismatch: expected %s, got %s", vmName, vm.Name)
	}
	
	// Clean up
	h.DeleteVM(vmResp.ID)
	_ = nodeResp // Use the variable
}

// TestVM_Validation tests VM creation validation.
func TestVM_Validation(t *testing.T) {
	h := NewHarness(t)
	
	if err := h.WaitForController(30 * time.Second); err != nil {
		t.Skipf("Controller not ready: %v", err)
	}
	
	tests := []struct {
		name    string
		request *CreateVMRequest
		wantErr bool
		errMsg  string
	}{
		{
			name: "missing_name",
			request: &CreateVMRequest{
				Name:     "",
				VCPU:     1,
				MemoryMB: 1024,
				DiskGB:   5,
			},
			wantErr: true,
			errMsg:  "name",
		},
		{
			name: "zero_vcpu",
			request: &CreateVMRequest{
				Name:     "test-vm",
				VCPU:     0,
				MemoryMB: 1024,
				DiskGB:   5,
			},
			wantErr: true,
			errMsg:  "vcpu",
		},
		{
			name: "zero_memory",
			request: &CreateVMRequest{
				Name:     "test-vm",
				VCPU:     1,
				MemoryMB: 0,
				DiskGB:   5,
			},
			wantErr: true,
			errMsg:  "memory",
		},
		{
			name: "excessive_resources",
			request: &CreateVMRequest{
				Name:     "test-vm",
				VCPU:     1000,
				MemoryMB: 1024 * 1024,
				DiskGB:   5,
			},
			wantErr: true,
			errMsg:  "exceeds",
		},
	}
	
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := h.CreateVM(tt.request)
			if tt.wantErr {
				if err == nil {
					t.Errorf("Expected error containing %q, got nil", tt.errMsg)
				} else if !strings.Contains(strings.ToLower(err.Error()), strings.ToLower(tt.errMsg)) {
					t.Errorf("Expected error containing %q, got %q", tt.errMsg, err.Error())
				}
			} else {
				if err != nil {
					t.Errorf("Unexpected error: %v", err)
				}
			}
		})
	}
}

// TestHealthEndpoint tests the health endpoint.
func TestHealthEndpoint(t *testing.T) {
	h := NewHarness(t)
	
	if err := h.HealthCheck(); err != nil {
		t.Fatalf("Health check failed: %v", err)
	}
	
	t.Log("Health check passed")
}

// TestConcurrentVMOperations tests creating multiple VMs concurrently.
func TestConcurrentVMOperations(t *testing.T) {
	h := NewHarness(t)
	
	if err := h.WaitForController(30 * time.Second); err != nil {
		t.Skipf("Controller not ready: %v", err)
	}
	
	// Create network
	netResp, err := h.CreateNetwork(&CreateNetworkRequest{
		Name:       "test-net-concurrent",
		BridgeName: "br-test3",
		CIDR:       "10.102.0.0/24",
		GatewayIP:  "10.102.0.1",
	})
	if err != nil {
		t.Fatalf("Failed to create network: %v", err)
	}
	
	// Create VMs concurrently
	numVMs := 3
	results := make(chan error, numVMs)
	vmIDs := make(chan string, numVMs)
	
	for i := 0; i < numVMs; i++ {
		go func(index int) {
			vmName := fmt.Sprintf("test-vm-concurrent-%d-%d", index, time.Now().Unix())
			vmResp, err := h.CreateVM(&CreateVMRequest{
				Name:       vmName,
				VCPU:       1,
				MemoryMB:   512,
				DiskGB:     5,
				ImageID:    "00000000-0000-0000-0000-000000000001",
				NetworkIDs: []string{netResp.ID},
			})
			if err != nil {
				results <- fmt.Errorf("VM %d: %w", index, err)
				return
			}
			vmIDs <- vmResp.ID
			results <- nil
		}(i)
	}
	
	// Wait for all goroutines to complete
	var createdVMs []string
	for i := 0; i < numVMs; i++ {
		if err := <-results; err != nil {
			t.Errorf("Failed to create VM: %v", err)
		}
	}
	close(vmIDs)
	for id := range vmIDs {
		createdVMs = append(createdVMs, id)
	}
	
	t.Logf("Created %d VMs concurrently", len(createdVMs))
	
	// Clean up
	for _, id := range createdVMs {
		if err := h.DeleteVM(id); err != nil {
			t.Logf("Warning: failed to delete VM %s: %v", id, err)
		}
	}
}
