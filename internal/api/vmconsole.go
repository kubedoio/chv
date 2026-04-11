package api

import (
	"log/slog"
	"net/http"
	"os"
	"strings"

	"github.com/gorilla/websocket"
)

// VMConsoleWebSocket proxies WebSocket connections to the agent
func (h *Handler) vmConsoleWebSocket(w http.ResponseWriter, r *http.Request) {
	slog.Info("WebSocket console request", "remote", r.RemoteAddr, "url", r.URL.String())
	
	// Authenticate via query parameter (browsers can't send custom headers for WebSocket)
	token := r.URL.Query().Get("token")
	if token == "" {
		// Try Authorization header as fallback
		authHeader := r.Header.Get("Authorization")
		if len(authHeader) > 7 && strings.ToLower(authHeader[:7]) == "bearer " {
			token = authHeader[7:]
		}
	}
	if token == "" {
		slog.Warn("WebSocket: no token provided", "remote", r.RemoteAddr)
		http.Error(w, "token required", http.StatusUnauthorized)
		return
	}
	if _, err := h.auth.ValidateToken(r.Context(), "Bearer "+token); err != nil {
		slog.Warn("WebSocket: invalid token", "remote", r.RemoteAddr, "error", err)
		http.Error(w, "invalid token", http.StatusUnauthorized)
		return
	}

	vmID := r.URL.Query().Get("vm_id")
	apiSocket := r.URL.Query().Get("api_socket")

	if vmID == "" || apiSocket == "" {
		slog.Warn("WebSocket: missing params", "vm_id", vmID, "api_socket", apiSocket)
		http.Error(w, "vm_id and api_socket query params required", http.StatusBadRequest)
		return
	}

	// Get agent URL from config or use default
	agentURL := h.config.AgentURL
	if agentURL == "" {
		// Default to localhost:9090 (agent runs on same machine as controller)
		agentURL = "ws://localhost:9090"
	}

	// Convert http/https to ws/wss if needed
	if strings.HasPrefix(agentURL, "http://") {
		agentURL = "ws://" + strings.TrimPrefix(agentURL, "http://")
	} else if strings.HasPrefix(agentURL, "https://") {
		agentURL = "wss://" + strings.TrimPrefix(agentURL, "https://")
	} else if !strings.HasPrefix(agentURL, "ws://") && !strings.HasPrefix(agentURL, "wss://") {
		agentURL = "ws://" + agentURL
	}

	// Build target URL
	targetURL := agentURL + "/v1/vms/console?vm_id=" + vmID + "&api_socket=" + apiSocket
	slog.Info("WebSocket: connecting to agent", "target", targetURL, "vm_id", vmID)

	// Upgrade the HTTP connection to a WebSocket
	upgrader := websocket.Upgrader{
		ReadBufferSize:  1024,
		WriteBufferSize: 1024,
		CheckOrigin: func(r *http.Request) bool {
			// Allow same-origin requests
			origin := r.Header.Get("Origin")
			if origin == "" {
				return true
			}
			// Parse allowed origins from env var
			allowedOrigins := parseAllowedOrigins()
			for _, allowed := range allowedOrigins {
				if origin == allowed {
					return true
				}
			}
			// Also allow if origin matches the request host (same-origin)
			if origin == "http://"+r.Host || origin == "https://"+r.Host {
				return true
			}
			slog.Warn("WebSocket: rejected cross-origin request", "origin", origin, "host", r.Host)
			return false
		},
	}

	clientConn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		slog.Error("WebSocket: upgrade failed", "error", err)
		return
	}
	defer clientConn.Close()
	slog.Info("WebSocket: client connected", "vm_id", vmID)

	// Connect to agent WebSocket with auth token
	var wsHeaders http.Header
	if h.config.AgentToken != "" {
		wsHeaders = http.Header{
			"Authorization": []string{"Bearer " + h.config.AgentToken},
		}
	}
	agentConn, _, err := websocket.DefaultDialer.Dial(targetURL, wsHeaders)
	if err != nil {
		slog.Error("WebSocket: failed to connect to agent", "target", targetURL, "error", err)
		clientConn.WriteMessage(websocket.TextMessage, []byte("Failed to connect to agent: "+err.Error()))
		return
	}
	defer agentConn.Close()
	slog.Info("WebSocket: connected to agent", "vm_id", vmID)

	// Proxy bidirectionally
	errChan := make(chan error, 2)

	// Client -> Agent
	go func() {
		for {
			msgType, data, err := clientConn.ReadMessage()
			if err != nil {
				errChan <- err
				return
			}
			if err := agentConn.WriteMessage(msgType, data); err != nil {
				errChan <- err
				return
			}
		}
	}()

	// Agent -> Client
	go func() {
		for {
			msgType, data, err := agentConn.ReadMessage()
			if err != nil {
				errChan <- err
				return
			}
			if err := clientConn.WriteMessage(msgType, data); err != nil {
				errChan <- err
				return
			}
		}
	}()

	// Wait for either direction to close
	<-errChan
}

// parseAllowedOrigins reads CHV_CORS_ORIGINS env var and returns allowed origins for WebSocket
func parseAllowedOrigins() []string {
	var origins []string
	if env := os.Getenv("CHV_CORS_ORIGINS"); env != "" {
		for _, o := range strings.Split(env, ",") {
			o = strings.TrimSpace(o)
			if o != "" {
				origins = append(origins, o)
			}
		}
	}
	return origins
}
