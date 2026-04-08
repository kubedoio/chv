#!/bin/bash

# CHV Installation Script

set -e

CHV_DIR="/srv/data02/projects/chv"
DATA_ROOT="/var/lib/chv"

echo "=========================================="
echo "CHV (Cloud Hypervisor Virtualization)"
echo "Installation Script"
echo "=========================================="
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then 
    echo "Please run as root (use sudo)"
    exit 1
fi

# Create directories
echo "Creating directories..."
mkdir -p $DATA_ROOT/{images,vms,seed,storage/localdisk,logs,configs}

# Install binaries
echo "Installing binaries..."
cp $CHV_DIR/chv-agent /usr/local/bin/
cp $CHV_DIR/chv-controller /usr/local/bin/
chmod +x /usr/local/bin/chv-*

# Install systemd services
echo "Installing systemd services..."
cp $CHV_DIR/chv-agent.service /etc/systemd/system/
cp $CHV_DIR/chv-controller.service /etc/systemd/system/

# Reload systemd
systemctl daemon-reload

# Enable services
systemctl enable chv-agent.service
systemctl enable chv-controller.service

echo ""
echo "=========================================="
echo "Installation Complete!"
echo "=========================================="
echo ""
echo "To start CHV:"
echo "  sudo systemctl start chv-agent"
echo "  sudo systemctl start chv-controller"
echo ""
echo "Or use the startup script:"
echo "  sudo $CHV_DIR/start-chv.sh"
echo ""
echo "WebUI will be available at:"
echo "  http://localhost:8888"
echo ""
echo "=========================================="
