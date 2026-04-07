# CHV TLS/mTLS Setup Guide

This guide covers setting up TLS encryption and mutual TLS (mTLS) authentication for CHV platform communications.

## Overview

CHV supports TLS 1.3 encryption for:
- Controller HTTP API (HTTPS)
- Controller gRPC API
- Agent gRPC API
- Agent HTTP API (WebSocket console)

With mTLS enabled:
- Controller verifies agent client certificates
- Agents verify controller server certificates
- All communication is encrypted and authenticated

## Quick Start

### 1. Generate Certificates

Use the `chv-certgen` tool to generate all required certificates:

```bash
# Build the certgen tool
go build -o chv-certgen ./cmd/chv-certgen

# Generate CA and controller certificates
sudo ./chv-certgen -ca -controller -cert-dir /etc/chv/certs

# Generate agent certificates (repeat for each agent)
sudo ./chv-certgen -agent agent-1 -cert-dir /etc/chv/certs
sudo ./chv-certgen -agent agent-2 -cert-dir /etc/chv/certs
```

### 2. Configure Controller

Create `/etc/chv/controller.yaml`:

```yaml
http_addr: ":8080"
grpc_addr: ":9090"
database_path: "/var/lib/chv/chv.db"

tls:
  enabled: true
  cert: "/etc/chv/certs/controller.crt"
  key: "/etc/chv/certs/controller.key"
  ca: "/etc/chv/certs/ca.crt"  # Required for mTLS
```

Or use environment variables:

```bash
export CHV_TLS_ENABLED=true
export CHV_TLS_CERT=/etc/chv/certs/controller.crt
export CHV_TLS_KEY=/etc/chv/certs/controller.key
export CHV_TLS_CA=/etc/chv/certs/ca.crt
```

### 3. Configure Agent

Create `/etc/chv/agent.yaml`:

```yaml
node_id: "agent-1"
listen_addr: ":9091"
controller_addr: "controller:9090"
data_dir: "/var/lib/chv-agent"

tls:
  enabled: true
  cert: "/etc/chv/certs/agent-agent-1-server.crt"
  key: "/etc/chv/certs/agent-agent-1-server.key"
  ca: "/etc/chv/certs/ca.crt"
  client_cert: "/etc/chv/certs/agent-agent-1-client.crt"
  client_key: "/etc/chv/certs/agent-agent-1-client.key"
  server_name: "chv-controller"
```

Or use environment variables:

```bash
export CHV_TLS_ENABLED=true
export CHV_TLS_CERT=/etc/chv/certs/agent-agent-1-server.crt
export CHV_TLS_KEY=/etc/chv/certs/agent-agent-1-server.key
export CHV_TLS_CA=/etc/chv/certs/ca.crt
export CHV_TLS_CLIENT_CERT=/etc/chv/certs/agent-agent-1-client.crt
export CHV_TLS_CLIENT_KEY=/etc/chv/certs/agent-agent-1-client.key
export CHV_TLS_SERVER_NAME=chv-controller
```

### 4. Start Services

```bash
# Start controller
sudo chv-controller -config /etc/chv/controller.yaml

# Start agent
sudo chv-agent -config /etc/chv/agent.yaml
```

## Certificate Management

### Directory Structure

```
/etc/chv/certs/
├── ca.crt                  # CA certificate (shared)
├── ca.key                  # CA private key (keep secure!)
├── controller.crt          # Controller server certificate
├── controller.key          # Controller private key
├── agent-<id>-server.crt   # Agent server certificate
├── agent-<id>-server.key   # Agent server private key
├── agent-<id>-client.crt   # Agent client certificate (mTLS)
└── agent-<id>-client.key   # Agent client private key
```

### File Permissions

```bash
# CA private key should be readable only by root
sudo chmod 600 /etc/chv/certs/ca.key

# Private keys should be readable only by the service user
sudo chmod 600 /etc/chv/certs/*.key

# Certificates can be readable by all
sudo chmod 644 /etc/chv/certs/*.crt
```

## Configuration Reference

### TLS Configuration Options

#### Controller

| Option | Environment Variable | Description |
|--------|---------------------|-------------|
| `tls.enabled` | `CHV_TLS_ENABLED` | Enable TLS (default: false) |
| `tls.cert` | `CHV_TLS_CERT` | Path to server certificate |
| `tls.key` | `CHV_TLS_KEY` | Path to server private key |
| `tls.ca` | `CHV_TLS_CA` | Path to CA certificate (enables mTLS) |
| `tls.auto_generate` | `CHV_TLS_AUTO_GENERATE` | Auto-generate certificates |
| `tls.cert_dir` | `CHV_TLS_CERT_DIR` | Directory for auto-generated certs |

#### Agent

| Option | Environment Variable | Description |
|--------|---------------------|-------------|
| `tls.enabled` | `CHV_TLS_ENABLED` | Enable TLS (default: false) |
| `tls.cert` | `CHV_TLS_CERT` | Path to server certificate |
| `tls.key` | `CHV_TLS_KEY` | Path to server private key |
| `tls.ca` | `CHV_TLS_CA` | Path to CA certificate |
| `tls.client_cert` | `CHV_TLS_CLIENT_CERT` | Path to client certificate (mTLS) |
| `tls.client_key` | `CHV_TLS_CLIENT_KEY` | Path to client private key |
| `tls.server_name` | `CHV_TLS_SERVER_NAME` | Controller hostname for verification |
| `tls.auto_generate` | `CHV_TLS_AUTO_GENERATE` | Auto-generate certificates |
| `tls.cert_dir` | `CHV_TLS_CERT_DIR` | Directory for auto-generated certs |

## Advanced Configuration

### Certificate Generation Options

```bash
# Generate with custom names
./chv-certgen -controller -controller-name my-controller
./chv-certgen -agent node-1 -agent-name agent-host-1

# Generate in custom directory
./chv-certgen -ca -controller -cert-dir /opt/chv/certs
```

### Auto-Generate Certificates (Development Only)

For development environments, you can enable automatic certificate generation:

```yaml
tls:
  enabled: true
  auto_generate: true
  cert_dir: "/etc/chv/certs"
```

**Warning**: Auto-generated certificates are not suitable for production use as they:
- Use self-signed CA
- Have short validity periods
- Are not backed up automatically

### Using External CA

If you have an existing PKI infrastructure:

1. Request a server certificate for the controller
2. Request server and client certificates for each agent
3. Place certificates in `/etc/chv/certs/`
4. Configure paths in the YAML files

Example certificate request for controller:

```bash
# Create CSR
openssl req -new -newkey rsa:4096 -nodes \
  -keyout controller.key \
  -out controller.csr \
  -subj "/CN=chv-controller/O=CHV Platform" \
  -addext "subjectAltName=DNS:localhost,DNS:controller,DNS:controller.chv.svc.cluster.local"

# Submit CSR to your CA and obtain controller.crt
```

## Troubleshooting

### Certificate Errors

#### "certificate signed by unknown authority"

The CA certificate is not trusted. Ensure:
1. The CA certificate is correctly installed
2. The `ca` path in configuration is correct
3. For agents, the controller's CA is used

#### "certificate has expired or is not yet valid"

Check system time on all nodes:
```bash
timedatectl status
```

Renew certificates if expired:
```bash
./chv-certgen -ca -controller -agent <agent-id>
```

#### "tls: bad certificate"

Verify certificate and key match:
```bash
openssl x509 -noout -modulus -in controller.crt | openssl md5
openssl rsa -noout -modulus -in controller.key | openssl md5
```

Both commands should output the same hash.

### Connection Issues

#### Controller cannot connect to agent

Check agent's server certificate includes the hostname/IP used by the controller:

```bash
openssl x509 -in agent-server.crt -text -noout | grep -A1 "Subject Alternative Name"
```

#### Agent cannot connect to controller

Verify the `server_name` in agent configuration matches the controller certificate's CN or SAN:

```bash
openssl x509 -in controller.crt -text -noout | grep "Subject:"
openssl x509 -in controller.crt -text -noout | grep -A1 "Subject Alternative Name"
```

### Debug Logging

Enable debug logging to see TLS handshake details:

```yaml
log_level: "debug"
```

## Security Best Practices

1. **Use mTLS in production**: Always enable client certificate verification
2. **Secure private keys**: Set file permissions to 0600 for all private keys
3. **Use strong keys**: Default 4096-bit RSA keys are recommended
4. **Rotate certificates**: Set up automated certificate rotation before expiry
5. **Use a proper CA**: For production, use your organization's CA or a trusted public CA
6. **Monitor certificate expiry**: Set up alerts for certificates expiring soon
7. **Secure CA private key**: Store the CA private key offline or in an HSM

## Migration from Plaintext

To migrate an existing plaintext deployment to TLS:

1. Generate certificates for all components
2. Deploy certificates to all nodes
3. Update configuration files to enable TLS
4. Restart agents first (they can handle temporary controller unavailability)
5. Restart the controller
6. Verify connectivity with `chvctl` or API calls

During migration, you can run with TLS disabled on one side for gradual rollout:
- Controller with TLS, agents without: Agents won't be able to connect
- Controller without TLS, agents with TLS: Won't work (agents require controller cert)

**Recommendation**: Plan a maintenance window for TLS migration.

## Kubernetes Deployment

For Kubernetes deployments, store certificates as secrets:

```bash
# Create CA secret
kubectl create secret tls chv-ca \
  --cert=ca.crt \
  --key=ca.key \
  --namespace=chv

# Create controller certificate secret
kubectl create secret tls chv-controller-tls \
  --cert=controller.crt \
  --key=controller.key \
  --namespace=chv

# Create agent certificate secret
kubectl create secret tls chv-agent-tls \
  --cert=agent-server.crt \
  --key=agent-server.key \
  --namespace=chv
```

Mount secrets in your pod specifications and reference them in the configuration.
