#!/bin/bash
set -e

# Configuration
AGENT_TARGET="bin/chv-agent-linux-amd64"
REMOTE_HOST="10.5.199.83"
REMOTE_USER="ubuntu"

echo "Building chv-agent for Linux/AMD64..."
GOOS=linux GOARCH=amd64 go build -o $AGENT_TARGET ./cmd/chv-agent

echo "Build complete: $AGENT_TARGET"
echo ""
echo "To deploy to your remote server, run:"
echo "scp $AGENT_TARGET ${REMOTE_USER}@${REMOTE_HOST}:~/chv-agent"
echo ""
echo "On the remote server, you may need to install dependencies:"
echo "sudo apt update && sudo apt install -y qemu-utils genisoimage cloud-hypervisor"
echo ""
echo "Then start the agent:"
echo "chmod +x ~/chv-agent"
echo "sudo ./chv-agent"
