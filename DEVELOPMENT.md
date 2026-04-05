# Development Guide

This guide provides detailed instructions for setting up a CHV development environment.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Development Environment](#development-environment)
- [Testing](#testing)
- [Debugging](#debugging)
- [Common Tasks](#common-tasks)
- [Troubleshooting](#troubleshooting)

## Prerequisites

### Required

- **Go 1.22+**: [Download](https://golang.org/dl/)
- **Docker 20.10+**: [Install](https://docs.docker.com/get-docker/)
- **Docker Compose 2.0+**: Included with Docker Desktop

### Optional (for VM testing)

- **Linux host with KVM**: For running actual VMs
- **cloud-hypervisor binary**: The VMM binary
- **qemu-img**: For image format conversion

### macOS Users

CHV requires Linux for VM operations. On macOS:

1. Use Docker Desktop with virtualization
2. Develop code locally, test in Docker containers
3. For full VM testing, use a Linux VM or remote Linux host

## Quick Start

```bash
# Clone the repository
git clone https://github.com/yourusername/chv.git
cd chv

# Start the development environment
make docker-up

# Verify it's working
curl http://localhost:8081/health
```

## Development Environment

### Using Docker Compose (Recommended)

This provides a complete environment with PostgreSQL and the controller.

```bash
# Start all services
make docker-up

# View logs
make docker-logs

# Stop services
make docker-down

# Clean up (removes volumes)
make docker-clean
```

**Services:**
- PostgreSQL: `localhost:5433`
- Controller HTTP: `localhost:8081`
- Controller gRPC: `localhost:9092`

### Local Development (without Docker)

For faster iteration when working on specific components:

```bash
# 1. Start PostgreSQL locally
# Install PostgreSQL 16 and create database:
# CREATE DATABASE chv;
# CREATE USER chv WITH PASSWORD 'chv';
# GRANT ALL PRIVILEGES ON DATABASE chv TO chv;

# 2. Run database migrations
psql -U chv -d chv -f configs/schema.sql

# 3. Run controller locally
export CHV_DATABASE_URL="postgres://chv:chv@localhost:5432/chv?sslmode=disable"
go run ./cmd/chv-controller

# 4. In another terminal, run agent locally
go run ./cmd/chv-agent -node-id test-node -controller localhost:9090
```

### IDE Setup

#### VS Code

Recommended extensions:
- Go (golang.go)
- Docker (ms-azuretools.vscode-docker)
- PostgreSQL (ckolkman.vscode-postgres)

Settings (`settings.json`):
```json
{
  "go.formatTool": "gofmt",
  "go.lintTool": "golangci-lint",
  "go.testFlags": ["-v", "-race"],
  "go.coverOnSave": true,
  "go.coverageDecorator": {
    "type": "gutter",
    "coveredHighlightColor": "rgba(64,128,128,0.5)",
    "uncoveredHighlightColor": "rgba(128,64,64,0.5)"
  }
}
```

#### GoLand / IntelliJ

1. Import project as Go module
2. Enable Go modules integration
3. Configure run configurations for controller and agent

## Testing

### Running Tests

```bash
# Run all tests
make test

# Run with race detector
make test-race

# Run with coverage
make test-coverage

# Run specific package tests
go test -v ./internal/hypervisor/...
go test -v ./internal/reconcile/...
go test -v ./internal/agent/...

# Run specific test
go test -v -run TestValidateSafeForPath ./pkg/uuidx/...
```

### Test Structure

```
package/
├── code.go
├── code_test.go          # Unit tests
├── integration_test.go   # Integration tests (if needed)
└── mock_test.go          # Mock implementations
```

### Writing Tests

#### Unit Tests

```go
func TestMyFunction(t *testing.T) {
    tests := []struct {
        name    string
        input   string
        want    string
        wantErr bool
    }{
        {"valid case", "input", "output", false},
        {"error case", "bad", "", true},
    }
    
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            got, err := MyFunction(tt.input)
            if (err != nil) != tt.wantErr {
                t.Errorf("MyFunction() error = %v, wantErr %v", err, tt.wantErr)
                return
            }
            if got != tt.want {
                t.Errorf("MyFunction() = %v, want %v", got, tt.want)
            }
        })
    }
}
```

#### Mocking

Use interfaces for testability:

```go
// In production code
type Store interface {
    GetVM(ctx context.Context, id uuid.UUID) (*models.VM, error)
}

// In test
-type mockStore struct {
    vms map[uuid.UUID]*models.VM
}

func (m *mockStore) GetVM(ctx context.Context, id uuid.UUID) (*models.VM, error) {
    return m.vms[id], nil
}
```

### E2E Tests

E2E tests require a running environment:

```bash
# Start environment
make docker-up

# Run E2E tests
go test -v ./e2e/... -timeout 5m

# Or use the pre-built binary
./e2e.test
```

## Debugging

### Logging

CHV uses structured logging. Set log level:

```bash
export CHV_LOG_LEVEL=debug  # debug, info, warn, error
```

View logs:

```bash
# Docker logs
docker-compose logs -f controller
docker-compose logs -f agent

# Local logs (if redirected to files)
tail -f /var/log/chv/*.log
```

### Using Delve

Debug the controller:

```bash
# Install delve
go install github.com/go-delve/delve/cmd/dlv@latest

# Debug controller
dlv debug ./cmd/chv-controller

# Inside delve:
(dlv) break main.main
(dlv) continue
(dlv) print variable
(dlv) stack
(dlv) quit
```

### Debugging Tests

```bash
# Debug specific test
dlv test ./internal/hypervisor -- -test.run TestLauncher_StartVM

# In VS Code, use "Debug Test" code lens above test function
```

## Common Tasks

### Adding a New API Endpoint

1. Define handler in `internal/api/`:
```go
func (h *Handler) myNewEndpoint(w http.ResponseWriter, r *http.Request) {
    // Implementation
}
```

2. Register in `internal/api/handler.go`:
```go
router.Post("/api/v1/my-endpoint", h.authMiddleware(h.myNewEndpoint))
```

3. Add tests in `internal/api/handler_test.go`

4. Update README with endpoint documentation

### Adding a Database Migration

1. Create migration file in `configs/migrations/`:
```sql
-- 0001_add_new_table.sql
CREATE TABLE new_table (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL
);
```

2. Update `configs/schema.sql` with final state

3. Add model in `internal/models/`

4. Add store methods in `internal/store/`

### Adding a New Scheduler Strategy

1. Implement strategy in `internal/scheduler/strategy.go`:
```go
type MyStrategy struct{}

func (s *MyStrategy) SelectNode(candidates []*models.Node, spec *models.VMSpec) *models.Node {
    // Implementation
}
```

2. Register in `internal/scheduler/service.go`:
```go
strategies["my_strategy"] = &MyStrategy{}
```

3. Add tests

## Troubleshooting

### Build Issues

```bash
# Clean build cache
go clean -cache
go clean -modcache

# Re-download dependencies
go mod download

# Tidy modules
go mod tidy
```

### Docker Issues

```bash
# Reset Docker environment
make docker-down
make docker-clean
make docker-up

# Rebuild images
make docker-build

# Check service health
docker-compose ps
docker-compose logs
```

### Database Connection Issues

```bash
# Test PostgreSQL connection
psql -h localhost -p 5433 -U chv -d chv

# Reset database
docker-compose exec postgres psql -U chv -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"
docker-compose exec postgres psql -U chv -d chv -f /docker-entrypoint-initdb.d/01-schema.sql
```

### Test Failures

```bash
# Run with verbose output
go test -v ./...

# Run without cache
go test -count=1 ./...

# Run specific failing test
go test -v -run TestName ./package/...

# Check for race conditions
go test -race ./...
```

### Port Already in Use

```bash
# Find process using port
lsof -i :8080
lsof -i :5432

# Kill process or use different ports
export CHV_HTTP_ADDR=:8082
```

## Performance Profiling

### CPU Profiling

```bash
# Run with profiling
go test -cpuprofile=cpu.prof -bench=. ./...

# Analyze
go tool pprof cpu.prof
(pprof) top
(pprof) web  # Open in browser
```

### Memory Profiling

```bash
# Run with profiling
go test -memprofile=mem.prof -bench=. ./...

# Analyze
go tool pprof mem.prof
```

## Continuous Integration

### Pre-commit Checks

Run before committing:

```bash
#!/bin/bash
# .git/hooks/pre-commit or pre-commit script

set -e

echo "Running gofmt..."
gofmt -d .

echo "Running go vet..."
go vet ./...

echo "Running tests..."
go test -race ./...

echo "Running coverage..."
go test -coverprofile=coverage.out ./...
go tool cover -func=coverage.out | grep total

echo "All checks passed!"
```

### GitHub Actions (Example)

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-go@v4
        with:
          go-version: '1.22'
      - run: go mod download
      - run: go test -race ./...
      - run: go build ./...
```

## Getting Help

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: Questions and general discussion
- **Discord/Slack**: [Community chat link]

## Resources

- [Go Documentation](https://golang.org/doc/)
- [Cloud Hypervisor Docs](https://github.com/cloud-hypervisor/cloud-hypervisor/tree/main/docs)
- [PostgreSQL Documentation](https://www.postgresql.org/docs/)
- [Docker Documentation](https://docs.docker.com/)
