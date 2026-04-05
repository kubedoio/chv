// Package agent provides a gRPC client for controller-to-agent communication.
package agent

import (
	"context"
	"fmt"
	"sync"
	"time"

	"github.com/chv/chv/internal/pb/agent"
	"google.golang.org/grpc"
	"google.golang.org/grpc/backoff"
	"google.golang.org/grpc/connectivity"
	"google.golang.org/grpc/credentials/insecure"
)

// Client provides an interface for communicating with agents.
type Client interface {
	Ping(ctx context.Context, nodeID string) error
	ProvisionVM(ctx context.Context, nodeID string, req *agent.ProvisionVMRequest) error
	StartVM(ctx context.Context, nodeID string, vmID string) error
	StopVM(ctx context.Context, nodeID string, vmID string) error
	RebootVM(ctx context.Context, nodeID string, vmID string) error
	DeleteVM(ctx context.Context, nodeID string, vmID string) error
	GetVMState(ctx context.Context, nodeID string, vmID string) (*agent.VMStateResponse, error)
	CreateVolume(ctx context.Context, nodeID string, req *agent.VolumeCreateRequest) error
	ResizeVolume(ctx context.Context, nodeID string, req *agent.VolumeResizeRequest) error
	EnsureBridge(ctx context.Context, nodeID string, req *agent.EnsureBridgeRequest) error
	ImportImage(ctx context.Context, nodeID string, req *agent.ImageImportRequest) error
	Close() error
}

// client implements the Client interface.
type client struct {
	mu          sync.RWMutex
	connections map[string]*grpc.ClientConn
	timeout     time.Duration
}

// NewClient creates a new agent client.
func NewClient() Client {
	return &client{
		connections: make(map[string]*grpc.ClientConn),
		timeout:     30 * time.Second,
	}
}

// SetTimeout sets the default timeout for operations.
func (c *client) SetTimeout(timeout time.Duration) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.timeout = timeout
}

// getConnection gets or creates a connection to an agent.
func (c *client) getConnection(nodeID string) (*grpc.ClientConn, error) {
	c.mu.RLock()
	conn, exists := c.connections[nodeID]
	c.mu.RUnlock()

	if exists && conn.GetState() != connectivity.Shutdown {
		return conn, nil
	}

	c.mu.Lock()
	defer c.mu.Unlock()

	// Double-check after acquiring write lock
	if conn, exists := c.connections[nodeID]; exists && conn.GetState() != connectivity.Shutdown {
		return conn, nil
	}

	// Close old connection if exists
	if exists {
		conn.Close()
	}

	// Create new connection
	// In production, would look up node address from database
	// For now, assume nodeID is the address (host:port)
	conn, err := c.dial(nodeID)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to agent %s: %w", nodeID, err)
	}

	c.connections[nodeID] = conn
	return conn, nil
}

// dial creates a new gRPC connection with retry configuration.
func (c *client) dial(address string) (*grpc.ClientConn, error) {
	backoffConfig := backoff.DefaultConfig
	backoffConfig.MaxDelay = 5 * time.Second

	conn, err := grpc.Dial(
		address,
		grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithConnectParams(grpc.ConnectParams{
			Backoff: backoffConfig,
		}),
	)
	if err != nil {
		return nil, err
	}

	return conn, nil
}

// Ping checks if an agent is reachable.
func (c *client) Ping(ctx context.Context, nodeID string) error {
	conn, err := c.getConnection(nodeID)
	if err != nil {
		return err
	}

	client := agent.NewAgentServiceClient(conn)

	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	_, err = client.Ping(ctx, &agent.Empty{})
	return err
}

// ProvisionVM provisions a VM on an agent.
func (c *client) ProvisionVM(ctx context.Context, nodeID string, req *agent.ProvisionVMRequest) error {
	conn, err := c.getConnection(nodeID)
	if err != nil {
		return err
	}

	client := agent.NewAgentServiceClient(conn)

	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	resp, err := client.ProvisionVM(ctx, req)
	if err != nil {
		return fmt.Errorf("provision VM failed: %w", err)
	}

	if resp.Error != nil {
		return fmt.Errorf("provision VM failed: %s (%s)", resp.Error.Message, resp.Error.Code)
	}

	return nil
}

// StartVM starts a VM on an agent.
func (c *client) StartVM(ctx context.Context, nodeID string, vmID string) error {
	conn, err := c.getConnection(nodeID)
	if err != nil {
		return err
	}

	client := agent.NewAgentServiceClient(conn)

	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	resp, err := client.StartVM(ctx, &agent.VMStateRequest{VmId: vmID})
	if err != nil {
		return fmt.Errorf("start VM failed: %w", err)
	}

	if resp.Error != nil {
		return fmt.Errorf("start VM failed: %s (%s)", resp.Error.Message, resp.Error.Code)
	}

	return nil
}

// StopVM stops a VM on an agent.
func (c *client) StopVM(ctx context.Context, nodeID string, vmID string) error {
	conn, err := c.getConnection(nodeID)
	if err != nil {
		return err
	}

	client := agent.NewAgentServiceClient(conn)

	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	resp, err := client.StopVM(ctx, &agent.VMStateRequest{VmId: vmID})
	if err != nil {
		return fmt.Errorf("stop VM failed: %w", err)
	}

	if resp.Error != nil {
		return fmt.Errorf("stop VM failed: %s (%s)", resp.Error.Message, resp.Error.Code)
	}

	return nil
}

// RebootVM reboots a VM on an agent.
func (c *client) RebootVM(ctx context.Context, nodeID string, vmID string) error {
	conn, err := c.getConnection(nodeID)
	if err != nil {
		return err
	}

	client := agent.NewAgentServiceClient(conn)

	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	resp, err := client.RebootVM(ctx, &agent.VMStateRequest{VmId: vmID})
	if err != nil {
		return fmt.Errorf("reboot VM failed: %w", err)
	}

	if resp.Error != nil {
		return fmt.Errorf("reboot VM failed: %s (%s)", resp.Error.Message, resp.Error.Code)
	}

	return nil
}

// DeleteVM deletes a VM on an agent.
func (c *client) DeleteVM(ctx context.Context, nodeID string, vmID string) error {
	conn, err := c.getConnection(nodeID)
	if err != nil {
		return err
	}

	client := agent.NewAgentServiceClient(conn)

	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	resp, err := client.DeleteVM(ctx, &agent.VMDeleteRequest{VmId: vmID})
	if err != nil {
		return fmt.Errorf("delete VM failed: %w", err)
	}

	if resp.Error != nil {
		return fmt.Errorf("delete VM failed: %s (%s)", resp.Error.Message, resp.Error.Code)
	}

	return nil
}

// GetVMState gets the state of a VM from an agent.
func (c *client) GetVMState(ctx context.Context, nodeID string, vmID string) (*agent.VMStateResponse, error) {
	conn, err := c.getConnection(nodeID)
	if err != nil {
		return nil, err
	}

	client := agent.NewAgentServiceClient(conn)

	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	return client.GetVMState(ctx, &agent.VMStateRequest{VmId: vmID})
}

// CreateVolume creates a volume on an agent.
func (c *client) CreateVolume(ctx context.Context, nodeID string, req *agent.VolumeCreateRequest) error {
	conn, err := c.getConnection(nodeID)
	if err != nil {
		return err
	}

	client := agent.NewAgentServiceClient(conn)

	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	resp, err := client.CreateVolume(ctx, req)
	if err != nil {
		return fmt.Errorf("create volume failed: %w", err)
	}

	if !resp.Ok {
		if len(resp.Errors) > 0 {
			return fmt.Errorf("create volume failed: %s", resp.Errors[0].Message)
		}
		return fmt.Errorf("create volume failed")
	}

	return nil
}

// ResizeVolume resizes a volume on an agent.
func (c *client) ResizeVolume(ctx context.Context, nodeID string, req *agent.VolumeResizeRequest) error {
	conn, err := c.getConnection(nodeID)
	if err != nil {
		return err
	}

	client := agent.NewAgentServiceClient(conn)

	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	resp, err := client.ResizeVolume(ctx, req)
	if err != nil {
		return fmt.Errorf("resize volume failed: %w", err)
	}

	if !resp.Ok {
		if len(resp.Errors) > 0 {
			return fmt.Errorf("resize volume failed: %s", resp.Errors[0].Message)
		}
		return fmt.Errorf("resize volume failed")
	}

	return nil
}

// EnsureBridge ensures a bridge exists on an agent.
func (c *client) EnsureBridge(ctx context.Context, nodeID string, req *agent.EnsureBridgeRequest) error {
	conn, err := c.getConnection(nodeID)
	if err != nil {
		return err
	}

	client := agent.NewAgentServiceClient(conn)

	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	resp, err := client.EnsureBridge(ctx, req)
	if err != nil {
		return fmt.Errorf("ensure bridge failed: %w", err)
	}

	if !resp.Ok {
		if len(resp.Errors) > 0 {
			return fmt.Errorf("ensure bridge failed: %s", resp.Errors[0].Message)
		}
		return fmt.Errorf("ensure bridge failed")
	}

	return nil
}

// ImportImage imports an image on an agent.
func (c *client) ImportImage(ctx context.Context, nodeID string, req *agent.ImageImportRequest) error {
	conn, err := c.getConnection(nodeID)
	if err != nil {
		return err
	}

	client := agent.NewAgentServiceClient(conn)

	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	resp, err := client.ImportImage(ctx, req)
	if err != nil {
		return fmt.Errorf("import image failed: %w", err)
	}

	if !resp.Ok {
		if len(resp.Errors) > 0 {
			return fmt.Errorf("import image failed: %s", resp.Errors[0].Message)
		}
		return fmt.Errorf("import image failed")
	}

	return nil
}

// Close closes all connections.
func (c *client) Close() error {
	c.mu.Lock()
	defer c.mu.Unlock()

	var lastErr error
	for nodeID, conn := range c.connections {
		if err := conn.Close(); err != nil {
			lastErr = err
		}
		delete(c.connections, nodeID)
	}

	return lastErr
}
