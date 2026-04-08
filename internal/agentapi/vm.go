package agentapi

// VMStartRequest requests a VM to be started
type VMStartRequest struct {
	VMID            string   `json:"vm_id"`
	KernelPath      string   `json:"kernel_path"`
	DiskPath        string   `json:"disk_path"`
	SeedISOPath     string   `json:"seed_iso_path"`
	TapDevice       string   `json:"tap_device"`
	MACAddress      string   `json:"mac_address"`
	IPAddress       string   `json:"ip_address"`
	Netmask         string   `json:"netmask"`
	VCPU            int      `json:"vcpu"`
	MemoryMB        int      `json:"memory_mb"`
	WorkspacePath   string   `json:"workspace_path"`
	CloudHypervisorPath string `json:"cloud_hypervisor_path,omitempty"`
	BridgeName      string   `json:"bridge_name,omitempty"`
}

// VMStartResponse returns the PID of the started VM
type VMStartResponse struct {
	PID int `json:"pid"`
}

// VMStopRequest requests a VM to be stopped
type VMStopRequest struct {
	VMID string `json:"vm_id"`
	PID  int    `json:"pid"`
}

// VMStopResponse confirms the VM was stopped
type VMStopResponse struct {
	Stopped bool `json:"stopped"`
}

// VMStatusRequest checks VM status
type VMStatusRequest struct {
	VMID string `json:"vm_id"`
	PID  int    `json:"pid"`
}

// VMStatusResponse returns VM process status
type VMStatusResponse struct {
	Running bool   `json:"running"`
	PID     int    `json:"pid"`
	Uptime  string `json:"uptime,omitempty"`
}
