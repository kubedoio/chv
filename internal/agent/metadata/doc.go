// Package metadata provides a cloud-init metadata service for VMs.
//
// The metadata service implements an HTTP-based alternative to cloud-init
// ISO configuration. It follows the API pattern used by major cloud providers
// (AWS/Azure/GCP Instance Metadata Service).
//
// # Quick Start
//
//	server := metadata.NewServer()
//	if err := server.Start(); err != nil {
//	    log.Fatalf("Failed to start: %v", err)
//	}
//	defer server.Stop()
//
//	// Register a VM
//	server.RegisterVM("vm-123", &metadata.Config{
//	    InstanceID:    "vm-123",
//	    Hostname:      "my-vm",
//	    NetworkConfig: `{"version": 2, "ethernets": {...}}`,
//	    UserData:      "#cloud-config\n...",
//	})
//
//	// When VM is deleted, unregister it
//	server.UnregisterVM("vm-123")
//
// # API Endpoints
//
// The metadata service provides these endpoints:
//
//   - GET /latest/meta-data/      - List available metadata
//   - GET /latest/meta-data/*     - Get specific metadata item
//   - GET /latest/user-data       - Cloud-init user-data
//   - GET /latest/network-config  - Network configuration (v2 JSON)
//
// # VM Identification
//
// VMs are identified by:
//   1. X-VM-ID header (for testing)
//   2. Source IP address mapping
//
// # Network Address
//
// By default, the server tries to listen on the standard link-local address
// 169.254.169.254:80. If that fails (permission denied or address unavailable),
// it falls back to :8080.
//
// For testing, use NewServerWithAddress(":0") to get a random port.
//
package metadata
