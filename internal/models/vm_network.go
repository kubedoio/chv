package models

import (
	"time"

	"github.com/google/uuid"
)

// VMNetworkAttachment represents a VM network attachment.
type VMNetworkAttachment struct {
	ID         uuid.UUID  `json:"id" db:"id"`
	VMID       uuid.UUID  `json:"vm_id" db:"vm_id"`
	NetworkID  uuid.UUID  `json:"network_id" db:"network_id"`
	MACAddress string     `json:"mac_address" db:"mac_address"`
	IPAddress  *string    `json:"ip_address" db:"ip_address"`
	NICIndex   int32      `json:"nic_index" db:"nic_index"`
	CreatedAt  time.Time  `json:"created_at" db:"created_at"`
}
