// Package agent provides a gRPC client for controller-to-agent communication.
package agent

import (
	"context"
	"crypto/tls"
	"fmt"
	"net"
	"strings"
	"sync"
	"time"

	"github.com/chv/chv/internal/pb/agent"
	"google.golang.org/grpc"
	"google.golang.org/grpc/backoff"
	"google.golang.org/grpc/connectivity"
	"google.golang.org/grpc/credentials"
	"google.golang.org/grpc/credentials/insecure"
)

const (
	// DefaultMaxConnections is the default maximum number of connections.
	DefaultMaxConnections = 1000
	// DefaultConnectionTimeout is the default timeout for connection operations.
	// Increased to 120s to accommodate VM boot operations with qcow2 images and hypervisor-fw.
	DefaultConnectionTimeout = 120 * time.Second
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
	StreamConsole(ctx context.Context, nodeID string) (agent.AgentService_StreamConsoleClient, error)
	Close() error
}

// client implements the Client interface.
type client struct {
	mu             sync.RWMutex
	connections    map[string]*grpc.ClientConn
	timeout        time.Duration
	maxConns       int
	tlsCredentials credentials.TransportCredentials
	serverName     string
}

// ClientOption is a functional option for configuring the client.
type ClientOption func(*client)

// WithMaxConnections sets the maximum number of connections.
func WithMaxConnections(max int) ClientOption {
	return func(c *client) {
		c.maxConns = max
	}
}

// WithTimeout sets the default timeout for operations.
func WithTimeout(timeout time.Duration) ClientOption {
	return func(c *client) {
		c.timeout = timeout
	}
}

// SetTimeout sets the timeout for the client (used for testing).
func (c *client) SetTimeout(timeout time.Duration) {
	c.timeout = timeout
}

// WithTLS sets the TLS credentials for secure connections.
func WithTLS(creds credentials.TransportCredentials) ClientOption {
	return func(c *client) {
		c.tlsCredentials = creds
	}
}

// WithServerName sets the server name for TLS verification.
func WithServerName(serverName string) ClientOption {
	return func(c *client) {
		c.serverName = serverName
	}
}

// NewClient creates a new agent client.
func NewClient(opts ...ClientOption) Client {
	c := &client{
		connections: make(map[string]*grpc.ClientConn),
		timeout:     DefaultConnectionTimeout,
		maxConns:    DefaultMaxConnections,
	}
	for _, opt := range opts {
		opt(c)
	}
	return c
}



// getConnection gets or creates a connection to an agent.
func (c *client) getConnection(nodeID string) (*grpc.ClientConn, error) {
	c.mu.RLock()
	conn, exists := c.connections[nodeID]
	c.mu.RUnlock()

	// Only reuse connection if it's in a ready state
	if exists && conn.GetState() == connectivity.Ready {
		return conn, nil
	}

	c.mu.Lock()
	defer c.mu.Unlock()

	// Double-check after acquiring write lock
	if conn, exists := c.connections[nodeID]; exists && conn.GetState() == connectivity.Ready {
		return conn, nil
	}

	// Check connection limit
	if len(c.connections) >= c.maxConns && !exists {
		return nil, fmt.Errorf("connection limit reached (%d)", c.maxConns)
	}

	// Close old connection if exists
	if exists {
		conn.Close()
	}

	// Create new connection
	conn, err := c.dial(nodeID)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to agent %s: %w", nodeID, err)
	}

	c.connections[nodeID] = conn
	return conn, nil
}

// parseNodeAddress parses a node address which may be in CIDR notation or plain IP/hostname.
// It returns the address with the agent gRPC port appended.
func parseNodeAddress(addr string) string {
	// Remove CIDR suffix if present (e.g., "192.168.1.1/24" -> "192.168.1.1")
	if strings.Contains(addr, "/") {
		ip, _, err := net.ParseCIDR(addr)
		if err == nil {
			addr = ip.String()
		} else {
			// Not a valid CIDR, just strip after /
			addr = strings.SplitN(addr, "/", 2)[0]
		}
	}
	
	// Add port if not present
	if !strings.Contains(addr, ":") {
		addr = net.JoinHostPort(addr, "9091")
	}
	
	return addr
}

// dial creates a new gRPC connection with retry configuration.
func (c *client) dial(address string) (*grpc.ClientConn, error) {
	// Parse the address to handle CIDR notation and add port
	parsedAddr := parseNodeAddress(address)
	
	backoffConfig := backoff.DefaultConfig
	backoffConfig.MaxDelay = 5 * time.Second

	// Use TLS credentials if provided, otherwise use insecure
	creds := c.tlsCredentials
	if creds == nil {
		creds = insecure.NewCredentials()
	}

	conn, err := grpc.Dial(
		parsedAddr,
		grpc.WithTransportCredentials(creds),
		grpc.WithConnectParams(grpc.ConnectParams{
			Backoff: backoffConfig,
		}),
	)
	if err != nil {
		return nil, err
	}

	return conn, nil
}

// NewClientWithTLS creates a new agent client with TLS configuration.
// This is a convenience function for creating a client with TLS/mTLS.
func NewClientWithTLS(tlsConfig *tls.Config, opts ...ClientOption) (Client, error) {
	if tlsConfig == nil {
		return nil, fmt.Errorf("TLS config is required")
	}

	creds := credentials.NewTLS(tlsConfig)
	opts = append([]ClientOption{WithTLS(creds)}, opts...)
	
	return NewClient(opts...), nil
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

// StreamConsole establishes a bidirectional streaming connection for console access.
func (c *client) StreamConsole(ctx context.Context, nodeID string) (agent.AgentService_StreamConsoleClient, error) {
	conn, err := c.getConnection(nodeID)
	if err != nil {
		return nil, err
	}

	client := agent.NewAgentServiceClient(conn)
	stream, err := client.StreamConsole(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to start console stream: %w", err)
	}

	return stream, nil
}

// Close closes all connections with timeout.
func (c *client) Close() error {
	c.mu.Lock()
	defer c.mu.Unlock()

	var lastErr error
	closeTimeout := 5 * time.Second

	for nodeID, conn := range c.connections {
		// Use timeout context for close
		ctx, cancel := context.WithTimeout(context.Background(), closeTimeout)
		
		// Create a channel to signal close completion
		done := make(chan error, 1)
		go func(c *grpc.ClientConn) {
			done <- c.Close()
		}(conn)

		select {
		case err := <-done:
			if err != nil {
				lastErr = err
			}
		case <-ctx.Done():
			lastErr = fmt.Errorf("timeout closing connection to %s", nodeID)
		}
		cancel()
		delete(c.connections, nodeID)
	}

	return lastErr
}
