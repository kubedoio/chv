// Package agent provides agent-specific Prometheus metrics.
package agent

import (
	"github.com/prometheus/client_golang/prometheus"
)

var (
	// AgentVMCount tracks the number of VMs managed by this agent.
	AgentVMCount = prometheus.NewGauge(prometheus.GaugeOpts{
		Name: "chv_agent_vm_count",
		Help: "Number of VMs on this agent",
	})

	// AgentCPUAvailable tracks the available CPU cores on this agent.
	AgentCPUAvailable = prometheus.NewGauge(prometheus.GaugeOpts{
		Name: "chv_agent_cpu_available",
		Help: "Available CPU cores on this agent",
	})

	// AgentCPUUsed tracks the used CPU cores on this agent.
	AgentCPUUsed = prometheus.NewGauge(prometheus.GaugeOpts{
		Name: "chv_agent_cpu_used",
		Help: "Used CPU cores on this agent",
	})

	// AgentMemoryAvailable tracks the available memory on this agent in bytes.
	AgentMemoryAvailable = prometheus.NewGauge(prometheus.GaugeOpts{
		Name: "chv_agent_memory_available_bytes",
		Help: "Available memory on this agent",
	})

	// AgentMemoryUsed tracks the used memory on this agent in bytes.
	AgentMemoryUsed = prometheus.NewGauge(prometheus.GaugeOpts{
		Name: "chv_agent_memory_used_bytes",
		Help: "Used memory on this agent",
	})

	// AgentDiskAvailable tracks the available disk space on this agent in bytes.
	AgentDiskAvailable = prometheus.NewGauge(prometheus.GaugeOpts{
		Name: "chv_agent_disk_available_bytes",
		Help: "Available disk space on this agent",
	})

	// AgentDiskUsed tracks the used disk space on this agent in bytes.
	AgentDiskUsed = prometheus.NewGauge(prometheus.GaugeOpts{
		Name: "chv_agent_disk_used_bytes",
		Help: "Used disk space on this agent",
	})

	// AgentHypervisorCalls counts hypervisor API calls by method.
	AgentHypervisorCalls = prometheus.NewCounterVec(prometheus.CounterOpts{
		Name: "chv_agent_hypervisor_calls_total",
		Help: "Total hypervisor API calls",
	}, []string{"method", "status"})

	// AgentHypervisorLatency measures hypervisor API call latency.
	AgentHypervisorLatency = prometheus.NewHistogramVec(prometheus.HistogramOpts{
		Name:    "chv_agent_hypervisor_latency_seconds",
		Help:    "Hypervisor API call latency",
		Buckets: prometheus.DefBuckets,
	}, []string{"method"})

	// AgentOperations counts operations performed by the agent.
	AgentOperations = prometheus.NewCounterVec(prometheus.CounterOpts{
		Name: "chv_agent_operations_total",
		Help: "Total operations by type",
	}, []string{"operation", "status"})

	// AgentOperationDuration measures operation duration.
	AgentOperationDuration = prometheus.NewHistogramVec(prometheus.HistogramOpts{
		Name:    "chv_agent_operation_duration_seconds",
		Help:    "Operation duration",
		Buckets: prometheus.DefBuckets,
	}, []string{"operation"})
)

func init() {
	// Register agent-specific metrics
	prometheus.MustRegister(
		AgentVMCount,
		AgentCPUAvailable,
		AgentCPUUsed,
		AgentMemoryAvailable,
		AgentMemoryUsed,
		AgentDiskAvailable,
		AgentDiskUsed,
		AgentHypervisorCalls,
		AgentHypervisorLatency,
		AgentOperations,
		AgentOperationDuration,
	)
}

// RecordAgentResources updates the agent resource metrics.
func RecordAgentResources(cpuAvailable, memoryAvailable, diskAvailable int64) {
	AgentCPUAvailable.Set(float64(cpuAvailable))
	AgentMemoryAvailable.Set(float64(memoryAvailable))
	AgentDiskAvailable.Set(float64(diskAvailable))
}

// RecordAgentResourceUsage updates the agent resource usage metrics.
func RecordAgentResourceUsage(cpuUsed, memoryUsed, diskUsed int64) {
	AgentCPUUsed.Set(float64(cpuUsed))
	AgentMemoryUsed.Set(float64(memoryUsed))
	AgentDiskUsed.Set(float64(diskUsed))
}

// RecordAgentVMCount updates the VM count metric.
func RecordAgentVMCount(count int) {
	AgentVMCount.Set(float64(count))
}

// RecordAgentOperation records an agent operation.
func RecordAgentOperation(operation string, success bool) {
	status := "success"
	if !success {
		status = "failed"
	}
	AgentOperations.WithLabelValues(operation, status).Inc()
}

// RecordAgentOperationDuration records the duration of an agent operation.
func RecordAgentOperationDuration(operation string, seconds float64) {
	AgentOperationDuration.WithLabelValues(operation).Observe(seconds)
}

// RecordHypervisorCall records a hypervisor API call.
func RecordHypervisorCall(method string, success bool) {
	status := "success"
	if !success {
		status = "failed"
	}
	AgentHypervisorCalls.WithLabelValues(method, status).Inc()
}

// RecordHypervisorLatency records hypervisor API call latency.
func RecordHypervisorLatency(method string, seconds float64) {
	AgentHypervisorLatency.WithLabelValues(method).Observe(seconds)
}
