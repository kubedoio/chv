package agentapi

// VMMetricsRequest requests metrics for a VM
type VMMetricsRequest struct {
	VMID       string `json:"vm_id"`
	PID        int    `json:"pid"`
	APISocket  string `json:"api_socket"`
}

// VMMetricsResponse returns VM metrics
type VMMetricsResponse struct {
	CPU     CPUMetrics     `json:"cpu"`
	Memory  MemoryMetrics  `json:"memory"`
	Disk    DiskMetrics    `json:"disk"`
	Network NetworkMetrics `json:"network"`
	Uptime  string         `json:"uptime"`
}

// CPUMetrics represents CPU usage
type CPUMetrics struct {
	UsagePercent float64 `json:"usage_percent"`
	VCPUs        int     `json:"vcpus"`
}

// MemoryMetrics represents memory usage
type MemoryMetrics struct {
	TotalMB     int `json:"total_mb"`
	UsedMB      int `json:"used_mb"`
	FreeMB      int `json:"free_mb"`
	UsagePercent float64 `json:"usage_percent"`
}

// DiskMetrics represents disk I/O
type DiskMetrics struct {
	ReadBytes  int64 `json:"read_bytes"`
	WriteBytes int64 `json:"write_bytes"`
	ReadOps    int64 `json:"read_ops"`
	WriteOps   int64 `json:"write_ops"`
}

// NetworkMetrics represents network I/O
type NetworkMetrics struct {
	RxBytes int64 `json:"rx_bytes"`
	TxBytes int64 `json:"tx_bytes"`
	RxPackets int64 `json:"rx_packets"`
	TxPackets int64 `json:"tx_packets"`
}
