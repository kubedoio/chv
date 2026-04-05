// Package metrics provides metrics collection from CloudHypervisor.
package metrics

import (
	"context"
	"fmt"
	"log"
	"sync"
	"time"

	"github.com/chv/chv/internal/agent/ch"
)

// MetricType represents the type of metric.
type MetricType string

const (
	MetricTypeCPU     MetricType = "cpu"
	MetricTypeMemory  MetricType = "memory"
	MetricTypeDisk    MetricType = "disk"
	MetricTypeNetwork MetricType = "network"
)

// Metric represents a single metric data point.
type Metric struct {
	Timestamp time.Time              `json:"timestamp"`
	Type      MetricType             `json:"type"`
	Name      string                 `json:"name"`      // e.g., "vcpu0", "eth0", "sda"
	Value     float64                `json:"value"`     // Current value
	Delta     float64                `json:"delta"`     // Change since last measurement
	Labels    map[string]string      `json:"labels"`    // Additional labels
	Raw       map[string]interface{} `json:"raw"`       // Raw data from CH
}

// VMCMetrics represents all metrics for a VM.
type VMCMetrics struct {
	VMID      string    `json:"vm_id"`
	Timestamp time.Time `json:"timestamp"`
	CPU       []Metric  `json:"cpu,omitempty"`
	Memory    *Metric   `json:"memory,omitempty"`
	Disks     []Metric  `json:"disks,omitempty"`
	Network   []Metric  `json:"network,omitempty"`
}

// Collector collects metrics from CloudHypervisor.
type Collector struct {
	chClient  *ch.Client
	interval  time.Duration
	history   map[string]*metricsHistory // vmID -> history
	historyMu sync.RWMutex
	stopCh    chan struct{}
	wg        sync.WaitGroup
}

// metricsHistory stores recent metrics for a VM.
type metricsHistory struct {
	vmID    string
	metrics []VMCMetrics
	maxSize int
	mu      sync.Mutex
}

// NewCollector creates a new metrics collector.
func NewCollector(chClient *ch.Client, interval time.Duration) *Collector {
	if interval == 0 {
		interval = 10 * time.Second
	}
	return &Collector{
		chClient: chClient,
		interval: interval,
		history:  make(map[string]*metricsHistory),
		stopCh:   make(chan struct{}),
	}
}

// Start begins collecting metrics.
func (c *Collector) Start(ctx context.Context) {
	c.wg.Add(1)
	go c.collectionLoop(ctx)
	log.Printf("Metrics collector started (interval: %v)", c.interval)
}

// Stop stops the collector.
func (c *Collector) Stop() {
	close(c.stopCh)
	c.wg.Wait()
	log.Println("Metrics collector stopped")
}

// collectionLoop runs the metrics collection loop.
func (c *Collector) collectionLoop(ctx context.Context) {
	defer c.wg.Done()

	ticker := time.NewTicker(c.interval)
	defer ticker.Stop()

	// Collect immediately on start
	c.collect(ctx)

	for {
		select {
		case <-ctx.Done():
			return
		case <-c.stopCh:
			return
		case <-ticker.C:
			c.collect(ctx)
		}
	}
}

// collect gathers metrics from CH.
func (c *Collector) collect(ctx context.Context) {
	// Get VM counters from CH
	counters, err := c.chClient.GetVMCounters(ctx)
	if err != nil {
		log.Printf("Failed to get VM counters: %v", err)
		return
	}

	// Get VM info for memory stats
	vmInfo, err := c.chClient.GetVMInfo(ctx)
	if err != nil {
		log.Printf("Failed to get VM info: %v", err)
		return
	}

	// Build metrics
	metrics := c.buildMetrics(counters, vmInfo)
	
	// Store in history
	c.storeMetrics(metrics)
}

// buildMetrics converts CH counters to our metric format.
func (c *Collector) buildMetrics(counters *ch.VMCounters, vmInfo *ch.VMInfo) VMCMetrics {
	now := time.Now()
	metrics := VMCMetrics{
		Timestamp: now,
		CPU:       make([]Metric, 0, len(counters.VCPUs)),
		Disks:     make([]Metric, 0, len(counters.Disks)),
		Network:   make([]Metric, 0, len(counters.Net)),
	}

	// CPU metrics
	for i, vcpu := range counters.VCPUs {
		metrics.CPU = append(metrics.CPU, Metric{
			Timestamp: now,
			Type:      MetricTypeCPU,
			Name:      fmt.Sprintf("vcpu%d", i),
			Value:     float64(vcpu.Instructions),
			Labels: map[string]string{
				"index": fmt.Sprintf("%d", i),
			},
			Raw: map[string]interface{}{
				"instructions": vcpu.Instructions,
				"cycles":       vcpu.Cycles,
			},
		})
	}

	// Memory metric (from VM info)
	if vmInfo != nil && vmInfo.Config.Memory != nil {
		metrics.Memory = &Metric{
			Timestamp: now,
			Type:      MetricTypeMemory,
			Name:      "memory",
			Value:     float64(vmInfo.MemorySize),
			Labels: map[string]string{
				"size_configured": fmt.Sprintf("%d", vmInfo.Config.Memory.Size),
			},
		}
	}

	// Disk metrics
	for i, disk := range counters.Disks {
		metrics.Disks = append(metrics.Disks, Metric{
			Timestamp: now,
			Type:      MetricTypeDisk,
			Name:      fmt.Sprintf("disk%d", i),
			Value:     float64(disk.ReadBytes + disk.WriteBytes),
			Labels: map[string]string{
				"index": fmt.Sprintf("%d", i),
			},
			Raw: map[string]interface{}{
				"read_bytes":  disk.ReadBytes,
				"write_bytes": disk.WriteBytes,
				"read_ops":    disk.ReadOps,
				"write_ops":   disk.WriteOps,
			},
		})
	}

	// Network metrics
	for i, net := range counters.Net {
		metrics.Network = append(metrics.Network, Metric{
			Timestamp: now,
			Type:      MetricTypeNetwork,
			Name:      fmt.Sprintf("net%d", i),
			Value:     float64(net.RXBytes + net.TXBytes),
			Labels: map[string]string{
				"index": fmt.Sprintf("%d", i),
			},
			Raw: map[string]interface{}{
				"rx_bytes":   net.RXBytes,
				"tx_bytes":   net.TXBytes,
				"rx_packets": net.RXPackets,
				"tx_packets": net.TXPackets,
			},
		})
	}

	return metrics
}

// storeMetrics stores metrics in history.
func (c *Collector) storeMetrics(metrics VMCMetrics) {
	c.historyMu.Lock()
	defer c.historyMu.Unlock()

	history, ok := c.history[metrics.VMID]
	if !ok {
		history = &metricsHistory{
			vmID:    metrics.VMID,
			maxSize: 30, // Keep last 30 samples (5 minutes at 10s interval)
		}
		c.history[metrics.VMID] = history
	}

	history.mu.Lock()
	defer history.mu.Unlock()

	history.metrics = append(history.metrics, metrics)
	if len(history.metrics) > history.maxSize {
		history.metrics = history.metrics[len(history.metrics)-history.maxSize:]
	}
}

// GetMetrics returns the latest metrics for a VM.
func (c *Collector) GetMetrics(vmID string) *VMCMetrics {
	c.historyMu.RLock()
	defer c.historyMu.RUnlock()

	history, ok := c.history[vmID]
	if !ok {
		return nil
	}

	history.mu.Lock()
	defer history.mu.Unlock()

	if len(history.metrics) == 0 {
		return nil
	}

	latest := history.metrics[len(history.metrics)-1]
	return &latest
}

// GetMetricsHistory returns metrics history for a VM.
func (c *Collector) GetMetricsHistory(vmID string) []VMCMetrics {
	c.historyMu.RLock()
	defer c.historyMu.RUnlock()

	history, ok := c.history[vmID]
	if !ok {
		return nil
	}

	history.mu.Lock()
	defer history.mu.Unlock()

	result := make([]VMCMetrics, len(history.metrics))
	copy(result, history.metrics)
	return result
}

// GetAllMetrics returns latest metrics for all VMs.
func (c *Collector) GetAllMetrics() []VMCMetrics {
	c.historyMu.RLock()
	defer c.historyMu.RUnlock()

	result := make([]VMCMetrics, 0, len(c.history))
	for _, history := range c.history {
		history.mu.Lock()
		if len(history.metrics) > 0 {
			latest := history.metrics[len(history.metrics)-1]
			result = append(result, latest)
		}
		history.mu.Unlock()
	}
	return result
}
