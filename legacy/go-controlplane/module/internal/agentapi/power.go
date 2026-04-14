package agentapi

// VMShutdownRequest requests a graceful VM shutdown via ACPI
type VMShutdownRequest struct {
	VMID    string `json:"vm_id"`
	PID     int    `json:"pid"`
	Timeout int    `json:"timeout,omitempty"` // Timeout in seconds
}

// VMShutdownResponse confirms shutdown was initiated
type VMShutdownResponse struct {
	Success bool   `json:"success"`
	Message string `json:"message,omitempty"`
}

// VMForceStopRequest requests immediate VM termination
type VMForceStopRequest struct {
	VMID string `json:"vm_id"`
	PID  int    `json:"pid"`
}

// VMForceStopResponse confirms VM was terminated
type VMForceStopResponse struct {
	Success bool   `json:"success"`
	Message string `json:"message,omitempty"`
}

// VMResetRequest requests a VM reset (power cycle without shutdown)
type VMResetRequest struct {
	VMID string `json:"vm_id"`
	PID  int    `json:"pid"`
}

// VMResetResponse confirms reset was initiated
type VMResetResponse struct {
	Success bool   `json:"success"`
	Message string `json:"message,omitempty"`
}

// VMBootLogRequest requests boot logs for a VM
type VMBootLogRequest struct {
	VMID  string `json:"vm_id"`
	Lines int    `json:"lines,omitempty"` // Number of lines to retrieve (0 = all)
}

// VMBootLogEntry represents a single boot log line
type VMBootLogEntry struct {
	LineNumber int    `json:"line_number"`
	Content    string `json:"content"`
	Timestamp  string `json:"timestamp"`
}

// VMBootLogResponse returns boot log entries
type VMBootLogResponse struct {
	VMID   string           `json:"vm_id"`
	Lines  []VMBootLogEntry `json:"lines"`
	Total  int              `json:"total"`
}
