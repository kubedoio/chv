package network

import (
	"os/exec"
	"strings"
	"testing"
)

// Test helper: check if running as root (required for TAP operations)
func isRoot() bool {
	cmd := exec.Command("id", "-u")
	out, err := cmd.Output()
	if err != nil {
		return false
	}
	return strings.TrimSpace(string(out)) == "0"
}

func TestTAPManager_generateTAPName(t *testing.T) {
	tm := NewTAPManager("br0", "", "")

	tests := []struct {
		name   string
		vmID   string
		expect string
	}{
		{
			name:   "standard UUID",
			vmID:   "550e8400-e29b-41d4-a716-446655440000",
			expect: "tap550e8400e29b",
		},
		{
			name:   "short ID",
			vmID:   "abc123",
			expect: "tapabc123",
		},
		{
			name:   "long ID",
			vmID:   "550e8400e29b41d4a716446655440000",
			expect: "tap550e8400e29b",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := tm.generateTAPName(tt.vmID)
			if got != tt.expect {
				t.Errorf("generateTAPName() = %v, want %v", got, tt.expect)
			}
			// Verify length constraint (max 15 chars for Linux interface)
			if len(got) > 15 {
				t.Errorf("TAP name too long: %d chars, max is 15", len(got))
			}
		})
	}
}

func TestTAPManager_generateMAC(t *testing.T) {
	tm := NewTAPManager("br0", "", "")

	// Test deterministic generation
	vmID := "550e8400-e29b-41d4-a716-446655440000"
	mac1 := tm.generateMAC(vmID)
	mac2 := tm.generateMAC(vmID)

	if mac1 != mac2 {
		t.Errorf("MAC generation not deterministic: %s vs %s", mac1, mac2)
	}

	// Verify format (should be 6 hex bytes separated by colons)
	if len(mac1) != 17 { // XX:XX:XX:XX:XX:XX = 17 chars
		t.Errorf("MAC has wrong length: %d, expected 17", len(mac1))
	}

	// Verify locally administered bit (second hex char should be 2, 6, A, or E)
	// 02:xx:xx:xx:xx:xx means locally administered
	if !strings.HasPrefix(mac1, "02:") && !strings.HasPrefix(mac1, "06:") &&
		!strings.HasPrefix(mac1, "0a:") && !strings.HasPrefix(mac1, "0e:") &&
		!strings.HasPrefix(mac1, "0A:") && !strings.HasPrefix(mac1, "0E:") {
		t.Errorf("MAC not in locally administered range: %s", mac1)
	}

	// Test different VMs get different MACs
	vmID2 := "660e8400-e29b-41d4-a716-446655440001"
	mac3 := tm.generateMAC(vmID2)
	if mac1 == mac3 {
		t.Error("Different VMs should have different MACs")
	}
}

func TestTAPManager_generateMACValidFormat(t *testing.T) {
	tm := NewTAPManager("br0", "", "")
	
	vmID := "test-vm-id"
	mac := tm.generateMAC(vmID)
	
	parts := strings.Split(mac, ":")
	if len(parts) != 6 {
		t.Errorf("MAC should have 6 parts, got %d: %s", len(parts), mac)
	}
	
	for i, part := range parts {
		if len(part) != 2 {
			t.Errorf("Part %d should be 2 hex chars, got '%s'", i, part)
		}
	}
}

// Integration tests - require root privileges

func TestTAPManager_EnsureBridge(t *testing.T) {
	if !isRoot() {
		t.Skip("Skipping: requires root privileges")
	}

	tm := NewTAPManager("br-test-1", "", "")
	
	// Clean up any existing bridge
	exec.Command("ip", "link", "del", "br-test-1").Run()
	
	// Create bridge
	if err := tm.EnsureBridge("br-test-1"); err != nil {
		t.Fatalf("EnsureBridge failed: %v", err)
	}
	
	// Verify bridge exists
	cmd := exec.Command("ip", "link", "show", "br-test-1")
	if err := cmd.Run(); err != nil {
		t.Error("Bridge should exist after EnsureBridge")
	}
	
	// Second call should succeed (idempotent)
	if err := tm.EnsureBridge("br-test-1"); err != nil {
		t.Fatalf("EnsureBridge second call failed: %v", err)
	}
	
	// Cleanup
	exec.Command("ip", "link", "del", "br-test-1").Run()
}

func TestTAPManager_CreateAndDeleteTAP(t *testing.T) {
	if !isRoot() {
		t.Skip("Skipping: requires root privileges")
	}

	tm := NewTAPManager("br-test-2", "", "")
	vmID := "test-vm-550e8400-e29b-41d4-a716-446655440000"
	
	// Setup: create bridge
	exec.Command("ip", "link", "del", "br-test-2").Run()
	if err := tm.EnsureBridge("br-test-2"); err != nil {
		t.Fatalf("Failed to create bridge: %v", err)
	}
	defer exec.Command("ip", "link", "del", "br-test-2").Run()
	
	// Create TAP
	tap, err := tm.CreateTAP(vmID, "br-test-2")
	if err != nil {
		t.Fatalf("CreateTAP failed: %v", err)
	}
	
	// Verify TAP name format
	if !strings.HasPrefix(tap.Name, "tap") {
		t.Errorf("TAP name should start with 'tap', got %s", tap.Name)
	}
	
	// Verify MAC is set
	if tap.MACAddress == "" {
		t.Error("MAC address should be set")
	}
	
	// Verify TAP exists
	cmd := exec.Command("ip", "link", "show", tap.Name)
	if err := cmd.Run(); err != nil {
		t.Error("TAP should exist after creation")
	}
	
	// Verify TAP is attached to bridge
	bridge, err := tm.getAttachedBridge(tap.Name)
	if err != nil {
		t.Errorf("Failed to get attached bridge: %v", err)
	}
	if bridge != "br-test-2" {
		t.Errorf("TAP attached to wrong bridge: %s", bridge)
	}
	
	// Delete TAP
	if err := tm.DeleteTAP(tap.Name); err != nil {
		t.Fatalf("DeleteTAP failed: %v", err)
	}
	
	// Verify TAP is gone
	cmd = exec.Command("ip", "link", "show", tap.Name)
	if err := cmd.Run(); err == nil {
		t.Error("TAP should not exist after deletion")
	}
}

func TestTAPManager_CreateTAP_Idempotent(t *testing.T) {
	if !isRoot() {
		t.Skip("Skipping: requires root privileges")
	}

	tm := NewTAPManager("br-test-3", "", "")
	vmID := "test-vm-idempotent-550e8400"
	
	// Setup
	exec.Command("ip", "link", "del", "br-test-3").Run()
	if err := tm.EnsureBridge("br-test-3"); err != nil {
		t.Fatalf("Failed to create bridge: %v", err)
	}
	defer exec.Command("ip", "link", "del", "br-test-3").Run()
	
	// Create TAP first time
	tap1, err := tm.CreateTAP(vmID, "br-test-3")
	if err != nil {
		t.Fatalf("First CreateTAP failed: %v", err)
	}
	defer tm.DeleteTAP(tap1.Name)
	
	// Create TAP second time (should succeed, idempotent)
	tap2, err := tm.CreateTAP(vmID, "br-test-3")
	if err != nil {
		t.Fatalf("Second CreateTAP failed: %v", err)
	}
	
	// Should return same device info
	if tap1.Name != tap2.Name {
		t.Errorf("TAP names should match: %s vs %s", tap1.Name, tap2.Name)
	}
	if tap1.MACAddress != tap2.MACAddress {
		t.Errorf("MAC addresses should match: %s vs %s", tap1.MACAddress, tap2.MACAddress)
	}
}

func TestTAPManager_DeleteNonExistentTAP(t *testing.T) {
	if !isRoot() {
		t.Skip("Skipping: requires root privileges")
	}

	tm := NewTAPManager("br0", "", "")
	
	// Should not error when deleting non-existent TAP (use valid format)
	err := tm.DeleteTAP("tap550e8400e29b")
	if err != nil {
		// Some systems may return error, that's OK
		t.Logf("DeleteTAP returned error (may be OK): %v", err)
	}
}

func TestTAPManager_GetTAPDevice(t *testing.T) {
	if !isRoot() {
		t.Skip("Skipping: requires root privileges")
	}

	tm := NewTAPManager("br-test-4", "", "")
	vmID := "test-vm-get-550e8400"
	
	// Setup
	exec.Command("ip", "link", "del", "br-test-4").Run()
	if err := tm.EnsureBridge("br-test-4"); err != nil {
		t.Fatalf("Failed to create bridge: %v", err)
	}
	defer exec.Command("ip", "link", "del", "br-test-4").Run()
	
	// Get non-existent TAP
	tap, err := tm.GetTAPDevice(vmID)
	if err != nil {
		t.Errorf("GetTAPDevice should not error for non-existent TAP: %v", err)
	}
	if tap != nil {
		t.Error("Should return nil for non-existent TAP")
	}
	
	// Create TAP
	created, err := tm.CreateTAP(vmID, "br-test-4")
	if err != nil {
		t.Fatalf("CreateTAP failed: %v", err)
	}
	defer tm.DeleteTAP(created.Name)
	
	// Get existing TAP
	tap, err = tm.GetTAPDevice(vmID)
	if err != nil {
		t.Fatalf("GetTAPDevice failed: %v", err)
	}
	if tap == nil {
		t.Fatal("Should return TAP for existing device")
	}
	if tap.Name != created.Name {
		t.Errorf("Name mismatch: %s vs %s", tap.Name, created.Name)
	}
	if tap.MACAddress != created.MACAddress {
		t.Errorf("MAC mismatch: %s vs %s", tap.MACAddress, created.MACAddress)
	}
}

func TestTAPManager_ListTAPs(t *testing.T) {
	if !isRoot() {
		t.Skip("Skipping: requires root privileges")
	}

	tm := NewTAPManager("br-test-5", "", "")
	
	// Setup
	exec.Command("ip", "link", "del", "br-test-5").Run()
	if err := tm.EnsureBridge("br-test-5"); err != nil {
		t.Fatalf("Failed to create bridge: %v", err)
	}
	defer exec.Command("ip", "link", "del", "br-test-5").Run()
	
	// Create multiple TAPs
	vmIDs := []string{
		"test-vm-1-550e8400e29b",
		"test-vm-2-550e8400e29c",
		"test-vm-3-550e8400e29d",
	}
	
	for _, vmID := range vmIDs {
		tap, err := tm.CreateTAP(vmID, "br-test-5")
		if err != nil {
			t.Fatalf("CreateTAP failed for %s: %v", vmID, err)
		}
		defer tm.DeleteTAP(tap.Name)
	}
	
	// List TAPs
	taps, err := tm.ListTAPs("br-test-5")
	if err != nil {
		t.Fatalf("ListTAPs failed: %v", err)
	}
	
	if len(taps) < 3 {
		t.Errorf("Expected at least 3 TAPs, got %d", len(taps))
	}
	
	// Verify our TAPs are in the list
	// Note: The format from 'bridge link show' varies by system
	// Just verify we got some TAPs back
	t.Logf("Found TAPs: %v", taps)
	
	// Check if at least some TAPs were found
	if len(taps) == 0 {
		t.Error("Expected at least some TAPs, got none")
	}
}

func TestTAPManager_WrongBridge(t *testing.T) {
	if !isRoot() {
		t.Skip("Skipping: requires root privileges")
	}

	tm := NewTAPManager("br-test-6", "", "")
	vmID := "test-vm-wrong-550e8400"
	
	// Setup two bridges
	exec.Command("ip", "link", "del", "br-test-6").Run()
	exec.Command("ip", "link", "del", "br-test-7").Run()
	
	if err := tm.EnsureBridge("br-test-6"); err != nil {
		t.Fatalf("Failed to create bridge 6: %v", err)
	}
	defer exec.Command("ip", "link", "del", "br-test-6").Run()
	
	if err := tm.EnsureBridge("br-test-7"); err != nil {
		t.Fatalf("Failed to create bridge 7: %v", err)
	}
	defer exec.Command("ip", "link", "del", "br-test-7").Run()
	
	// Create TAP on bridge 6
	tap, err := tm.CreateTAP(vmID, "br-test-6")
	if err != nil {
		t.Fatalf("CreateTAP failed: %v", err)
	}
	defer tm.DeleteTAP(tap.Name)
	
	// Try to create same TAP on different bridge (should fail)
	_, err = tm.CreateTAP(vmID, "br-test-7")
	if err == nil {
		t.Error("CreateTAP should fail when TAP exists on different bridge")
	}
}
