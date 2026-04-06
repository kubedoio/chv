// Package server provides the gRPC and HTTP server implementation for the CHV Agent.
package server

import (
	"context"
	"fmt"
	"log"
	"net/http"
	"path"
	"strings"
	"time"

	"github.com/chv/chv/internal/agent/console"
	"github.com/chv/chv/internal/validation"
)

// HTTPServer provides HTTP endpoints including WebSocket console access.
type HTTPServer struct {
	server         *http.Server
	consoleManager *console.Manager
	wsServer       *console.WebSocketServer
	listenAddr     string
}

// NewHTTPServer creates a new HTTP server.
func NewHTTPServer(listenAddr string, consoleManager *console.Manager) *HTTPServer {
	return NewHTTPServerWithAuth(listenAddr, consoleManager, nil)
}

// NewHTTPServerWithAuth creates a new HTTP server with custom auth function.
// If authFunc is nil, a default implementation that validates non-empty tokens is used.
func NewHTTPServerWithAuth(listenAddr string, consoleManager *console.Manager, authFunc console.AuthFunc) *HTTPServer {
	// Use provided auth function or create default
	if authFunc == nil {
		// Default auth: reject empty tokens
		// Production should use JWT validation against controller
		authFunc = func(token string, vmID string) (userID string, allowed bool, err error) {
			if token == "" {
				return "", false, fmt.Errorf("authentication required")
			}
			// TODO: Implement proper JWT validation
			// For now, just validate token format (non-empty, reasonable length)
			if len(token) < 10 || len(token) > 4096 {
				return "", false, fmt.Errorf("invalid token format")
			}
			return "user", true, nil
		}
	}

	wsServer := console.NewWebSocketServer(consoleManager, authFunc)

	mux := http.NewServeMux()

	// Health check endpoint
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"status":"ok"}`))
	})

	// Console WebSocket endpoint - path: /vms/{vm-id}/console
	mux.HandleFunc("/vms/", func(w http.ResponseWriter, r *http.Request) {
		// Parse and validate VM ID from path
		vmID, err := extractVMIDFromPath(r.URL.Path)
		if err != nil {
			http.Error(w, fmt.Sprintf("Invalid VM ID: %v", err), http.StatusBadRequest)
			return
		}

		// Add vm_id to query
		q := r.URL.Query()
		q.Set("vm_id", vmID)
		r.URL.RawQuery = q.Encode()

		wsServer.ServeHTTP(w, r)
	})

	// Console info endpoint
	mux.HandleFunc("/consoles", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		info := wsServer.GetConsoleInfo()
		w.Header().Set("Content-Type", "application/json")
		console.JSONResponse(w, http.StatusOK, info)
	})

	server := &http.Server{
		Addr:         listenAddr,
		Handler:      mux,
		ReadTimeout:  30 * time.Second,
		WriteTimeout: 30 * time.Second,
		IdleTimeout:  120 * time.Second,
	}

	return &HTTPServer{
		server:         server,
		consoleManager: consoleManager,
		wsServer:       wsServer,
		listenAddr:     listenAddr,
	}
}

// Start starts the HTTP server in a goroutine.
func (s *HTTPServer) Start() error {
	go func() {
		log.Printf("Starting CHV Agent HTTP server on %s", s.listenAddr)
		if err := s.server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatalf("HTTP server error: %v", err)
		}
	}()
	return nil
}

// Stop gracefully stops the HTTP server.
func (s *HTTPServer) Stop(ctx context.Context) error {
	if s.wsServer != nil {
		s.wsServer.Close()
	}
	return s.server.Shutdown(ctx)
}

// extractVMIDFromPath extracts and validates the VM ID from URL path.
// Expected format: /vms/{vm-id}/console or /vms/{vm-id}/
func extractVMIDFromPath(urlPath string) (string, error) {
	// Clean the path to remove any . or .. components
	cleanPath := path.Clean(urlPath)
	
	// Path must start with /vms/
	const prefix = "/vms/"
	if !strings.HasPrefix(cleanPath, prefix) {
		return "", fmt.Errorf("path must start with %s", prefix)
	}
	
	// Extract the part after /vms/
	afterPrefix := cleanPath[len(prefix):]
	
	// Remove trailing /console if present
	const consoleSuffix = "/console"
	if strings.HasSuffix(afterPrefix, consoleSuffix) {
		afterPrefix = afterPrefix[:len(afterPrefix)-len(consoleSuffix)]
	}
	
	// Remove any trailing slashes
	afterPrefix = strings.TrimSuffix(afterPrefix, "/")
	
	// Validate the VM ID
	if err := validation.ValidateID(afterPrefix); err != nil {
		return "", err
	}
	
	return afterPrefix, nil
}

// GetWebSocketServer returns the WebSocket server for testing.
func (s *HTTPServer) GetWebSocketServer() *console.WebSocketServer {
	return s.wsServer
}
