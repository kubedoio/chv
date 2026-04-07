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

# Copy schema file
COPY --from=builder /build/configs/schema_sqlite.sql /app/configs/schema_sqlite.sql

# Create non-root user
RUN adduser -D -u 1000 chv

# Create data directory with proper permissions for chv user
# The directory must be writable for SQLite WAL files
RUN mkdir -p /var/lib/chv && \
    chown -R chv:chv /var/lib/chv && \
    chmod 775 /var/lib/chv

# Also ensure the app configs directory is writable for any runtime files
RUN chown -R chv:chv /app

USER chv

EXPOSE 8080 9090

ENTRYPOINT ["chv-controller"]
