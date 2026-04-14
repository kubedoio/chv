package networking

import (
	"context"
	"database/sql"
	"fmt"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
)

// VLANNetwork represents a VLAN configuration for network segmentation
type VLANNetwork struct {
	ID        string `json:"id"`
	NetworkID string `json:"network_id"`
	VLANID    int    `json:"vlan_id"` // 1-4094
	Name      string `json:"name"`
	CIDR      string `json:"cidr"`
	GatewayIP string `json:"gateway_ip"`
	CreatedAt string `json:"created_at,omitempty"`
}

// Validate validates the VLAN configuration
func (v *VLANNetwork) Validate() error {
	if v.NetworkID == "" {
		return fmt.Errorf("network_id is required")
	}
	if v.VLANID < 1 || v.VLANID > 4094 {
		return fmt.Errorf("vlan_id must be between 1 and 4094")
	}
	if v.Name == "" {
		return fmt.Errorf("name is required")
	}
	if v.CIDR == "" {
		return fmt.Errorf("cidr is required")
	}
	if v.GatewayIP == "" {
		return fmt.Errorf("gateway_ip is required")
	}
	return nil
}

// VLANRepository defines the interface for VLAN database operations
type VLANRepository interface {
	CreateVLAN(ctx context.Context, vlan *VLANNetwork) error
	GetVLANByID(ctx context.Context, id string) (*VLANNetwork, error)
	DeleteVLAN(ctx context.Context, id string) error
	ListVLANsByNetwork(ctx context.Context, networkID string) ([]VLANNetwork, error)
	GetVLANByNetworkAndVLANID(ctx context.Context, networkID string, vlanID int) (*VLANNetwork, error)
}

// VLANService handles VLAN operations
type VLANService struct {
	repo VLANRepository
}

// NewVLANService creates a new VLAN service
func NewVLANService(repo VLANRepository) *VLANService {
	return &VLANService{repo: repo}
}

// CreateVLAN creates a new VLAN network
func (s *VLANService) CreateVLAN(ctx context.Context, vlan *VLANNetwork) error {
	if err := vlan.Validate(); err != nil {
		return fmt.Errorf("invalid vlan: %w", err)
	}

	// Check for duplicate VLAN ID on this network
	existing, err := s.repo.GetVLANByNetworkAndVLANID(ctx, vlan.NetworkID, vlan.VLANID)
	if err != nil {
		return fmt.Errorf("failed to check existing vlan: %w", err)
	}
	if existing != nil {
		return fmt.Errorf("vlan %d already exists on network %s", vlan.VLANID, vlan.NetworkID)
	}

	vlan.ID = uuid.NewString()
	vlan.CreatedAt = time.Now().UTC().Format(time.RFC3339)

	if err := s.repo.CreateVLAN(ctx, vlan); err != nil {
		return fmt.Errorf("failed to create vlan: %w", err)
	}

	return nil
}

// DeleteVLAN removes a VLAN network
func (s *VLANService) DeleteVLAN(ctx context.Context, id string) error {
	vlan, err := s.repo.GetVLANByID(ctx, id)
	if err != nil {
		return fmt.Errorf("failed to get vlan: %w", err)
	}
	if vlan == nil {
		return fmt.Errorf("vlan not found")
	}

	if err := s.repo.DeleteVLAN(ctx, id); err != nil {
		return fmt.Errorf("failed to delete vlan: %w", err)
	}

	return nil
}

// ListVLANsByNetwork returns all VLANs for a network
func (s *VLANService) ListVLANsByNetwork(ctx context.Context, networkID string) ([]VLANNetwork, error) {
	return s.repo.ListVLANsByNetwork(ctx, networkID)
}

// GetVLAN retrieves a VLAN by ID
func (s *VLANService) GetVLAN(ctx context.Context, id string) (*VLANNetwork, error) {
	return s.repo.GetVLANByID(ctx, id)
}

// ToModelNetwork converts a VLANNetwork to a models.Network for compatibility
func (v *VLANNetwork) ToModelNetwork() *models.Network {
	return &models.Network{
		ID:         v.ID,
		Name:       fmt.Sprintf("%s (VLAN %d)", v.Name, v.VLANID),
		Mode:       "vlan",
		BridgeName: "",
		CIDR:       v.CIDR,
		GatewayIP:  v.GatewayIP,
		Status:     "active",
		CreatedAt:  v.CreatedAt,
	}
}

// DBVLANRepository implements VLANRepository using SQLite
type DBVLANRepository struct {
	db *sql.DB
}

// NewDBVLANRepository creates a new database repository for VLANs
func NewDBVLANRepository(db *sql.DB) VLANRepository {
	return &DBVLANRepository{db: db}
}

// CreateVLAN creates a VLAN in the database
func (r *DBVLANRepository) CreateVLAN(ctx context.Context, vlan *VLANNetwork) error {
	_, err := r.db.ExecContext(ctx,
		`INSERT INTO vlan_networks (id, network_id, vlan_id, name, cidr, gateway_ip, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`,
		vlan.ID, vlan.NetworkID, vlan.VLANID, vlan.Name, vlan.CIDR, vlan.GatewayIP, vlan.CreatedAt,
	)
	return err
}

// GetVLANByID retrieves a VLAN by ID
func (r *DBVLANRepository) GetVLANByID(ctx context.Context, id string) (*VLANNetwork, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, network_id, vlan_id, name, cidr, gateway_ip, created_at 
		 FROM vlan_networks WHERE id = ?`, id)

	var vlan VLANNetwork
	var createdAt sql.NullString
	err := row.Scan(&vlan.ID, &vlan.NetworkID, &vlan.VLANID, &vlan.Name, &vlan.CIDR, &vlan.GatewayIP, &createdAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if createdAt.Valid {
		vlan.CreatedAt = createdAt.String
	}
	return &vlan, nil
}

// DeleteVLAN removes a VLAN from the database
func (r *DBVLANRepository) DeleteVLAN(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM vlan_networks WHERE id = ?`, id)
	return err
}

// ListVLANsByNetwork returns all VLANs for a network
func (r *DBVLANRepository) ListVLANsByNetwork(ctx context.Context, networkID string) ([]VLANNetwork, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, network_id, vlan_id, name, cidr, gateway_ip, created_at 
		 FROM vlan_networks WHERE network_id = ? ORDER BY vlan_id ASC`, networkID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var vlans []VLANNetwork
	for rows.Next() {
		var vlan VLANNetwork
		var createdAt sql.NullString
		if err := rows.Scan(&vlan.ID, &vlan.NetworkID, &vlan.VLANID, &vlan.Name, &vlan.CIDR, &vlan.GatewayIP, &createdAt); err != nil {
			return nil, err
		}
		if createdAt.Valid {
			vlan.CreatedAt = createdAt.String
		}
		vlans = append(vlans, vlan)
	}
	return vlans, rows.Err()
}

// GetVLANByNetworkAndVLANID retrieves a VLAN by network ID and VLAN ID
func (r *DBVLANRepository) GetVLANByNetworkAndVLANID(ctx context.Context, networkID string, vlanID int) (*VLANNetwork, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, network_id, vlan_id, name, cidr, gateway_ip, created_at 
		 FROM vlan_networks WHERE network_id = ? AND vlan_id = ?`,
		networkID, vlanID)

	var vlan VLANNetwork
	var createdAt sql.NullString
	err := row.Scan(&vlan.ID, &vlan.NetworkID, &vlan.VLANID, &vlan.Name, &vlan.CIDR, &vlan.GatewayIP, &createdAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if createdAt.Valid {
		vlan.CreatedAt = createdAt.String
	}
	return &vlan, nil
}
