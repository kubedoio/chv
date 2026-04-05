package api

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"

	"github.com/chv/chv/internal/hypervisor"
	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/pkg/uuidx"
	"github.com/go-chi/chi/v5"
	"github.com/google/uuid"
	"github.com/gorilla/websocket"
)

var upgrader = websocket.Upgrader{
	ReadBufferSize:  1024,
	WriteBufferSize: 1024,
	CheckOrigin: func(r *http.Request) bool {
		// Allow all origins (CORS middleware handles this)
		return true
	},
}

// ConsoleMessage represents a console message
// Types: "input", "output", "error", "status"
type ConsoleMessage struct {
	Type string `json:"type"`
	Data string `json:"data"` // base64-encoded for binary data
}

// vmConsole handles WebSocket connections for VM console access
func (h *Handler) vmConsole(w http.ResponseWriter, r *http.Request) {
	// Get VM ID from URL
	id := chi.URLParam(r, "id")
	vmID, err := uuidx.Parse(id)
	if err != nil {
		http.Error(w, "Invalid VM ID", http.StatusBadRequest)
		return
	}

	// Validate VM ID is safe for path usage
	if err := uuidx.ValidateSafeForPath(id); err != nil {
		http.Error(w, "Invalid VM ID format", http.StatusBadRequest)
		return
	}

	// Get token from query parameter (WebSocket connections don't use Authorization header)
	token := r.URL.Query().Get("token")
	if token == "" {
		http.Error(w, "Missing token", http.StatusUnauthorized)
		return
	}

	// Validate token and get user info
	tokenModel, err := h.auth.ValidateToken(r.Context(), token)
	if err != nil {
		http.Error(w, "Invalid or expired token", http.StatusUnauthorized)
		return
	}

	// Get user ID from token
	userID := tokenModel.ID.String()

	// Check if VM exists
	vm, err := h.store.GetVM(r.Context(), vmID)
	if err != nil {
		http.Error(w, "Failed to get VM", http.StatusInternalServerError)
		return
	}
	if vm == nil {
		http.Error(w, "VM not found", http.StatusNotFound)
		return
	}

	// Check if VM is running (console only available for running VMs)
	if vm.ActualState != models.VMActualStateRunning {
		http.Error(w, "VM must be running to access console", http.StatusBadRequest)
		return
	}

	// Rate limiting: max 1 connection per VM per user
	if h.consoleSessions.HasSession(vmID.String(), userID) {
		http.Error(w, "Console session already active for this VM", http.StatusTooManyRequests)
		return
	}

	// Acquire session slot
	if err := h.consoleSessions.AcquireSession(vmID.String(), userID); err != nil {
		http.Error(w, err.Error(), http.StatusTooManyRequests)
		return
	}
	// Note: We don't release the slot here; it will be released when the connection closes

	// Create operation record for audit
	op, _ := h.operations.Start(r.Context(), models.OpVMConsole, models.OpCategorySync,
		"vm", &vmID, models.ActorTypeUser, userID, map[string]string{
			"vm_name": vm.Name,
			"vm_id":   vmID.String(),
		})

	// Upgrade to WebSocket
	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		h.consoleSessions.ReleaseSession(vmID.String(), userID)
		h.operations.Fail(r.Context(), op.ID, fmt.Errorf("websocket upgrade failed: %w", err))
		return
	}

	// Register the session
	session := &hypervisor.ConsoleSession{
		VMID:      vmID.String(),
		UserID:    userID,
		StartedAt: time.Now(),
		Conn:      conn,
	}
	h.consoleSessions.RegisterSession(session)

	// Handle the WebSocket connection
	h.handleConsoleConnection(r.Context(), conn, vmID.String(), userID, op.ID)
}

// handleConsoleConnection handles the bidirectional WebSocket console connection
func (h *Handler) handleConsoleConnection(ctx context.Context, conn *websocket.Conn, vmID, userID string, opID uuid.UUID) {
	defer func() {
		conn.Close()
		h.consoleSessions.ReleaseSession(vmID, userID)
		// Complete the operation
		h.operations.Complete(ctx, opID, map[string]string{
			"vm_id":     vmID,
			"user_id":   userID,
			"status":    "disconnected",
			"ended_at":  time.Now().Format(time.RFC3339),
		})
	}()

	// Set up ping/pong to keep connection alive
	conn.SetReadDeadline(time.Now().Add(60 * time.Second))
	conn.SetPongHandler(func(string) error {
		conn.SetReadDeadline(time.Now().Add(60 * time.Second))
		return nil
	})

	// Start ping ticker
	ticker := time.NewTicker(30 * time.Second)
	defer ticker.Stop()

	// Channel to signal when to stop
	done := make(chan struct{})
	defer close(done)

	// Start ping goroutine
	go func() {
		for {
			select {
			case <-ticker.C:
				if err := conn.WriteMessage(websocket.PingMessage, nil); err != nil {
					return
				}
			case <-done:
				return
			}
		}
	}()

	// Create a proxy to the cloud-hypervisor serial console
	// The socket path is typically /var/lib/chv/sockets/{vm-id}.sock
	apiSocketPath := fmt.Sprintf("/var/lib/chv/sockets/%s.sock", vmID)
	proxy := hypervisor.NewConsoleProxy(apiSocketPath)

	// Check if console is available
	ctxTimeout, cancel := context.WithTimeout(ctx, 5*time.Second)
	if !proxy.IsAvailable(ctxTimeout) {
		cancel()
		sendConsoleError(conn, "Console not available - VM API socket not accessible")
		return
	}
	cancel()

	// Create pipes for bidirectional communication
	// We need to convert between WebSocket messages and raw bytes
	consoleInputReader, consoleInputWriter := io.Pipe()
	consoleOutputReader, consoleOutputWriter := io.Pipe()

	// Start streaming in background
	streamErr := make(chan error, 1)
	go func() {
		streamCtx, streamCancel := context.WithCancel(ctx)
		defer streamCancel()
		streamErr <- proxy.StreamConsole(streamCtx, consoleOutputWriter, consoleInputReader)
	}()

	// Handle console output (read from VM, write to WebSocket)
	go func() {
		buf := make([]byte, 4096)
		for {
			n, err := consoleOutputReader.Read(buf)
			if err != nil {
				if err != io.EOF {
					sendConsoleError(conn, fmt.Sprintf("Console read error: %v", err))
				}
				return
			}
			if n > 0 {
				msg := ConsoleMessage{
					Type: "output",
					Data: base64.StdEncoding.EncodeToString(buf[:n]),
				}
				if err := conn.WriteJSON(msg); err != nil {
					return
				}
			}
		}
	}()

	// Handle WebSocket messages (read from client, write to VM)
	for {
		conn.SetReadDeadline(time.Now().Add(60 * time.Second))
		
		var msg ConsoleMessage
		if err := conn.ReadJSON(&msg); err != nil {
			if websocket.IsUnexpectedCloseError(err, websocket.CloseGoingAway, websocket.CloseAbnormalClosure) {
				// Log unexpected closure
			}
			return
		}

		switch msg.Type {
		case "input":
			// Decode base64 data and write to console input
			data, err := base64.StdEncoding.DecodeString(msg.Data)
			if err != nil {
				sendConsoleError(conn, fmt.Sprintf("Invalid input data: %v", err))
				continue
			}
			if _, err := consoleInputWriter.Write(data); err != nil {
				sendConsoleError(conn, fmt.Sprintf("Console write error: %v", err))
				return
			}

		case "resize":
			// Resize is not supported in MVP-1
			// Send status message
			sendConsoleStatus(conn, "Resize not supported in MVP-1")

		case "ping":
			// Client ping, send pong
			sendConsoleStatus(conn, "pong")

		default:
			sendConsoleError(conn, fmt.Sprintf("Unknown message type: %s", msg.Type))
		}
	}
}

// sendConsoleError sends an error message to the client
func sendConsoleError(conn *websocket.Conn, message string) {
	msg := ConsoleMessage{
		Type: "error",
		Data: base64.StdEncoding.EncodeToString([]byte(message)),
	}
	conn.WriteJSON(msg)
}

// sendConsoleStatus sends a status message to the client
func sendConsoleStatus(conn *websocket.Conn, message string) {
	msg := ConsoleMessage{
		Type: "status",
		Data: base64.StdEncoding.EncodeToString([]byte(message)),
	}
	conn.WriteJSON(msg)
}

// writeConsoleMessage writes a raw message to the console connection
func writeConsoleMessage(w io.Writer, msgType string, data []byte) error {
	msg := ConsoleMessage{
		Type: msgType,
		Data: base64.StdEncoding.EncodeToString(data),
	}
	return json.NewEncoder(w).Encode(msg)
}
