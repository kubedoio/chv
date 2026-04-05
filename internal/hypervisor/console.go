// Package hypervisor provides VM lifecycle management for Cloud Hypervisor.
package hypervisor

import (
	"context"
	"fmt"
	"io"
	"net"
	"net/http"
	"sync"
	"time"
)

// ConsoleProxy proxies between WebSocket and cloud-hypervisor API socket
// for VM serial console access.
type ConsoleProxy struct {
	apiSocketPath string
	client        *http.Client
}

// NewConsoleProxy creates a new console proxy
func NewConsoleProxy(apiSocketPath string) *ConsoleProxy {
	return &ConsoleProxy{
		apiSocketPath: apiSocketPath,
		client: &http.Client{
			Transport: &http.Transport{
				DialContext: func(ctx context.Context, network, addr string) (net.Conn, error) {
					var d net.Dialer
					return d.DialContext(ctx, "unix", apiSocketPath)
				},
			},
			Timeout: 30 * time.Second,
		},
	}
}

// ConsoleStream represents a bidirectional console stream
type ConsoleStream struct {
	Input  io.WriteCloser
	Output io.ReadCloser
}

// OpenConsole opens a console stream to the VM via cloud-hypervisor API.
// For MVP-1, this connects directly to the CH API socket.
func (p *ConsoleProxy) OpenConsole(ctx context.Context) (*ConsoleStream, error) {
	// Cloud Hypervisor doesn't have a direct console streaming API endpoint.
	// For serial console access, we typically need to:
	// 1. Use the PTY if available (via --serial tty mode)
	// 2. Or use the API for input/output relay
	//
	// For MVP-1, we implement a simple passthrough that connects to
	// a Unix socket for console data if CH is configured with --serial socket=...
	
	// Since CH's default serial is stdout/stderr, we need to use a different approach.
	// For now, we return an error indicating the limitation.
	return nil, fmt.Errorf("console streaming requires serial socket mode: configure VM with serial socket")
}

// GetAPISocketPath returns the API socket path for direct access
func (p *ConsoleProxy) GetAPISocketPath() string {
	return p.apiSocketPath
}

// StreamConsole streams console data bidirectionally between the provided
// reader/writer and the cloud-hypervisor serial console.
// This is a simplified MVP-1 implementation that:
// - Writes input to a serial socket if available
// - Reads output from VM log files as a fallback
func (p *ConsoleProxy) StreamConsole(ctx context.Context, w io.Writer, r io.Reader) error {
	// For MVP-1, implement a basic relay that:
	// 1. Reads input from 'r' and forwards to CH serial
	// 2. Reads from CH serial and writes to 'w'
	
	// Try to connect to serial socket (if configured)
	serialSocket := p.apiSocketPath + ".serial"
	
	conn, err := net.Dial("unix", serialSocket)
	if err != nil {
		// Serial socket not available, use API-based approach
		return p.streamViaAPI(ctx, w, r)
	}
	defer conn.Close()
	
	return p.streamConn(ctx, conn, w, r)
}

// streamConn handles bidirectional streaming over a net.Conn
func (p *ConsoleProxy) streamConn(ctx context.Context, conn net.Conn, w io.Writer, r io.Reader) error {
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()
	
	errChan := make(chan error, 2)
	
	// Read from connection, write to writer
	go func() {
		defer cancel()
		buf := make([]byte, 4096)
		for {
			select {
			case <-ctx.Done():
				return
			default:
			}
			
			n, err := conn.Read(buf)
			if err != nil {
				if err != io.EOF {
					errChan <- fmt.Errorf("console read error: %w", err)
				}
				return
			}
			
			if _, err := w.Write(buf[:n]); err != nil {
				errChan <- fmt.Errorf("console write error: %w", err)
				return
			}
		}
	}()
	
	// Read from reader, write to connection
	go func() {
		defer cancel()
		buf := make([]byte, 4096)
		for {
			select {
			case <-ctx.Done():
				return
			default:
			}
			
			n, err := r.Read(buf)
			if err != nil {
				if err != io.EOF {
					errChan <- fmt.Errorf("input read error: %w", err)
				}
				return
			}
			
			if _, err := conn.Write(buf[:n]); err != nil {
				errChan <- fmt.Errorf("input write error: %w", err)
				return
			}
		}
	}()
	
	// Wait for context cancellation or error
	select {
	case <-ctx.Done():
		return nil
	case err := <-errChan:
		return err
	}
}

// streamViaAPI streams console data via HTTP API requests to cloud-hypervisor.
// This is used as a fallback when direct serial socket is not available.
// Note: Cloud Hypervisor doesn't have a native console streaming API, so this
// is a placeholder for future enhancement or custom CH builds.
func (p *ConsoleProxy) streamViaAPI(ctx context.Context, w io.Writer, r io.Reader) error {
	// For MVP-1 without serial socket, return a descriptive error
	// In production, this could:
	// - Poll VM logs and stream them
	// - Use custom CH builds with console streaming support
	// - Use a PTY wrapper
	return fmt.Errorf("console streaming requires serial socket mode; ensure VM is configured with --serial socket=path")
}

// IsAvailable checks if the console proxy can connect to the VM
func (p *ConsoleProxy) IsAvailable(ctx context.Context) bool {
	// Try to connect to the API socket
	conn, err := net.Dial("unix", p.apiSocketPath)
	if err != nil {
		return false
	}
	conn.Close()
	return true
}

// ConsoleSession represents an active console session
type ConsoleSession struct {
	VMID      string
	UserID    string
	StartedAt time.Time
	Conn      io.Closer
}

// SessionManager manages active console sessions with rate limiting
type SessionManager struct {
	sessions map[string]*ConsoleSession // key: "vmID:userID"
	mu       sync.RWMutex
}

// NewSessionManager creates a new session manager
func NewSessionManager() *SessionManager {
	return &SessionManager{
		sessions: make(map[string]*ConsoleSession),
	}
}

// AcquireSession attempts to acquire a console session for a user/VM combination.
// Returns error if a session already exists (rate limiting: max 1 per VM per user).
func (sm *SessionManager) AcquireSession(vmID, userID string) error {
	key := vmID + ":" + userID
	
	sm.mu.Lock()
	defer sm.mu.Unlock()
	
	if existing, ok := sm.sessions[key]; ok {
		return fmt.Errorf("console session already active for VM %s (started at %s)", vmID, existing.StartedAt.Format(time.RFC3339))
	}
	
	return nil
}

// RegisterSession registers an active console session
func (sm *SessionManager) RegisterSession(session *ConsoleSession) {
	key := session.VMID + ":" + session.UserID
	
	sm.mu.Lock()
	defer sm.mu.Unlock()
	
	sm.sessions[key] = session
}

// ReleaseSession releases a console session
func (sm *SessionManager) ReleaseSession(vmID, userID string) {
	key := vmID + ":" + userID
	
	sm.mu.Lock()
	defer sm.mu.Unlock()
	
	delete(sm.sessions, key)
}

// HasSession checks if a session exists
func (sm *SessionManager) HasSession(vmID, userID string) bool {
	key := vmID + ":" + userID
	
	sm.mu.RLock()
	defer sm.mu.RUnlock()
	
	_, ok := sm.sessions[key]
	return ok
}
