package models

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

type Network struct {
	ID              string `json:"id"`
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
	LastError          string `json:"last_error,omitempty"`
	CreatedAt          string `json:"created_at,omitempty"`
	UpdatedAt          string `json:"updated_at,omitempty"`
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
