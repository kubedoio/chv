package services

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"sync"
	"time"

	"github.com/chv/chv/internal/agentapi"
)

// VMHealthService monitors VM health via Cloud Hypervisor API
type VMHealthService struct {
	httpClient *http.Client
	mu         sync.Mutex
	lastStats  map[string]lastCPUState // VMID -> state
}

type lastCPUState struct {
	cpuSeconds float64
	timestamp  time.Time
}

// NewVMHealthService creates a new VM health service
func NewVMHealthService() *VMHealthService {
	return &VMHealthService{
		httpClient: &http.Client{
			Timeout: 5 * time.Second,
			Transport: &http.Transport{
				DialContext: (&net.Dialer{
					Timeout: 2 * time.Second,
				}).DialContext,
			},
		},
		lastStats: make(map[string]lastCPUState),
	}
}

// CheckHealth checks if a VM is healthy via its API socket
func (s *VMHealthService) CheckHealth(apiSocket string) (bool, error) {
	// For MVP, we just check if the socket exists and is connectable
	// In future, we can query the CH API for detailed health
	conn, err := net.Dial("unix", apiSocket)
	if err != nil {
		return false, fmt.Errorf("cannot connect to VM API socket: %w", err)
	}
	conn.Close()
	return true, nil
}

// GetMetrics retrieves VM metrics from Cloud Hypervisor
func (s *VMHealthService) GetMetrics(ctx context.Context, req *agentapi.VMMetricsRequest) (*agentapi.VMMetricsResponse, error) {
	if req.APISocket == "" {
		return nil, fmt.Errorf("API socket path not provided")
	}

	// Create HTTP client with Unix socket transport
	client := &http.Client{
		Timeout: 10 * time.Second,
		Transport: &http.Transport{
			DialContext: func(ctx context.Context, network, addr string) (net.Conn, error) {
				return net.Dial("unix", req.APISocket)
			},
		},
	}

	// Query CH API for counters (CPU, memory, disk, network)
	counters, err := s.queryCounters(ctx, client)
	if err != nil {
		// Return basic metrics even if counters fail
		return s.getBasicMetrics(req), nil
	}

	// Query CH API for VM info
	info, err := s.queryVMInfo(ctx, client)
	if err != nil {
		info = vmInfo{}
	}

	return &agentapi.VMMetricsResponse{
		CPU: agentapi.CPUMetrics{
			UsagePercent: s.calculateCPUUsage(req.VMID, counters, info.CPUs),
			VCPUs:        info.CPUs,
		},
		Memory: agentapi.MemoryMetrics{
			TotalMB:      info.MemorySize / (1024 * 1024),
			UsedMB:       int(counters.VmmMetrics.MemoryWorkingSetBytes / (1024 * 1024)),
			FreeMB:       (info.MemorySize - int(counters.VmmMetrics.MemoryWorkingSetBytes)) / (1024 * 1024),
			UsagePercent: float64(counters.VmmMetrics.MemoryWorkingSetBytes) / float64(info.MemorySize) * 100,
		},
		Disk: agentapi.DiskMetrics{
			ReadBytes:  counters.BlockMetrics.ReadBytes,
			WriteBytes: counters.BlockMetrics.WriteBytes,
			ReadOps:    counters.BlockMetrics.ReadIops,
			WriteOps:   counters.BlockMetrics.WriteIops,
		},
		Network: agentapi.NetworkMetrics{
			RxBytes:   counters.NetMetrics.RxBytes,
			TxBytes:   counters.NetMetrics.TxBytes,
			RxPackets: counters.NetMetrics.RxPackets,
			TxPackets: counters.NetMetrics.TxPackets,
		},
		Uptime: counters.VmmMetrics.Uptime,
	}, nil
}

// getBasicMetrics returns minimal metrics when CH API is unavailable
func (s *VMHealthService) getBasicMetrics(req *agentapi.VMMetricsRequest) *agentapi.VMMetricsResponse {
	return &agentapi.VMMetricsResponse{
		CPU:     agentapi.CPUMetrics{VCPUs: 0},
		Memory:  agentapi.MemoryMetrics{},
		Disk:    agentapi.DiskMetrics{},
		Network: agentapi.NetworkMetrics{},
		Uptime:  "unknown",
	}
}

// chCounters represents Cloud Hypervisor counters response
type chCounters struct {
	VmmMetrics   vmmMetrics   `json:"vmm_metrics"`
	BlockMetrics blockMetrics `json:"block_metrics"`
	NetMetrics   netMetrics   `json:"net_metrics"`
}

type vmmMetrics struct {
	CPUSeconds            float64 `json:"cpu_seconds"`
	MemoryWorkingSetBytes int64   `json:"memory_working_set_bytes"`
	Uptime                string  `json:"uptime"`
}

type blockMetrics struct {
	ReadBytes  int64 `json:"read_bytes"`
	WriteBytes int64 `json:"write_bytes"`
	ReadIops   int64 `json:"read_iops"`
	WriteIops  int64 `json:"write_iops"`
}

type netMetrics struct {
	RxBytes   int64 `json:"rx_bytes"`
	TxBytes   int64 `json:"tx_bytes"`
	RxPackets int64 `json:"rx_packets"`
	TxPackets int64 `json:"tx_packets"`
}

// vmInfo represents Cloud Hypervisor VM info
type vmInfo struct {
	CPUs        int `json:"cpus"`
	MemorySize  int `json:"memory_size"`
}

// queryCounters queries CH API for counters
func (s *VMHealthService) queryCounters(ctx context.Context, client *http.Client) (chCounters, error) {
	var counters chCounters
	
	req, err := http.NewRequestWithContext(ctx, "GET", "http://localhost/api/v1/vmm.counters", nil)
	if err != nil {
		return counters, err
	}

	resp, err := client.Do(req)
	if err != nil {
		return counters, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return counters, fmt.Errorf("unexpected status: %d", resp.StatusCode)
	}

	if err := json.NewDecoder(resp.Body).Decode(&counters); err != nil {
		return counters, err
	}

	return counters, nil
}

// queryVMInfo queries CH API for VM info
func (s *VMHealthService) queryVMInfo(ctx context.Context, client *http.Client) (vmInfo, error) {
	var info vmInfo
	
	req, err := http.NewRequestWithContext(ctx, "GET", "http://localhost/api/v1/vm.info", nil)
	if err != nil {
		return info, err
	}

	resp, err := client.Do(req)
	if err != nil {
		return info, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return info, fmt.Errorf("unexpected status: %d", resp.StatusCode)
	}

	if err := json.NewDecoder(resp.Body).Decode(&info); err != nil {
		return info, err
	}

	return info, nil
}

// calculateCPUUsage calculates CPU usage percentage from counters
func (s *VMHealthService) calculateCPUUsage(vmID string, counters chCounters, vcpus int) float64 {
	s.mu.Lock()
	defer s.mu.Unlock()

	newSeconds := counters.VmmMetrics.CPUSeconds
	newTime := time.Now()

	last, exists := s.lastStats[vmID]
	s.lastStats[vmID] = lastCPUState{
		cpuSeconds: newSeconds,
		timestamp:  newTime,
	}

	if !exists || vcpus == 0 {
		return 0
	}

	deltaSeconds := newSeconds - last.cpuSeconds
	deltaTime := newTime.Sub(last.timestamp).Seconds()

	if deltaTime <= 0 {
		return 0
	}

	// Usage % = (delta CPU seconds / delta wall seconds) * 100 / vcpus
	usage := (deltaSeconds / deltaTime) * 100
	if usage > float64(vcpus*100) {
		usage = float64(vcpus * 100)
	}

	return usage
}

// HealthStatus represents VM health status
type HealthStatus struct {
	Healthy   bool   `json:"healthy"`
	Message   string `json:"message"`
	Timestamp int64  `json:"timestamp"`
}

// HealthCheck performs a comprehensive health check
func (s *VMHealthService) HealthCheck(apiSocket string) HealthStatus {
	healthy, err := s.CheckHealth(apiSocket)
	if err != nil {
		return HealthStatus{
			Healthy:   false,
			Message:   err.Error(),
			Timestamp: time.Now().Unix(),
		}
	}

	return HealthStatus{
		Healthy:   healthy,
		Message:   "VM is healthy",
		Timestamp: time.Now().Unix(),
	}
}
