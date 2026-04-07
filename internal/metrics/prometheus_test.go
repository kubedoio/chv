package metrics

import (
	"testing"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/testutil"
	"github.com/stretchr/testify/assert"
)

func TestVMCountMetric(t *testing.T) {
	// Reset the metric
	VMCount.Reset()

	// Set some values
	VMCount.WithLabelValues("running").Set(5)
	VMCount.WithLabelValues("stopped").Set(3)

	// Test the values
	runningCount := testutil.ToFloat64(VMCount.WithLabelValues("running"))
	stoppedCount := testutil.ToFloat64(VMCount.WithLabelValues("stopped"))

	assert.Equal(t, 5.0, runningCount)
	assert.Equal(t, 3.0, stoppedCount)
}

func TestVMCreatedMetric(t *testing.T) {
	// Get initial count
	initialCount := testutil.ToFloat64(VMCreated.WithLabelValues("success"))

	// Increment
	VMCreated.WithLabelValues("success").Inc()
	VMCreated.WithLabelValues("success").Inc()
	VMCreated.WithLabelValues("failed").Inc()

	// Test the values
	successCount := testutil.ToFloat64(VMCreated.WithLabelValues("success"))
	failedCount := testutil.ToFloat64(VMCreated.WithLabelValues("failed"))

	assert.Equal(t, initialCount+2, successCount)
	assert.Equal(t, 1.0, failedCount)
}

func TestVMDeletedMetric(t *testing.T) {
	// Increment
	VMDeleted.WithLabelValues("success").Inc()

	// Test the value
	count := testutil.ToFloat64(VMDeleted.WithLabelValues("success"))
	assert.GreaterOrEqual(t, count, 1.0)
}

func TestVMOperationsMetric(t *testing.T) {
	// Record operations
	VMOperations.WithLabelValues("start", "success").Inc()
	VMOperations.WithLabelValues("stop", "success").Inc()
	VMOperations.WithLabelValues("start", "failed").Inc()

	// Test values
	startSuccess := testutil.ToFloat64(VMOperations.WithLabelValues("start", "success"))
	stopSuccess := testutil.ToFloat64(VMOperations.WithLabelValues("stop", "success"))
	startFailed := testutil.ToFloat64(VMOperations.WithLabelValues("start", "failed"))

	assert.GreaterOrEqual(t, startSuccess, 1.0)
	assert.GreaterOrEqual(t, stopSuccess, 1.0)
	assert.GreaterOrEqual(t, startFailed, 1.0)
}

func TestVMLifecycleDurationMetric(t *testing.T) {
	// Record duration
	VMLifecycleDuration.WithLabelValues("create").Observe(1.5)
	VMLifecycleDuration.WithLabelValues("start").Observe(0.5)

	// Just verify no panic occurs
	assert.True(t, true)
}

func TestResourceUsageMetrics(t *testing.T) {
	// Set values
	CPUUsage.Set(8)
	MemoryUsage.Set(16 * 1024 * 1024 * 1024) // 16 GB

	// Test values
	cpu := testutil.ToFloat64(CPUUsage)
	memory := testutil.ToFloat64(MemoryUsage)

	assert.Equal(t, 8.0, cpu)
	assert.Equal(t, 16*1024*1024*1024.0, memory)
}

func TestDiskUsageMetric(t *testing.T) {
	// Reset
	DiskUsage.Reset()

	// Set values
	DiskUsage.WithLabelValues("default").Set(100 * 1024 * 1024 * 1024) // 100 GB
	DiskUsage.WithLabelValues("ssd").Set(50 * 1024 * 1024 * 1024)      // 50 GB

	// Test values
	defaultPool := testutil.ToFloat64(DiskUsage.WithLabelValues("default"))
	ssdPool := testutil.ToFloat64(DiskUsage.WithLabelValues("ssd"))

	assert.Equal(t, 100*1024*1024*1024.0, defaultPool)
	assert.Equal(t, 50*1024*1024*1024.0, ssdPool)
}

func TestAPIRequestsMetric(t *testing.T) {
	// Record requests
	APIRequests.WithLabelValues("GET", "/api/v1/vms", "200").Inc()
	APIRequests.WithLabelValues("POST", "/api/v1/vms", "201").Inc()
	APIRequests.WithLabelValues("GET", "/api/v1/vms/:id", "404").Inc()

	// Just verify no panic occurs
	assert.True(t, true)
}

func TestAPILatencyMetric(t *testing.T) {
	// Record latencies
	APILatency.WithLabelValues("GET", "/api/v1/vms").Observe(0.01)
	APILatency.WithLabelValues("POST", "/api/v1/vms").Observe(0.1)

	// Just verify no panic occurs
	assert.True(t, true)
}

func TestAgentConnectionsMetric(t *testing.T) {
	// Set value
	AgentConnections.Set(3)

	// Test value
	count := testutil.ToFloat64(AgentConnections)
	assert.Equal(t, 3.0, count)
}

func TestAgentHeartbeatMetric(t *testing.T) {
	// Record heartbeat
	now := time.Now()
	AgentHeartbeat.WithLabelValues("agent-1").Set(float64(now.Unix()))

	// Test value
	timestamp := testutil.ToFloat64(AgentHeartbeat.WithLabelValues("agent-1"))
	assert.Equal(t, float64(now.Unix()), timestamp)
}

func TestOperationsActiveMetric(t *testing.T) {
	// Increment and decrement
	OperationsActive.Inc()
	OperationsActive.Inc()
	OperationsActive.Dec()

	// Test value
	count := testutil.ToFloat64(OperationsActive)
	assert.Equal(t, 1.0, count)
}

func TestOperationDurationMetric(t *testing.T) {
	// Record duration
	OperationDuration.WithLabelValues("vm_create").Observe(2.5)
	OperationDuration.WithLabelValues("vm_start").Observe(0.8)

	// Just verify no panic occurs
	assert.True(t, true)
}

func TestErrorsMetric(t *testing.T) {
	// Record errors
	Errors.WithLabelValues("database_error", "store").Inc()
	Errors.WithLabelValues("api_error", "handler").Inc()

	// Test values
	dbErrors := testutil.ToFloat64(Errors.WithLabelValues("database_error", "store"))
	apiErrors := testutil.ToFloat64(Errors.WithLabelValues("api_error", "handler"))

	assert.GreaterOrEqual(t, dbErrors, 1.0)
	assert.GreaterOrEqual(t, apiErrors, 1.0)
}

func TestNodeCountMetric(t *testing.T) {
	// Reset
	NodeCount.Reset()

	// Set values
	NodeCount.WithLabelValues("online").Set(3)
	NodeCount.WithLabelValues("offline").Set(1)

	// Test values
	online := testutil.ToFloat64(NodeCount.WithLabelValues("online"))
	offline := testutil.ToFloat64(NodeCount.WithLabelValues("offline"))

	assert.Equal(t, 3.0, online)
	assert.Equal(t, 1.0, offline)
}

func TestImageCountMetric(t *testing.T) {
	// Reset
	ImageCount.Reset()

	// Set values
	ImageCount.WithLabelValues("ready").Set(5)
	ImageCount.WithLabelValues("importing").Set(2)

	// Test values
	ready := testutil.ToFloat64(ImageCount.WithLabelValues("ready"))
	importing := testutil.ToFloat64(ImageCount.WithLabelValues("importing"))

	assert.Equal(t, 5.0, ready)
	assert.Equal(t, 2.0, importing)
}

func TestSchedulerMetrics(t *testing.T) {
	// Record placement duration
	SchedulerPlacementDuration.Observe(0.5)

	// Record placement failure
	initialFailures := testutil.ToFloat64(SchedulerPlacementFailures)
	SchedulerPlacementFailures.Inc()
	failures := testutil.ToFloat64(SchedulerPlacementFailures)

	assert.Equal(t, initialFailures+1, failures)
}

func TestMetricRegistration(t *testing.T) {
	// Verify all metrics are registered by collecting them
	registry := prometheus.NewRegistry()

	// Register all metrics
	registry.MustRegister(
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

	// Collect metrics
	metrics, err := registry.Gather()
	assert.NoError(t, err)
	assert.NotEmpty(t, metrics)
}
