package agentapi

// Common error type
type Error struct {
	Code      string `json:"code"`
	Message   string `json:"message"`
	Retryable bool   `json:"retryable"`
}

func (e Error) Error() string {
	return e.Message
}

// InstallCheckRequest - empty, just GET
type InstallCheckRequest struct{}

// InstallCheckResponse - structured install status
type InstallCheckResponse struct {
	DataRoot             string `json:"data_root"`
	DatabasePath         string `json:"database_path"`
	BridgeName           string `json:"bridge_name"`
	BridgeExists         bool   `json:"bridge_exists"`
	BridgeIPExpected     string `json:"bridge_ip_expected"`
	BridgeIPActual       string `json:"bridge_ip_actual"`
	BridgeUp             bool   `json:"bridge_up"`
	LocaldiskPath        string `json:"localdisk_path"`
	LocaldiskReady       bool   `json:"localdisk_ready"`
	CloudHypervisorFound bool   `json:"cloud_hypervisor_found"`
	CloudHypervisorPath  string `json:"cloud_hypervisor_path,omitempty"`
	CloudInitSupported   bool   `json:"cloudinit_supported"`
	OverallState         string `json:"overall_state"`
}

// BootstrapRequest - what to bootstrap
type BootstrapRequest struct {
	DataRoot      string `json:"data_root"`
	BridgeName    string `json:"bridge_name"`
	BridgeCIDR    string `json:"bridge_cidr"`
	LocaldiskPath string `json:"localdisk_path"`
}

// BootstrapResponse - actions taken
type BootstrapResponse struct {
	ActionsTaken []string `json:"actions_taken"`
	Warnings     []string `json:"warnings,omitempty"`
	Errors       []string `json:"errors,omitempty"`
}

// RepairRequest - what to repair
type RepairRequest struct {
	DataRoot      string `json:"data_root"`
	BridgeName    string `json:"bridge_name"`
	BridgeCIDR    string `json:"bridge_cidr"`
	LocaldiskPath string `json:"localdisk_path"`
	RepairBridge  bool   `json:"repair_bridge"`
	RepairDirs    bool   `json:"repair_directories"`
}

// RepairResponse - actions taken
type RepairResponse = BootstrapResponse

// ImageImportRequest - download image from URL
type ImageImportRequest struct {
	ImageID      string `json:"image_id"`
	SourceURL    string `json:"source_url"`
	DestPath     string `json:"dest_path"`
	Checksum     string `json:"checksum,omitempty"`
	ExpectedSize int64  `json:"expected_size,omitempty"`
}

// ImageImportResponse - download result
type ImageImportResponse struct {
	DownloadedBytes int64  `json:"downloaded_bytes"`
	LocalPath       string `json:"local_path"`
	Checksum        string `json:"checksum,omitempty"` // SHA256 of downloaded file
}

// ImageValidateRequest - validate checksum
type ImageValidateRequest struct {
	LocalPath      string `json:"local_path"`
	ExpectedSHA256 string `json:"expected_sha256"` // without "sha256:" prefix
}

// ImageValidateResponse - validation result
type ImageValidateResponse struct {
	Valid  bool   `json:"valid"`
	Actual string `json:"actual_sha256"`
}

// ProgressUpdate - for streaming progress (future)
type ProgressUpdate struct {
	Task      string `json:"task"`      // "download", "validate"
	Completed int64  `json:"completed"` // bytes or percent
	Total     int64  `json:"total"`
}

// VMValidationRequest requests validation of running VMs
type VMValidationRequest struct {
	// Optional: if provided, only validate these VM IDs
	ExpectedVMIDs []string `json:"expected_vm_ids,omitempty"`
	// Optional: data root path to identify managed VMs
	DataRoot string `json:"data_root,omitempty"`
}

// RunningVMInfo contains information about a running VM process
type RunningVMInfo struct {
	PID          int    `json:"pid"`
	VMID         string `json:"vm_id"`
	SocketPath   string `json:"socket_path"`
	DiskPath     string `json:"disk_path"`
	SeedISOPath  string `json:"seed_iso_path,omitempty"`
	VCPU         int    `json:"vcpu"`
	MemoryMB     int    `json:"memory_mb"`
	TAPDevice    string `json:"tap_device,omitempty"`
	MACAddress   string `json:"mac_address,omitempty"`
	IPAddress    string `json:"ip_address,omitempty"`
	KernelPath   string `json:"kernel_path"`
	CommandLine  string `json:"command_line"`
	IsManaged    bool   `json:"is_managed"`     // Whether this VM is in the expected list
	WorkspacePath string `json:"workspace_path,omitempty"`
}

// VMValidationResponse returns the validation results
type VMValidationResponse struct {
	// All running VMs found on the system
	RunningVMs []RunningVMInfo `json:"running_vms"`
	// VMs that are running but not in the expected list (orphans)
	OrphanVMs []RunningVMInfo `json:"orphan_vms"`
	// VMs that were expected but not found running (missing)
	MissingVMs []string `json:"missing_vm_ids"`
	// VMs that are running as expected (valid)
	ValidVMs []RunningVMInfo `json:"valid_vms"`
	// Summary counts
	Summary ValidationSummary `json:"summary"`
}

// ValidationSummary provides quick counts for validation results
type ValidationSummary struct {
	TotalRunning int `json:"total_running"`
	Valid        int `json:"valid"`
	Orphans      int `json:"orphans"`
	Missing      int `json:"missing"`
}
