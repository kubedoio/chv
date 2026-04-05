package api

import (
	"time"

	"github.com/google/uuid"
)

// SwagImage represents an image for Swagger documentation.
// This is used instead of models.Image to avoid json.RawMessage issues.
type SwagImage struct {
	ID                 uuid.UUID `json:"id"`
	Name               string    `json:"name"`
	OSFamily           string    `json:"os_family"`
	SourceFormat       string    `json:"source_format"`
	NormalizedFormat   string    `json:"normalized_format"`
	Architecture       string    `json:"architecture"`
	CloudInitSupported bool      `json:"cloud_init_supported"`
	DefaultUsername    string    `json:"default_username"`
	Checksum           string    `json:"checksum"`
	Status             string    `json:"status"`
	SizeBytes          uint64    `json:"size_bytes"`
	Metadata           map[string]interface{} `json:"metadata"`
	CreatedAt          time.Time `json:"created_at"`
	ImportedAt         *time.Time `json:"imported_at,omitempty"`
}

// SwagNode represents a node for Swagger documentation.
type SwagNode struct {
	ID                  uuid.UUID `json:"id"`
	Hostname            string    `json:"hostname"`
	ManagementIP        string    `json:"management_ip"`
	Status              string    `json:"status"`
	MaintenanceMode     bool      `json:"maintenance_mode"`
	TotalCPUcores       int32     `json:"total_cpu_cores"`
	TotalRAMMB          int64     `json:"total_ram_mb"`
	AllocatableCPUCores int32     `json:"allocatable_cpu_cores"`
	AllocatableRAMMB    int64     `json:"allocatable_ram_mb"`
	Labels              map[string]string `json:"labels"`
	Capabilities        map[string]string `json:"capabilities"`
	AgentVersion        string    `json:"agent_version"`
	HypervisorVersion   string    `json:"hypervisor_version"`
	LastHeartbeatAt     *time.Time `json:"last_heartbeat_at,omitempty"`
	CreatedAt           time.Time `json:"created_at"`
	UpdatedAt           time.Time `json:"updated_at"`
}

// SwagNetwork represents a network for Swagger documentation.
type SwagNetwork struct {
	ID         uuid.UUID `json:"id"`
	Name       string    `json:"name"`
	BridgeName string    `json:"bridge_name"`
	CIDR       string    `json:"cidr"`
	GatewayIP  string    `json:"gateway_ip"`
	DNSServers []string  `json:"dns_servers"`
	MTU        int32     `json:"mtu"`
	Mode       string    `json:"mode"`
	Status     string    `json:"status"`
	CreatedAt  time.Time `json:"created_at"`
}

// SwagStoragePool represents a storage pool for Swagger documentation.
type SwagStoragePool struct {
	ID                   uuid.UUID `json:"id"`
	Name                 string    `json:"name"`
	NodeID               *uuid.UUID `json:"node_id,omitempty"`
	PoolType             string    `json:"pool_type"`
	PathOrExport         string    `json:"path_or_export"`
	Status               string    `json:"status"`
	SupportsOnlineResize bool      `json:"supports_online_resize"`
	SupportsClone        bool      `json:"supports_clone"`
	SupportsSnapshot     bool      `json:"supports_snapshot"`
	CreatedAt            time.Time `json:"created_at"`
}

// SwagOperation represents an operation for Swagger documentation.
type SwagOperation struct {
	ID           uuid.UUID `json:"id"`
	OperationType string   `json:"operation_type"`
	Category     string    `json:"category"`
	Status       string    `json:"status"`
	ResourceType string    `json:"resource_type"`
	ResourceID   *uuid.UUID `json:"resource_id,omitempty"`
	NodeID       *uuid.UUID `json:"node_id,omitempty"`
	ActorType    string    `json:"actor_type"`
	ActorID      string    `json:"actor_id"`
	Input        map[string]interface{} `json:"input"`
	Output       map[string]interface{} `json:"output"`
	ErrorMessage string    `json:"error_message,omitempty"`
	StartedAt    time.Time `json:"started_at"`
	CompletedAt  *time.Time `json:"completed_at,omitempty"`
}

// SwagOperationLog represents an operation log entry for Swagger documentation.
type SwagOperationLog struct {
	ID          uuid.UUID `json:"id"`
	OperationID uuid.UUID `json:"operation_id"`
	Level       string    `json:"level"`
	Message     string    `json:"message"`
	Metadata    map[string]interface{} `json:"metadata"`
	Timestamp   time.Time `json:"timestamp"`
}

// SwagVirtualMachine represents a VM for Swagger documentation.
type SwagVirtualMachine struct {
	ID              uuid.UUID `json:"id"`
	Name            string    `json:"name"`
	NodeID          *uuid.UUID `json:"node_id,omitempty"`
	DesiredState    string    `json:"desired_state"`
	ActualState     string    `json:"actual_state"`
	PlacementStatus string    `json:"placement_status"`
	CreatedAt       time.Time `json:"created_at"`
	UpdatedAt       time.Time `json:"updated_at"`
}

// SwagVolume represents a volume for Swagger documentation.
type SwagVolume struct {
	ID              uuid.UUID `json:"id"`
	VMID            *uuid.UUID `json:"vm_id,omitempty"`
	StoragePoolID   *uuid.UUID `json:"storage_pool_id,omitempty"`
	BackingImageID  *uuid.UUID `json:"backing_image_id,omitempty"`
	Format          string    `json:"format"`
	SizeBytes       int64     `json:"size_bytes"`
	AttachmentState string    `json:"attachment_state"`
	ResizeState     string    `json:"resize_state"`
	CreatedAt       time.Time `json:"created_at"`
}
