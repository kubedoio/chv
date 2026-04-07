#!/bin/bash
# TLS Demo Script for CHV Platform
# This script demonstrates how to generate and use TLS certificates

set -e

echo "=== CHV TLS Demo ==="
echo ""

# Create temporary directory for demo
DEMO_DIR=$(mktemp -d)
echo "Using temporary directory: $DEMO_DIR"

# Build certgen tool
echo "Building certgen tool..."
cd "$(dirname "$0")/.."
go build -o "$DEMO_DIR/chv-certgen" ./cmd/chv-certgen

# Generate certificates
echo ""
echo "=== Generating Certificates ==="
"$DEMO_DIR/chv-certgen" -ca -controller -agent agent-1 -cert-dir "$DEMO_DIR/certs"

# List generated files
echo ""
echo "=== Generated Certificate Files ==="
ls -la "$DEMO_DIR/certs/"

# Show certificate details
echo ""
echo "=== CA Certificate Details ==="
openssl x509 -in "$DEMO_DIR/certs/ca.crt" -noout -text | grep -E "(Subject:|Issuer:|Not Before|Not After)"

echo ""
echo "=== Controller Certificate Details ==="
openssl x509 -in "$DEMO_DIR/certs/controller.crt" -noout -text | grep -E "(Subject:|Issuer:|DNS:|IP Address)"

echo ""
echo "=== Agent Server Certificate Details ==="
openssl x509 -in "$DEMO_DIR/certs/agent-agent-1-server.crt" -noout -text | grep -E "(Subject:|Issuer:|DNS:)"

echo ""
echo "=== Agent Client Certificate Details ==="
openssl x509 -in "$DEMO_DIR/certs/agent-agent-1-client.crt" -noout -text | grep -E "(Subject:|Issuer:)"

# Create example configuration files
echo ""
echo "=== Creating Example Configuration Files ==="

# Controller config
cat > "$DEMO_DIR/controller.yaml" << 'EOF'
http_addr: ":8080"
grpc_addr: ":9090"
database_path: "/var/lib/chv/chv.db"
log_level: "info"

tls:
  enabled: true
  cert: "CERT_DIR/controller.crt"
  key: "CERT_DIR/controller.key"
  ca: "CERT_DIR/ca.crt"
EOF

sed -i "s|CERT_DIR|$DEMO_DIR/certs|g" "$DEMO_DIR/controller.yaml"

# Agent config  
cat > "$DEMO_DIR/agent.yaml" << 'EOF'
node_id: "agent-1"
listen_addr: ":9091"
controller_addr: "localhost:9090"
data_dir: "/var/lib/chv-agent"
log_level: "info"

tls:
  enabled: true
  cert: "CERT_DIR/agent-agent-1-server.crt"
  key: "CERT_DIR/agent-agent-1-server.key"
  ca: "CERT_DIR/ca.crt"
  client_cert: "CERT_DIR/agent-agent-1-client.crt"
  client_key: "CERT_DIR/agent-agent-1-client.key"
  server_name: "chv-controller"
EOF

sed -i "s|CERT_DIR|$DEMO_DIR/certs|g" "$DEMO_DIR/agent.yaml"

echo "Controller config: $DEMO_DIR/controller.yaml"
echo "Agent config: $DEMO_DIR/agent.yaml"

# Show environment variables
echo ""
echo "=== Environment Variable Configuration ==="
echo ""
echo "# Controller environment variables:"
echo "export CHV_TLS_ENABLED=true"
echo "export CHV_TLS_CERT=$DEMO_DIR/certs/controller.crt"
echo "export CHV_TLS_KEY=$DEMO_DIR/certs/controller.key"
echo "export CHV_TLS_CA=$DEMO_DIR/certs/ca.crt"
echo ""
echo "# Agent environment variables:"
echo "export CHV_TLS_ENABLED=true"
echo "export CHV_TLS_CERT=$DEMO_DIR/certs/agent-agent-1-server.crt"
echo "export CHV_TLS_KEY=$DEMO_DIR/certs/agent-agent-1-server.key"
echo "export CHV_TLS_CA=$DEMO_DIR/certs/ca.crt"
echo "export CHV_TLS_CLIENT_CERT=$DEMO_DIR/certs/agent-agent-1-client.crt"
echo "export CHV_TLS_CLIENT_KEY=$DEMO_DIR/certs/agent-agent-1-client.key"
echo "export CHV_TLS_SERVER_NAME=chv-controller"

# Verify certificate chain
echo ""
echo "=== Verifying Certificate Chain ==="
echo ""
echo "Verifying controller certificate against CA..."
openssl verify -CAfile "$DEMO_DIR/certs/ca.crt" "$DEMO_DIR/certs/controller.crt"

echo "Verifying agent server certificate against CA..."
openssl verify -CAfile "$DEMO_DIR/certs/ca.crt" "$DEMO_DIR/certs/agent-agent-1-server.crt"

echo "Verifying agent client certificate against CA..."
openssl verify -CAfile "$DEMO_DIR/certs/ca.crt" "$DEMO_DIR/certs/agent-agent-1-client.crt"

echo ""
echo "=== Demo Complete ==="
echo ""
echo "Certificate files are in: $DEMO_DIR/certs/"
echo "Configuration files are in: $DEMO_DIR/"
echo ""
echo "To clean up, run:"
echo "  rm -rf $DEMO_DIR"
