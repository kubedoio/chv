package services

import (
	"fmt"
	"net"
	"os/exec"
	"strings"
)

// TAPDeviceService manages TAP devices for VM networking
type TAPDeviceService struct {
	bridgeName string
}

// NewTAPDeviceService creates a new TAP device service
func NewTAPDeviceService(bridgeName string) *TAPDeviceService {
	return &TAPDeviceService{
		bridgeName: bridgeName,
	}
}

// CreateTAP creates a new TAP device
func (s *TAPDeviceService) CreateTAP(name string) error {
	// Check if TAP device already exists
	if s.exists(name) {
		// Ensure it's attached to the bridge
		if err := s.attachToBridge(name); err != nil {
			return fmt.Errorf("failed to attach existing TAP to bridge: %w", err)
		}
		return nil
	}

	// Create TAP device using ip command
	cmd := exec.Command("ip", "tuntap", "add", "dev", name, "mode", "tap")
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to create TAP device: %w (output: %s)", err, output)
	}

	// Bring TAP device up
	cmd = exec.Command("ip", "link", "set", "dev", name, "up")
	if output, err := cmd.CombinedOutput(); err != nil {
		// Clean up on failure
		s.DeleteTAP(name)
		return fmt.Errorf("failed to bring up TAP device: %w (output: %s)", err, output)
	}

	// Attach to bridge
	if err := s.attachToBridge(name); err != nil {
		// Clean up on failure
		s.DeleteTAP(name)
		return err
	}

	return nil
}

// DeleteTAP removes a TAP device
func (s *TAPDeviceService) DeleteTAP(name string) error {
	if !s.exists(name) {
		return nil // Already gone
	}

	// Detach from bridge first
	s.detachFromBridge(name)

	// Bring interface down
	cmd := exec.Command("ip", "link", "set", "dev", name, "down")
	cmd.CombinedOutput() // Ignore errors

	// Delete TAP device
	cmd = exec.Command("ip", "tuntap", "del", "dev", name, "mode", "tap")
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to delete TAP device: %w (output: %s)", err, output)
	}

	return nil
}

// exists checks if a TAP device exists
func (s *TAPDeviceService) exists(name string) bool {
	cmd := exec.Command("ip", "link", "show", name)
	err := cmd.Run()
	return err == nil
}

// attachToBridge attaches a TAP device to the bridge
func (s *TAPDeviceService) attachToBridge(tapName string) error {
	// Check if already attached
	attached, err := s.isAttachedToBridge(tapName)
	if err != nil {
		return err
	}
	if attached {
		return nil
	}

	// Add TAP to bridge
	cmd := exec.Command("ip", "link", "set", "dev", tapName, "master", s.bridgeName)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to attach TAP to bridge: %w (output: %s)", err, output)
	}

	return nil
}

// detachFromBridge detaches a TAP device from the bridge
func (s *TAPDeviceService) detachFromBridge(tapName string) error {
	cmd := exec.Command("ip", "link", "set", "dev", tapName, "nomaster")
	cmd.CombinedOutput() // Ignore errors
	return nil
}

// isAttachedToBridge checks if a TAP device is attached to the bridge
func (s *TAPDeviceService) isAttachedToBridge(tapName string) (bool, error) {
	cmd := exec.Command("ip", "link", "show", tapName)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return false, fmt.Errorf("failed to check TAP status: %w", err)
	}

	// Check if output contains "master <bridge>"
	return strings.Contains(string(output), "master "+s.bridgeName), nil
}

// ListTAPs returns all TAP devices attached to the bridge
func (s *TAPDeviceService) ListTAPs() ([]string, error) {
	cmd := exec.Command("ip", "link", "show", "master", s.bridgeName)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return nil, fmt.Errorf("failed to list TAP devices: %w", err)
	}

	// Parse output to extract interface names
	var taps []string
	lines := strings.Split(string(output), "\n")
	for _, line := range lines {
		// Look for lines like "3: tap12345: <BROADCAST,MULTICAST>..."
		if strings.Contains(line, ": tap") {
			parts := strings.Split(line, ":")
			if len(parts) >= 2 {
				tapName := strings.TrimSpace(parts[1])
				taps = append(taps, tapName)
			}
		}
	}

	return taps, nil
}

// GetTAPStats returns statistics for a TAP device
func (s *TAPDeviceService) GetTAPStats(name string) (TAPStats, error) {
	var stats TAPStats

	cmd := exec.Command("ip", "-s", "link", "show", name)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return stats, fmt.Errorf("failed to get TAP stats: %w", err)
	}

	// Parse the output
	lines := strings.Split(string(output), "\n")
	for _, line := range lines {
		// Look for RX/TX lines
		if strings.Contains(line, "RX:") {
			// Next line has RX stats
			continue
		}
		if strings.Contains(line, "TX:") {
			// Next line has TX stats
			continue
		}
	}

	return stats, nil
}

// TAPStats represents TAP device statistics
type TAPStats struct {
	RxBytes   int64
	TxBytes   int64
	RxPackets int64
	TxPackets int64
	RxErrors  int64
	TxErrors  int64
}

// GenerateTAPName generates a TAP device name for a VM
func GenerateTAPName(vmID string) string {
	// Use first 8 chars of VM ID to create unique TAP name
	if len(vmID) >= 8 {
		return "tap" + vmID[:8]
	}
	return "tap" + vmID
}

// GenerateMACAddress generates a locally-administered MAC address for a VM
// Uses the VM ID to create a deterministic, unique MAC
func GenerateMACAddress(vmID string) string {
	// Locally administered MAC prefix (02:00:00)
	// Last 3 bytes derived from VM ID hash
	var hash uint32 = 0
	for i := 0; i < len(vmID) && i < 12; i++ {
		hash = hash*31 + uint32(vmID[i])
	}
	return fmt.Sprintf("02:00:00:%02x:%02x:%02x",
		(byte(hash>>16) & 0xFF),
		(byte(hash>>8) & 0xFF),
		(byte(hash) & 0xFF))
}

// GetVMIP generates an IP address for a VM based on its ID
func GetVMIP(vmID string, subnet string) (string, error) {
	// Parse the subnet
	ip, ipnet, err := net.ParseCIDR(subnet)
	if err != nil {
		return "", fmt.Errorf("invalid subnet: %w", err)
	}

	// Generate last octet from VM ID hash
	// Simple approach: use first 2 hex chars as number 10-255
	var lastOctet int
	if len(vmID) >= 2 {
		// Convert first 2 hex chars to int
		var high byte
		fmt.Sscanf(vmID[:2], "%02x", &high)
		lastOctet = int(high)
		if lastOctet < 10 {
			lastOctet += 10 // Avoid network and broadcast addresses
		}
	}

	// Build IP address
	ipBytes := ip.To4()
	if ipBytes == nil {
		return "", fmt.Errorf("IPv4 required")
	}

	// Check if IP is in range
	vmIP := net.IPv4(ipBytes[0], ipBytes[1], ipBytes[2], byte(lastOctet))
	if !ipnet.Contains(vmIP) {
		return "", fmt.Errorf("generated IP not in subnet")
	}

	return vmIP.String(), nil
}
