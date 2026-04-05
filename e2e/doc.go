// Package e2e provides end-to-end tests for the CHV platform.
//
// These tests verify the full system behavior by making HTTP calls to a
// running controller instance. They are designed to run against a Docker
// Compose environment.
//
// Quick Start:
//
//	# Run full E2E suite (starts Docker, runs tests, cleans up)
//	make e2e
//
//	# Or manually:
//	docker compose up -d
//	go test -v ./e2e/...
//	docker compose down
//
// Environment Variables:
//
//   - CHV_E2E_URL: Controller URL (default: http://localhost:8080)
//   - CHV_E2E_TIMEOUT: Request timeout (default: 30s)
//
// Test Organization:
//
//   - vm_lifecycle_test.go: VM creation, start, stop, delete
//   - node_operations_test.go: Node registration and management
//   - harness_test.go: Test utilities and helpers
//
// Notes:
//   - Tests assume a fresh database on each run
//   - VM operations require a running agent for full testing
//   - Without an agent, tests verify API responses only
package e2e
