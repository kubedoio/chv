// Package console provides VM serial console streaming for the agent.
package console

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"net/http"
	"sync"
	"time"

	"github.com/google/uuid"
	"github.com/gorilla/websocket"
)

const (
	// Time allowed to write a message to the peer.
	writeWait = 10 * time.Second

	// Time allowed to read the next pong message from the peer.
	pongWait = 60 * time.Second

	// Send pings to peer with this period. Must be less than pongWait.
	pingPeriod = 30 * time.Second

	// Maximum message size allowed from peer.
	maxMessageSize = 4096

	// WebSocket message types
	MsgTypeOutput = "output"
	MsgTypeInput  = "input"
	MsgTypeError  = "error"
	MsgTypeStatus = "status"
	MsgTypeResize = "resize"
	MsgTypePing   = "ping"
	MsgTypePong   = "pong"
	MsgTypeHistory = "history"
)

// WebSocketMessage represents a message exchanged over WebSocket.
type WebSocketMessage struct {
	Type string `json:"type"`
	Data string `json:"data,omitempty"` // base64 encoded for binary data
	Rows int    `json:"rows,omitempty"` // for resize
	Cols int    `json:"cols,omitempty"` // for resize
}

// WebSocketServer handles WebSocket connections for console access.
type WebSocketServer struct {
	manager    *Manager
	upgrader   websocket.Upgrader
	authFunc   AuthFunc
	mu         sync.RWMutex
	connections map[string]*WebSocketConnection // key: connID
}

// AuthFunc is a function that authenticates console access.
type AuthFunc func(token string, vmID string) (userID string, allowed bool, err error)

// WebSocketConnection represents an active WebSocket connection.
type WebSocketConnection struct {
	ID       string
	VMID     string
	UserID   string
	Conn     *websocket.Conn
	Client   *Client
	Server   *WebSocketServer
	Cancel   context.CancelFunc
	mu       sync.Mutex
}

// NewWebSocketServer creates a new WebSocket server.
// By default, it uses a same-origin policy. Use SetCheckOrigin to configure CORS.
func NewWebSocketServer(manager *Manager, authFunc AuthFunc) *WebSocketServer {
	return &WebSocketServer{
		manager: manager,
		upgrader: websocket.Upgrader{
			ReadBufferSize:  1024,
			WriteBufferSize: 1024,
			CheckOrigin: func(r *http.Request) bool {
				// Same-origin policy by default - reject cross-origin requests
				// This can be overridden with SetCheckOrigin for specific allowed origins
				origin := r.Header.Get("Origin")
				if origin == "" {
					// No origin header (e.g., non-browser clients) - allow
					return true
				}
				// By default, only allow same-origin requests
				// The caller should use SetCheckOrigin to allow specific origins
				return false
			},
		},
		authFunc:    authFunc,
		connections: make(map[string]*WebSocketConnection),
	}
}

// NewWebSocketServerWithOrigins creates a WebSocket server with specific allowed origins.
func NewWebSocketServerWithOrigins(manager *Manager, authFunc AuthFunc, allowedOrigins []string) *WebSocketServer {
	s := NewWebSocketServer(manager, authFunc)
	s.SetAllowedOrigins(allowedOrigins)
	return s
}

// SetAllowedOrigins sets the allowed origins for CORS.
func (s *WebSocketServer) SetAllowedOrigins(origins []string) {
	originSet := make(map[string]bool, len(origins))
	for _, o := range origins {
		originSet[o] = true
	}
	
	s.upgrader.CheckOrigin = func(r *http.Request) bool {
		origin := r.Header.Get("Origin")
		if origin == "" {
			// No origin header (e.g., non-browser clients) - allow
			return true
		}
		// Check exact match
		if originSet[origin] {
			return true
		}
		// Check wildcard match (e.g., https://*.example.com)
		for allowed := range originSet {
			if matchWildcard(origin, allowed) {
				return true
			}
		}
		return false
	}
}

// matchWildcard checks if origin matches a pattern with wildcards.
func matchWildcard(origin, pattern string) bool {
	if pattern == "*" {
		return true
	}
	// Simple wildcard support: *.example.com matches sub.example.com
	if len(pattern) > 2 && pattern[:2] == "*." {
		suffix := pattern[1:] // .example.com
		return len(origin) > len(suffix) && origin[len(origin)-len(suffix):] == suffix
	}
	return origin == pattern
}

// SetCheckOrigin sets the origin check function for the upgrader.
func (s *WebSocketServer) SetCheckOrigin(fn func(r *http.Request) bool) {
	s.upgrader.CheckOrigin = fn
}

// HandleWebSocket handles WebSocket upgrade requests.
func (s *WebSocketServer) HandleWebSocket(w http.ResponseWriter, r *http.Request) {
	vmID := r.URL.Query().Get("vm_id")
	if vmID == "" {
		http.Error(w, "Missing vm_id parameter", http.StatusBadRequest)
		return
	}

	// Get token from query or header
	token := r.URL.Query().Get("token")
	if token == "" {
		token = r.Header.Get("Authorization")
		if len(token) > 7 && token[:7] == "Bearer " {
			token = token[7:]
		}
	}

	if token == "" {
		http.Error(w, "Missing authentication token", http.StatusUnauthorized)
		return
	}

	// Authenticate
	userID, allowed, err := s.authFunc(token, vmID)
	if err != nil {
		http.Error(w, fmt.Sprintf("Authentication error: %v", err), http.StatusInternalServerError)
		return
	}
	if !allowed {
		http.Error(w, "Access denied", http.StatusForbidden)
		return
	}

	// Get or create session
	session, err := s.manager.GetOrCreateSession(vmID)
	if err != nil {
		http.Error(w, fmt.Sprintf("Failed to create console session: %v", err), http.StatusInternalServerError)
		return
	}

	// Upgrade to WebSocket
	conn, err := s.upgrader.Upgrade(w, r, nil)
	if err != nil {
		// Upgrade already writes error response
		return
	}

	// Create connection
	connID := uuid.New().String()
	ctx, cancel := context.WithCancel(r.Context())

	wsConn := &WebSocketConnection{
		ID:     connID,
		VMID:   vmID,
		UserID: userID,
		Conn:   conn,
		Client: session.AddClient(connID),
		Server: s,
		Cancel: cancel,
	}

	s.mu.Lock()
	s.connections[connID] = wsConn
	s.mu.Unlock()

	// Handle connection
	go wsConn.writePump()
	go wsConn.readPump()

	// Send history immediately
	history := session.GetHistory()
	if len(history) > 0 {
		wsConn.sendMessage(&WebSocketMessage{
			Type: MsgTypeHistory,
			Data: base64.StdEncoding.EncodeToString(history),
		})
	}

	// Wait for context cancellation
	<-ctx.Done()
}

// readPump pumps messages from the WebSocket connection to the console.
func (c *WebSocketConnection) readPump() {
	defer func() {
		c.Cancel()
		c.cleanup()
	}()

	c.Conn.SetReadLimit(maxMessageSize)
	c.Conn.SetReadDeadline(time.Now().Add(pongWait))
	c.Conn.SetPongHandler(func(string) error {
		c.Conn.SetReadDeadline(time.Now().Add(pongWait))
		return nil
	})

	for {
		var msg WebSocketMessage
		err := c.Conn.ReadJSON(&msg)
		if err != nil {
			if websocket.IsUnexpectedCloseError(err, websocket.CloseGoingAway, websocket.CloseAbnormalClosure) {
				// Log unexpected close
			}
			break
		}

		c.Conn.SetReadDeadline(time.Now().Add(pongWait))

		switch msg.Type {
		case MsgTypeInput:
			// Decode base64 input data
			data, err := base64.StdEncoding.DecodeString(msg.Data)
			if err != nil {
				c.sendError(fmt.Sprintf("Invalid input data: %v", err))
				continue
			}

			// Send to client's recv channel
			select {
			case c.Client.Recv <- data:
			default:
				c.sendError("Input buffer full")
			}

			// Also write to session input if TTY enabled
			if err := c.Client.Session.WriteInput(data); err != nil {
				// Input not supported or error - non-fatal
			}

		case MsgTypeResize:
			// Handle resize (store for future use)
			c.sendStatus("Resize acknowledged (not implemented in MVP-1)")

		case MsgTypePing:
			c.sendMessage(&WebSocketMessage{Type: MsgTypePong})

		default:
			c.sendError(fmt.Sprintf("Unknown message type: %s", msg.Type))
		}
	}
}

// writePump pumps messages from the console to the WebSocket connection.
func (c *WebSocketConnection) writePump() {
	ticker := time.NewTicker(pingPeriod)
	defer func() {
		ticker.Stop()
		c.Conn.Close()
	}()

	for {
		select {
		case message, ok := <-c.Client.Send:
			c.Conn.SetWriteDeadline(time.Now().Add(writeWait))
			if !ok {
				// Channel closed
				c.Conn.WriteMessage(websocket.CloseMessage, []byte{})
				return
			}

			// Send as binary message
			msg := WebSocketMessage{
				Type: MsgTypeOutput,
				Data: base64.StdEncoding.EncodeToString(message),
			}
			if err := c.Conn.WriteJSON(msg); err != nil {
				return
			}

		case <-ticker.C:
			c.Conn.SetWriteDeadline(time.Now().Add(writeWait))
			if err := c.Conn.WriteMessage(websocket.PingMessage, nil); err != nil {
				return
			}
		}
	}
}

// cleanup removes the connection from tracking and closes resources.
func (c *WebSocketConnection) cleanup() {
	c.mu.Lock()
	defer c.mu.Unlock()

	// Remove from server tracking
	c.Server.mu.Lock()
	delete(c.Server.connections, c.ID)
	c.Server.mu.Unlock()

	// Remove client from session
	if c.Client != nil && c.Client.Session != nil {
		c.Client.Session.RemoveClient(c.ID)
	}

	// Close WebSocket connection
	c.Conn.Close()
}

// sendMessage sends a message to the client.
func (c *WebSocketConnection) sendMessage(msg *WebSocketMessage) error {
	c.mu.Lock()
	defer c.mu.Unlock()

	c.Conn.SetWriteDeadline(time.Now().Add(writeWait))
	return c.Conn.WriteJSON(msg)
}

// sendError sends an error message to the client.
func (c *WebSocketConnection) sendError(message string) {
	c.sendMessage(&WebSocketMessage{
		Type: MsgTypeError,
		Data: base64.StdEncoding.EncodeToString([]byte(message)),
	})
}

// sendStatus sends a status message to the client.
func (c *WebSocketConnection) sendStatus(message string) {
	c.sendMessage(&WebSocketMessage{
		Type: MsgTypeStatus,
		Data: base64.StdEncoding.EncodeToString([]byte(message)),
	})
}

// GetConnectionCount returns the number of active WebSocket connections.
func (s *WebSocketServer) GetConnectionCount() int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return len(s.connections)
}

// GetConnectionCountForVM returns the number of connections for a specific VM.
func (s *WebSocketServer) GetConnectionCountForVM(vmID string) int {
	s.mu.RLock()
	defer s.mu.RUnlock()

	count := 0
	for _, conn := range s.connections {
		if conn.VMID == vmID {
			count++
		}
	}
	return count
}

// Close closes all WebSocket connections.
func (s *WebSocketServer) Close() error {
	s.mu.Lock()
	defer s.mu.Unlock()

	for _, conn := range s.connections {
		conn.Cancel()
		conn.Conn.Close()
	}
	s.connections = make(map[string]*WebSocketConnection)
	return nil
}

// ConsoleInfo represents console information for a VM.
type ConsoleInfo struct {
	VMID          string    `json:"vm_id"`
	Active        bool      `json:"active"`
	ClientCount   int       `json:"client_count"`
	LastActivity  time.Time `json:"last_activity"`
	LogPath       string    `json:"log_path"`
}

// GetConsoleInfo returns console information for all active sessions.
func (s *WebSocketServer) GetConsoleInfo() []ConsoleInfo {
	s.mu.RLock()
	defer s.mu.RUnlock()

	// Group by VM
	vmInfo := make(map[string]*ConsoleInfo)
	for _, conn := range s.connections {
		if info, ok := vmInfo[conn.VMID]; ok {
			info.ClientCount++
		} else {
			vmInfo[conn.VMID] = &ConsoleInfo{
				VMID:         conn.VMID,
				Active:       true,
				ClientCount:  1,
				LastActivity: time.Now(),
			}
		}
	}

	// Convert to slice
	result := make([]ConsoleInfo, 0, len(vmInfo))
	for _, info := range vmInfo {
		result = append(result, *info)
	}
	return result
}

// ServeHTTP implements http.Handler.
func (s *WebSocketServer) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	s.HandleWebSocket(w, r)
}

// JSONResponse writes a JSON response.
func JSONResponse(w http.ResponseWriter, status int, data interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(data)
}
