// Package e2e provides end-to-end tests for CHV.
package e2e

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"testing"
	"time"
	
	"github.com/jackc/pgx/v5"
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

// VMNetworkRequest represents a network attachment for VM creation.
type VMNetworkRequest struct {
	NetworkID string `json:"network_id"`
}

// CreateVMRequest is a request to create a VM.
type CreateVMRequest struct {
	Name        string            `json:"name"`
	Description string            `json:"description,omitempty"`
	VCPU        int32             `json:"vcpu"`
	MemoryMB    int64             `json:"memory_mb"`
	DiskGB      int               `json:"disk_gb,omitempty"`  // Alternative to DiskSizeBytes (will be converted)
	DiskSizeBytes int64           `json:"disk_size_bytes,omitempty"`
	NetworkIDs  []string          `json:"network_ids,omitempty"` // Will be converted to Networks
	Networks    []VMNetworkRequest `json:"networks,omitempty"`
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
// This helper converts from the simplified E2E request format to the API format.
func (h *Harness) CreateVM(req *CreateVMRequest) (*CreateVMResponse, error) {
	// Convert to API format
	apiReq := struct {
		Name          string             `json:"name"`
		Description   string             `json:"description,omitempty"`
		CPU           int32              `json:"vcpu"`
		MemoryMB      int64              `json:"memory_mb"`
		DiskSizeBytes int64              `json:"disk_size_bytes,omitempty"`
		ImageID       string             `json:"image_id,omitempty"`
		Networks      []VMNetworkRequest `json:"networks,omitempty"`
		UserData      string             `json:"user_data,omitempty"`
	}{
		Name:        req.Name,
		Description: req.Description,
		CPU:         req.VCPU,
		MemoryMB:    req.MemoryMB,
		ImageID:     req.ImageID,
		UserData:    req.UserData,
	}
	
	// Convert disk size (prefer DiskSizeBytes, fallback to DiskGB)
	if req.DiskSizeBytes > 0 {
		apiReq.DiskSizeBytes = req.DiskSizeBytes
	} else if req.DiskGB > 0 {
		apiReq.DiskSizeBytes = int64(req.DiskGB) * 1024 * 1024 * 1024
	}
	
	// Convert network IDs to network objects
	if len(req.NetworkIDs) > 0 {
		for _, netID := range req.NetworkIDs {
			apiReq.Networks = append(apiReq.Networks, VMNetworkRequest{NetworkID: netID})
		}
	} else if len(req.Networks) > 0 {
		apiReq.Networks = req.Networks
	}
	
	resp, err := h.DoRequest("POST", "/vms", apiReq)
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
	Name       string `json:"name"`
	BridgeName string `json:"bridge_name"`
	CIDR       string `json:"cidr"`
	GatewayIP  string `json:"gateway_ip"`
}

// CreateNetworkResponse is the response from creating a network.
type CreateNetworkResponse struct {
	ID        string `json:"id"`
	Name      string `json:"name"`
	CIDR      string `json:"cidr"`
	GatewayIP string `json:"gateway_ip"`
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

// ImageImportRequest represents a request to import an image.
type ImageImportRequest struct {
	Name               string `json:"name"`
	OSFamily           string `json:"os_family"`
	SourceURL          string `json:"source_url"`
	SourceFormat       string `json:"source_format,omitempty"`
	Architecture       string `json:"architecture,omitempty"`
	CloudInitSupported bool   `json:"cloud_init_supported"`
	DefaultUsername    string `json:"default_username,omitempty"`
}

// ImageImportResponse represents the response from importing an image.
type ImageImportResponse struct {
	ID     string `json:"id"`
	Name   string `json:"name"`
	Status string `json:"status"`
}

// ImageServer manages a local HTTP server for serving test images.
type ImageServer struct {
	server   *http.Server
	listener net.Listener
	baseURL  string
}

// StartImageServer starts a local HTTP server to serve test images.
// It returns the server and the base URL to access images.
func StartImageServer() (*ImageServer, error) {
	// Find the testdata directory
	testdataPath := filepath.Join("testdata")
	if _, err := os.Stat(testdataPath); os.IsNotExist(err) {
		// Try from the e2e directory
		testdataPath = filepath.Join("e2e", "testdata")
	}
	
	// Create a file server
	fs := http.FileServer(http.Dir(testdataPath))
	
	// Find an available port
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return nil, fmt.Errorf("failed to find available port: %w", err)
	}
	
	server := &http.Server{
		Handler: fs,
	}
	
	// Start server in background
	go func() {
		_ = server.Serve(listener)
	}()
	
	baseURL := fmt.Sprintf("http://%s", listener.Addr().String())
	
	// Give server a moment to start
	time.Sleep(100 * time.Millisecond)
	
	return &ImageServer{
		server:   server,
		listener: listener,
		baseURL:  baseURL,
	}, nil
}

// Stop stops the image server.
func (s *ImageServer) Stop() error {
	if s.server != nil {
		return s.server.Close()
	}
	return nil
}

// BaseURL returns the base URL to access images.
func (s *ImageServer) BaseURL() string {
	return s.baseURL
}

// GetImageResponse represents an image in the system.
type GetImageResponse struct {
	ID     string `json:"id"`
	Name   string `json:"name"`
	Status string `json:"status"`
}

// GetImage gets an image by ID.
func (h *Harness) GetImage(id string) (*GetImageResponse, error) {
	resp, err := h.DoRequest("GET", fmt.Sprintf("/images/%s", id), nil)
	if err != nil {
		return nil, err
	}
	
	var result GetImageResponse
	if err := h.ParseResponse(resp, &result); err != nil {
		return nil, err
	}
	
	return &result, nil
}

// WaitForImageReady waits for an image to reach 'ready' status.
func (h *Harness) WaitForImageReady(id string, timeout time.Duration) error {
	ctx, cancel := context.WithTimeout(h.ctx, timeout)
	defer cancel()
	
	ticker := time.NewTicker(500 * time.Millisecond)
	defer ticker.Stop()
	
	for {
		select {
		case <-ctx.Done():
			return fmt.Errorf("timeout waiting for image %s to be ready", id)
		case <-ticker.C:
			image, err := h.GetImage(id)
			if err != nil {
				continue
			}
			if image.Status == "ready" {
				return nil
			}
		}
	}
}

// dbConnString returns the database connection string for direct DB access.
func (h *Harness) dbConnString() string {
	host := os.Getenv("CHV_DB_HOST")
	if host == "" {
		host = "localhost"
	}
	port := os.Getenv("CHV_DB_PORT")
	if port == "" {
		port = "5433" // Default E2E port
	}
	user := os.Getenv("CHV_DB_USER")
	if user == "" {
		user = "chv"
	}
	pass := os.Getenv("CHV_DB_PASSWORD")
	if pass == "" {
		pass = "chv"
	}
	dbname := os.Getenv("CHV_DB_NAME")
	if dbname == "" {
		dbname = "chv"
	}
	
	return fmt.Sprintf("postgres://%s:%s@%s:%s/%s", user, pass, host, port, dbname)
}

// MarkImageReady directly updates the database to mark an image as ready.
// This is a test helper that bypasses the async import process.
func (h *Harness) MarkImageReady(imageID string) error {
	conn, err := pgx.Connect(h.ctx, h.dbConnString())
	if err != nil {
		return fmt.Errorf("failed to connect to database: %w", err)
	}
	defer conn.Close(h.ctx)
	
	_, err = conn.Exec(h.ctx, 
		"UPDATE images SET status = 'ready' WHERE id = $1",
		imageID)
	if err != nil {
		return fmt.Errorf("failed to update image status: %w", err)
	}
	
	return nil
}

// ImportCirrosImage imports the Cirros test image and marks it as ready.
// This helper starts an HTTP server to serve the image, imports it via the API,
// then directly updates the database to mark it ready (bypassing async import for E2E tests).
// Uses a unique name based on timestamp to avoid conflicts between tests.
func (h *Harness) ImportCirrosImage() (string, error) {
	// Start local image server
	imgServer, err := StartImageServer()
	if err != nil {
		return "", fmt.Errorf("failed to start image server: %w", err)
	}
	defer imgServer.Stop()
	
	// Import the image with a unique name to avoid conflicts
	req := &ImageImportRequest{
		Name:               fmt.Sprintf("cirros-0.5.2-%d", time.Now().Unix()),
		OSFamily:           "linux",
		SourceURL:          imgServer.BaseURL() + "/cirros-0.5.2-x86_64-disk.img",
		SourceFormat:       "qcow2",
		Architecture:       "x86_64",
		CloudInitSupported: true,
		DefaultUsername:    "cirros",
	}
	
	resp, err := h.DoRequest("POST", "/images/import", req)
	if err != nil {
		return "", fmt.Errorf("failed to import image: %w", err)
	}
	
	var result ImageImportResponse
	if err := h.ParseResponse(resp, &result); err != nil {
		return "", fmt.Errorf("failed to parse import response: %w", err)
	}
	
	// Mark image as ready directly in database (bypass async import for E2E tests)
	if err := h.MarkImageReady(result.ID); err != nil {
		return "", fmt.Errorf("failed to mark image ready: %w", err)
	}
	
	return result.ID, nil
}
