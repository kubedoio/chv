package services

import (
	"bufio"
	"bytes"
	"context"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"

	"github.com/gorilla/websocket"
)

// VMConsoleService provides WebSocket proxy to VM serial console
type VMConsoleService struct {
	upgrader   websocket.Upgrader
	sessions   map[string]*ConsoleSession
	sessionsMu sync.RWMutex
}

// ConsoleSession represents an active console session
type ConsoleSession struct {
	VMID       string
	Conn       *websocket.Conn
	SerialConn io.ReadWriteCloser
	Cancel     context.CancelFunc
	mu         sync.Mutex
}

// NewVMConsoleService creates a new VM console service
func NewVMConsoleService() *VMConsoleService {
	return &VMConsoleService{
		upgrader: websocket.Upgrader{
			ReadBufferSize:  1024,
			WriteBufferSize: 1024,
			CheckOrigin: func(r *http.Request) bool {
				// Allow all origins for MVP - restrict in production
				return true
			},
		},
		sessions: make(map[string]*ConsoleSession),
	}
}

// HandleWebSocket handles WebSocket connections for VM console
func (s *VMConsoleService) HandleWebSocket(w http.ResponseWriter, r *http.Request, vmID, apiSocket string) error {
	// Check if VM is running
	if !s.isVMRunning(apiSocket) {
		return fmt.Errorf("VM is not running or API socket not found")
	}

	// Upgrade HTTP to WebSocket
	conn, err := s.upgrader.Upgrade(w, r, nil)
	if err != nil {
		return fmt.Errorf("failed to upgrade connection: %w", err)
	}
	defer conn.Close()

	// Get serial console PTY from CH
	serialPath, err := s.getSerialConsole(apiSocket)
	if err != nil {
		conn.WriteMessage(websocket.TextMessage, []byte("Error: "+err.Error()))
		return err
	}

	// Connect to serial console PTY (it's a device file, not a unix socket)
	serialConn, err := os.OpenFile(serialPath, os.O_RDWR, 0)
	if err != nil {
		conn.WriteMessage(websocket.TextMessage, []byte("Error connecting to console: "+err.Error()))
		return fmt.Errorf("failed to connect to serial console: %w", err)
	}
	defer serialConn.Close()

	// Create session
	ctx, cancel := context.WithCancel(r.Context())
	session := &ConsoleSession{
		VMID:       vmID,
		Conn:       conn,
		SerialConn: serialConn,
		Cancel:     cancel,
	}

	s.sessionsMu.Lock()
	s.sessions[vmID] = session
	s.sessionsMu.Unlock()

	defer func() {
		s.sessionsMu.Lock()
		delete(s.sessions, vmID)
		s.sessionsMu.Unlock()
		cancel()
	}()

	// Send welcome message
	conn.WriteMessage(websocket.TextMessage, []byte("\r\nConnected to VM console. Press Enter to activate.\r\n"))

	// Start bidirectional copying
	errChan := make(chan error, 2)

	// WebSocket -> Serial
	go func() {
		errChan <- s.wsToSerial(session)
	}()

	// Serial -> WebSocket
	go func() {
		errChan <- s.serialToWS(session)
	}()

	// Wait for either direction to error or context cancellation
	select {
	case err := <-errChan:
		return err
	case <-ctx.Done():
		return ctx.Err()
	}
}

// wsToSerial copies data from WebSocket to serial console
func (s *VMConsoleService) wsToSerial(session *ConsoleSession) error {
	for {
		messageType, data, err := session.Conn.ReadMessage()
		if err != nil {
			if websocket.IsUnexpectedCloseError(err, websocket.CloseGoingAway, websocket.CloseAbnormalClosure) {
				return fmt.Errorf("websocket error: %w", err)
			}
			return nil // Clean close
		}

		if messageType == websocket.TextMessage || messageType == websocket.BinaryMessage {
			session.mu.Lock()
			_, err := session.SerialConn.Write(data)
			session.mu.Unlock()
			if err != nil {
				return fmt.Errorf("serial write error: %w", err)
			}
		}
	}
}

// serialToWS copies data from serial console to WebSocket
func (s *VMConsoleService) serialToWS(session *ConsoleSession) error {
	buf := make([]byte, 1024)
	for {
		n, err := session.SerialConn.Read(buf)
		if err != nil {
			if err == io.EOF {
				return nil
			}
			return fmt.Errorf("serial read error: %w", err)
		}

		if n > 0 {
			err = session.Conn.WriteMessage(websocket.TextMessage, buf[:n])
			if err != nil {
				if websocket.IsUnexpectedCloseError(err, websocket.CloseGoingAway, websocket.CloseAbnormalClosure) {
					return fmt.Errorf("websocket write error: %w", err)
				}
				return nil // Clean close
			}
		}
	}
}

// isVMRunning checks if VM API socket exists
func (s *VMConsoleService) isVMRunning(apiSocket string) bool {
	conn, err := net.Dial("unix", apiSocket)
	if err != nil {
		return false
	}
	conn.Close()
	return true
}

// getSerialConsole reads the PTY path from the workspace and returns it
// The PTY path is written by VMManagementService when starting the VM
func (s *VMConsoleService) getSerialConsole(apiSocket string) (string, error) {
	// Derive workspace path from apiSocket path
	// apiSocket is at: <workspace>/api.sock
	workspaceDir := filepath.Dir(apiSocket)
	ptyFile := filepath.Join(workspaceDir, "serial.ptty")

	// Read PTY path from file
	data, err := os.ReadFile(ptyFile)
	if err != nil {
		if os.IsNotExist(err) {
			return "", fmt.Errorf("serial console not available: VM may not be running or PTY not yet created")
		}
		return "", fmt.Errorf("failed to read PTY path: %w", err)
	}

	ptyPath := strings.TrimSpace(string(data))
	if ptyPath == "" {
		return "", fmt.Errorf("PTY path is empty")
	}

	// Verify the PTY device exists
	if _, err := os.Stat(ptyPath); err != nil {
		return "", fmt.Errorf("PTY device does not exist: %s", ptyPath)
	}

	return ptyPath, nil
}

// GetActiveSessions returns list of active console sessions
func (s *VMConsoleService) GetActiveSessions() []string {
	s.sessionsMu.RLock()
	defer s.sessionsMu.RUnlock()

	sessions := make([]string, 0, len(s.sessions))
	for vmID := range s.sessions {
		sessions = append(sessions, vmID)
	}
	return sessions
}

// CloseSession forcefully closes a console session
func (s *VMConsoleService) CloseSession(vmID string) {
	s.sessionsMu.Lock()
	session, exists := s.sessions[vmID]
	s.sessionsMu.Unlock()

	if exists {
		session.Cancel()
		session.Conn.Close()
		session.SerialConn.Close()
	}
}

// Alternative implementation using CH API directly for console
// CH provides console output via HTTP API when --serial is configured

type ConsoleStreamer struct {
	vmID       string
	apiSocket  string
	wsConn     *websocket.Conn
	httpClient *http.Client
}

// NewConsoleStreamer creates a console streamer using CH HTTP API
func NewConsoleStreamer(vmID, apiSocket string, wsConn *websocket.Conn) *ConsoleStreamer {
	return &ConsoleStreamer{
		vmID:      vmID,
		apiSocket: apiSocket,
		wsConn:    wsConn,
		httpClient: &http.Client{
			Timeout: 0, // No timeout for streaming
			Transport: &http.Transport{
				DialContext: func(ctx context.Context, network, addr string) (net.Conn, error) {
					return net.Dial("unix", apiSocket)
				},
			},
		},
	}
}

// Stream starts streaming console output from CH to WebSocket
func (cs *ConsoleStreamer) Stream(ctx context.Context) error {
	// CH API endpoint for console (if available)
	// Note: This requires CH to be started with console access enabled
	req, err := http.NewRequestWithContext(ctx, "GET", "http://localhost/api/v1/vm.console", nil)
	if err != nil {
		return err
	}

	resp, err := cs.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("failed to connect to console: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("console not available: %d", resp.StatusCode)
	}

	// Stream body to WebSocket
	reader := bufio.NewReader(resp.Body)
	for {
		line, err := reader.ReadBytes('\n')
		if err != nil {
			if err == io.EOF {
				return nil
			}
			return err
		}

		if len(line) > 0 {
			err = cs.wsConn.WriteMessage(websocket.TextMessage, line)
			if err != nil {
				return err
			}
		}
	}
}

// Write sends input to VM console via CH API
func (cs *ConsoleStreamer) Write(data []byte) error {
	// CH API endpoint for console input
	req, err := http.NewRequest("PUT", "http://localhost/api/v1/vm.console", bytes.NewReader(data))
	if err != nil {
		return err
	}

	resp, err := cs.httpClient.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("failed to write to console: %d", resp.StatusCode)
	}

	return nil
}
