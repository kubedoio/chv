package metrics

import (
	"context"
	"time"

	"github.com/chv/chv/internal/agentapi"
	"github.com/chv/chv/internal/agentclient"
	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/logger"
)

type Collector struct {
	repo        *db.Repository
	agentClient *agentclient.Client
	interval    time.Duration
	stopChan    chan struct{}
}

func NewCollector(repo *db.Repository) *Collector {
	return &Collector{
		repo:     repo,
		interval: 15 * time.Second,
		stopChan: make(chan struct{}),
	}
}

// SetAgentClient sets the agent client for collecting VM metrics
func (c *Collector) SetAgentClient(client *agentclient.Client) {
	c.agentClient = client
}

func (c *Collector) Start() {
	logger.L().Info("Starting metrics collector", logger.F("interval", c.interval))
	
	ticker := time.NewTicker(c.interval)
	defer ticker.Stop()

	// Do an initial collection immediately
	c.collect()

	for {
		select {
		case <-ticker.C:
			c.collect()
		case <-c.stopChan:
			logger.L().Info("Metrics collector stopped")
			return
		}
	}
}

func (c *Collector) collect() {
	ctx := context.Background()

	// Get all VMs and update metrics
	vms, err := c.repo.ListVMs(ctx)
	if err != nil {
		logger.L().Error("Failed to list VMs for metrics", logger.ErrorField(err))
		return
	}

	// Reset VM count metrics before setting new values
	VMCount.Reset()

	// Count by state
	stateCounts := make(map[string]map[string]int)
	for _, vm := range vms {
		if stateCounts[vm.NodeID] == nil {
			stateCounts[vm.NodeID] = make(map[string]int)
		}
		stateCounts[vm.NodeID][vm.ActualState]++
	}

	for nodeID, counts := range stateCounts {
		for state, count := range counts {
			VMCount.WithLabelValues(nodeID, state).Set(float64(count))
		}
	}

	// Get all nodes and update health metrics
	nodes, err := c.repo.ListNodes(ctx)
	if err != nil {
		logger.L().Error("Failed to list nodes for metrics", logger.ErrorField(err))
		// Continue to collect VM metrics even if node list fails
		nodes = nil
	}

	for _, node := range nodes {
		health := 0.0
		if node.Status == "online" {
			health = 1.0
		}
		NodeHealth.WithLabelValues(node.ID).Set(health)
	}

	// For running VMs, fetch actual metrics from agent
	if c.agentClient != nil {
		for _, vm := range vms {
			if vm.ActualState == "running" && vm.CloudHypervisorPID > 0 {
				req := &agentapi.VMMetricsRequest{
					VMID:      vm.ID,
					PID:       vm.CloudHypervisorPID,
					APISocket: vm.WorkspacePath + "/api.sock",
				}
				resp, err := c.agentClient.GetVMMetrics(ctx, req)
				if err != nil {
					logger.L().Debug("Failed to get VM metrics",
						logger.F("vm_id", vm.ID),
						logger.ErrorField(err))
					continue
				}
				VMCPUUsage.WithLabelValues(vm.ID, vm.NodeID).Set(resp.CPU.UsagePercent)
				VMMemoryUsage.WithLabelValues(vm.ID, vm.NodeID).Set(float64(resp.Memory.UsedMB * 1024 * 1024))
			} else if vm.ActualState == "running" {
				// Running but no PID yet — use placeholder
				VMCPUUsage.WithLabelValues(vm.ID, vm.NodeID).Set(0)
				VMMemoryUsage.WithLabelValues(vm.ID, vm.NodeID).Set(float64(vm.MemoryMB * 1024 * 1024))
			}
		}
	} else {
		// No agent client — use placeholder values
		for _, vm := range vms {
			if vm.ActualState == "running" {
				VMCPUUsage.WithLabelValues(vm.ID, vm.NodeID).Set(0)
				VMMemoryUsage.WithLabelValues(vm.ID, vm.NodeID).Set(float64(vm.MemoryMB * 1024 * 1024))
			}
		}
	}

	logger.L().Debug("Metrics collection completed",
		logger.F("vm_count", len(vms)),
		logger.F("node_count", len(nodes)))
}

func (c *Collector) Stop() {
	close(c.stopChan)
}
