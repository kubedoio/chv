package reconcile

import (
	"context"
	"log"
	"time"

	"github.com/chv/chv/internal/metrics"
)

// recordVMMetrics collects and records VM-related metrics.
// It updates VM counts by state and resource usage statistics.
func (s *Service) recordVMMetrics() {
	ctx := context.Background()

	vms, err := s.store.ListVMs(ctx)
	if err != nil {
		log.Printf("Failed to list VMs for metrics: %v", err)
		return
	}

	// Count by state
	stateCounts := make(map[string]float64)
	var totalCPU, totalMemory int64

	for _, vm := range vms {
		state := string(vm.ActualState)
		if state == "" {
			state = "unknown"
		}
		stateCounts[state]++

		spec, err := vm.GetSpec()
		if err == nil && spec != nil {
			totalCPU += int64(spec.CPU)
			totalMemory += int64(spec.MemoryMB) * 1024 * 1024 // Convert MB to bytes
		}
	}

	// Reset and update VM count gauge
	metrics.VMCount.Reset()
	for state, count := range stateCounts {
		metrics.VMCount.WithLabelValues(state).Set(count)
	}

	// Update resource usage gauges
	metrics.CPUUsage.Set(float64(totalCPU))
	metrics.MemoryUsage.Set(float64(totalMemory))
}

// recordNodeMetrics collects and records node-related metrics.
func (s *Service) recordNodeMetrics() {
	ctx := context.Background()

	nodes, err := s.store.ListNodes(ctx)
	if err != nil {
		log.Printf("Failed to list nodes for metrics: %v", err)
		return
	}

	// Count by status
	statusCounts := make(map[string]float64)
	var connectedAgents float64

	for _, node := range nodes {
		status := string(node.Status)
		if status == "" {
			status = "unknown"
		}
		statusCounts[status]++

		if node.Status == "online" {
			connectedAgents++
		}

		// Update heartbeat timestamp for online nodes
		if node.LastHeartbeatAt != nil {
			metrics.AgentHeartbeat.WithLabelValues(node.ID.String()).Set(float64(node.LastHeartbeatAt.Unix()))
		}
	}

	// Reset and update node count gauge
	metrics.NodeCount.Reset()
	for status, count := range statusCounts {
		metrics.NodeCount.WithLabelValues(status).Set(count)
	}

	// Update agent connections gauge
	metrics.AgentConnections.Set(connectedAgents)
}

// recordImageMetrics collects and records image-related metrics.
func (s *Service) recordImageMetrics() {
	ctx := context.Background()

	images, err := s.store.ListImages(ctx)
	if err != nil {
		log.Printf("Failed to list images for metrics: %v", err)
		return
	}

	// Count by status
	statusCounts := make(map[string]float64)

	for _, image := range images {
		status := string(image.Status)
		if status == "" {
			status = "unknown"
		}
		statusCounts[status]++
	}

	// Reset and update image count gauge
	metrics.ImageCount.Reset()
	for status, count := range statusCounts {
		metrics.ImageCount.WithLabelValues(status).Set(count)
	}
}

// startMetricsCollection starts a background goroutine that periodically collects metrics.
func (s *Service) startMetricsCollection(ctx context.Context) {
	ticker := time.NewTicker(30 * time.Second)
	defer ticker.Stop()

	// Collect metrics immediately on start
	s.recordVMMetrics()
	s.recordNodeMetrics()
	s.recordImageMetrics()

	for {
		select {
		case <-ticker.C:
			s.recordVMMetrics()
			s.recordNodeMetrics()
			s.recordImageMetrics()
		case <-ctx.Done():
			return
		case <-s.stopCh:
			return
		}
	}
}

// RecordOperationStart records the start of an operation.
func RecordOperationStart(opType string) {
	metrics.OperationsActive.Inc()
}

// RecordOperationFinish records the completion of an operation.
func RecordOperationFinish(opType string, start time.Time, err error) {
	metrics.OperationsActive.Dec()
	metrics.OperationDuration.WithLabelValues(opType).Observe(time.Since(start).Seconds())

	if err != nil {
		metrics.Errors.WithLabelValues("operation_failed", opType).Inc()
	}
}

// RecordVMCreated records a VM creation event.
func RecordVMCreated(success bool) {
	status := "success"
	if !success {
		status = "failed"
	}
	metrics.VMCreated.WithLabelValues(status).Inc()
}

// RecordVMDeleted records a VM deletion event.
func RecordVMDeleted(success bool) {
	status := "success"
	if !success {
		status = "failed"
	}
	metrics.VMDeleted.WithLabelValues(status).Inc()
}

// RecordVMOperation records a VM operation (start, stop, reboot).
func RecordVMOperation(operation string, success bool) {
	status := "success"
	if !success {
		status = "failed"
	}
	metrics.VMOperations.WithLabelValues(operation, status).Inc()
}

// RecordVMLifecycleDuration records the duration of a VM lifecycle operation.
func RecordVMLifecycleDuration(operation string, duration time.Duration) {
	metrics.VMLifecycleDuration.WithLabelValues(operation).Observe(duration.Seconds())
}

// RecordError records an error occurrence.
func RecordError(errorType, component string) {
	metrics.Errors.WithLabelValues(errorType, component).Inc()
}
