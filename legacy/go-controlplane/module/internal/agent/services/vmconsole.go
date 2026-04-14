package services

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
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
// Uses Unix Domain Socket for reliable bidirectional communication
func (s *VMConsoleService) HandleWebSocket(w http.ResponseWriter, r *http.Request, vmID, workspacePath string) error {
	consoleSocket := filepath.Join(workspacePath, "console.sock")

	// Check if console socket exists (VM is running)
	if _, err := os.Stat(consoleSocket); err != nil {
		return fmt.Errorf("VM console not available. Is the VM running?")
	}

	// Upgrade HTTP to WebSocket
	conn, err := s.upgrader.Upgrade(w, r, nil)
	if err != nil {
		return fmt.Errorf("failed to upgrade connection: %w", err)
	}
	defer conn.Close()

	// Connect to serial console Unix socket
	vmSocket, err := net.Dial("unix", consoleSocket)
	if err != nil {
		conn.WriteMessage(websocket.TextMessage, []byte("Error connecting to VM console: "+err.Error()))
		return fmt.Errorf("failed to connect to console socket: %w", err)
	}
	defer vmSocket.Close()

	// Create session
	ctx, cancel := context.WithCancel(r.Context())
	session := &ConsoleSession{
		VMID:       vmID,
		Conn:       conn,
		SerialConn: vmSocket,
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
// It first attempts to read from serial.ptty file, then falls back to CH API
func (s *VMConsoleService) getSerialConsole(apiSocket string) (string, error) {
	// Option 1: Try PTY path from file (created by VMManagementService during start)
	workspace := filepath.Dir(apiSocket)
	ptyFile := filepath.Join(workspace, "serial.ptty")

	// Read PTY path from file
	if data, err := os.ReadFile(ptyFile); err == nil {
		ptyPath := strings.TrimSpace(string(data))
		if ptyPath != "" {
			// Verify it exists
			if _, err := os.Stat(ptyPath); err == nil {
				return ptyPath, nil
			}
		}
	}

	// Option 2: Query CH API vm.info endpoint for PTY path
	conn, err := net.Dial("unix", apiSocket)
	if err != nil {
		return "", fmt.Errorf("console not available: cannot connect to VM API: %w", err)
	}
	defer conn.Close()

	// Query CH API for VM info which includes serial configuration
	req, _ := http.NewRequest("GET", "http://localhost/api/v1/vm.info", nil)
	if err := req.Write(conn); err != nil {
		return "", fmt.Errorf("failed to query VM info via API: %w", err)
	}

	resp, err := http.ReadResponse(bufio.NewReader(conn), req)
	if err != nil {
		return "", fmt.Errorf("failed to read API response: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode == http.StatusOK {
		var result struct {
			Config struct {
				Serial struct {
					File string `json:"file"`
					Mode string `json:"mode"`
				} `json:"serial"`
			} `json:"config"`
		}
		if err := json.NewDecoder(resp.Body).Decode(&result); err == nil {
			if result.Config.Serial.Mode == "Pty" && result.Config.Serial.File != "" {
				// Verify it exists
				if _, err := os.Stat(result.Config.Serial.File); err == nil {
					return result.Config.Serial.File, nil
				}
			}
		}
	}

	return "", fmt.Errorf("console not available: VM may not have serial enabled or socket not yet created")
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
