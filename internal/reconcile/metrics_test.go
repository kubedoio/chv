package reconcile

import (
	"errors"
	"testing"
	"time"

	"github.com/chv/chv/internal/metrics"
	"github.com/prometheus/client_golang/prometheus/testutil"
	"github.com/stretchr/testify/assert"
)

func TestRecordOperationStart(t *testing.T) {
	// Get initial count
	initialCount := testutil.ToFloat64(metrics.OperationsActive)

	// Record operation start
	RecordOperationStart("vm_create")

	// Verify active operations increased
	count := testutil.ToFloat64(metrics.OperationsActive)
	assert.Equal(t, initialCount+1, count)
}

func TestRecordOperationFinish(t *testing.T) {
	// Start an operation first
	RecordOperationStart("vm_create")
	initialActive := testutil.ToFloat64(metrics.OperationsActive)

	// Record successful finish
	start := time.Now()
	RecordOperationFinish("vm_create", start, nil)

	// Verify active operations decreased
	activeCount := testutil.ToFloat64(metrics.OperationsActive)
	assert.Equal(t, initialActive-1, activeCount)
}

func TestRecordOperationFinishWithError(t *testing.T) {
	// Get initial error count
	initialErrors := testutil.ToFloat64(metrics.Errors.WithLabelValues("operation_failed", "vm_create"))

	// Record failed finish
	start := time.Now()
	RecordOperationFinish("vm_create", start, errors.New("test error"))

	// Verify error was recorded
	errorCount := testutil.ToFloat64(metrics.Errors.WithLabelValues("operation_failed", "vm_create"))
	assert.Equal(t, initialErrors+1, errorCount)
}

func TestRecordVMCreated(t *testing.T) {
	// Get initial counts
	initialSuccess := testutil.ToFloat64(metrics.VMCreated.WithLabelValues("success"))
	initialFailed := testutil.ToFloat64(metrics.VMCreated.WithLabelValues("failed"))

	// Record successful creation
	RecordVMCreated(true)

	// Record failed creation
	RecordVMCreated(false)
	RecordVMCreated(false)

	// Verify counts
	successCount := testutil.ToFloat64(metrics.VMCreated.WithLabelValues("success"))
	failedCount := testutil.ToFloat64(metrics.VMCreated.WithLabelValues("failed"))

	assert.Equal(t, initialSuccess+1, successCount)
	assert.Equal(t, initialFailed+2, failedCount)
}

func TestRecordVMDeleted(t *testing.T) {
	// Get initial count
	initialSuccess := testutil.ToFloat64(metrics.VMDeleted.WithLabelValues("success"))
	initialFailed := testutil.ToFloat64(metrics.VMDeleted.WithLabelValues("failed"))

	// Record successful deletion
	RecordVMDeleted(true)
	RecordVMDeleted(true)

	// Record failed deletion
	RecordVMDeleted(false)

	// Verify counts
	successCount := testutil.ToFloat64(metrics.VMDeleted.WithLabelValues("success"))
	failedCount := testutil.ToFloat64(metrics.VMDeleted.WithLabelValues("failed"))

	assert.Equal(t, initialSuccess+2, successCount)
	assert.Equal(t, initialFailed+1, failedCount)
}

func TestRecordVMOperation(t *testing.T) {
	// Get initial counts
	initialStartSuccess := testutil.ToFloat64(metrics.VMOperations.WithLabelValues("start", "success"))
	initialStopSuccess := testutil.ToFloat64(metrics.VMOperations.WithLabelValues("stop", "success"))
	initialRebootFailed := testutil.ToFloat64(metrics.VMOperations.WithLabelValues("reboot", "failed"))

	// Record operations
	RecordVMOperation("start", true)
	RecordVMOperation("stop", true)
	RecordVMOperation("reboot", false)

	// Verify counts
	startSuccess := testutil.ToFloat64(metrics.VMOperations.WithLabelValues("start", "success"))
	stopSuccess := testutil.ToFloat64(metrics.VMOperations.WithLabelValues("stop", "success"))
	rebootFailed := testutil.ToFloat64(metrics.VMOperations.WithLabelValues("reboot", "failed"))

	assert.Equal(t, initialStartSuccess+1, startSuccess)
	assert.Equal(t, initialStopSuccess+1, stopSuccess)
	assert.Equal(t, initialRebootFailed+1, rebootFailed)
}

func TestRecordVMLifecycleDuration(t *testing.T) {
	// Record duration
	RecordVMLifecycleDuration("create", 2*time.Second)
	RecordVMLifecycleDuration("start", 500*time.Millisecond)

	// Just verify no panic occurs
	assert.True(t, true)
}

func TestRecordError(t *testing.T) {
	// Get initial count
	initialCount := testutil.ToFloat64(metrics.Errors.WithLabelValues("database_error", "store"))

	// Record errors
	RecordError("database_error", "store")
	RecordError("database_error", "store")

	// Verify count
	errorCount := testutil.ToFloat64(metrics.Errors.WithLabelValues("database_error", "store"))
	assert.Equal(t, initialCount+2, errorCount)
}

func TestMultipleErrorTypes(t *testing.T) {
	// Record different error types
	RecordError("database_error", "store")
	RecordError("api_error", "handler")
	RecordError("validation_error", "api")

	// Verify each was recorded
	dbErrors := testutil.ToFloat64(metrics.Errors.WithLabelValues("database_error", "store"))
	apiErrors := testutil.ToFloat64(metrics.Errors.WithLabelValues("api_error", "handler"))
	validationErrors := testutil.ToFloat64(metrics.Errors.WithLabelValues("validation_error", "api"))

	assert.GreaterOrEqual(t, dbErrors, 1.0)
	assert.GreaterOrEqual(t, apiErrors, 1.0)
	assert.GreaterOrEqual(t, validationErrors, 1.0)
}
