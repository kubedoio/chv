#!/bin/bash
# setup-bridge.sh - Configure Linux bridge for CHV VMs
# Usage: ./setup-bridge.sh {create|add-iface|route|dhcp|all}

set -e

BRIDGE_NAME="br0"
BRIDGE_IP="10.0.0.1/24"
BRIDGE_CIDR="10.0.0.0/24"
PHYSICAL_IFACE="eth0"  # Change to your physical interface

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Create bridge if it doesn't exist
create_bridge() {
    if ! ip link show "$BRIDGE_NAME" &>/dev/null; then
        log_info "Creating bridge $BRIDGE_NAME..."
        ip link add name "$BRIDGE_NAME" type bridge
        ip addr add "$BRIDGE_IP" dev "$BRIDGE_NAME"
        ip link set "$BRIDGE_NAME" up
        log_info "Bridge $BRIDGE_NAME created with IP $BRIDGE_IP"
    else
        log_warn "Bridge $BRIDGE_NAME already exists"
    fi
}

# Add physical interface to bridge (optional, for external connectivity)
add_to_bridge() {
    if ip link show "$PHYSICAL_IFACE" &>/dev/null; then
        log_info "Adding $PHYSICAL_IFACE to bridge..."
        ip link set "$PHYSICAL_IFACE" master "$BRIDGE_NAME"
        ip link set "$PHYSICAL_IFACE" up
        log_info "$PHYSICAL_IFACE added to bridge"
    else
        log_error "Physical interface $PHYSICAL_IFACE not found"
        return 1
    fi
}

# Enable IP forwarding and NAT
setup_routing() {
    log_info "Enabling IP forwarding..."
    
    # Enable IP forwarding
    sysctl -w net.ipv4.ip_forward=1
    
    # Make it persistent
    if ! grep -q "net.ipv4.ip_forward=1" /etc/sysctl.conf 2>/dev/null; then
        echo "net.ipv4.ip_forward=1" >> /etc/sysctl.conf
    fi
    
    # Add NAT rule if not exists
    if ! iptables -t nat -C POSTROUTING -o "$PHYSICAL_IFACE" -j MASQUERADE 2>/dev/null; then
        log_info "Adding NAT rule..."
        iptables -t nat -A POSTROUTING -o "$PHYSICAL_IFACE" -j MASQUERADE
    else
        log_warn "NAT rule already exists"
    fi
    
    # Allow forwarding on bridge
    iptables -A FORWARD -i "$BRIDGE_NAME" -o "$PHYSICAL_IFACE" -j ACCEPT 2>/dev/null || true
    iptables -A FORWARD -i "$PHYSICAL_IFACE" -o "$BRIDGE_NAME" -m state --state RELATED,ESTABLISHED -j ACCEPT 2>/dev/null || true
    
    log_info "Routing configured"
}

# Setup DHCP with dnsmasq (optional)
setup_dhcp() {
    if ! command -v dnsmasq &>/dev/null; then
        log_warn "dnsmasq not installed. Install with: apt-get install dnsmasq"
        return 1
    fi
    
    log_info "Configuring dnsmasq for DHCP..."
    
    cat > /etc/dnsmasq.d/chv-bridge.conf << EOF
# CHV Bridge DHCP Configuration
interface=$BRIDGE_NAME
dhcp-range=10.0.0.50,10.0.0.200,255.255.255.0,12h
dhcp-option=3,10.0.0.1
dhcp-option=6,8.8.8.8,8.8.4.4
# Static leases (optional)
# dhcp-host=02:00:00:00:00:01,vm1,10.0.0.10,infinite
EOF

    # Restart dnsmasq
    if systemctl is-active --quiet dnsmasq; then
        systemctl restart dnsmasq
    else
        systemctl enable dnsmasq
        systemctl start dnsmasq
    fi
    
    log_info "DHCP configured"
}

# Show current bridge status
show_status() {
    echo ""
    echo "=== Bridge Status ==="
    ip link show "$BRIDGE_NAME" 2>/dev/null || log_error "Bridge $BRIDGE_NAME not found"
    echo ""
    echo "=== Bridge Links ==="
    bridge link show 2>/dev/null || log_warn "No bridge links"
    echo ""
    echo "=== IP Forwarding ==="
    sysctl net.ipv4.ip_forward
    echo ""
    echo "=== NAT Rules ==="
    iptables -t nat -L POSTROUTING -v -n 2>/dev/null | grep MASQUERADE || log_warn "No NAT rules"
}

# Cleanup bridge
cleanup() {
    log_info "Cleaning up bridge $BRIDGE_NAME..."
    
    # Remove physical interface from bridge
    ip link set "$PHYSICAL_IFACE" nomaster 2>/dev/null || true
    
    # Delete bridge
    ip link del "$BRIDGE_NAME" 2>/dev/null || true
    
    # Remove NAT rules
    iptables -t nat -D POSTROUTING -o "$PHYSICAL_IFACE" -j MASQUERADE 2>/dev/null || true
    
    log_info "Bridge cleanup complete"
}

# Main
case "${1:-all}" in
    create)
        create_bridge
        ;;
    add-iface)
        add_to_bridge
        ;;
    route)
        setup_routing
        ;;
    dhcp)
        setup_dhcp
        ;;
    status)
        show_status
        ;;
    cleanup)
        cleanup
        ;;
    all)
        create_bridge
        setup_routing
        setup_dhcp || log_warn "DHCP setup failed, continuing..."
        show_status
        ;;
    *)
        echo "Usage: $0 {create|add-iface|route|dhcp|status|cleanup|all}"
        echo ""
        echo "Commands:"
        echo "  create    - Create the bridge interface"
        echo "  add-iface - Add physical interface to bridge"
        echo "  route     - Enable IP forwarding and NAT"
        echo "  dhcp      - Configure dnsmasq DHCP server"
        echo "  status    - Show bridge status"
        echo "  cleanup   - Remove bridge and rules"
        echo "  all       - Run create, route, dhcp, and status"
        exit 1
        ;;
esac
