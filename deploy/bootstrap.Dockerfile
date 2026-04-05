# Build stage for bootstrap and agent
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

# Build binaries
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-w -s" -o /build/chv-agent ./cmd/chv-agent
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-w -s" -o /build/chv-bootstrap ./cmd/chv-bootstrap

# Final stage - privileged bootstrap container
FROM alpine:3.21

RUN apk add --no-cache ca-certificates curl iputils bridge-utils qemu-img nfs-utils

WORKDIR /app

# Copy binaries from builder
COPY --from=builder /build/chv-agent /usr/local/bin/chv-agent
COPY --from=builder /build/chv-bootstrap /usr/local/bin/chv-bootstrap

# Copy systemd units
COPY deploy/systemd/chv-agent.service /etc/chv/systemd/chv-agent.service

# Entrypoint is the bootstrap installer
ENTRYPOINT ["chv-bootstrap"]
