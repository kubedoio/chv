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
	VMID  string `json:"vm_id"`
	PID   int    `json:"pid"`
	Force bool   `json:"force"` // Send SIGKILL immediately
}

// VMStopResponse confirms the VM was stopped
type VMStopResponse struct {
	Stopped bool `json:"stopped"`
}

// VMDestroyRequest requests a VM to be completely removed
type VMDestroyRequest struct {
	VMID          string `json:"vm_id"`
	WorkspacePath string `json:"workspace_path"`
}

// VMDestroyResponse confirms the VM was destroyed
type VMDestroyResponse struct {
	Destroyed bool `json:"destroyed"`
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

// VMSnapshotInfo represents a single internal snapshot
type VMSnapshotInfo struct {
	ID        string `json:"id"`
	Name      string `json:"name"`
	Tag       string `json:"tag"`
	Date      string `json:"date"`
	VMID      string `json:"vm_id"`
}

// VMSnapshotCreateRequest requests a new internal snapshot
type VMSnapshotCreateRequest struct {
	VMID     string `json:"vm_id"`
	DiskPath string `json:"disk_path"`
	Name     string `json:"name"`
}

// VMSnapshotListRequest requests the list of snapshots for a disk
type VMSnapshotListRequest struct {
	VMID     string `json:"vm_id"`
	DiskPath string `json:"disk_path"`
}

// VMSnapshotActionResponse confirms a snapshot action
type VMSnapshotActionResponse struct {
	Success bool   `json:"success"`
	Message string `json:"message,omitempty"`
}

// VMSnapshotRestoreRequest reverts a disk to a snapshot
type VMSnapshotRestoreRequest struct {
	VMID     string `json:"vm_id"`
	DiskPath string `json:"disk_path"`
	Name     string `json:"name"`
}

// VMSnapshotDeleteRequest removes a snapshot
type VMSnapshotDeleteRequest struct {
	VMID     string `json:"vm_id"`
	DiskPath string `json:"disk_path"`
	Name     string `json:"name"`
}

// VMProvisionRequest requests a VM to be provisioned (disk clone + cloud-init)
type VMProvisionRequest struct {
	VMID              string   `json:"vm_id"`
	VMName            string   `json:"vm_name"`
	ImagePath         string   `json:"image_path"`
	DiskPath          string   `json:"disk_path"`
	WorkspacePath     string   `json:"workspace_path"`
	Username          string   `json:"username"`
	Password          string   `json:"password"`
	SSHAuthorizedKeys []string `json:"ssh_authorized_keys"`
	UserData          string   `json:"user_data"`
}

// VMProvisionResponse confirms provisioning completion
type VMProvisionResponse struct {
	Success bool   `json:"success"`
	Message string `json:"message,omitempty"`
}
