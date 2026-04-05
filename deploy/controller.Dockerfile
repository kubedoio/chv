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

# Build controller binary
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-w -s" -o /build/chv-controller ./cmd/chv-controller

# Final stage
FROM alpine:3.21

RUN apk add --no-cache ca-certificates

WORKDIR /app

# Copy binary from builder
COPY --from=builder /build/chv-controller /usr/local/bin/chv-controller

# Create non-root user
RUN adduser -D -u 1000 chv
USER chv

EXPOSE 8080 9090

ENTRYPOINT ["chv-controller"]
