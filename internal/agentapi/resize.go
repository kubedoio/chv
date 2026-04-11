package agentapi

// VMResizeRequest requests a resize of VM resources (CPU/memory hot-plug)
type VMResizeRequest struct {
	VMID     string `json:"vm_id"`
	VCPUs    int    `json:"vcpus,omitempty"`    // Desired number of vCPUs (0 = no change)
	MemoryMB int    `json:"memory_mb,omitempty"` // Desired memory in MB (0 = no change)
}

// VMResizeResponse confirms the resize operation and returns new configuration
type VMResizeResponse struct {
	VCPUs    int    `json:"vcpus"`
	MemoryMB int    `json:"memory_mb"`
	Message  string `json:"message,omitempty"`
}

// VMPauseRequest requests to pause a running VM
type VMPauseRequest struct {
	VMID string `json:"vm_id"`
}

// VMPauseResponse confirms the pause operation
type VMPauseResponse struct {
	Success bool   `json:"success"`
	Message string `json:"message,omitempty"`
}

// VMResumeRequest requests to resume a paused VM
type VMResumeRequest struct {
	VMID string `json:"vm_id"`
}

// VMResumeResponse confirms the resume operation
type VMResumeResponse struct {
	Success bool   `json:"success"`
	Message string `json:"message,omitempty"`
}

// VMStateRequest retrieves the current state of a VM
type VMStateRequest struct {
	VMID string `json:"vm_id"`
}

// VMStateResponse returns the VM state
// States: "Running", "Paused", "Shutdown", "Created"
type VMStateResponse struct {
	State   string `json:"state"`
	Running bool   `json:"running"`
}
