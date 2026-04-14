#!/bin/bash

# CHV (Cloud Hypervisor Virtualization) Startup Utility
# Orchestrates systemd services and provides console feedback

set -e

# ANSI Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}==========================================${NC}"
echo -e "${GREEN}CHV Service Manager${NC}"
echo -e "${BLUE}==========================================${NC}"

if [ "$EUID" -ne 0 ]; then 
    echo -e "${RED}Error: Please run as root (use sudo)${NC}"
    exit 1
fi

echo "Restarting CHV services..."
systemctl restart chv-agent.service
systemctl restart chv-controller.service

echo -n "Waiting for services to stabilize..."
sleep 2
echo " OK."

echo ""
echo -e "Service Status:"
systemctl is-active chv-agent.service | xargs echo -e "  - Agent:      "
systemctl is-active chv-controller.service | xargs echo -e "  - Controller: "

echo ""
echo -e "WebUI is active at: ${GREEN}http://localhost:8888${NC}"
echo "Agent API is at:    http://localhost:9090"
echo ""
echo -e "${BLUE}==========================================${NC}"
