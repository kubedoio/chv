// Package hypervisor provides VM lifecycle management for Cloud Hypervisor.
package hypervisor

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"time"
)

// CHVClient provides an HTTP client for Cloud Hypervisor's API.
// It communicates via Unix domain socket.
type CHVClient struct {
	socketPath string
	httpClient *http.Client
	baseURL    string // Always "http://localhost" - the socket is the actual transport
}

// NewCHVClient creates a new Cloud Hypervisor API client.
func NewCHVClient(socketPath string) *CHVClient {
	return &CHVClient{
		socketPath: socketPath,
		baseURL:    "http://localhost",
		httpClient: &http.Client{
			Timeout: 30 * time.Second,
			Transport: &http.Transport{
				DialContext: func(ctx context.Context, network, addr string) (net.Conn, error) {
					var d net.Dialer
					return d.DialContext(ctx, "unix", socketPath)
				},
			},
		},
	}
}

// VMInfo represents the response from /api/v1/vm.info
type VMInfo struct {
	Config struct {
		Cpus struct {
			BootVcpus   int `json:"boot_vcpus"`
			MaxVcpus    int `json:"max_vcpus"`
		} `json:"cpus"`
		Memory struct {
			Size            int  `json:"size"`
			Mergeable       bool `json:"mergeable"`
			HotplugMethod   string `json:"hotplug_method"`
			HotplugSize     int  `json:"hotplug_size,omitempty"`
			HotpluggedSize  int  `json:"hotplugged_size,omitempty"`
			Shared          bool `json:"shared"`
			Hugepages       bool `json:"hugepages"`
		} `json:"memory"`
	} `json:"config"`
	State string `json:"state"`
	MemoryActualSize int `json:"memory_actual_size,omitempty"`
}

// VMCounters represents performance counters from /api/v1/vm.counters
type VMCounters struct {
	VcpuCalibrate struct {
		Errors   int `json:"errors"`
		Executed int `json:"executed"`
		Missed   int `json:"missed"`
	} `json:"vcpu_calibrate,omitempty"`
}

// ShutdownMode defines the shutdown behavior.
type ShutdownMode string

const (
	ShutdownModeReboot ShutdownMode = "Reboot"
	ShutdownModeHalt   ShutdownMode = "Halt"
	ShutdownModePowerOff ShutdownMode = "PowerOff"
)

// Ping checks if the VM is responding to API calls.
func (c *CHVClient) Ping(ctx context.Context) error {
	_, err := c.GetVMInfo(ctx)
	return err
}

// GetVMInfo retrieves information about the VM.
func (c *CHVClient) GetVMInfo(ctx context.Context) (*VMInfo, error) {
	resp, err := c.get(ctx, "/api/v1/vm.info")
	if err != nil {
		return nil, fmt.Errorf("failed to get VM info: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("unexpected status %d: %s", resp.StatusCode, string(body))
	}

	var info VMInfo
	if err := json.NewDecoder(resp.Body).Decode(&info); err != nil {
		return nil, fmt.Errorf("failed to decode response: %w", err)
	}

	return &info, nil
}

// Shutdown initiates a graceful shutdown of the VM.
func (c *CHVClient) Shutdown(ctx context.Context) error {
	return c.shutdown(ctx, ShutdownModePowerOff)
}

// Reboot initiates a graceful reboot of the VM.
func (c *CHVClient) Reboot(ctx context.Context) error {
	return c.shutdown(ctx, ShutdownModeReboot)
}

func (c *CHVClient) shutdown(ctx context.Context, mode ShutdownMode) error {
	body := map[string]string{"mode": string(mode)}
	resp, err := c.put(ctx, "/api/v1/vm.shutdown", body)
	if err != nil {
		return fmt.Errorf("failed to send shutdown request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent && resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("shutdown failed with status %d: %s", resp.StatusCode, string(body))
	}

	return nil
}

// Pause pauses the VM.
func (c *CHVClient) Pause(ctx context.Context) error {
	resp, err := c.put(ctx, "/api/v1/vm.pause", nil)
	if err != nil {
		return fmt.Errorf("failed to pause VM: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent && resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("pause failed with status %d: %s", resp.StatusCode, string(body))
	}

	return nil
}

// Resume resumes a paused VM.
func (c *CHVClient) Resume(ctx context.Context) error {
	resp, err := c.put(ctx, "/api/v1/vm.resume", nil)
	if err != nil {
		return fmt.Errorf("failed to resume VM: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent && resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("resume failed with status %d: %s", resp.StatusCode, string(body))
	}

	return nil
}

// GetVMCounters retrieves VM performance counters.
func (c *CHVClient) GetVMCounters(ctx context.Context) (*VMCounters, error) {
	resp, err := c.get(ctx, "/api/v1/vm.counters")
	if err != nil {
		return nil, fmt.Errorf("failed to get VM counters: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("unexpected status %d: %s", resp.StatusCode, string(body))
	}

	var counters VMCounters
	if err := json.NewDecoder(resp.Body).Decode(&counters); err != nil {
		return nil, fmt.Errorf("failed to decode response: %w", err)
	}

	return &counters, nil
}

// IsRunning checks if the VM is in the "Running" state.
func (c *CHVClient) IsRunning(ctx context.Context) (bool, error) {
	info, err := c.GetVMInfo(ctx)
	if err != nil {
		return false, err
	}
	return info.State == "Running", nil
}

// WaitForRunning waits until the VM reaches the Running state or timeout.
func (c *CHVClient) WaitForRunning(ctx context.Context, timeout time.Duration) error {
	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	ticker := time.NewTicker(100 * time.Millisecond)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return fmt.Errorf("timeout waiting for VM to start")
		case <-ticker.C:
			running, err := c.IsRunning(ctx)
			if err != nil {
				// API might not be ready yet, keep trying
				continue
			}
			if running {
				return nil
			}
		}
	}
}

// WaitForStopped waits until the VM is no longer running or timeout.
func (c *CHVClient) WaitForStopped(ctx context.Context, timeout time.Duration) error {
	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	ticker := time.NewTicker(100 * time.Millisecond)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return fmt.Errorf("timeout waiting for VM to stop")
		case <-ticker.C:
			running, err := c.IsRunning(ctx)
			if err != nil {
				// API socket might be gone, VM is stopped
				return nil
			}
			if !running {
				return nil
			}
		}
	}
}

// Helper methods for HTTP requests

func (c *CHVClient) get(ctx context.Context, path string) (*http.Response, error) {
	url := c.baseURL + path
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	return c.httpClient.Do(req)
}

func (c *CHVClient) put(ctx context.Context, path string, body interface{}) (*http.Response, error) {
	url := c.baseURL + path
	
	var bodyReader io.Reader
	if body != nil {
		data, err := json.Marshal(body)
		if err != nil {
			return nil, err
		}
		bodyReader = bytes.NewReader(data)
	}
	
	req, err := http.NewRequestWithContext(ctx, http.MethodPut, url, bodyReader)
	if err != nil {
		return nil, err
	}
	
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	
	return c.httpClient.Do(req)
}
