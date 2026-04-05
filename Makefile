# CHV - Cloud Hypervisor Virtualization Platform
# MVP-1 Build System

.PHONY: all build clean test docker-up docker-down proto e2e

# Variables
GO := go
DOCKER := docker
DOCKER_COMPOSE := docker compose

# Default target
all: build

# Generate protobuf code
proto:
	@echo "Generating protobuf code..."
	@which protoc > /dev/null || (echo "protoc not found, please install it" && exit 1)
	@which protoc-gen-go > /dev/null || go install google.golang.org/protobuf/cmd/protoc-gen-go@latest
	protoc --go_out=. --go_opt=paths=source_relative \
		--go-grpc_out=. --go-grpc_opt=paths=source_relative \
		internal/pb/agent/agent.proto

# Download dependencies
deps:
	$(GO) mod download
	$(GO) mod tidy

# Build all binaries
build: build-controller build-agent build-bootstrap

# Build controller
build-controller:
	@echo "Building chv-controller..."
	CGO_ENABLED=0 GOOS=linux $(GO) build -ldflags="-w -s" -o bin/chv-controller ./cmd/chv-controller

# Build agent
build-agent:
	@echo "Building chv-agent..."
	CGO_ENABLED=0 GOOS=linux $(GO) build -ldflags="-w -s" -o bin/chv-agent ./cmd/chv-agent

# Build bootstrap
build-bootstrap:
	@echo "Building chv-bootstrap..."
	CGO_ENABLED=0 GOOS=linux $(GO) build -ldflags="-w -s" -o bin/chv-bootstrap ./cmd/chv-bootstrap

# Run tests
test:
	$(GO) test -v ./...

# Run tests with coverage
test-coverage:
	$(GO) test -v -coverprofile=coverage.out ./...
	$(GO) tool cover -html=coverage.out -o coverage.html

# E2E Tests
e2e: e2e-setup e2e-run e2e-cleanup

# Setup E2E environment
e2e-setup:
	@echo "Setting up E2E environment..."
	$(DOCKER_COMPOSE) up -d --build postgres controller
	@echo "Waiting for services to be ready..."
	@sleep 5
	@for i in 1 2 3 4 5; do \
		if curl -s http://localhost:8081/health > /dev/null 2>&1; then \
			echo "Controller is ready!"; \
			exit 0; \
		fi; \
		echo "Waiting for controller... ($$i/5)"; \
		sleep 2; \
	done; \
	echo "Controller failed to start"; \
	exit 1

# Run E2E tests
e2e-run:
	@echo "Running E2E tests..."
	CHV_E2E_URL=http://localhost:8081 $(GO) test -v ./e2e/... -count=1

# Run E2E tests with agent (for integration testing)
e2e-full: e2e-setup
	@echo "Starting agent..."
	$(DOCKER_COMPOSE) --profile with-agent up -d --build agent
	@sleep 3
	@echo "Running full E2E tests..."
	CHV_E2E_URL=http://localhost:8081 $(GO) test -v ./e2e/... -count=1 -run 'Full|Integration'
	$(MAKE) e2e-cleanup

# Cleanup E2E environment
e2e-cleanup:
	@echo "Cleaning up E2E environment..."
	$(DOCKER_COMPOSE) down -v --remove-orphans

# Run E2E tests (alias)
test-e2e: e2e

# Docker Compose operations
docker-up:
	$(DOCKER_COMPOSE) up -d --build

docker-down:
	$(DOCKER_COMPOSE) down

docker-logs:
	$(DOCKER_COMPOSE) logs -f

docker-clean:
	$(DOCKER_COMPOSE) down -v --remove-orphans

# Start full stack with agent
docker-up-full:
	$(DOCKER_COMPOSE) --profile with-agent up -d --build

# Bootstrap a node (requires running controller)
bootstrap-node:
	$(DOCKER_COMPOSE) run --rm bootstrap --controller=controller:9090

# Format code
fmt:
	$(GO) fmt ./...

# Run linter
lint:
	@which golangci-lint > /dev/null || go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest
	golangci-lint run

# Clean build artifacts
clean:
	rm -rf bin/
	rm -f coverage.out coverage.html
	$(GO) clean

# Clean everything including Docker
clean-all: clean
	$(DOCKER_COMPOSE) down -v --remove-orphans
	$(DOCKER) system prune -f

# Development helpers
dev-up: docker-up
dev-down: docker-down

# Create required directories
init-dirs:
	mkdir -p bin
	mkdir -p deploy/bootstrap-container

# Full setup for new development environment
setup: init-dirs deps proto build

# Show help
help:
	@echo "CHV Build System"
	@echo ""
	@echo "Build Targets:"
	@echo "  all              - Build all binaries (default)"
	@echo "  build            - Build all binaries"
	@echo "  build-controller - Build controller binary"
	@echo "  build-agent      - Build agent binary"
	@echo "  build-bootstrap  - Build bootstrap binary"
	@echo ""
	@echo "Test Targets:"
	@echo "  test             - Run unit tests"
	@echo "  test-coverage    - Run tests with coverage report"
	@echo "  e2e              - Run E2E tests (starts/stops Docker)"
	@echo "  e2e-setup        - Setup E2E environment only"
	@echo "  e2e-run          - Run E2E tests only (requires e2e-setup)"
	@echo "  e2e-cleanup      - Cleanup E2E environment"
	@echo "  test-e2e         - Alias for e2e"
	@echo ""
	@echo "Docker Targets:"
	@echo "  docker-up        - Start Docker Compose (controller + postgres)"
	@echo "  docker-up-full   - Start full stack including agent"
	@echo "  docker-down      - Stop Docker Compose"
	@echo "  docker-logs      - Follow Docker Compose logs"
	@echo "  docker-clean     - Clean Docker Compose volumes"
	@echo ""
	@echo "Development Targets:"
	@echo "  proto            - Generate protobuf code"
	@echo "  deps             - Download and tidy Go dependencies"
	@echo "  fmt              - Format Go code"
	@echo "  lint             - Run linter"
	@echo "  clean            - Clean build artifacts"
	@echo "  clean-all        - Clean everything including Docker"
	@echo "  setup            - Full setup for new environment"
	@echo "  help             - Show this help"
