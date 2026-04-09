#!/bin/bash

# CHV (Cloud Hypervisor Virtualization) Installation Script
# Refined for Stage 1 - Bare-Metal Bootstrapping

set -e

# Configuration
DATA_ROOT="/var/lib/chv"
LOG_DIR="$DATA_ROOT/logs"
BIN_DIR="/usr/local/bin"
CH_VERSION="v51.1"
CH_URL="https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/${CH_VERSION}/cloud-hypervisor-static"
BRIDGE_NAME="chvbr0"
BRIDGE_CIDR="10.0.0.1/24"
BRIDGE_NET="10.0.0.0/24"

# ANSI Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}==========================================${NC}"
echo -e "${GREEN}CHV - Cloud Hypervisor Virtualization${NC}"
echo -e "Bare-Metal Installer (Refined Stage 1)"
echo -e "${BLUE}==========================================${NC}"
echo ""

# 1. Check if running as root
if [ "$EUID" -ne 0 ]; then 
    echo -e "${RED}Error: Please run as root (use sudo)${NC}"
    exit 1
fi

# 2. Dependency Management
echo -e "${BLUE}[1/5] Installing system dependencies...${NC}"
apt-get update -qq
apt-get install -y -qq nftables iproute2 genisoimage qemu-utils curl > /dev/null

if [ ! -f "$BIN_DIR/cloud-hypervisor" ]; then
    echo "Downloading static cloud-hypervisor binary (${CH_VERSION})..."
    curl -sL "$CH_URL" -o "$BIN_DIR/cloud-hypervisor"
    chmod +x "$BIN_DIR/cloud-hypervisor"
fi
echo -e "${GREEN}Dependencies OK.${NC}"

# 3. Create Hierarchies
echo -e "${BLUE}[2/5] Setting up directory structures...${NC}"
mkdir -p $DATA_ROOT/{images,vms,cloudinit,storage/localdisk,logs,configs}
echo -e "${GREEN}Directories created at $DATA_ROOT${NC}"

# 4. Binary Installation
echo -e "${BLUE}[3/5] Installing CHV binaries...${NC}"
# Check if binaries exist in current dir (from repo build)
for bin in chv-agent chv-controller; do
    if [ -f "./$bin" ]; then
        cp "./$bin" "$BIN_DIR/"
        chmod +x "$BIN_DIR/$bin"
        echo "Installed $bin to $BIN_DIR"
    elif [ -f "./cmd/$bin/$bin" ]; then
        cp "./cmd/$bin/$bin" "$BIN_DIR/"
        chmod +x "$BIN_DIR/$bin"
        echo "Installed $bin from cmd/ to $BIN_DIR"
    else
        echo -e "${RED}Warning: $bin source not found. Skipping binary copy.${NC}"
    fi
done

# 5. Network & NAT Configuration
echo -e "${BLUE}[4/5] Configuring Networking & NAT...${NC}"

# Enable IPv4 Forwarding
echo "Enabling IPv4 forwarding..."
sysctl -w net.ipv4.ip_forward=1 > /dev/null
echo "net.ipv4.ip_forward=1" > /etc/sysctl.d/99-chv.conf

# Detect Primary Interface
PRIMARY_IF=$(ip route show | grep default | awk '{print $5}' | head -n 1)
if [ -z "$PRIMARY_IF" ]; then
    echo -e "${RED}Warning: Could not detect primary interface. NAT might fail.${NC}"
else
    echo "Detected primary interface: $PRIMARY_IF"
    
    # Setup NAT via nftables
    echo "Configuring nftables MASQUERADE for $BRIDGE_NET..."
    nft add table inet chv_nat || true
    nft add chain inet chv_nat postrouting "{ type nat hook postrouting priority 100; policy accept; }" || true
    nft add rule inet chv_nat postrouting oifname "$PRIMARY_IF" ip saddr "$BRIDGE_NET" masquerade
fi
echo -e "${GREEN}Network configuration complete.${NC}"

# 6. Service Installation
echo -e "${BLUE}[5/5] Installing systemd services...${NC}"
if [ -f "./chv-agent.service" ] && [ -f "./chv-controller.service" ]; then
    cp ./chv-agent.service /etc/systemd/system/
    cp ./chv-controller.service /etc/systemd/system/
    systemctl daemon-reload
    systemctl enable chv-agent.service > /dev/null 2>&1
    systemctl enable chv-controller.service > /dev/null 2>&1
    echo "Services enabled."
else
    echo -e "${RED}Error: systemd service files not found in current directory.${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}==========================================${NC}"
echo -e "Installation Complete!"
echo -e "${GREEN}==========================================${NC}"
echo ""
echo "To manage CHV:"
echo -e "  - Start:   ${BLUE}sudo systemctl start chv-agent chv-controller${NC}"
echo -e "  - WebUI:   ${BLUE}http://localhost:8888${NC}"
echo ""
echo "Configuration is stored in ${DATA_ROOT}"
echo "=========================================="
