package metadata

import (
	"fmt"
	"io"
	"net/http"
	"strings"
	"testing"
	"time"
)

func TestServerStartStop(t *testing.T) {
	server := NewServerWithAddress(":0")

	// Start the server
	err := server.Start()
	if err != nil {
		t.Fatalf("Failed to start server: %v", err)
	}

	// Verify server is listening
	addr := server.Addr()
	if addr == "" {
		t.Fatal("Server address is empty")
	}

	// Stop the server
	err = server.Stop()
	if err != nil {
		t.Fatalf("Failed to stop server: %v", err)
	}
}

func TestRegisterVM(t *testing.T) {
	server := NewServerWithAddress(":0")

	err := server.Start()
	if err != nil {
		t.Fatalf("Failed to start server: %v", err)
	}
	defer server.Stop()

	// Register a VM
	config := &Config{
		InstanceID:    "vm-123",
		Hostname:      "test-vm",
		NetworkConfig: `{"version": 2, "ethernets": {"eth0": {"dhcp4": true}}}`,
		UserData:      "#cloud-config\nusers:\n  - name: admin\n",
		MetaData:      "instance-id: vm-123\nlocal-hostname: test-vm\n",
	}

	server.RegisterVM("vm-123", config)

	// Verify the VM is registered
	retrieved, ok := server.GetConfig("vm-123")
	if !ok {
		t.Fatal("VM was not registered")
	}

	if retrieved.InstanceID != "vm-123" {
		t.Errorf("Expected InstanceID 'vm-123', got '%s'", retrieved.InstanceID)
	}

	if retrieved.Hostname != "test-vm" {
		t.Errorf("Expected Hostname 'test-vm', got '%s'", retrieved.Hostname)
	}
}

func TestUnregisterVM(t *testing.T) {
	server := NewServerWithAddress(":0")

	err := server.Start()
	if err != nil {
		t.Fatalf("Failed to start server: %v", err)
	}
	defer server.Stop()

	// Register then unregister
	config := &Config{
		InstanceID: "vm-123",
		Hostname:   "test-vm",
	}

	server.RegisterVM("vm-123", config)
	server.UnregisterVM("vm-123")

	// Verify the VM is unregistered
	_, ok := server.GetConfig("vm-123")
	if ok {
		t.Fatal("VM should have been unregistered")
	}
}

func TestHandleUserData(t *testing.T) {
	server := NewServerWithAddress(":0")

	err := server.Start()
	if err != nil {
		t.Fatalf("Failed to start server: %v", err)
	}
	defer server.Stop()

	// Wait for server to be ready
	time.Sleep(100 * time.Millisecond)

	// Register a VM
	config := &Config{
		InstanceID: "vm-123",
		Hostname:   "test-vm",
		UserData:   "#cloud-config\nusers:\n  - name: admin\n",
	}
	server.RegisterVM("vm-123", config)

	// Make request with VM ID header
	addr := server.Addr()
	url := fmt.Sprintf("http://%s/latest/user-data", addr)
	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		t.Fatalf("Failed to create request: %v", err)
	}
	req.Header.Set("X-VM-ID", "vm-123")

	client := &http.Client{Timeout: 5 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		t.Fatalf("Failed to make request: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		t.Errorf("Expected status 200, got %d", resp.StatusCode)
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		t.Fatalf("Failed to read body: %v", err)
	}

	if string(body) != config.UserData {
		t.Errorf("Expected user-data '%s', got '%s'", config.UserData, string(body))
	}
}

func TestHandleNetworkConfig(t *testing.T) {
	server := NewServerWithAddress(":0")

	err := server.Start()
	if err != nil {
		t.Fatalf("Failed to start server: %v", err)
	}
	defer server.Stop()

	// Wait for server to be ready
	time.Sleep(100 * time.Millisecond)

	// Register a VM with network config
	config := &Config{
		InstanceID:    "vm-123",
		Hostname:      "test-vm",
		NetworkConfig: `{"version": 2, "ethernets": {"eth0": {"dhcp4": true}}}`,
	}
	server.RegisterVM("vm-123", config)

	// Make request with VM ID header
	addr := server.Addr()
	url := fmt.Sprintf("http://%s/latest/network-config", addr)
	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		t.Fatalf("Failed to create request: %v", err)
	}
	req.Header.Set("X-VM-ID", "vm-123")

	client := &http.Client{Timeout: 5 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		t.Fatalf("Failed to make request: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		t.Errorf("Expected status 200, got %d", resp.StatusCode)
	}

	contentType := resp.Header.Get("Content-Type")
	if contentType != "application/json" {
		t.Errorf("Expected Content-Type 'application/json', got '%s'", contentType)
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		t.Fatalf("Failed to read body: %v", err)
	}

	if string(body) != config.NetworkConfig {
		t.Errorf("Expected network-config '%s', got '%s'", config.NetworkConfig, string(body))
	}
}

func TestHandleMetaData(t *testing.T) {
	server := NewServerWithAddress(":0")

	err := server.Start()
	if err != nil {
		t.Fatalf("Failed to start server: %v", err)
	}
	defer server.Stop()

	// Wait for server to be ready
	time.Sleep(100 * time.Millisecond)

	// Register a VM
	config := &Config{
		InstanceID: "vm-123",
		Hostname:   "test-vm",
	}
	server.RegisterVM("vm-123", config)

	addr := server.Addr()
	client := &http.Client{Timeout: 5 * time.Second}

	// Test listing metadata
	url := fmt.Sprintf("http://%s/latest/meta-data/", addr)
	req, _ := http.NewRequest("GET", url, nil)
	req.Header.Set("X-VM-ID", "vm-123")
	resp, err := client.Do(req)
	if err != nil {
		t.Fatalf("Failed to make request: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		t.Errorf("Expected status 200, got %d", resp.StatusCode)
	}

	body, _ := io.ReadAll(resp.Body)
	bodyStr := string(body)
	if !strings.Contains(bodyStr, "instance-id") || !strings.Contains(bodyStr, "local-hostname") {
		t.Errorf("Expected metadata list to contain 'instance-id' and 'local-hostname', got: %s", bodyStr)
	}

	// Test instance-id
	url = fmt.Sprintf("http://%s/latest/meta-data/instance-id", addr)
	req, _ = http.NewRequest("GET", url, nil)
	req.Header.Set("X-VM-ID", "vm-123")
	resp, err = client.Do(req)
	if err != nil {
		t.Fatalf("Failed to make request: %v", err)
	}
	defer resp.Body.Close()

	body, _ = io.ReadAll(resp.Body)
	if string(body) != "vm-123" {
		t.Errorf("Expected instance-id 'vm-123', got '%s'", string(body))
	}

	// Test hostname
	url = fmt.Sprintf("http://%s/latest/meta-data/hostname", addr)
	req, _ = http.NewRequest("GET", url, nil)
	req.Header.Set("X-VM-ID", "vm-123")
	resp, err = client.Do(req)
	if err != nil {
		t.Fatalf("Failed to make request: %v", err)
	}
	defer resp.Body.Close()

	body, _ = io.ReadAll(resp.Body)
	if string(body) != "test-vm" {
		t.Errorf("Expected hostname 'test-vm', got '%s'", string(body))
	}
}

func TestHandleVMNotFound(t *testing.T) {
	server := NewServerWithAddress(":0")

	err := server.Start()
	if err != nil {
		t.Fatalf("Failed to start server: %v", err)
	}
	defer server.Stop()

	// Wait for server to be ready
	time.Sleep(100 * time.Millisecond)

	// Make request for non-existent VM
	addr := server.Addr()
	url := fmt.Sprintf("http://%s/latest/user-data", addr)
	req, _ := http.NewRequest("GET", url, nil)
	req.Header.Set("X-VM-ID", "non-existent")

	client := &http.Client{Timeout: 5 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		t.Fatalf("Failed to make request: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNotFound {
		t.Errorf("Expected status 404, got %d", resp.StatusCode)
	}
}

func TestDefaultConfigValues(t *testing.T) {
	server := NewServerWithAddress(":0")

	err := server.Start()
	if err != nil {
		t.Fatalf("Failed to start server: %v", err)
	}
	defer server.Stop()

	// Register VM with minimal config
	config := &Config{}
	server.RegisterVM("vm-123", config)

	// Check defaults were set
	retrieved, ok := server.GetConfig("vm-123")
	if !ok {
		t.Fatal("VM was not registered")
	}

	if retrieved.InstanceID != "vm-123" {
		t.Errorf("Expected InstanceID to default to 'vm-123', got '%s'", retrieved.InstanceID)
	}

	if retrieved.Hostname != "vm-123" {
		t.Errorf("Expected Hostname to default to 'vm-123', got '%s'", retrieved.Hostname)
	}

	if retrieved.MetaData == "" {
		t.Error("Expected MetaData to be auto-generated")
	}
}

func TestEmptyNetworkConfig(t *testing.T) {
	server := NewServerWithAddress(":0")

	err := server.Start()
	if err != nil {
		t.Fatalf("Failed to start server: %v", err)
	}
	defer server.Stop()

	// Wait for server to be ready
	time.Sleep(100 * time.Millisecond)

	// Register a VM without network config
	config := &Config{
		InstanceID: "vm-123",
		Hostname:   "test-vm",
	}
	server.RegisterVM("vm-123", config)

	// Make request
	addr := server.Addr()
	url := fmt.Sprintf("http://%s/latest/network-config", addr)
	req, _ := http.NewRequest("GET", url, nil)
	req.Header.Set("X-VM-ID", "vm-123")

	client := &http.Client{Timeout: 5 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		t.Fatalf("Failed to make request: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		t.Errorf("Expected status 200, got %d", resp.StatusCode)
	}

	body, _ := io.ReadAll(resp.Body)
	expected := `{"version": 2}`
	if string(body) != expected {
		t.Errorf("Expected default network config '%s', got '%s'", expected, string(body))
	}
}

func TestEmptyUserData(t *testing.T) {
	server := NewServerWithAddress(":0")

	err := server.Start()
	if err != nil {
		t.Fatalf("Failed to start server: %v", err)
	}
	defer server.Stop()

	// Wait for server to be ready
	time.Sleep(100 * time.Millisecond)

	// Register a VM without user data
	config := &Config{
		InstanceID: "vm-123",
		Hostname:   "test-vm",
	}
	server.RegisterVM("vm-123", config)

	// Make request
	addr := server.Addr()
	url := fmt.Sprintf("http://%s/latest/user-data", addr)
	req, _ := http.NewRequest("GET", url, nil)
	req.Header.Set("X-VM-ID", "vm-123")

	client := &http.Client{Timeout: 5 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		t.Fatalf("Failed to make request: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		t.Errorf("Expected status 200, got %d", resp.StatusCode)
	}

	body, _ := io.ReadAll(resp.Body)
	expected := "#cloud-config\n{}\n"
	if string(body) != expected {
		t.Errorf("Expected default user-data '%s', got '%s'", expected, string(body))
	}
}

func TestRegisterVMWithIP(t *testing.T) {
	server := NewServerWithAddress(":0")

	err := server.Start()
	if err != nil {
		t.Fatalf("Failed to start server: %v", err)
	}
	defer server.Stop()

	// Register VM with IP
	config := &Config{
		InstanceID: "vm-123",
		Hostname:   "test-vm",
	}
	server.RegisterVMWithIP("vm-123", "10.0.0.5", config)

	// Verify the VM is registered
	retrieved, ok := server.GetConfig("vm-123")
	if !ok {
		t.Fatal("VM was not registered")
	}

	if retrieved.InstanceID != "vm-123" {
		t.Errorf("Expected InstanceID 'vm-123', got '%s'", retrieved.InstanceID)
	}
}

func TestUpdateVMIP(t *testing.T) {
	server := NewServerWithAddress(":0")

	err := server.Start()
	if err != nil {
		t.Fatalf("Failed to start server: %v", err)
	}
	defer server.Stop()

	// Register VM with initial IP
	config := &Config{
		InstanceID: "vm-123",
		Hostname:   "test-vm",
	}
	server.RegisterVMWithIP("vm-123", "10.0.0.5", config)

	// Update IP
	server.UpdateVMIP("vm-123", "10.0.0.6")

	// Verify VM is still registered
	retrieved, ok := server.GetConfig("vm-123")
	if !ok {
		t.Fatal("VM should still be registered after IP update")
	}

	if retrieved.InstanceID != "vm-123" {
		t.Errorf("Expected InstanceID 'vm-123', got '%s'", retrieved.InstanceID)
	}
}
