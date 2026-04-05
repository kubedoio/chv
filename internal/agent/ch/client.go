// Package ch provides a client for CloudHypervisor's HTTP REST API.
package ch

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"
)

// Client provides access to the CloudHypervisor REST API.
type Client struct {
	baseURL string
	client  *http.Client
}

// NewClient creates a new CloudHypervisor client.
func NewClient(socketPath string) *Client {
	// CH listens on a Unix socket or TCP
	// For TCP: http://localhost:8080
	// For Unix socket: we use a custom transport (see below)
	baseURL := "http://localhost/"
	if socketPath == "" {
		socketPath = "/run/cloud-hypervisor/cloud-hypervisor.sock"
	}

	return &Client{
		baseURL: baseURL,
		client: &http.Client{
			Timeout: 30 * time.Second,
			Transport: newUnixSocketTransport(socketPath),
		},
	}
}

// NewClientTCP creates a client for TCP-connected CH.
func NewClientTCP(host string) *Client {
	if host == "" {
		host = "localhost:8080"
	}
	return &Client{
		baseURL: fmt.Sprintf("http://%s/", host),
		client:  &http.Client{Timeout: 30 * time.Second},
	}
}

// do makes an HTTP request to CH.
func (c *Client) do(ctx context.Context, method, path string, body interface{}) (*http.Response, error) {
	var bodyReader io.Reader
	if body != nil {
		data, err := json.Marshal(body)
		if err != nil {
			return nil, fmt.Errorf("marshal body: %w", err)
		}
		bodyReader = bytes.NewReader(data)
	}

	url := c.baseURL + path
	req, err := http.NewRequestWithContext(ctx, method, url, bodyReader)
	if err != nil {
		return nil, fmt.Errorf("create request: %w", err)
	}

	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	req.Header.Set("Accept", "application/json")

	return c.client.Do(req)
}

// get performs a GET request.
func (c *Client) get(ctx context.Context, path string, result interface{}) error {
	resp, err := c.do(ctx, "GET", path, nil)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("unexpected status: %d", resp.StatusCode)
	}

	if result != nil {
		if err := json.NewDecoder(resp.Body).Decode(result); err != nil {
			return fmt.Errorf("decode response: %w", err)
		}
	}
	return nil
}

// put performs a PUT request.
func (c *Client) put(ctx context.Context, path string, body, result interface{}) error {
	resp, err := c.do(ctx, "PUT", path, body)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusNoContent {
		return fmt.Errorf("unexpected status: %d", resp.StatusCode)
	}

	if result != nil && resp.StatusCode == http.StatusOK {
		if err := json.NewDecoder(resp.Body).Decode(result); err != nil {
			return fmt.Errorf("decode response: %w", err)
		}
	}
	return nil
}
