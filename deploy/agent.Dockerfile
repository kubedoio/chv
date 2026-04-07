# Build stage
FROM golang:1.26-alpine AS builder

WORKDIR /build

# Install dependencies
RUN apk add --no-cache git make protoc protobuf-dev

# Copy go mod files
COPY go.mod go.sum ./
RUN go mod download

# Copy source code
COPY . .

# Generate protobuf code
RUN go generate ./...

# Build agent binary
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-w -s" -o /build/chv-agent ./cmd/chv-agent

# Final stage
FROM alpine:3.21

RUN apk add --no-cache ca-certificates curl iputils bridge-utils qemu-img nfs-utils iproute2 xorriso openssh-keygen

# Generate SSH key for cloud-init
RUN mkdir -p /root/.ssh && \
    ssh-keygen -t ed25519 -C "chv@local" -f /root/.ssh/chv_id_ed25519 -N ""

WORKDIR /app

# Copy binary from builder
COPY --from=builder /build/chv-agent /usr/local/bin/chv-agent

# Note: cloud-hypervisor binary is mounted from host at /usr/bin/cloud-hypervisor
# This is required for proper KVM access - the binary must run on the host kernel

# Download hypervisor firmware for booting cloud images
RUN curl -L -o /usr/local/bin/hypervisor-fw \
    https://github.com/cloud-hypervisor/rust-hypervisor-firmware/releases/download/0.4.2/hypervisor-fw \
    && chmod +x /usr/local/bin/hypervisor-fw

# Run as root (required for VM management)
ENTRYPOINT ["chv-agent"]
