package networking

import (
	"context"
	"database/sql"
	"fmt"
	"net"
	"sync"
	"time"

	"github.com/google/uuid"
)

// DHCPServer represents a DHCP server configuration for a network
type DHCPServer struct {
	ID         string        `json:"id"`
	NetworkID  string        `json:"network_id"`
	RangeStart string        `json:"range_start"` // e.g., 10.0.0.100
	RangeEnd   string        `json:"range_end"`   // e.g., 10.0.0.200
	LeaseTime  time.Duration `json:"lease_time"`
	IsRunning  bool          `json:"is_running"`
	CreatedAt  string        `json:"created_at,omitempty"`
	UpdatedAt  string        `json:"updated_at,omitempty"`
}

// DHCPLease represents an active DHCP lease
type DHCPLease struct {
	ID         string `json:"id"`
	NetworkID  string `json:"network_id"`
	MACAddress string `json:"mac_address"`
	IPAddress  string `json:"ip_address"`
	Hostname   string `json:"hostname,omitempty"`
	LeaseStart string `json:"lease_start"`
	LeaseEnd   string `json:"lease_end"`
}

// Validate validates the DHCP server configuration
func (d *DHCPServer) Validate() error {
	if d.NetworkID == "" {
		return fmt.Errorf("network_id is required")
	}
	if d.RangeStart == "" || d.RangeEnd == "" {
		return fmt.Errorf("range_start and range_end are required")
	}
	
	// Validate IP addresses
	startIP := net.ParseIP(d.RangeStart)
	if startIP == nil {
		return fmt.Errorf("invalid range_start IP address")
	}
	endIP := net.ParseIP(d.RangeEnd)
	if endIP == nil {
		return fmt.Errorf("invalid range_end IP address")
	}
	
	// Check that start is before end
	if compareIP(startIP, endIP) >= 0 {
		return fmt.Errorf("range_start must be before range_end")
	}
	
	if d.LeaseTime == 0 {
		d.LeaseTime = 1 * time.Hour // Default 1 hour
	}
	
	return nil
}

// compareIP compares two IP addresses
// Returns -1 if a < b, 0 if a == b, 1 if a > b
func compareIP(a, b net.IP) int {
	a = a.To4()
	b = b.To4()
	if a == nil || b == nil {
		return 0
	}
	for i := 0; i < 4; i++ {
		if a[i] < b[i] {
			return -1
		}
		if a[i] > b[i] {
			return 1
		}
	}
	return 0
}

// DHCPRepository defines the interface for DHCP database operations
type DHCPRepository interface {
	CreateDHCPServer(ctx context.Context, server *DHCPServer) error
	GetDHCPServerByNetwork(ctx context.Context, networkID string) (*DHCPServer, error)
	UpdateDHCPServer(ctx context.Context, server *DHCPServer) error
	DeleteDHCPServer(ctx context.Context, networkID string) error
	
	CreateDHCPLease(ctx context.Context, lease *DHCPLease) error
	GetDHCPLeaseByMAC(ctx context.Context, networkID, macAddress string) (*DHCPLease, error)
	GetDHCPLeaseByIP(ctx context.Context, networkID, ipAddress string) (*DHCPLease, error)
	ListDHCPLeases(ctx context.Context, networkID string) ([]DHCPLease, error)
	DeleteDHCPLease(ctx context.Context, id string) error
	UpdateDHCPLease(ctx context.Context, lease *DHCPLease) error
}

// DHCPServerManager manages DHCP servers
type DHCPServerManager struct {
	repo       DHCPRepository
	activeServers map[string]*dhcpServerInstance
	mu         sync.RWMutex
}

type dhcpServerInstance struct {
	server   *DHCPServer
	stopChan chan struct{}
}

// NewDHCPServerManager creates a new DHCP server manager
func NewDHCPServerManager(repo DHCPRepository) *DHCPServerManager {
	return &DHCPServerManager{
		repo:          repo,
		activeServers: make(map[string]*dhcpServerInstance),
	}
}

// StartDHCPServer starts the DHCP server for a network
func (m *DHCPServerManager) StartDHCPServer(ctx context.Context, networkID string) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	// Check if already running
	if _, exists := m.activeServers[networkID]; exists {
		return fmt.Errorf("DHCP server already running for network %s", networkID)
	}

	// Get the server configuration
	server, err := m.repo.GetDHCPServerByNetwork(ctx, networkID)
	if err != nil {
		return fmt.Errorf("failed to get DHCP server config: %w", err)
	}
	if server == nil {
		return fmt.Errorf("DHCP server not configured for network %s", networkID)
	}

	// Mark as running
	server.IsRunning = true
	server.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	if err := m.repo.UpdateDHCPServer(ctx, server); err != nil {
		return fmt.Errorf("failed to update DHCP server status: %w", err)
	}

	// Start the server instance (simulated)
	stopChan := make(chan struct{})
	m.activeServers[networkID] = &dhcpServerInstance{
		server:   server,
		stopChan: stopChan,
	}

	// Start a goroutine to manage lease expiration
	go m.manageLeases(networkID, stopChan)

	return nil
}

// StopDHCPServer stops the DHCP server for a network
func (m *DHCPServerManager) StopDHCPServer(ctx context.Context, networkID string) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	instance, exists := m.activeServers[networkID]
	if !exists {
		return fmt.Errorf("DHCP server not running for network %s", networkID)
	}

	// Signal stop
	close(instance.stopChan)
	delete(m.activeServers, networkID)

	// Update status in database
	server, err := m.repo.GetDHCPServerByNetwork(ctx, networkID)
	if err != nil {
		return fmt.Errorf("failed to get DHCP server: %w", err)
	}
	if server != nil {
		server.IsRunning = false
		server.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
		if err := m.repo.UpdateDHCPServer(ctx, server); err != nil {
			return fmt.Errorf("failed to update DHCP server status: %w", err)
		}
	}

	return nil
}

// GetLeases returns all active DHCP leases for a network
func (m *DHCPServerManager) GetLeases(ctx context.Context, networkID string) ([]DHCPLease, error) {
	return m.repo.ListDHCPLeases(ctx, networkID)
}

// ConfigureDHCPServer creates or updates DHCP server configuration
func (m *DHCPServerManager) ConfigureDHCPServer(ctx context.Context, server *DHCPServer) error {
	if err := server.Validate(); err != nil {
		return err
	}

	// Check if already exists
	existing, err := m.repo.GetDHCPServerByNetwork(ctx, server.NetworkID)
	if err != nil {
		return fmt.Errorf("failed to check existing server: %w", err)
	}

	now := time.Now().UTC().Format(time.RFC3339)
	if existing == nil {
		// Create new
		server.ID = uuid.NewString()
		server.IsRunning = false
		server.CreatedAt = now
		server.UpdatedAt = now
		return m.repo.CreateDHCPServer(ctx, server)
	}

	// Update existing
	existing.RangeStart = server.RangeStart
	existing.RangeEnd = server.RangeEnd
	existing.LeaseTime = server.LeaseTime
	existing.UpdatedAt = now
	return m.repo.UpdateDHCPServer(ctx, existing)
}

// RequestLease requests a new DHCP lease for a MAC address
func (m *DHCPServerManager) RequestLease(ctx context.Context, networkID, macAddress, hostname string) (*DHCPLease, error) {
	// Check if server is running
	m.mu.RLock()
	_, running := m.activeServers[networkID]
	m.mu.RUnlock()
	
	if !running {
		return nil, fmt.Errorf("DHCP server is not running for this network")
	}

	// Check if lease already exists
	existingLease, err := m.repo.GetDHCPLeaseByMAC(ctx, networkID, macAddress)
	if err != nil {
		return nil, fmt.Errorf("failed to check existing lease: %w", err)
	}

	now := time.Now()
	if existingLease != nil {
		// Update lease
		existingLease.LeaseStart = now.Format(time.RFC3339)
		existingLease.LeaseEnd = now.Add(1 * time.Hour).Format(time.RFC3339)
		if hostname != "" {
			existingLease.Hostname = hostname
		}
		if err := m.repo.UpdateDHCPLease(ctx, existingLease); err != nil {
			return nil, fmt.Errorf("failed to update lease: %w", err)
		}
		return existingLease, nil
	}

	// Get server config to determine IP range
	server, err := m.repo.GetDHCPServerByNetwork(ctx, networkID)
	if err != nil {
		return nil, fmt.Errorf("failed to get DHCP server: %w", err)
	}

	// Allocate new IP (simplified)
	ip, err := m.allocateIP(ctx, networkID, server)
	if err != nil {
		return nil, fmt.Errorf("failed to allocate IP: %w", err)
	}

	// Create new lease
	lease := &DHCPLease{
		ID:         uuid.NewString(),
		NetworkID:  networkID,
		MACAddress: macAddress,
		IPAddress:  ip,
		Hostname:   hostname,
		LeaseStart: now.Format(time.RFC3339),
		LeaseEnd:   now.Add(server.LeaseTime).Format(time.RFC3339),
	}

	if err := m.repo.CreateDHCPLease(ctx, lease); err != nil {
		return nil, fmt.Errorf("failed to create lease: %w", err)
	}

	return lease, nil
}

// allocateIP finds an available IP in the range (simplified implementation)
func (m *DHCPServerManager) allocateIP(ctx context.Context, networkID string, server *DHCPServer) (string, error) {
	// Get existing leases
	leases, err := m.repo.ListDHCPLeases(ctx, networkID)
	if err != nil {
		return "", err
	}

	// Build a set of used IPs
	usedIPs := make(map[string]bool)
	for _, lease := range leases {
		usedIPs[lease.IPAddress] = true
	}

	// Parse range
	startIP := net.ParseIP(server.RangeStart).To4()
	endIP := net.ParseIP(server.RangeEnd).To4()

	// Find first available IP
	ip := make(net.IP, 4)
	copy(ip, startIP)

	for compareIP(ip, endIP) <= 0 {
		ipStr := ip.String()
		if !usedIPs[ipStr] {
			return ipStr, nil
		}
		// Increment IP
		for i := 3; i >= 0; i-- {
			ip[i]++
			if ip[i] != 0 {
				break
			}
		}
	}

	return "", fmt.Errorf("no available IPs in range")
}

// manageLeases handles lease expiration for a network
func (m *DHCPServerManager) manageLeases(networkID string, stopChan <-chan struct{}) {
	ticker := time.NewTicker(1 * time.Minute)
	defer ticker.Stop()

	for {
		select {
		case <-stopChan:
			return
		case <-ticker.C:
			// Clean up expired leases
			ctx := context.Background()
			leases, err := m.repo.ListDHCPLeases(ctx, networkID)
			if err != nil {
				continue
			}

			now := time.Now()
			for _, lease := range leases {
				leaseEnd, err := time.Parse(time.RFC3339, lease.LeaseEnd)
				if err != nil {
					continue
				}
				if now.After(leaseEnd) {
					// Lease expired, delete it
					m.repo.DeleteDHCPLease(ctx, lease.ID)
				}
			}
		}
	}
}

// DBDHCPRepository implements DHCPRepository using SQLite
type DBDHCPRepository struct {
	db *sql.DB
}

// NewDBDHCPRepository creates a new DHCP database repository
func NewDBDHCPRepository(db *sql.DB) DHCPRepository {
	return &DBDHCPRepository{db: db}
}

// CreateDHCPServer creates a DHCP server configuration
func (r *DBDHCPRepository) CreateDHCPServer(ctx context.Context, server *DHCPServer) error {
	leaseTimeSeconds := int64(server.LeaseTime.Seconds())
	_, err := r.db.ExecContext(ctx,
		`INSERT INTO dhcp_servers (id, network_id, range_start, range_end, lease_time_seconds, is_running, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
		server.ID, server.NetworkID, server.RangeStart, server.RangeEnd, leaseTimeSeconds, boolToInt(server.IsRunning),
		server.CreatedAt, server.UpdatedAt,
	)
	return err
}

// GetDHCPServerByNetwork retrieves the DHCP server for a network
func (r *DBDHCPRepository) GetDHCPServerByNetwork(ctx context.Context, networkID string) (*DHCPServer, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, network_id, range_start, range_end, lease_time_seconds, is_running, created_at, updated_at
		 FROM dhcp_servers WHERE network_id = ?`, networkID)
	
	var server DHCPServer
	var leaseTimeSeconds int64
	var isRunning int
	err := row.Scan(&server.ID, &server.NetworkID, &server.RangeStart, &server.RangeEnd, &leaseTimeSeconds, 
		&isRunning, &server.CreatedAt, &server.UpdatedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	server.IsRunning = isRunning != 0
	server.LeaseTime = time.Duration(leaseTimeSeconds) * time.Second
	return &server, nil
}

// UpdateDHCPServer updates a DHCP server configuration
func (r *DBDHCPRepository) UpdateDHCPServer(ctx context.Context, server *DHCPServer) error {
	leaseTimeSeconds := int64(server.LeaseTime.Seconds())
	_, err := r.db.ExecContext(ctx,
		`UPDATE dhcp_servers SET range_start = ?, range_end = ?, lease_time_seconds = ?, is_running = ?, updated_at = ?
		 WHERE id = ?`,
		server.RangeStart, server.RangeEnd, leaseTimeSeconds, boolToInt(server.IsRunning), server.UpdatedAt, server.ID,
	)
	return err
}

// DeleteDHCPServer removes a DHCP server configuration
func (r *DBDHCPRepository) DeleteDHCPServer(ctx context.Context, networkID string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM dhcp_servers WHERE network_id = ?`, networkID)
	return err
}

// CreateDHCPLease creates a DHCP lease
func (r *DBDHCPRepository) CreateDHCPLease(ctx context.Context, lease *DHCPLease) error {
	_, err := r.db.ExecContext(ctx,
		`INSERT INTO dhcp_leases (id, network_id, mac_address, ip_address, hostname, lease_start, lease_end)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`,
		lease.ID, lease.NetworkID, lease.MACAddress, lease.IPAddress, nullableString(lease.Hostname), lease.LeaseStart, lease.LeaseEnd,
	)
	return err
}

// GetDHCPLeaseByMAC retrieves a lease by MAC address
func (r *DBDHCPRepository) GetDHCPLeaseByMAC(ctx context.Context, networkID, macAddress string) (*DHCPLease, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, network_id, mac_address, ip_address, hostname, lease_start, lease_end
		 FROM dhcp_leases WHERE network_id = ? AND mac_address = ?`, networkID, macAddress)
	
	return r.scanLease(row)
}

// GetDHCPLeaseByIP retrieves a lease by IP address
func (r *DBDHCPRepository) GetDHCPLeaseByIP(ctx context.Context, networkID, ipAddress string) (*DHCPLease, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, network_id, mac_address, ip_address, hostname, lease_start, lease_end
		 FROM dhcp_leases WHERE network_id = ? AND ip_address = ?`, networkID, ipAddress)
	
	return r.scanLease(row)
}

// ListDHCPLeases returns all leases for a network
func (r *DBDHCPRepository) ListDHCPLeases(ctx context.Context, networkID string) ([]DHCPLease, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, network_id, mac_address, ip_address, hostname, lease_start, lease_end
		 FROM dhcp_leases WHERE network_id = ? ORDER BY ip_address ASC`, networkID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var leases []DHCPLease
	for rows.Next() {
		var lease DHCPLease
		var hostname sql.NullString
		err := rows.Scan(&lease.ID, &lease.NetworkID, &lease.MACAddress, &lease.IPAddress, &hostname, &lease.LeaseStart, &lease.LeaseEnd)
		if err != nil {
			return nil, err
		}
		if hostname.Valid {
			lease.Hostname = hostname.String
		}
		leases = append(leases, lease)
	}
	return leases, rows.Err()
}

// DeleteDHCPLease removes a DHCP lease
func (r *DBDHCPRepository) DeleteDHCPLease(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM dhcp_leases WHERE id = ?`, id)
	return err
}

// UpdateDHCPLease updates a DHCP lease
func (r *DBDHCPRepository) UpdateDHCPLease(ctx context.Context, lease *DHCPLease) error {
	_, err := r.db.ExecContext(ctx,
		`UPDATE dhcp_leases SET ip_address = ?, hostname = ?, lease_start = ?, lease_end = ?
		 WHERE id = ?`,
		lease.IPAddress, nullableString(lease.Hostname), lease.LeaseStart, lease.LeaseEnd, lease.ID,
	)
	return err
}

func (r *DBDHCPRepository) scanLease(row *sql.Row) (*DHCPLease, error) {
	var lease DHCPLease
	var hostname sql.NullString
	err := row.Scan(&lease.ID, &lease.NetworkID, &lease.MACAddress, &lease.IPAddress, &hostname, &lease.LeaseStart, &lease.LeaseEnd)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if hostname.Valid {
		lease.Hostname = hostname.String
	}
	return &lease, nil
}

func boolToInt(b bool) int {
	if b {
		return 1
	}
	return 0
}

func nullableString(s string) interface{} {
	if s == "" {
		return nil
	}
	return s
}
