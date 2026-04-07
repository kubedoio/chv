package agent

import (
	"testing"
	"time"

	"github.com/prometheus/client_golang/prometheus/testutil"
	"github.com/stretchr/testify/assert"
)

func TestRecordAgentResources(t *testing.T) {
	// Record resource availability
	RecordAgentResources(8, 32*1024*1024*1024, 500*1024*1024*1024)

	// Verify values
	cpuAvailable := testutil.ToFloat64(AgentCPUAvailable)
	memoryAvailable := testutil.ToFloat64(AgentMemoryAvailable)
	diskAvailable := testutil.ToFloat64(AgentDiskAvailable)

	assert.Equal(t, 8.0, cpuAvailable)
	assert.Equal(t, 32*1024*1024*1024.0, memoryAvailable)
	assert.Equal(t, 500*1024*1024*1024.0, diskAvailable)
}

func TestRecordAgentResourceUsage(t *testing.T) {
	// Record resource usage
	RecordAgentResourceUsage(4, 16*1024*1024*1024, 100*1024*1024*1024)

	// Verify values
	cpuUsed := testutil.ToFloat64(AgentCPUUsed)
	memoryUsed := testutil.ToFloat64(AgentMemoryUsed)
	diskUsed := testutil.ToFloat64(AgentDiskUsed)

	assert.Equal(t, 4.0, cpuUsed)
	assert.Equal(t, 16*1024*1024*1024.0, memoryUsed)
	assert.Equal(t, 100*1024*1024*1024.0, diskUsed)
}

func TestRecordAgentVMCount(t *testing.T) {
	// Record VM count
	RecordAgentVMCount(5)

	// Verify value
	count := testutil.ToFloat64(AgentVMCount)
	assert.Equal(t, 5.0, count)
}

func TestRecordAgentOperation(t *testing.T) {
	// Get initial counts
	initialStartSuccess := testutil.ToFloat64(AgentOperations.WithLabelValues("vm_start", "success"))
	initialStopSuccess := testutil.ToFloat64(AgentOperations.WithLabelValues("vm_stop", "success"))
	initialCreateFailed := testutil.ToFloat64(AgentOperations.WithLabelValues("vm_create", "failed"))

	// Record operations
	RecordAgentOperation("vm_start", true)
	RecordAgentOperation("vm_stop", true)
	RecordAgentOperation("vm_create", false)

	// Verify counts
	startSuccess := testutil.ToFloat64(AgentOperations.WithLabelValues("vm_start", "success"))
	stopSuccess := testutil.ToFloat64(AgentOperations.WithLabelValues("vm_stop", "success"))
	createFailed := testutil.ToFloat64(AgentOperations.WithLabelValues("vm_create", "failed"))

	assert.Equal(t, initialStartSuccess+1, startSuccess)
	assert.Equal(t, initialStopSuccess+1, stopSuccess)
	assert.Equal(t, initialCreateFailed+1, createFailed)
}

func TestRecordAgentOperationDuration(t *testing.T) {
	// Record durations
	RecordAgentOperationDuration("vm_start", 1.5)
	RecordAgentOperationDuration("vm_stop", 0.5)

	// Just verify no panic occurs
	assert.True(t, true)
}

func TestRecordHypervisorCall(t *testing.T) {
	// Get initial counts
	initialSuccess := testutil.ToFloat64(AgentHypervisorCalls.WithLabelValues("create_vm", "success"))
	initialFailed := testutil.ToFloat64(AgentHypervisorCalls.WithLabelValues("create_vm", "failed"))

	// Record calls
	RecordHypervisorCall("create_vm", true)
	RecordHypervisorCall("create_vm", true)
	RecordHypervisorCall("create_vm", false)

	// Verify counts
	successCount := testutil.ToFloat64(AgentHypervisorCalls.WithLabelValues("create_vm", "success"))
	failedCount := testutil.ToFloat64(AgentHypervisorCalls.WithLabelValues("create_vm", "failed"))

	assert.Equal(t, initialSuccess+2, successCount)
	assert.Equal(t, initialFailed+1, failedCount)
}

func TestRecordHypervisorLatency(t *testing.T) {
	// Record latencies
	RecordHypervisorLatency("create_vm", 0.1)
	RecordHypervisorLatency("start_vm", 0.05)
	RecordHypervisorLatency("stop_vm", 0.02)

	// Just verify no panic occurs
	assert.True(t, true)
}

func TestAgentMetricsIntegration(t *testing.T) {
	// Simulate a typical agent workflow

	// 1. Agent starts and reports resources
	RecordAgentResources(16, 64*1024*1024*1024, 1000*1024*1024*1024)

	// 2. VMs are created
	RecordAgentVMCount(3)
	RecordAgentResourceUsage(6, 24*1024*1024*1024, 150*1024*1024*1024)

	// 3. VM operations occur
	RecordAgentOperation("vm_start", true)
	RecordAgentOperationDuration("vm_start", 1.2)

	// 4. Hypervisor calls are made
	RecordHypervisorCall("create_vm", true)
	RecordHypervisorLatency("create_vm", 0.5)

	// Verify final state
	assert.Equal(t, 16.0, testutil.ToFloat64(AgentCPUAvailable))
	assert.Equal(t, 6.0, testutil.ToFloat64(AgentCPUUsed))
	assert.Equal(t, 3.0, testutil.ToFloat64(AgentVMCount))
}

func TestMultipleHypervisorMethods(t *testing.T) {
	methods := []string{"create_vm", "start_vm", "stop_vm", "delete_vm", "get_info"}

	for _, method := range methods {
		RecordHypervisorCall(method, true)
		RecordHypervisorLatency(method, 0.1)
	}

	// Verify all methods were recorded
	for _, method := range methods {
		count := testutil.ToFloat64(AgentHypervisorCalls.WithLabelValues(method, "success"))
		assert.GreaterOrEqual(t, count, 1.0, "Method %s should have at least 1 call", method)
	}
}

func TestAgentMetricsWithTime(t *testing.T) {
	// Record an operation with actual timing
	start := time.Now()
	time.Sleep(10 * time.Millisecond) // Small delay
	duration := time.Since(start).Seconds()

	RecordAgentOperationDuration("vm_create", duration)

	// Just verify no panic occurs
	assert.True(t, duration > 0)
}
