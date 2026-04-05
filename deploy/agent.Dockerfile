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

RUN apk add --no-cache ca-certificates curl iputils bridge-utils qemu-img nfs-utils

WORKDIR /app

# Copy binary from builder
COPY --from=builder /build/chv-agent /usr/local/bin/chv-agent

# Run as root (required for VM management)
ENTRYPOINT ["chv-agent"]
