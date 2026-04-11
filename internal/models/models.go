package models

import "time"

type InstallState string

const (
	InstallStateReady                InstallState = "ready"
	InstallStateDegraded             InstallState = "degraded"
	InstallStateMissingPrerequisites InstallState = "missing_prerequisites"
	InstallStateDriftDetected        InstallState = "drift_detected"
	InstallStateBootstrapRequired    InstallState = "bootstrap_required"
	InstallStateError                InstallState = "error"
)

type InstallStatus struct {
	ID                   string       `json:"id"`
	DataRoot             string       `json:"data_root"`
	DatabasePath         string       `json:"database_path"`
	BridgeName           string       `json:"bridge_name"`
	BridgeExists         bool         `json:"bridge_exists"`
	BridgeIPExpected     string       `json:"bridge_ip_expected"`
	BridgeIPActual       string       `json:"bridge_ip_actual,omitempty"`
	BridgeUp             bool         `json:"bridge_up"`
	LocaldiskPath        string       `json:"localdisk_path"`
	LocaldiskReady       bool         `json:"localdisk_ready"`
	CloudHypervisorPath  string       `json:"cloud_hypervisor_path,omitempty"`
	CloudHypervisorFound bool         `json:"cloud_hypervisor_found"`
	CloudInitSupported   bool         `json:"cloudinit_supported"`
	OverallState         InstallState `json:"overall_state"`
	LastCheckedAt        string       `json:"last_checked_at"`
	LastBootstrappedAt   string       `json:"last_bootstrapped_at,omitempty"`
	LastError            string       `json:"last_error,omitempty"`
}

type InstallActionResult struct {
	Status       *InstallStatus `json:"-"`
	OverallState InstallState   `json:"overall_state"`
	ActionsTaken []string       `json:"actions_taken"`
	Warnings     []string       `json:"warnings"`
	Errors       []string       `json:"errors"`
}

// Node represents a CHV node in the cluster
type Node struct {
	ID             string `json:"id"`
	Name           string `json:"name"`
	Hostname       string `json:"hostname"`
	IPAddress      string `json:"ip_address"`
	Status         string `json:"status"`
	IsLocal        bool   `json:"is_local"`
	AgentURL       string `json:"agent_url,omitempty"`
	AgentToken     string `json:"-"` // Never expose token in API responses
	AgentTokenHash string `json:"-"` // Never expose token hash in API responses
	Capabilities   string `json:"capabilities,omitempty"`
	LastSeenAt     string `json:"last_seen_at,omitempty"`
	CreatedAt      string `json:"created_at,omitempty"`
	UpdatedAt      string `json:"updated_at,omitempty"`
}

// NodeStatus constants
const (
	NodeStatusOnline      = "online"
	NodeStatusOffline     = "offline"
	NodeStatusMaintenance = "maintenance"
	NodeStatusError       = "error"
)

// Role constants for RBAC
const (
	RoleAdmin     = "admin"
	RoleOperator  = "operator"
	RoleViewer    = "viewer"
)

// NodeResourceCount contains counts of resources on a node
type NodeResourceCount struct {
	VMs          int `json:"vms"`
	Images       int `json:"images"`
	StoragePools int `json:"storage_pools"`
	Networks     int `json:"networks"`
}

// NodeWithResources extends Node with resource counts
type NodeWithResources struct {
	Node
	Resources NodeResourceCount `json:"resources"`
}

type Network struct {
	ID              string `json:"id"`
	NodeID          string `json:"node_id"`
	Name            string `json:"name"`
	Mode            string `json:"mode"`
	BridgeName      string `json:"bridge_name"`
	CIDR            string `json:"cidr"`
	GatewayIP       string `json:"gateway_ip"`
	IsSystemManaged bool   `json:"is_system_managed"`
	Status          string `json:"status"`
	CreatedAt       string `json:"created_at,omitempty"`
}

type StoragePool struct {
	ID               string `json:"id"`
	NodeID           string `json:"node_id"`
	Name             string `json:"name"`
	PoolType         string `json:"pool_type"`
	Path             string `json:"path"`
	IsDefault        bool   `json:"is_default"`
	Status           string `json:"status"`
	CapacityBytes    int64  `json:"capacity_bytes,omitempty"`
	AllocatableBytes int64  `json:"allocatable_bytes,omitempty"`
	CreatedAt        string `json:"created_at,omitempty"`
}

type Image struct {
	ID                 string `json:"id"`
	NodeID             string `json:"node_id"`
	Name               string `json:"name"`
	OSFamily           string `json:"os_family"`
	Architecture       string `json:"architecture"`
	Format             string `json:"format"`
	SourceFormat       string `json:"source_format"`
	NormalizedFormat   string `json:"normalized_format"`
	SourceURL          string `json:"source_url"`
	Checksum           string `json:"checksum,omitempty"`
	LocalPath          string `json:"local_path"`
	CloudInitSupported bool   `json:"cloud_init_supported"`
	Status             string `json:"status"`
	CreatedAt          string `json:"created_at,omitempty"`
}

type VirtualMachine struct {
	ID                 string `json:"id"`
	NodeID             string `json:"node_id"`
	Name               string `json:"name"`
	ImageID            string `json:"image_id"`
	StoragePoolID      string `json:"storage_pool_id"`
	NetworkID          string `json:"network_id"`
	DesiredState       string `json:"desired_state"`
	ActualState        string `json:"actual_state"`
	VCPU               int    `json:"vcpu"`
	MemoryMB           int    `json:"memory_mb"`
	DiskPath           string `json:"disk_path"`
	SeedISOPath        string `json:"seed_iso_path"`
	WorkspacePath      string `json:"workspace_path"`
	CloudHypervisorPID int    `json:"cloud_hypervisor_pid,omitempty"`
	IPAddress          string `json:"ip_address,omitempty"`
	MACAddress         string `json:"mac_address,omitempty"`
	ConsoleType        string `json:"console_type,omitempty"` // "serial" only
	LastError          string `json:"last_error,omitempty"`
	CreatedAt          string `json:"created_at,omitempty"`
	UpdatedAt          string `json:"updated_at,omitempty"`
}

type VMSnapshot struct {
	ID        string `json:"id"`
	VMID      string `json:"vm_id"`
	Name      string `json:"name"`
	CreatedAt string `json:"created_at"`
	Status    string `json:"status"`
}

// VMBootLogEntry represents a single line from a VM's boot log
type VMBootLogEntry struct {
	LineNumber int       `json:"line_number"`
	Content    string    `json:"content"`
	Timestamp  time.Time `json:"timestamp"`
}

type User struct {
	ID           string  `json:"id"`
	Username     string  `json:"username"`
	PasswordHash string  `json:"-"`
	Email        string  `json:"email,omitempty"`
	Role         string  `json:"role"`
	IsActive     bool    `json:"is_active"`
	LastLoginAt  *string `json:"last_login_at,omitempty"`
	CreatedAt    string  `json:"created_at,omitempty"`
	UpdatedAt    string  `json:"updated_at,omitempty"`
}

type APIToken struct {
	ID        string  `json:"id"`
	Name      string  `json:"name"`
	TokenHash string  `json:"-"`
	CreatedAt string  `json:"created_at"`
	ExpiresAt *string `json:"expires_at,omitempty"`
	RevokedAt *string `json:"revoked_at,omitempty"`
}

type Operation struct {
	ID             string `json:"id"`
	ResourceType   string `json:"resource_type"`
	ResourceID     string `json:"resource_id"`
	OperationType  string `json:"operation_type"`
	State          string `json:"state"`
	RequestPayload string `json:"request_payload,omitempty"`
	ResultPayload  string `json:"result_payload,omitempty"`
	ErrorPayload   string `json:"error_payload,omitempty"`
	StartedAt      string `json:"started_at,omitempty"`
	FinishedAt     string `json:"finished_at,omitempty"`
	CreatedAt      string `json:"created_at,omitempty"`
}

// Role represents a user role with permissions
type Role struct {
	ID          string       `json:"id"`
	Name        string       `json:"name"`
	Permissions []Permission `json:"permissions"`
	CreatedAt   string       `json:"created_at"`
}

// Permission represents a single permission
type Permission struct {
	Resource string `json:"resource"`
	Action   string `json:"action"`
}

// AuditLog represents an audit log entry
type AuditLog struct {
	ID           string `json:"id"`
	UserID       string `json:"user_id"`
	UserName     string `json:"user_name"`
	Action       string `json:"action"`
	ResourceType string `json:"resource_type"`
	ResourceID   string `json:"resource_id,omitempty"`
	Details      string `json:"details,omitempty"`
	IPAddress    string `json:"ip_address,omitempty"`
	Success      bool   `json:"success"`
	Error        string `json:"error,omitempty"`
	CreatedAt    string `json:"created_at"`
}

// VMTemplate represents a reusable VM template for rapid provisioning
type VMTemplate struct {
	ID              string   `json:"id"`
	NodeID          string   `json:"node_id"`
	Name            string   `json:"name"`
	Description     string   `json:"description"`
	VCPU            int      `json:"vcpu"`
	MemoryMB        int      `json:"memory_mb"`
	ImageID         string   `json:"image_id"`
	NetworkID       string   `json:"network_id"`
	StoragePoolID   string   `json:"storage_pool_id"`
	CloudInitConfig string   `json:"cloud_init_config,omitempty"`
	Tags            []string `json:"tags,omitempty"`
	CreatedAt       string   `json:"created_at"`
}

// CloudInitTemplate represents a reusable cloud-init template
type CloudInitTemplate struct {
	ID          string   `json:"id"`
	Name        string   `json:"name"`
	Description string   `json:"description"`
	Content     string   `json:"content"`
	Variables   []string `json:"variables"`
	CreatedAt   string   `json:"created_at"`
}

// DefaultCloudInitTemplates returns the default cloud-init templates
func DefaultCloudInitTemplates() []CloudInitTemplate {
	return []CloudInitTemplate{
		{
			ID:          "cit-basic",
			Name:        "Basic User Setup",
			Description: "Creates a user with sudo access and SSH key",
			Content: `#cloud-config
hostname: {{.Hostname}}
manage_etc_hosts: true
users:
  - name: {{.Username}}
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - {{.SSHKey}}
chpasswd:
  list: |
    {{.Username}}:{{.Password}}
  expire: False
package_update: true
packages:
  - qemu-guest-agent`,
			Variables: []string{"Hostname", "Username", "SSHKey", "Password"},
		},
		{
			ID:          "cit-docker",
			Name:        "Docker Ready",
			Description: "Ubuntu with Docker pre-installed",
			Content: `#cloud-config
package_update: true
packages:
  - docker.io
  - qemu-guest-agent
users:
  - name: {{.Username}}
    groups: docker
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - {{.SSHKey}}
runcmd:
  - systemctl enable docker
  - systemctl start docker`,
			Variables: []string{"Username", "SSHKey"},
		},
		{
			ID:          "cit-kubernetes",
			Name:        "Kubernetes Node",
			Description: "Ubuntu with containerd and Kubernetes tools",
			Content: `#cloud-config
package_update: true
packages:
  - apt-transport-https
  - ca-certificates
  - curl
  - gnupg
  - lsb-release
  - qemu-guest-agent
users:
  - name: {{.Username}}
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - {{.SSHKey}}
runcmd:
  - sysctl -w net.ipv4.ip_forward=1
  - echo "net.ipv4.ip_forward=1" >> /etc/sysctl.conf`,
			Variables: []string{"Username", "SSHKey"},
		},
	}
}


// Quota represents resource limits for a user
type Quota struct {
	ID          string `json:"id"`
	UserID      string `json:"user_id"`
	MaxVMs      int    `json:"max_vms"`
	MaxCPUs     int    `json:"max_cpu"`
	MaxMemoryGB int    `json:"max_memory_gb"`
	MaxStorageGB int   `json:"max_storage_gb"`
	MaxNetworks int    `json:"max_networks"`
	CreatedAt   string `json:"created_at"`
	UpdatedAt   string `json:"updated_at"`
}

// ResourceUsage represents current resource consumption for a user
type ResourceUsage struct {
	VMs      int `json:"vms"`
	CPUs     int `json:"cpus"`
	MemoryGB int `json:"memory_gb"`
	StorageGB int `json:"storage_gb"`
	Networks int `json:"networks"`
}
