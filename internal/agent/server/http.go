// Package server provides the gRPC and HTTP server implementation for the CHV Agent.
package server

import (
	"context"
	"log"
	"net/http"
	"time"

	"github.com/chv/chv/internal/agent/console"
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
	// Create a simple auth function that allows all requests for MVP-1
	// In production, this should validate tokens against the controller
	authFunc := func(token string, vmID string) (userID string, allowed bool, err error) {
		// MVP-1: Single tenant, allow all valid-looking tokens
		if token == "" {
			return "", false, nil
		}
		return "user", true, nil
	}

	wsServer := console.NewWebSocketServer(consoleManager, authFunc)

	mux := http.NewServeMux()

	// Health check endpoint
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"status":"ok"}`))
	})

	// Console WebSocket endpoint
	mux.HandleFunc("/vms/", func(w http.ResponseWriter, r *http.Request) {
		// Extract VM ID from path /vms/{vm-id}/console
		vmID := r.URL.Path[len("/vms/"):]
		if len(vmID) > 8 && vmID[len(vmID)-8:] == "/console" {
			vmID = vmID[:len(vmID)-8]
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

// GetWebSocketServer returns the WebSocket server for testing.
func (s *HTTPServer) GetWebSocketServer() *console.WebSocketServer {
	return s.wsServer
}
