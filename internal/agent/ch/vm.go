package ch

import (
	"context"
	"fmt"
	"net/http"
)

// VMConfig represents the configuration for creating a VM.
type VMConfig struct {
	CPUs     *CPUConfig     `json:"cpus,omitempty"`
	Memory   *MemoryConfig  `json:"memory,omitempty"`
	Kernel   *KernelConfig  `json:"kernel,omitempty"`
	Cmdline  *CmdlineConfig `json:"cmdline,omitempty"`
	Disks    []DiskConfig   `json:"disks,omitempty"`
	Net      []NetConfig    `json:"net,omitempty"`
	Console  *ConsoleConfig `json:"console,omitempty"`
	Serial   *ConsoleConfig `json:"serial,omitempty"`
	Payload  *PayloadConfig `json:"payload,omitempty"`
}

// CPUConfig represents CPU configuration.
type CPUConfig struct {
	BootVCPUs   int  `json:"boot_vcpus"`
	MaxVCPUs    int  `json:"max_vcpus"`
}

// MemoryConfig represents memory configuration.
type MemoryConfig struct {
	Size         int64  `json:"size"`
	HotplugMethod string `json:"hotplug_method,omitempty"`
	HotplugSize   *int64 `json:"hotplug_size,omitempty"`
	Mergeable     bool   `json:"mergeable,omitempty"`
	Shared        bool   `json:"shared,omitempty"`
	Hugepages     bool   `json:"hugepages,omitempty"`
	HugepageSize  *int64 `json:"hugepage_size,omitempty"`
}

// KernelConfig represents the kernel to boot.
type KernelConfig struct {
	Path string `json:"path"`
}

// CmdlineConfig represents kernel command line arguments.
type CmdlineConfig struct {
	Args string `json:"args"`
}

// DiskConfig represents a disk device.
type DiskConfig struct {
	Path     string `json:"path"`
	Readonly bool   `json:"readonly,omitempty"`
	Direct   bool   `json:"direct,omitempty"`
	Iommu    bool   `json:"iommu,omitempty"`
}

// NetConfig represents a network device.
type NetConfig struct {
	Tap   string `json:"tap,omitempty"`
	Mac   string `json:"mac,omitempty"`
	Iommu bool   `json:"iommu,omitempty"`
}

// ConsoleConfig represents console/serial configuration.
type ConsoleConfig struct {
	Mode string `json:"mode"` // "File", "Tty", "Off", "Null"
	File string `json:"file,omitempty"`
}

// PayloadConfig represents firmware/payload configuration.
type PayloadConfig struct {
	Firmware string `json:"firmware,omitempty"`
}

// VMInfo represents information about a running VM.
type VMInfo struct {
	Config     VMConfig `json:"config"`
	State      string   `json:"state"` // "Running", "Paused", "Shutdown"
	MemorySize int64    `json:"memory_size,omitempty"`
}

// PMCounters represents performance monitor counters.
type PMCounters struct {
	Instructions uint64 `json:"instructions,omitempty"`
	Cycles       uint64 `json:"cycles,omitempty"`
}

// NetCounters represents network counters.
type NetCounters struct {
	RXBytes uint64 `json:"rx_bytes,omitempty"`
	TXBytes uint64 `json:"tx_bytes,omitempty"`
	RXPackets uint64 `json:"rx_packets,omitempty"`
	TXPackets uint64 `json:"tx_packets,omitempty"`
}

// DiskCounters represents disk counters.
type DiskCounters struct {
	ReadBytes  uint64 `json:"read_bytes,omitempty"`
	WriteBytes uint64 `json:"write_bytes,omitempty"`
	ReadOps    uint64 `json:"read_ops,omitempty"`
	WriteOps   uint64 `json:"write_ops,omitempty"`
}

// VMCounters represents VM performance counters.
type VMCounters struct {
	VCPUs   []PMCounters  `json:"vcpus,omitempty"`
	Net     []NetCounters  `json:"net,omitempty"`
	Disks   []DiskCounters `json:"disks,omitempty"`
}

// CreateVM creates a new VM with the given configuration.
func (c *Client) CreateVM(ctx context.Context, config *VMConfig) error {
	resp, err := c.do(ctx, "PUT", "vm.create", config)
	if err != nil {
		return fmt.Errorf("create vm: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent {
		return fmt.Errorf("create vm failed: %d", resp.StatusCode)
	}
	return nil
}

// BootVM boots the created VM.
func (c *Client) BootVM(ctx context.Context) error {
	resp, err := c.do(ctx, "PUT", "vm.boot", nil)
	if err != nil {
		return fmt.Errorf("boot vm: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent {
		return fmt.Errorf("boot vm failed: %d", resp.StatusCode)
	}
	return nil
}

// ShutdownVM gracefully shuts down the VM.
func (c *Client) ShutdownVM(ctx context.Context) error {
	resp, err := c.do(ctx, "PUT", "vm.shutdown", nil)
	if err != nil {
		return fmt.Errorf("shutdown vm: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent {
		return fmt.Errorf("shutdown vm failed: %d", resp.StatusCode)
	}
	return nil
}

// RebootVM reboots the VM.
func (c *Client) RebootVM(ctx context.Context) error {
	resp, err := c.do(ctx, "PUT", "vm.reboot", nil)
	if err != nil {
		return fmt.Errorf("reboot vm: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent {
		return fmt.Errorf("reboot vm failed: %d", resp.StatusCode)
	}
	return nil
}

// PauseVM pauses the VM.
func (c *Client) PauseVM(ctx context.Context) error {
	resp, err := c.do(ctx, "PUT", "vm.pause", nil)
	if err != nil {
		return fmt.Errorf("pause vm: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent {
		return fmt.Errorf("pause vm failed: %d", resp.StatusCode)
	}
	return nil
}

// ResumeVM resumes a paused VM.
func (c *Client) ResumeVM(ctx context.Context) error {
	resp, err := c.do(ctx, "PUT", "vm.resume", nil)
	if err != nil {
		return fmt.Errorf("resume vm: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent {
		return fmt.Errorf("resume vm failed: %d", resp.StatusCode)
	}
	return nil
}

// DeleteVM deletes the VM (must be shutdown first).
func (c *Client) DeleteVM(ctx context.Context) error {
	resp, err := c.do(ctx, "PUT", "vm.delete", nil)
	if err != nil {
		return fmt.Errorf("delete vm: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent {
		return fmt.Errorf("delete vm failed: %d", resp.StatusCode)
	}
	return nil
}

// GetVMInfo returns information about the VM.
func (c *Client) GetVMInfo(ctx context.Context) (*VMInfo, error) {
	var info VMInfo
	if err := c.get(ctx, "vm.info", &info); err != nil {
		return nil, err
	}
	return &info, nil
}

// GetVMCounters returns performance counters.
func (c *Client) GetVMCounters(ctx context.Context) (*VMCounters, error) {
	var counters VMCounters
	if err := c.get(ctx, "vm.counters", &counters); err != nil {
		return nil, err
	}
	return &counters, nil
}
