// Package metrics provides Prometheus metrics for CHV.
package metrics

import (
	"github.com/prometheus/client_golang/prometheus"
)

var (
	// VM metrics
	// VMCount tracks the number of VMs by their current state (e.g., running, stopped).
	VMCount = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Name: "chv_vm_count",
		Help: "Number of VMs by state",
	}, []string{"state"})

	// VMCreated counts the total number of VM creation attempts and their outcomes.
	VMCreated = prometheus.NewCounterVec(prometheus.CounterOpts{
		Name: "chv_vm_created_total",
		Help: "Total VMs created",
	}, []string{"status"})

	// VMDeleted counts the total number of VM deletion attempts and their outcomes.
	VMDeleted = prometheus.NewCounterVec(prometheus.CounterOpts{
		Name: "chv_vm_deleted_total",
		Help: "Total VMs deleted",
	}, []string{"status"})

	// VMOperations counts various VM lifecycle operations (start, stop, reboot) and their results.
	VMOperations = prometheus.NewCounterVec(prometheus.CounterOpts{
		Name: "chv_vm_operations_total",
		Help: "VM operations by type",
	}, []string{"operation", "status"})

	// VMLifecycleDuration measures the time taken for VM lifecycle operations.
	VMLifecycleDuration = prometheus.NewHistogramVec(prometheus.HistogramOpts{
		Name:    "chv_vm_lifecycle_duration_seconds",
		Help:    "Time taken for VM lifecycle operations",
		Buckets: prometheus.DefBuckets,
	}, []string{"operation"})

	// Resource metrics
	// CPUUsage tracks the total number of CPU cores currently allocated to VMs.
	CPUUsage = prometheus.NewGauge(prometheus.GaugeOpts{
		Name: "chv_cpu_usage_cores",
		Help: "Total CPU cores allocated to VMs",
	})

	// MemoryUsage tracks the total memory (in bytes) currently allocated to VMs.
	MemoryUsage = prometheus.NewGauge(prometheus.GaugeOpts{
		Name: "chv_memory_usage_bytes",
		Help: "Total memory allocated to VMs",
	})

	// DiskUsage tracks disk usage (in bytes) categorized by storage pool.
	DiskUsage = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Name: "chv_disk_usage_bytes",
		Help: "Disk usage by pool",
	}, []string{"pool"})

	// API metrics
	// APIRequests counts total HTTP API requests, labeled by method, endpoint, and status code.
	APIRequests = prometheus.NewCounterVec(prometheus.CounterOpts{
		Name: "chv_api_requests_total",
		Help: "Total API requests",
	}, []string{"method", "endpoint", "status"})

	// APILatency measures the latency of HTTP API requests in seconds.
	APILatency = prometheus.NewHistogramVec(prometheus.HistogramOpts{
		Name:    "chv_api_latency_seconds",
		Help:    "API request latency",
		Buckets: []float64{.001, .005, .01, .025, .05, .1, .25, .5, 1, 2.5, 5, 10},
	}, []string{"method", "endpoint"})

	// Agent metrics
	// AgentConnections tracks the current number of connected agents.
	AgentConnections = prometheus.NewGauge(prometheus.GaugeOpts{
		Name: "chv_agent_connections",
		Help: "Number of connected agents",
	})

	// AgentHeartbeat records the last heartbeat timestamp for each agent.
	AgentHeartbeat = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Name: "chv_agent_heartbeat_timestamp",
		Help: "Last heartbeat timestamp per agent",
	}, []string{"agent_id"})

	// Operation metrics
	// OperationsActive tracks the number of currently active (in-flight) operations.
	OperationsActive = prometheus.NewGauge(prometheus.GaugeOpts{
		Name: "chv_operations_active",
		Help: "Number of active operations",
	})

	// OperationDuration measures the duration of various operations in seconds.
	OperationDuration = prometheus.NewHistogramVec(prometheus.HistogramOpts{
		Name:    "chv_operation_duration_seconds",
		Help:    "Operation duration",
		Buckets: prometheus.DefBuckets,
	}, []string{"type"})

	// Error metrics
	// Errors counts total errors, categorized by type and component.
	Errors = prometheus.NewCounterVec(prometheus.CounterOpts{
		Name: "chv_errors_total",
		Help: "Total errors by type",
	}, []string{"type", "component"})

	// Node metrics
	// NodeCount tracks the number of nodes by their current status.
	NodeCount = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Name: "chv_node_count",
		Help: "Number of nodes by status",
	}, []string{"status"})

	// Image metrics
	// ImageCount tracks the number of images by their current status.
	ImageCount = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Name: "chv_image_count",
		Help: "Number of images by status",
	}, []string{"status"})

	// Scheduler metrics
	// SchedulerPlacementDuration measures the time taken to schedule/placement VMs.
	SchedulerPlacementDuration = prometheus.NewHistogram(prometheus.HistogramOpts{
		Name:    "chv_scheduler_placement_duration_seconds",
		Help:    "VM placement duration",
		Buckets: prometheus.DefBuckets,
	})

	// SchedulerPlacementFailures counts the total number of failed VM placements.
	SchedulerPlacementFailures = prometheus.NewCounter(prometheus.CounterOpts{
		Name: "chv_scheduler_placement_failures_total",
		Help: "Total VM placement failures",
	})
)

func init() {
	// Register all metrics
	prometheus.MustRegister(
		VMCount,
		VMCreated,
		VMDeleted,
		VMOperations,
		VMLifecycleDuration,
		CPUUsage,
		MemoryUsage,
		DiskUsage,
		APIRequests,
		APILatency,
		AgentConnections,
		AgentHeartbeat,
		OperationsActive,
		OperationDuration,
		Errors,
		NodeCount,
		ImageCount,
		SchedulerPlacementDuration,
		SchedulerPlacementFailures,
	)
}
