package e2e

import (
	"fmt"
	"strings"
	"testing"
	"time"
)

// TestNodeLifecycle tests the full node lifecycle.
func TestNodeLifecycle(t *testing.T) {
	h := NewHarness(t)
	
	if err := h.WaitForController(30 * time.Second); err != nil {
		t.Skipf("Controller not ready: %v", err)
	}
	
	// Create a node
	nodeReq := &CreateNodeRequest{
		Hostname:      fmt.Sprintf("test-node-%d", time.Now().Unix()),
		ManagementIP:  "172.20.0.20",
		TotalCPUCores: 16,
		TotalRAMMB:    32768,
	}
	
	t.Log("Creating node...")
	nodeResp, err := h.CreateNode(nodeReq)
	if err != nil {
		t.Fatalf("Failed to create node: %v", err)
	}
	
	if nodeResp.ID == "" {
		t.Error("Node ID is empty")
	}
	if nodeResp.Hostname != nodeReq.Hostname {
		t.Errorf("Hostname mismatch: expected %s, got %s", nodeReq.Hostname, nodeResp.Hostname)
	}
	
	t.Logf("Created node: %s", nodeResp.ID)
	
	// Get node
	t.Log("Getting node...")
	node, err := h.GetNode(nodeResp.ID)
	if err != nil {
		t.Fatalf("Failed to get node: %v", err)
	}
	if node.ID != nodeResp.ID {
		t.Errorf("Node ID mismatch: expected %s, got %s", nodeResp.ID, node.ID)
	}
	
	t.Logf("Retrieved node: %s (%s)", node.ID, node.Hostname)
}

// TestNode_Validation tests node creation validation.
func TestNode_Validation(t *testing.T) {
	h := NewHarness(t)
	
	if err := h.WaitForController(30 * time.Second); err != nil {
		t.Skipf("Controller not ready: %v", err)
	}
	
	tests := []struct {
		name    string
		request *CreateNodeRequest
		wantErr bool
		errMsg  string
	}{
		{
			name: "missing_hostname",
			request: &CreateNodeRequest{
				Hostname:      "",
				ManagementIP:  "172.20.0.1",
				TotalCPUCores: 4,
				TotalRAMMB:    8192,
			},
			wantErr: true,
			errMsg:  "hostname",
		},
		{
			name: "missing_ip",
			request: &CreateNodeRequest{
				Hostname:      "test-node",
				ManagementIP:  "",
				TotalCPUCores: 4,
				TotalRAMMB:    8192,
			},
			wantErr: true,
			errMsg:  "ip",
		},
		{
			name: "invalid_ip",
			request: &CreateNodeRequest{
				Hostname:      "test-node",
				ManagementIP:  "not-an-ip",
				TotalCPUCores: 4,
				TotalRAMMB:    8192,
			},
			wantErr: true,
			errMsg:  "ip",
		},
		{
			name: "zero_cpu",
			request: &CreateNodeRequest{
				Hostname:      "test-node",
				ManagementIP:  "172.20.0.1",
				TotalCPUCores: 0,
				TotalRAMMB:    8192,
			},
			wantErr: true,
			errMsg:  "cpu",
		},
		{
			name: "zero_ram",
			request: &CreateNodeRequest{
				Hostname:      "test-node",
				ManagementIP:  "172.20.0.1",
				TotalCPUCores: 4,
				TotalRAMMB:    0,
			},
			wantErr: true,
			errMsg:  "ram",
		},
	}
	
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := h.CreateNode(tt.request)
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

// TestNode_DuplicateHostname tests that duplicate hostnames are rejected.
func TestNode_DuplicateHostname(t *testing.T) {
	h := NewHarness(t)
	
	if err := h.WaitForController(30 * time.Second); err != nil {
		t.Skipf("Controller not ready: %v", err)
	}
	
	hostname := fmt.Sprintf("duplicate-test-%d", time.Now().Unix())
	
	// Create first node
	node1, err := h.CreateNode(&CreateNodeRequest{
		Hostname:      hostname,
		ManagementIP:  "172.20.0.30",
		TotalCPUCores: 4,
		TotalRAMMB:    8192,
	})
	if err != nil {
		t.Fatalf("Failed to create first node: %v", err)
	}
	t.Logf("Created first node: %s", node1.ID)
	
	// Try to create second node with same hostname
	_, err = h.CreateNode(&CreateNodeRequest{
		Hostname:      hostname,
		ManagementIP:  "172.20.0.31",
		TotalCPUCores: 4,
		TotalRAMMB:    8192,
	})
	if err == nil {
		t.Error("Expected error for duplicate hostname, got nil")
	} else if !strings.Contains(strings.ToLower(err.Error()), "duplicate") && !strings.Contains(strings.ToLower(err.Error()), "exists") {
		t.Errorf("Expected duplicate error, got: %v", err)
	}
}
