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
