package hypervisor

import (
	"strings"
	"testing"
)

func TestLauncher_BuildCommand(t *testing.T) {
	launcher := NewLauncher(&Config{
		BinaryPath: "/usr/bin/cloud-hypervisor",
		KernelPath: "/test/vmlinux",
	})

	params := VMParams{
		ID:        "vm-123",
		Name:      "test-vm",
		VCPU:      2,
		MemoryMB:  2048,
		DiskPath:  "/var/lib/chv/vms/vm-123/disk.qcow2",
		SeedISO:   "/var/lib/chv/vms/vm-123/seed.iso",
		TapDevice: "chvtap0",
		MacAddr:   "AA:BB:CC:DD:EE:FF",
		IPAddr:    "10.0.0.10",
		Netmask:   "255.255.255.0",
		Workspace: "/var/lib/chv/vms/vm-123",
	}

	cmd, err := launcher.BuildCommand(params)
	if err != nil {
		t.Fatalf("BuildCommand failed: %v", err)
	}

	// Verify command path
	if cmd.Path != "/usr/bin/cloud-hypervisor" {
		t.Errorf("expected binary path /usr/bin/cloud-hypervisor, got %s", cmd.Path)
	}

	// Verify args contain expected elements
	args := strings.Join(cmd.Args, " ")

	if !strings.Contains(args, "--kernel /test/vmlinux") {
		t.Error("expected --kernel arg")
	}

	if !strings.Contains(args, "--disk path=/var/lib/chv/vms/vm-123/disk.qcow2") {
		t.Error("expected disk path")
	}

	if !strings.Contains(args, "--disk path=/var/lib/chv/vms/vm-123/seed.iso,readonly=on") {
		t.Error("expected seed ISO with readonly")
	}

	if !strings.Contains(args, "--net tap=chvtap0") {
		t.Error("expected tap device")
	}

	if !strings.Contains(args, "mac=AA:BB:CC:DD:EE:FF") {
		t.Error("expected MAC address")
	}

	if !strings.Contains(args, "--cpus boot=2") {
		t.Error("expected CPU count")
	}

	if !strings.Contains(args, "--memory size=2048M") {
		t.Error("expected memory size")
	}

	if !strings.Contains(args, "--api-socket /var/lib/chv/vms/vm-123/ch-api.sock") {
		t.Error("expected API socket")
	}
}

func TestLauncher_BuildCommand_Minimal(t *testing.T) {
	launcher := NewLauncher(&Config{
		BinaryPath: "/usr/bin/cloud-hypervisor",
		KernelPath: "/test/vmlinux",
	})

	params := VMParams{
		ID:       "vm-456",
		VCPU:     1,
		MemoryMB: 1024,
		DiskPath: "/disk.qcow2",
	}

	cmd, err := launcher.BuildCommand(params)
	if err != nil {
		t.Fatalf("BuildCommand failed: %v", err)
	}

	args := strings.Join(cmd.Args, " ")

	if !strings.Contains(args, "--cpus boot=1") {
		t.Error("expected CPU count")
	}

	if !strings.Contains(args, "--memory size=1024M") {
		t.Error("expected memory size")
	}

	// Should not have network without TapDevice
	if strings.Contains(args, "--net") {
		t.Error("should not have network without tap device")
	}
}
