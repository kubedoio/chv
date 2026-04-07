// Package network provides network management for VMs.
package network

import (
	"crypto/sha256"
	"fmt"
	"net"
	"os/exec"
	"strings"
)

// TAPManager manages TAP devices for VM networking.
type TAPManager struct {
	defaultBridge   string
	uplinkInterface string
	gatewayIP       string
}

// NewTAPManager creates a new TAP manager.
func NewTAPManager(defaultBridge, uplinkInterface, gatewayIP string) *TAPManager {
	return &TAPManager{
		defaultBridge:   defaultBridge,
		uplinkInterface: uplinkInterface,
		gatewayIP:       gatewayIP,
	}
}

// TAPDevice represents a created TAP device.
type TAPDevice struct {
	Name       string
	Bridge     string
	MACAddress string
}

// CreateTAP creates a TAP device for a VM and attaches it to a bridge.
// The TAP name includes the full VM UUID to ensure uniqueness.
func (tm *TAPManager) CreateTAP(vmID string, bridgeName string) (*TAPDevice, error) {
	if bridgeName == "" {
		bridgeName = tm.defaultBridge
	}

	// Generate TAP device name from VM ID
	// Use full UUID but truncate if too long (max interface name is 15 chars)
	tapName := tm.generateTAPName(vmID)

	// Check if TAP already exists
	if tm.exists(tapName) {
		// Verify it's attached to the correct bridge
		attachedBridge, err := tm.getAttachedBridge(tapName)
		if err != nil {
			return nil, fmt.Errorf("failed to check TAP bridge: %w", err)
		}
		if attachedBridge != bridgeName {
			return nil, fmt.Errorf("TAP %s exists but attached to different bridge %s", tapName, attachedBridge)
		}

		// TAP exists and is on correct bridge, generate MAC and return
		mac := tm.generateMAC(vmID)
		return &TAPDevice{
			Name:       tapName,
			Bridge:     bridgeName,
			MACAddress: mac,
		}, nil
	}

	// Create TAP device
	cmd := exec.Command("ip", "tuntap", "add", tapName, "mode", "tap")
	if out, err := cmd.CombinedOutput(); err != nil {
		return nil, fmt.Errorf("failed to create TAP device: %v (output: %s)", err, string(out))
	}

	// Attach to bridge
	cmd = exec.Command("ip", "link", "set", tapName, "master", bridgeName)
	if out, err := cmd.CombinedOutput(); err != nil {
		// Clean up TAP on failure
		tm.DeleteTAP(tapName)
		return nil, fmt.Errorf("failed to attach TAP to bridge: %v (output: %s)", err, string(out))
	}

	// Bring up TAP device
	cmd = exec.Command("ip", "link", "set", tapName, "up")
	if out, err := cmd.CombinedOutput(); err != nil {
		// Clean up on failure
		tm.DeleteTAP(tapName)
		return nil, fmt.Errorf("failed to bring up TAP device: %v (output: %s)", err, string(out))
	}

	// Generate deterministic MAC address
	mac := tm.generateMAC(vmID)

	return &TAPDevice{
		Name:       tapName,
		Bridge:     bridgeName,
		MACAddress: mac,
	}, nil
}

// DeleteTAP removes a TAP device.
func (tm *TAPManager) DeleteTAP(tapName string) error {
	// Bring down first
	exec.Command("ip", "link", "set", tapName, "down").Run()

	// Delete TAP device
	cmd := exec.Command("ip", "tuntap", "del", tapName, "mode", "tap")
	if out, err := cmd.CombinedOutput(); err != nil {
		// Ignore "does not exist" errors
		if strings.Contains(string(out), "does not exist") {
			return nil
		}
		return fmt.Errorf("failed to delete TAP device: %v (output: %s)", err, string(out))
	}

	return nil
}

// DeleteTAPByVMID removes a TAP device associated with a VM ID.
func (tm *TAPManager) DeleteTAPByVMID(vmID string) error {
	tapName := tm.generateTAPName(vmID)
	return tm.DeleteTAP(tapName)
}

// GetTAPDevice returns the TAP device info for a VM.
func (tm *TAPManager) GetTAPDevice(vmID string) (*TAPDevice, error) {
	tapName := tm.generateTAPName(vmID)

	if !tm.exists(tapName) {
		return nil, nil
	}

	bridge, err := tm.getAttachedBridge(tapName)
	if err != nil {
		return nil, err
	}

	mac := tm.generateMAC(vmID)

	return &TAPDevice{
		Name:       tapName,
		Bridge:     bridge,
		MACAddress: mac,
	}, nil
}

// exists checks if a TAP device exists.
func (tm *TAPManager) exists(tapName string) bool {
	cmd := exec.Command("ip", "link", "show", tapName)
	err := cmd.Run()
	return err == nil
}

// getAttachedBridge returns the bridge a TAP device is attached to.
func (tm *TAPManager) getAttachedBridge(tapName string) (string, error) {
	cmd := exec.Command("ip", "link", "show", tapName)
	out, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("failed to get TAP info: %w", err)
	}

	// Parse output for "master <bridge>"
	output := string(out)
	if idx := strings.Index(output, "master "); idx != -1 {
		rest := output[idx+7:]
		if endIdx := strings.IndexAny(rest, " \t\n"); endIdx != -1 {
			return rest[:endIdx], nil
		}
	}

	return "", nil
}

// generateTAPName generates a TAP device name from VM ID.
// Linux interface names are limited to 15 characters.
// We use "tap" + first 12 chars of VM UUID (12 chars = 48 bits, sufficient uniqueness)
func (tm *TAPManager) generateTAPName(vmID string) string {
	// Remove dashes from UUID and take first 12 chars
	cleanID := strings.ReplaceAll(vmID, "-", "")
	if len(cleanID) > 12 {
		cleanID = cleanID[:12]
	}
	return "tap" + cleanID
}

// generateMAC generates a deterministic MAC address from VM ID.
// Uses the locally administered address range (02:xx:xx:xx:xx:xx)
func (tm *TAPManager) generateMAC(vmID string) string {
	hash := sha256.Sum256([]byte(vmID))
	
	// Use locally administered MAC address (second least significant bit of first octet is 1)
	// 02:xx:xx:xx:xx:xx format
	mac := net.HardwareAddr{
		0x02, // Locally administered
		hash[0],
		hash[1],
		hash[2],
		hash[3],
		hash[4],
	}
	
	return mac.String()
}

// EnsureBridge checks if a bridge exists and creates it if needed.
// It also attaches the uplink interface and configures the gateway IP.
func (tm *TAPManager) EnsureBridge(bridgeName string) error {
	if bridgeName == "" {
		bridgeName = tm.defaultBridge
	}

	cmd := exec.Command("ip", "link", "show", bridgeName)
	if err := cmd.Run(); err == nil {
		// Bridge exists, ensure uplink is attached
		if err := tm.attachUplink(bridgeName); err != nil {
			return fmt.Errorf("failed to attach uplink to existing bridge: %w", err)
		}
		// Ensure gateway IP is configured
		if err := tm.configureGateway(bridgeName); err != nil {
			return fmt.Errorf("failed to configure gateway IP: %w", err)
		}
		return nil
	}

	// Create bridge
	cmd = exec.Command("ip", "link", "add", bridgeName, "type", "bridge")
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to create bridge: %v (output: %s)", err, string(out))
	}

	// Bring up bridge
	cmd = exec.Command("ip", "link", "set", bridgeName, "up")
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to bring up bridge: %v (output: %s)", err, string(out))
	}

	// Attach uplink interface
	if err := tm.attachUplink(bridgeName); err != nil {
		return fmt.Errorf("failed to attach uplink: %w", err)
	}

	// Configure gateway IP
	if err := tm.configureGateway(bridgeName); err != nil {
		return fmt.Errorf("failed to configure gateway IP: %w", err)
	}

	// Enable IP forwarding
	if err := tm.enableIPForwarding(); err != nil {
		return fmt.Errorf("failed to enable IP forwarding: %w", err)
	}

	return nil
}

// attachUplink attaches the uplink interface to the bridge if specified.
func (tm *TAPManager) attachUplink(bridgeName string) error {
	if tm.uplinkInterface == "" {
		return nil
	}

	// Check if uplink is already attached to this bridge
	attachedBridge, err := tm.getAttachedBridge(tm.uplinkInterface)
	if err == nil && attachedBridge == bridgeName {
		// Already attached to the correct bridge
		return nil
	}

	// Check if uplink interface exists
	cmd := exec.Command("ip", "link", "show", tm.uplinkInterface)
	if err := cmd.Run(); err != nil {
		// Uplink interface doesn't exist, skip silently
		return nil
	}

	// Bring down uplink before attaching to bridge
	cmd = exec.Command("ip", "link", "set", tm.uplinkInterface, "down")
	cmd.Run() // Ignore error, interface might already be down

	// Attach uplink to bridge
	cmd = exec.Command("ip", "link", "set", tm.uplinkInterface, "master", bridgeName)
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to attach %s to bridge %s: %v (output: %s)", tm.uplinkInterface, bridgeName, err, string(out))
	}

	// Bring up uplink
	cmd = exec.Command("ip", "link", "set", tm.uplinkInterface, "up")
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to bring up uplink %s: %v (output: %s)", tm.uplinkInterface, err, string(out))
	}

	return nil
}

// configureGateway configures the gateway IP on the bridge.
func (tm *TAPManager) configureGateway(bridgeName string) error {
	if tm.gatewayIP == "" {
		return nil
	}

	// Check if IP is already configured
	cmd := exec.Command("ip", "addr", "show", bridgeName)
	out, err := cmd.Output()
	if err == nil && strings.Contains(string(out), tm.gatewayIP) {
		// IP already configured
		return nil
	}

	// Add IP to bridge (assumes /24 subnet)
	ipWithCidr := tm.gatewayIP + "/24"
	cmd = exec.Command("ip", "addr", "add", ipWithCidr, "dev", bridgeName)
	if out, err := cmd.CombinedOutput(); err != nil {
		// Ignore "File exists" error (IP already set)
		if !strings.Contains(string(out), "File exists") {
			return fmt.Errorf("failed to add IP %s to bridge %s: %v (output: %s)", tm.gatewayIP, bridgeName, err, string(out))
		}
	}

	return nil
}

// enableIPForwarding enables IP forwarding for NAT/routing.
func (tm *TAPManager) enableIPForwarding() error {
	// Enable IP forwarding via sysctl
	cmd := exec.Command("sysctl", "-w", "net.ipv4.ip_forward=1")
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to enable IP forwarding: %v (output: %s)", err, string(out))
	}
	return nil
}

// EnableNAT enables NAT for the bridge network using iptables.
// This should be called after EnsureBridge if internet access is needed.
func (tm *TAPManager) EnableNAT() error {
	if tm.uplinkInterface == "" || tm.gatewayIP == "" {
		return nil
	}

	// Determine the subnet from gateway IP (assumes /24)
	ip := net.ParseIP(tm.gatewayIP)
	if ip == nil {
		return fmt.Errorf("invalid gateway IP: %s", tm.gatewayIP)
	}
	subnet := fmt.Sprintf("%d.%d.%d.0/24", ip[12], ip[13], ip[14])

	// Check if NAT rule already exists
	cmd := exec.Command("iptables", "-t", "nat", "-C", "POSTROUTING", "-s", subnet, "-o", tm.uplinkInterface, "-j", "MASQUERADE")
	if err := cmd.Run(); err == nil {
		// Rule already exists
		return nil
	}

	// Add NAT rule
	cmd = exec.Command("iptables", "-t", "nat", "-A", "POSTROUTING", "-s", subnet, "-o", tm.uplinkInterface, "-j", "MASQUERADE")
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to add NAT rule: %v (output: %s)", err, string(out))
	}

	return nil
}

// ListTAPs lists all TAP devices attached to a bridge.
func (tm *TAPManager) ListTAPs(bridgeName string) ([]string, error) {
	cmd := exec.Command("bridge", "link", "show")
	out, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("failed to list bridge links: %w", err)
	}

	var taps []string
	lines := strings.Split(string(out), "\n")
	for _, line := range lines {
		if strings.Contains(line, "master "+bridgeName) {
			// Parse interface name (first word)
			fields := strings.Fields(line)
			if len(fields) > 0 {
				taps = append(taps, fields[0])
			}
		}
	}

	return taps, nil
}
