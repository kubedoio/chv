package models

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

// ResourceQuota defines per-user resource limits.
type ResourceQuota struct {
	UserID      uuid.UUID `json:"user_id" db:"user_id"`
	MaxCPUs     int       `json:"max_cpus" db:"max_cpus"`
	MaxMemoryMB int64     `json:"max_memory_mb" db:"max_memory_mb"`
	MaxVMCount  int       `json:"max_vm_count" db:"max_vm_count"`
	MaxDiskGB   int64     `json:"max_disk_gb" db:"max_disk_gb"`
	CreatedAt   time.Time `json:"created_at" db:"created_at"`
	UpdatedAt   time.Time `json:"updated_at" db:"updated_at"`
}

// ResourceUsage tracks actual resource usage per user.
type ResourceUsage struct {
	UserID       uuid.UUID `json:"user_id" db:"user_id"`
	CPUsUsed     int       `json:"cpus_used" db:"cpus_used"`
	MemoryMBUsed int64     `json:"memory_mb_used" db:"memory_mb_used"`
	VMCount      int       `json:"vm_count" db:"vm_count"`
	DiskGBUsed   int64     `json:"disk_gb_used" db:"disk_gb_used"`
	UpdatedAt    time.Time `json:"updated_at" db:"updated_at"`
}

// DefaultQuota returns the default quota for new users.
func DefaultQuota(userID uuid.UUID) *ResourceQuota {
	return &ResourceQuota{
		UserID:      userID,
		MaxCPUs:     8,     // 8 vCPUs
		MaxMemoryMB: 16384, // 16 GB
		MaxVMCount:  5,     // 5 VMs
		MaxDiskGB:   100,   // 100 GB
		CreatedAt:   time.Now(),
		UpdatedAt:   time.Now(),
	}
}

// VMDesiredState represents the desired state of a VM.
type VMDesiredState string

const (
	VMDesiredStatePresent  VMDesiredState = "present"
	VMDesiredStateRunning  VMDesiredState = "running"
	VMDesiredStateStopped  VMDesiredState = "stopped"
	VMDesiredStateDeleted  VMDesiredState = "deleted"
)

// VMActualState represents the actual state of a VM.
type VMActualState string

const (
	VMActualStateProvisioning VMActualState = "provisioning"
	VMActualStateStarting     VMActualState = "starting"
	VMActualStateRunning      VMActualState = "running"
	VMActualStateStopping     VMActualState = "stopping"
	VMActualStateStopped      VMActualState = "stopped"
	VMActualStateDeleting     VMActualState = "deleting"
	VMActualStateError        VMActualState = "error"
	VMActualStateUnknown      VMActualState = "unknown"
)

// PlacementStatus represents the placement status of a VM.
type PlacementStatus string

const (
	PlacementStatusPending   PlacementStatus = "pending"
	PlacementStatusScheduled PlacementStatus = "scheduled"
	PlacementStatusFailed    PlacementStatus = "failed"
)

// VMSpec represents the VM specification.
type VMSpec struct {
	CPU       int32              `json:"cpu"`
	MemoryMB  int64              `json:"memory_mb"`
	Boot      BootSpec           `json:"boot"`
	Disks     []DiskAttachment   `json:"disks"`
	Networks  []NetworkAttachment `json:"networks"`
	CloudInit *CloudInitSpec     `json:"cloud_init,omitempty"`
}

// BootSpec represents the boot configuration.
type BootSpec struct {
	Mode         string `json:"mode"` // cloud_image, uefi, direct_kernel
	KernelPath   string `json:"kernel_path,omitempty"`
	InitrdPath   string `json:"initrd_path,omitempty"`
	Cmdline      string `json:"cmdline,omitempty"`
	FirmwarePath string `json:"firmware_path,omitempty"`
}

// DiskAttachment represents a disk attachment configuration.
type DiskAttachment struct {
	VolumeID string `json:"volume_id"`
	Bus      string `json:"bus"` // virtio-blk, virtio-scsi
	Boot     bool   `json:"boot"`
	Readonly bool   `json:"readonly"`
}

// NetworkAttachment represents a network attachment configuration.
type NetworkAttachment struct {
	NetworkID  string `json:"network_id"`
	MACAddress string `json:"mac_address,omitempty"`
	IPAddress  string `json:"ip_address,omitempty"`
	DHCP       bool   `json:"dhcp"`
}

// CloudInitSpec represents cloud-init configuration.
type CloudInitSpec struct {
	UserData      string `json:"user_data,omitempty"`
	MetaData      string `json:"meta_data,omitempty"`
	NetworkConfig string `json:"network_config,omitempty"`
}

// VirtualMachine represents a virtual machine.
type VirtualMachine struct {
	ID              uuid.UUID       `json:"id" db:"id"`
	Name            string          `json:"name" db:"name"`
	NodeID          *uuid.UUID      `json:"node_id" db:"node_id"`
	CreatedBy       string          `json:"created_by" db:"created_by"`
	DesiredState    VMDesiredState  `json:"desired_state" db:"desired_state"`
	ActualState     VMActualState   `json:"actual_state" db:"actual_state"`
	PlacementStatus PlacementStatus `json:"placement_status" db:"placement_status"`
	Spec            json.RawMessage `json:"spec" db:"spec"`
	LastError       json.RawMessage `json:"last_error,omitempty" db:"last_error"`
	CreatedAt       time.Time       `json:"created_at" db:"created_at"`
	UpdatedAt       time.Time       `json:"updated_at" db:"updated_at"`
}

// GetSpec parses and returns the VM spec.
func (vm *VirtualMachine) GetSpec() (*VMSpec, error) {
	var spec VMSpec
	if err := json.Unmarshal(vm.Spec, &spec); err != nil {
		return nil, err
	}
	return &spec, nil
}

// SetSpec sets the VM spec from a struct.
func (vm *VirtualMachine) SetSpec(spec *VMSpec) error {
	data, err := json.Marshal(spec)
	if err != nil {
		return err
	}
	vm.Spec = data
	return nil
}

// GetLastError parses and returns the last error.
func (vm *VirtualMachine) GetLastError() (map[string]interface{}, error) {
	if vm.LastError == nil {
		return nil, nil
	}
	var err map[string]interface{}
	if e := json.Unmarshal(vm.LastError, &err); e != nil {
		return nil, e
	}
	return err, nil
}

// CanStart returns true if the VM can be started.
func (vm *VirtualMachine) CanStart() bool {
	return vm.ActualState == VMActualStateStopped || 
		vm.ActualState == VMActualStateError ||
		vm.ActualState == VMActualStateUnknown
}

// CanStop returns true if the VM can be stopped.
func (vm *VirtualMachine) CanStop() bool {
	return vm.ActualState == VMActualStateRunning || vm.ActualState == VMActualStateStarting
}

// NeedsReconciliation returns true if desired and actual states differ.
func (vm *VirtualMachine) NeedsReconciliation() bool {
	switch vm.DesiredState {
	case VMDesiredStateRunning:
		return vm.ActualState != VMActualStateRunning
	case VMDesiredStateStopped:
		return vm.ActualState != VMActualStateStopped && vm.ActualState != VMActualStateProvisioning
	case VMDesiredStateDeleted:
		return vm.ActualState != VMActualStateDeleting
	}
	return false
}
