package models

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

// NetworkMode represents the network mode.
type NetworkMode string

const (
	NetworkModeBridge NetworkMode = "bridge"
)

// NetworkStatus represents the network status.
type NetworkStatus string

const (
	NetworkStatusActive   NetworkStatus = "active"
	NetworkStatusInactive NetworkStatus = "inactive"
	NetworkStatusError    NetworkStatus = "error"
)

// Network represents a host-networking-backed network definition.
type Network struct {
	ID          uuid.UUID     `json:"id" db:"id"`
	Name        string        `json:"name" db:"name"`
	BridgeName  string        `json:"bridge_name" db:"bridge_name"`
	CIDR        string        `json:"cidr" db:"cidr"`
	GatewayIP   string        `json:"gateway_ip" db:"gateway_ip"`
	DNSServers  json.RawMessage `json:"dns_servers" db:"dns_servers"`
	MTU         int32         `json:"mtu" db:"mtu"`
	Mode        NetworkMode   `json:"mode" db:"mode"`
	Status      NetworkStatus `json:"status" db:"status"`
	CreatedAt   time.Time     `json:"created_at" db:"created_at"`
}

// GetDNSServers returns the DNS servers as a string slice.
func (n *Network) GetDNSServers() []string {
	var servers []string
	if n.DNSServers != nil {
		json.Unmarshal(n.DNSServers, &servers)
	}
	return servers
}
