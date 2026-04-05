package ch

import (
	"context"
	"fmt"
	"net/http"
)

// VMMInfo represents VMM (hypervisor) information.
type VMMInfo struct {
	Version string `json:"version"`
}

// VmmPingResponse represents the ping response.
type VmmPingResponse struct {
	Version string `json:"version"`
	State   string `json:"state"` // "Running", "Shutdown", etc.
}

// Ping checks if the VMM is responsive.
func (c *Client) Ping(ctx context.Context) (*VmmPingResponse, error) {
	var resp VmmPingResponse
	if err := c.get(ctx, "vmm.ping", &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// Shutdown requests the VMM to shut down.
func (c *Client) ShutdownVMM(ctx context.Context) error {
	resp, err := c.do(ctx, "PUT", "vmm.shutdown", nil)
	if err != nil {
		return fmt.Errorf("shutdown vmm: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent {
		return fmt.Errorf("shutdown vmm failed: %d", resp.StatusCode)
	}
	return nil
}
