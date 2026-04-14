package services

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"time"
)

// CHAPIClient provides a client for Cloud Hypervisor's HTTP API
type CHAPIClient struct {
	httpClient *http.Client
	apiSocket  string
}

// NewCHAPIClient creates a new Cloud Hypervisor API client
func NewCHAPIClient(apiSocket string) *CHAPIClient {
	return &CHAPIClient{
		apiSocket: apiSocket,
		httpClient: &http.Client{
			Timeout: 30 * time.Second,
			Transport: &http.Transport{
				DialContext: func(ctx context.Context, network, addr string) (net.Conn, error) {
					return net.Dial("unix", apiSocket)
				},
			},
		},
	}
}

// VMInfo represents the response from /api/v1/vm.info
type VMInfo struct {
	Config struct {
		Cpus struct {
			BootVcpus  int `json:"boot_vcpus"`
			MaxVcpus   int `json:"max_vcpus"`
			Topology struct {
				ThreadsPerCore int `json:"threads_per_core"`
				CoresPerDie    int `json:"cores_per_die"`
				DiesPerPackage int `json:"dies_per_package"`
				Packages       int `json:"packages"`
			} `json:"topology"`
		} `json:"cpus"`
		Memory struct {
			Size           int  `json:"size"`
			Mergeable      bool `json:"mergeable"`
			HotplugMethod  string `json:"hotplug_method"`
			HotplugSize    int  `json:"hotplug_size,omitempty"`
			HotpluggedSize int  `json:"hotplugged_size,omitempty"`
			Shared         bool `json:"shared"`
			Hugepages      bool `json:"hugepages"`
		} `json:"memory"` 
	} `json:"config"`
	State string `json:"state"`
}

// VMState represents VM state constants
const (
	VMStateRunning = "Running"
	VMStatePaused  = "Paused"
	VMStateShutDown = "ShutDown"
)

// ResizeRequest represents a VM resize request
type ResizeRequest struct {
	DesiredVcpus   int `json:"desired_vcpus,omitempty"`
	DesiredRam     int `json:"desired_ram,omitempty"`
	DesiredBalloon int `json:"desired_balloon,omitempty"`
}

// ResizeResponse represents a VM resize response
type ResizeResponse struct {
	Vcpus   int `json:"vcpus"`
	Ram     int `json:"ram"`
	Balloon int `json:"balloon,omitempty"`
}

// Shutdown sends a graceful shutdown request to the VM via ACPI
func (c *CHAPIClient) Shutdown(ctx context.Context) error {
	req, err := http.NewRequestWithContext(ctx, "PUT", "http://localhost/api/v1/vm.shutdown", nil)
	if err != nil {
		return fmt.Errorf("failed to create shutdown request: %w", err)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("failed to send shutdown request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusAccepted {
		return fmt.Errorf("shutdown request failed with status: %d", resp.StatusCode)
	}

	return nil
}

// Reboot sends a reboot request to the VM
func (c *CHAPIClient) Reboot(ctx context.Context) error {
	req, err := http.NewRequestWithContext(ctx, "PUT", "http://localhost/api/v1/vm.reboot", nil)
	if err != nil {
		return fmt.Errorf("failed to create reboot request: %w", err)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("failed to send reboot request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusAccepted {
		return fmt.Errorf("reboot request failed with status: %d", resp.StatusCode)
	}

	return nil
}

// Pause pauses the VM
func (c *CHAPIClient) Pause(ctx context.Context) error {
	req, err := http.NewRequestWithContext(ctx, "PUT", "http://localhost/api/v1/vm.pause", nil)
	if err != nil {
		return fmt.Errorf("failed to create pause request: %w", err)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("failed to send pause request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusAccepted {
		return fmt.Errorf("pause request failed with status: %d", resp.StatusCode)
	}

	return nil
}

// Resume resumes a paused VM
func (c *CHAPIClient) Resume(ctx context.Context) error {
	req, err := http.NewRequestWithContext(ctx, "PUT", "http://localhost/api/v1/vm.resume", nil)
	if err != nil {
		return fmt.Errorf("failed to create resume request: %w", err)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("failed to send resume request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusAccepted {
		return fmt.Errorf("resume request failed with status: %d", resp.StatusCode)
	}

	return nil
}

// PowerButton simulates a power button press (ACPI shutdown)
func (c *CHAPIClient) PowerButton(ctx context.Context) error {
	req, err := http.NewRequestWithContext(ctx, "PUT", "http://localhost/api/v1/vm.power-button", nil)
	if err != nil {
		return fmt.Errorf("failed to create power-button request: %w", err)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("failed to send power-button request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusAccepted {
		return fmt.Errorf("power-button request failed with status: %d", resp.StatusCode)
	}

	return nil
}

// GetInfo retrieves VM information including state
func (c *CHAPIClient) GetInfo(ctx context.Context) (*VMInfo, error) {
	req, err := http.NewRequestWithContext(ctx, "GET", "http://localhost/api/v1/vm.info", nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create info request: %w", err)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to send info request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("info request failed with status: %d", resp.StatusCode)
	}

	var info VMInfo
	if err := json.NewDecoder(resp.Body).Decode(&info); err != nil {
		return nil, fmt.Errorf("failed to decode info response: %w", err)
	}

	return &info, nil
}

// Resize resizes the VM (CPU and/or memory hot-plug)
func (c *CHAPIClient) Resize(ctx context.Context, desiredVcpus, desiredRam int) (*ResizeResponse, error) {
	resizeReq := ResizeRequest{
		DesiredVcpus: desiredVcpus,
		DesiredRam:   desiredRam,
	}

	body, err := json.Marshal(resizeReq)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal resize request: %w", err)
	}

	req, err := http.NewRequestWithContext(ctx, "PUT", "http://localhost/api/v1/vm.resize", bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("failed to create resize request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to send resize request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusAccepted {
		return nil, fmt.Errorf("resize request failed with status: %d", resp.StatusCode)
	}

	var resizeResp ResizeResponse
	if err := json.NewDecoder(resp.Body).Decode(&resizeResp); err != nil {
		return nil, fmt.Errorf("failed to decode resize response: %w", err)
	}

	return &resizeResp, nil
}

// IsAPISocketAvailable checks if the API socket is connectable
func (c *CHAPIClient) IsAPISocketAvailable() bool {
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()
	
	conn, err := net.Dial("unix", c.apiSocket)
	if err != nil {
		return false
	}
	conn.Close()
	
	// Try a simple API call
	_, err = c.GetInfo(ctx)
	return err == nil
}
