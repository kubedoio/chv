// Package e2e provides end-to-end tests for CHV.
package e2e

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"testing"
	"time"
)

// Config holds E2E test configuration.
type Config struct {
	ControllerURL string
	APIVersion    string
	Timeout       time.Duration
}

// DefaultConfig returns default E2E configuration.
func DefaultConfig() *Config {
	url := os.Getenv("CHV_E2E_URL")
	if url == "" {
		// Default to port 8081 to avoid conflicts with local dev server
		url = "http://localhost:8081"
	}
	return &Config{
		ControllerURL: url,
		APIVersion:    "v1",
		Timeout:       30 * time.Second,
	}
}

// Harness provides test helpers for E2E tests.
type Harness struct {
	Config     *Config
	HTTPClient *http.Client
	ctx        context.Context
	token      string // Auth token for protected endpoints
}

// NewHarness creates a new E2E test harness.
func NewHarness(t *testing.T) *Harness {
	config := DefaultConfig()
	h := &Harness{
		Config: config,
		HTTPClient: &http.Client{
			Timeout: config.Timeout,
		},
		ctx: context.Background(),
	}
	
	// Try to create a token for authenticated requests
	// If it fails, tests requiring auth will fail appropriately
	_ = h.createTestToken()
	
	return h
}

// URL builds a full URL for an API endpoint.
func (h *Harness) URL(path string) string {
	return fmt.Sprintf("%s/api/%s%s", h.Config.ControllerURL, h.Config.APIVersion, path)
}

// createTestToken creates a test API token for authentication.
func (h *Harness) createTestToken() error {
	// Create a token using the public tokens endpoint
	url := fmt.Sprintf("%s/api/%s/tokens", h.Config.ControllerURL, h.Config.APIVersion)
	
	body := map[string]interface{}{
		"name":       "e2e-test-token",
		"expires_in": "1h",
	}
	
	data, _ := json.Marshal(body)
	resp, err := h.HTTPClient.Post(url, "application/json", bytes.NewReader(data))
	if err != nil {
		return fmt.Errorf("failed to create token: %w", err)
	}
	defer resp.Body.Close()
	
	if resp.StatusCode != http.StatusCreated && resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("token creation failed: HTTP %d: %s", resp.StatusCode, string(body))
	}
	
	var result struct {
		Token string `json:"token"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return fmt.Errorf("failed to decode token response: %w", err)
	}
	
	h.token = result.Token
	return nil
}

// DoRequest makes an HTTP request and returns the response.
func (h *Harness) DoRequest(method, path string, body interface{}) (*http.Response, error) {
	return h.DoRequestWithAuth(method, path, h.token, body)
}

// DoRequestWithAuth makes an authenticated HTTP request.
func (h *Harness) DoRequestWithAuth(method, path, token string, body interface{}) (*http.Response, error) {
	url := h.URL(path)
	
	var bodyReader io.Reader
	if body != nil {
		data, err := json.Marshal(body)
		if err != nil {
			return nil, fmt.Errorf("failed to marshal body: %w", err)
		}
		bodyReader = bytes.NewReader(data)
	}
	
	req, err := http.NewRequestWithContext(h.ctx, method, url, bodyReader)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}
	
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	req.Header.Set("Accept", "application/json")
	
	if token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	}
	
	return h.HTTPClient.Do(req)
}



// ParseResponse parses an HTTP response into the given target.
func (h *Harness) ParseResponse(resp *http.Response, target interface{}) error {
	defer resp.Body.Close()
	
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return fmt.Errorf("failed to read response body: %w", err)
	}
	
	if resp.StatusCode >= 400 {
		return fmt.Errorf("HTTP %d: %s", resp.StatusCode, string(body))
	}
	
	if target != nil && len(body) > 0 {
		if err := json.Unmarshal(body, target); err != nil {
			return fmt.Errorf("failed to unmarshal response: %w", err)
		}
	}
	
	return nil
}

// HealthCheck checks if the controller is healthy.
func (h *Harness) HealthCheck() error {
	// Health endpoint is at /health, not /api/v1/health
	resp, err := h.HTTPClient.Get(h.Config.ControllerURL + "/health")
	if err != nil {
		return fmt.Errorf("health check request failed: %w", err)
	}
	defer resp.Body.Close()
	
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("health check failed: HTTP %d", resp.StatusCode)
	}
	
	return nil
}

// WaitForController waits for the controller to become ready.
func (h *Harness) WaitForController(timeout time.Duration) error {
	ctx, cancel := context.WithTimeout(h.ctx, timeout)
	defer cancel()
	
	ticker := time.NewTicker(500 * time.Millisecond)
	defer ticker.Stop()
	
	for {
		select {
		case <-ctx.Done():
			return fmt.Errorf("timeout waiting for controller")
		case <-ticker.C:
			if err := h.HealthCheck(); err == nil {
				return nil
			}
		}
	}
}

// CreateNodeRequest is a request to create a node.
type CreateNodeRequest struct {
	Hostname      string `json:"hostname"`
	ManagementIP  string `json:"management_ip"`
	TotalCPUCores uint32 `json:"total_cpu_cores"`
	TotalRAMMB    uint64 `json:"total_ram_mb"`
}

// CreateNodeResponse is the response from creating a node.
type CreateNodeResponse struct {
	ID       string `json:"id"`
	Hostname string `json:"hostname"`
	Status   string `json:"status"`
}

// CreateNode creates a new node.
func (h *Harness) CreateNode(req *CreateNodeRequest) (*CreateNodeResponse, error) {
	// Note: Node registration endpoint is /nodes/register
	resp, err := h.DoRequest("POST", "/nodes/register", req)
	if err != nil {
		return nil, err
	}
	
	var result CreateNodeResponse
	if err := h.ParseResponse(resp, &result); err != nil {
		return nil, err
	}
	
	return &result, nil
}

// GetNode gets a node by ID.
func (h *Harness) GetNode(id string) (*CreateNodeResponse, error) {
	resp, err := h.DoRequest("GET", fmt.Sprintf("/nodes/%s", id), nil)
	if err != nil {
		return nil, err
	}
	
	var result CreateNodeResponse
	if err := h.ParseResponse(resp, &result); err != nil {
		return nil, err
	}
	
	return &result, nil
}

// CreateVMRequest is a request to create a VM.
type CreateVMRequest struct {
	Name        string            `json:"name"`
	Description string            `json:"description,omitempty"`
	VCPU        int32             `json:"vcpu"`
	MemoryMB    int64             `json:"memory_mb"`
	DiskGB      int               `json:"disk_gb"`
	NetworkIDs  []string          `json:"network_ids,omitempty"`
	ImageID     string            `json:"image_id,omitempty"`
	UserData    string            `json:"user_data,omitempty"`
	MetaData    map[string]string `json:"metadata,omitempty"`
}

// CreateVMResponse is the response from creating a VM.
type CreateVMResponse struct {
	ID            string `json:"id"`
	Name          string `json:"name"`
	DesiredState  string `json:"desired_state"`
	ActualState   string `json:"actual_state"`
	NodeID        string `json:"node_id,omitempty"`
}

// CreateVM creates a new VM.
func (h *Harness) CreateVM(req *CreateVMRequest) (*CreateVMResponse, error) {
	resp, err := h.DoRequest("POST", "/vms", req)
	if err != nil {
		return nil, err
	}
	
	var result CreateVMResponse
	if err := h.ParseResponse(resp, &result); err != nil {
		return nil, err
	}
	
	return &result, nil
}

// GetVM gets a VM by ID.
func (h *Harness) GetVM(id string) (*CreateVMResponse, error) {
	resp, err := h.DoRequest("GET", fmt.Sprintf("/vms/%s", id), nil)
	if err != nil {
		return nil, err
	}
	
	var result CreateVMResponse
	if err := h.ParseResponse(resp, &result); err != nil {
		return nil, err
	}
	
	return &result, nil
}

// StartVM starts a VM.
func (h *Harness) StartVM(id string) error {
	resp, err := h.DoRequest("POST", fmt.Sprintf("/vms/%s/start", id), nil)
	if err != nil {
		return err
	}
	
	return h.ParseResponse(resp, nil)
}

// StopVM stops a VM.
func (h *Harness) StopVM(id string) error {
	resp, err := h.DoRequest("POST", fmt.Sprintf("/vms/%s/stop", id), nil)
	if err != nil {
		return err
	}
	
	return h.ParseResponse(resp, nil)
}

// DeleteVM deletes a VM.
func (h *Harness) DeleteVM(id string) error {
	resp, err := h.DoRequest("DELETE", fmt.Sprintf("/vms/%s", id), nil)
	if err != nil {
		return err
	}
	
	return h.ParseResponse(resp, nil)
}

// WaitForVMState waits for a VM to reach a specific state.
func (h *Harness) WaitForVMState(id string, state string, timeout time.Duration) error {
	ctx, cancel := context.WithTimeout(h.ctx, timeout)
	defer cancel()
	
	ticker := time.NewTicker(1 * time.Second)
	defer ticker.Stop()
	
	for {
		select {
		case <-ctx.Done():
			return fmt.Errorf("timeout waiting for VM %s to reach state %s", id, state)
		case <-ticker.C:
			vm, err := h.GetVM(id)
			if err != nil {
				continue
			}
			if vm.ActualState == state {
				return nil
			}
		}
	}
}

// CreateNetworkRequest is a request to create a network.
type CreateNetworkRequest struct {
	Name   string `json:"name"`
	CIDR   string `json:"cidr"`
	Bridge string `json:"bridge,omitempty"`
}

// CreateNetworkResponse is the response from creating a network.
type CreateNetworkResponse struct {
	ID   string `json:"id"`
	Name string `json:"name"`
	CIDR string `json:"cidr"`
}

// CreateNetwork creates a new network.
func (h *Harness) CreateNetwork(req *CreateNetworkRequest) (*CreateNetworkResponse, error) {
	// Note: Network creation requires authentication
	resp, err := h.DoRequest("POST", "/networks", req)
	if err != nil {
		return nil, err
	}
	
	var result CreateNetworkResponse
	if err := h.ParseResponse(resp, &result); err != nil {
		return nil, err
	}
	
	return &result, nil
}
