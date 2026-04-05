package models

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

// NodeState represents the state of a node.
type NodeState string

const (
	NodeStateOnline      NodeState = "online"
	NodeStateDegraded    NodeState = "degraded"
	NodeStateOffline     NodeState = "offline"
	NodeStateMaintenance NodeState = "maintenance"
)

// NodeCapability represents a node capability key-value pair.
type NodeCapability struct {
	Key   string `json:"key"`
	Value string `json:"value"`
}

// Node represents a hypervisor host running chv-agent.
type Node struct {
	ID                   uuid.UUID        `json:"id" db:"id"`
	Hostname             string           `json:"hostname" db:"hostname"`
	ManagementIP         string           `json:"management_ip" db:"management_ip"`
	Status               NodeState        `json:"status" db:"status"`
	MaintenanceMode      bool             `json:"maintenance_mode" db:"maintenance_mode"`
	TotalCPUcores        int32            `json:"total_cpu_cores" db:"total_cpu_cores"`
	TotalRAMMB           int64            `json:"total_ram_mb" db:"total_ram_mb"`
	AllocatableCPUCores  int32            `json:"allocatable_cpu_cores" db:"allocatable_cpu_cores"`
	AllocatableRAMMB     int64            `json:"allocatable_ram_mb" db:"allocatable_ram_mb"`
	Labels               json.RawMessage  `json:"labels" db:"labels"`
	Capabilities         json.RawMessage  `json:"capabilities" db:"capabilities"`
	AgentVersion         string           `json:"agent_version" db:"agent_version"`
	HypervisorVersion    string           `json:"hypervisor_version" db:"hypervisor_version"`
	LastHeartbeatAt      *time.Time       `json:"last_heartbeat_at" db:"last_heartbeat_at"`
	CreatedAt            time.Time        `json:"created_at" db:"created_at"`
	UpdatedAt            time.Time        `json:"updated_at" db:"updated_at"`
}

// IsAvailable returns true if the node can accept new workloads.
func (n *Node) IsAvailable() bool {
	return n.Status == NodeStateOnline && !n.MaintenanceMode
}

// HasCapacity checks if the node has sufficient resources.
func (n *Node) HasCapacity(cpu int32, ramMB int64) bool {
	return n.AllocatableCPUCores >= cpu && n.AllocatableRAMMB >= ramMB
}

// GetCapabilities parses and returns node capabilities.
func (n *Node) GetCapabilities() map[string]string {
	var caps map[string]string
	if n.Capabilities != nil {
		json.Unmarshal(n.Capabilities, &caps)
	}
	return caps
}

// GetLabels parses and returns node labels.
func (n *Node) GetLabels() map[string]string {
	var labels map[string]string
	if n.Labels != nil {
		json.Unmarshal(n.Labels, &labels)
	}
	return labels
}
