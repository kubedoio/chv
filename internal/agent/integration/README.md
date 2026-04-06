# Cloud Hypervisor Integration Tests

This package contains integration tests for Cloud Hypervisor that run against a real CH binary.

## Prerequisites

1. **Cloud Hypervisor binary**: The tests expect a CH binary at `bin/cloud-hypervisor` by default.
   - Version: v51.1 or compatible
   - Can override with `CH_BINARY` environment variable

2. **Root privileges**: Required for TAP device management

3. **KVM**: `/dev/kvm` must be available

## Running Tests

### Run all integration tests:
```bash
cd /srv/data02/projects/chv
go test -v ./internal/agent/integration/...
```

### Run with custom CH binary:
```bash
CH_BINARY=/path/to/cloud-hypervisor go test -v ./internal/agent/integration/...
```

### Run specific test:
```bash
go test -v ./internal/agent/integration/ -run TestVMLifecycleCreateBootShutdown
```

### Skip integration tests (run only unit tests):
```bash
go test -v ./internal/agent/integration/ -short
```

## Test Files

- `helpers_test.go` - Test environment setup and helpers
- `integration_test.go` - CH client operation tests
- `vm_lifecycle_test.go` - Full VM lifecycle tests
- `console_test.go` - Console connection tests

## Test Categories

### Integration Tests
Tests that verify CH client operations against a real CH process:
- `TestCHClientOperations` - Basic CH client operations (ping, info, pause, resume)
- `TestMultipleVMs` - Multiple concurrent VMs
- `TestCHProcessLifecycle` - CH process lifecycle
- `TestLogFiles` - Log file generation

### VM Lifecycle Tests
Tests for complete VM lifecycle:
- `TestVMLifecycleCreateBootShutdown` - Full lifecycle
- `TestVMLifecycleRestart` - VM restart
- `TestVMManagerIntegration` - VM manager integration
- `TestVMIdempotency` - Operation idempotency
- `TestVMStatePersistence` - State persistence
- `TestVMCleanupOnFailure` - Cleanup on failure
- `TestConcurrentVMOperations` - Concurrent operations
- `TestVMMetricsCollection` - Metrics collection

### Console Tests
Tests for console functionality:
- `TestConsoleConnection` - Console connection
- `TestConsoleSessionManagement` - Session management
- `TestSerialConsoleLog` - Serial console logs
- `TestConsoleStreamWithMock` - Console streaming

## Test Isolation

Each test:
1. Creates isolated temp directories for state, VMs, images, sockets, and logs
2. Uses unique VM IDs to avoid conflicts
3. Cleans up all resources after completion (even on failure)
4. Skips gracefully if CH binary or KVM not available

## Environment Variables

- `CH_BINARY` - Path to cloud-hypervisor binary (default: `../../../bin/cloud-hypervisor`)

## Notes

- Tests skip automatically if CH binary not found or not executable
- Tests skip automatically if not running as root
- Tests skip automatically if KVM not available
- Each VM uses minimal resources (2 vCPUs, 512MB RAM, 100MB disk)
- Tests use temp directories that are cleaned up after each test
