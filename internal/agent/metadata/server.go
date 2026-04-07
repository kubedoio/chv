// Package metadata provides a cloud-init metadata service for VMs.
//
// This package implements a metadata service similar to AWS/Azure/GCP IMDS
// (Instance Metadata Service). VMs can fetch their cloud-init configuration
// over HTTP from a link-local address (169.254.169.254).
//
// # Endpoints
//
// The metadata service provides the following endpoints:
//
//   /latest/meta-data/           - Instance metadata (instance-id, hostname)
//   /latest/user-data            - Cloud-init user-data
//   /latest/network-config       - Network configuration (v2 JSON)
//
// # VM Identification
//
// VMs are identified by their source IP address. The metadata server
// maps the VM's IP address to its VM ID and returns the appropriate
// configuration.
//
// Example usage:
//
//	server := metadata.NewServer()
//	if err := server.Start(); err != nil {
//	    log.Fatalf("Failed to start metadata server: %v", err)
//	}
//	defer server.Stop()
//
//	// Register a VM with its configuration
//	server.RegisterVM("vm-123", &metadata.Config{
//	    InstanceID:    "vm-123",
//	    Hostname:      "myvm",
//	    NetworkConfig: `{"version": 2, "ethernets": {...}}`,
//	    UserData:      "#cloud-config\nusers:\n  - name: admin\n",
//	})
//
//	// When VM is deleted, unregister it
//	server.UnregisterVM("vm-123")
//
// # Security Considerations
//
// The metadata service listens on a link-local address which is only
// accessible from the host and VMs running on that host. Future
// enhancements may include IMDSv2-style session authentication.
package metadata

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net"
	"net/http"
	"strings"
	"sync"
	"time"
)

// Config represents cloud-init configuration for a VM.
type Config struct {
	InstanceID    string
	Hostname      string
	NetworkConfig string // v2 JSON
	UserData      string
	MetaData      string
}

// Server serves cloud-init metadata over HTTP.
type Server struct {
	listener   net.Listener
	configs    map[string]*Config // VM ID -> Config
	ipToVMID   map[string]string  // IP address -> VM ID
	mu         sync.RWMutex
	httpServer *http.Server
	addr       string
}

// NewServer creates a new metadata server.
func NewServer() *Server {
	return &Server{
		configs:  make(map[string]*Config),
		ipToVMID: make(map[string]string),
	}
}

// NewServerWithAddress creates a new metadata server that will listen on a specific address.
// Use ":0" to let the system assign a random available port (useful for testing).
func NewServerWithAddress(addr string) *Server {
	return &Server{
		configs:  make(map[string]*Config),
		ipToVMID: make(map[string]string),
		addr:     addr,
	}
}

// Start starts the metadata server.
// If a specific address was provided via NewServerWithAddress, it will use that.
// Otherwise, it tries to listen on the standard link-local address 169.254.169.254:80
// and falls back to :8080 if it cannot bind to port 80.
func (s *Server) Start() error {
	var ln net.Listener
	var err error

	if s.addr != "" {
		// Use the configured address
		ln, err = net.Listen("tcp", s.addr)
		if err != nil {
			return fmt.Errorf("failed to start metadata server on %s: %w", s.addr, err)
		}
	} else {
		// Try to listen on the standard link-local address
		ln, err = net.Listen("tcp", "169.254.169.254:80")
		if err != nil {
			// Fallback to port 8080 if can't bind to 80 (permission issues or address in use)
			log.Printf("Could not bind to 169.254.169.254:80 (%v), falling back to :8080", err)
			ln, err = net.Listen("tcp", ":8080")
			if err != nil {
				return fmt.Errorf("failed to start metadata server: %w", err)
			}
		}
	}

	s.listener = ln
	s.addr = ln.Addr().String()

	// Create HTTP mux with handlers
	mux := http.NewServeMux()
	mux.HandleFunc("/latest/meta-data/", s.handleMetaData)
	mux.HandleFunc("/latest/meta-data", s.handleMetaData) // Without trailing slash
	mux.HandleFunc("/latest/user-data", s.handleUserData)
	mux.HandleFunc("/latest/network-config", s.handleNetworkConfig)
	mux.HandleFunc("/", s.handleRoot)

	s.httpServer = &http.Server{
		Handler:      mux,
		ReadTimeout:  5 * time.Second,
		WriteTimeout: 5 * time.Second,
		IdleTimeout:  60 * time.Second,
	}

	go func() {
		if err := s.httpServer.Serve(ln); err != nil && err != http.ErrServerClosed {
			log.Printf("Metadata server error: %v", err)
		}
	}()

	log.Printf("Metadata server started on %s", s.addr)
	return nil
}

// Stop gracefully stops the metadata server.
func (s *Server) Stop() error {
	if s.httpServer == nil {
		return nil
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	return s.httpServer.Shutdown(ctx)
}

// Addr returns the address the server is listening on.
func (s *Server) Addr() string {
	return s.addr
}

// RegisterVM registers a VM with its metadata configuration.
// If the VM is already registered, its configuration will be updated.
func (s *Server) RegisterVM(vmID string, config *Config) {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Validate required fields
	if config.InstanceID == "" {
		config.InstanceID = vmID
	}
	if config.Hostname == "" {
		config.Hostname = vmID
	}
	// Generate metadata if not provided
	if config.MetaData == "" {
		config.MetaData = fmt.Sprintf("instance-id: %s\nlocal-hostname: %s\n", config.InstanceID, config.Hostname)
	}

	s.configs[vmID] = config
	log.Printf("Registered VM %s with metadata server", vmID)
}

// RegisterVMWithIP registers a VM and maps its IP address to the VM ID.
// This allows the server to identify VMs by their source IP.
func (s *Server) RegisterVMWithIP(vmID string, ip string, config *Config) {
	s.RegisterVM(vmID, config)

	s.mu.Lock()
	defer s.mu.Unlock()

	// Store IP to VMID mapping
	s.ipToVMID[ip] = vmID
	log.Printf("Registered VM %s with IP %s", vmID, ip)
}

// UnregisterVM removes a VM's metadata from the server.
// This should be called when the VM is deleted.
func (s *Server) UnregisterVM(vmID string) {
	s.mu.Lock()
	defer s.mu.Unlock()

	delete(s.configs, vmID)

	// Remove IP mappings for this VM
	for ip, id := range s.ipToVMID {
		if id == vmID {
			delete(s.ipToVMID, ip)
		}
	}

	log.Printf("Unregistered VM %s from metadata server", vmID)
}

// GetConfig returns a copy of a VM's configuration.
func (s *Server) GetConfig(vmID string) (*Config, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	config, ok := s.configs[vmID]
	if !ok {
		return nil, false
	}

	// Return a copy to prevent external modification
	configCopy := *config
	return &configCopy, true
}

// handleRoot handles requests to the root path.
func (s *Server) handleRoot(w http.ResponseWriter, r *http.Request) {
	if r.URL.Path == "/" {
		// Return a simple index
		w.Header().Set("Content-Type", "text/plain")
		w.Write([]byte(`cloud-init metadata service

Available endpoints:
  /latest/meta-data/      - Instance metadata
  /latest/user-data       - Cloud-init user-data
  /latest/network-config  - Network configuration (v2)
`))
		return
	}

	http.NotFound(w, r)
}

// handleMetaData handles requests for instance metadata.
func (s *Server) handleMetaData(w http.ResponseWriter, r *http.Request) {
	vmID := s.extractVMID(r)
	if vmID == "" {
		http.Error(w, "Could not identify VM", http.StatusNotFound)
		return
	}

	s.mu.RLock()
	config, ok := s.configs[vmID]
	s.mu.RUnlock()

	if !ok {
		http.Error(w, "VM not found", http.StatusNotFound)
		return
	}

	// Parse the path to determine which metadata to return
	path := strings.TrimPrefix(r.URL.Path, "/latest/meta-data")
	path = strings.TrimPrefix(path, "/")

	switch path {
	case "", "/":
		// Return list of available metadata
		w.Header().Set("Content-Type", "text/plain")
		w.Write([]byte("instance-id\nlocal-hostname\n"))
	case "instance-id":
		w.Header().Set("Content-Type", "text/plain")
		w.Write([]byte(config.InstanceID))
	case "local-hostname", "hostname":
		w.Header().Set("Content-Type", "text/plain")
		w.Write([]byte(config.Hostname))
	default:
		http.NotFound(w, r)
	}
}

// handleUserData handles requests for cloud-init user-data.
func (s *Server) handleUserData(w http.ResponseWriter, r *http.Request) {
	vmID := s.extractVMID(r)
	if vmID == "" {
		http.Error(w, "Could not identify VM", http.StatusNotFound)
		return
	}

	s.mu.RLock()
	config, ok := s.configs[vmID]
	s.mu.RUnlock()

	if !ok {
		http.Error(w, "VM not found", http.StatusNotFound)
		return
	}

	w.Header().Set("Content-Type", "text/plain")
	if config.UserData == "" {
		w.Write([]byte("#cloud-config\n{}\n"))
	} else {
		w.Write([]byte(config.UserData))
	}
}

// handleNetworkConfig handles requests for network configuration.
func (s *Server) handleNetworkConfig(w http.ResponseWriter, r *http.Request) {
	vmID := s.extractVMID(r)
	if vmID == "" {
		http.Error(w, "Could not identify VM", http.StatusNotFound)
		return
	}

	s.mu.RLock()
	config, ok := s.configs[vmID]
	s.mu.RUnlock()

	if !ok {
		http.Error(w, "VM not found", http.StatusNotFound)
		return
	}

	// Return network config or empty config if not set
	w.Header().Set("Content-Type", "application/json")
	if config.NetworkConfig == "" {
		// Return empty network config
		w.Write([]byte(`{"version": 2}`))
	} else {
		// Validate JSON before returning
		var jsonCheck map[string]interface{}
		if err := json.Unmarshal([]byte(config.NetworkConfig), &jsonCheck); err != nil {
			// Invalid JSON, return error
			http.Error(w, "Invalid network configuration", http.StatusInternalServerError)
			return
		}
		w.Write([]byte(config.NetworkConfig))
	}
}

// extractVMID extracts the VM ID from the request.
// First tries to extract from a header, then falls back to IP-based lookup.
func (s *Server) extractVMID(r *http.Request) string {
	// First, try to get VM ID from header (for testing or explicit identification)
	vmID := r.Header.Get("X-VM-ID")
	if vmID != "" {
		// Validate that this VM exists
		s.mu.RLock()
		_, ok := s.configs[vmID]
		s.mu.RUnlock()
		if ok {
			return vmID
		}
	}

	// Try to identify by source IP
	host, _, err := net.SplitHostPort(r.RemoteAddr)
	if err != nil {
		// If SplitHostPort fails, the address might not have a port
		host = r.RemoteAddr
	}

	s.mu.RLock()
	vmID, ok := s.ipToVMID[host]
	s.mu.RUnlock()
	if ok {
		return vmID
	}

	// As a fallback for development/testing, check for X-Forwarded-For
	// This should NOT be used in production as it can be spoofed
	forwardedFor := r.Header.Get("X-Forwarded-For")
	if forwardedFor != "" {
		// Take the first IP if multiple are present
		ips := strings.Split(forwardedFor, ",")
		if len(ips) > 0 {
			ip := strings.TrimSpace(ips[0])
			s.mu.RLock()
			vmID, ok := s.ipToVMID[ip]
			s.mu.RUnlock()
			if ok {
				return vmID
			}
		}
	}

	return ""
}

// UpdateVMIP updates the IP address mapping for a VM.
// This should be called when a VM's IP address changes.
func (s *Server) UpdateVMIP(vmID string, newIP string) {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Remove old IP mappings for this VM
	for ip, id := range s.ipToVMID {
		if id == vmID {
			delete(s.ipToVMID, ip)
		}
	}

	// Add new mapping
	s.ipToVMID[newIP] = vmID
}
