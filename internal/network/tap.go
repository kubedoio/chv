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
	defaultBridge string
}

// NewTAPManager creates a new TAP manager.
func NewTAPManager(defaultBridge string) *TAPManager {
	return &TAPManager{
		defaultBridge: defaultBridge,
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
func (tm *TAPManager) EnsureBridge(bridgeName string) error {
	cmd := exec.Command("ip", "link", "show", bridgeName)
	if err := cmd.Run(); err == nil {
		// Bridge exists
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
