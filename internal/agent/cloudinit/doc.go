// Package cloudinit provides cloud-init ISO generation for CloudHypervisor VMs.
//
// This package wraps the core cloudinit functionality in internal/cloudinit
// and provides VM-specific ISO management for the agent. It handles:
//
//   - ISO generation for cloud-init NoCloud datasource (FAT12/ISO9660)
//   - VM-specific file paths (cloudinit directory or VM directory)
//   - ISO cleanup on VM deletion
//   - Cleanup of orphaned ISOs on agent restart
//   - Integration with CloudHypervisor disk attachment
//
// # Cloud-init NoCloud Format
//
// The generated ISOs follow the cloud-init NoCloud datasource format:
//
//   /cidata/
//   ├── user-data      # Cloud-init user configuration
//   ├── meta-data      # Instance metadata (instance-id, local-hostname)
//   └── network-config # Network configuration (optional)
//
// The ISO is labeled "cidata" for automatic detection by cloud-init.
//
// # ISO Storage Locations
//
// ISOs can be stored in two locations:
//
//  1. Shared cloudinit directory: <dataDir>/cloudinit/<vmID>-cloudinit.iso
//  2. VM-specific directory: <dataDir>/instances/<vmID>/cloudinit.iso
//
// The shared directory is preferred for most use cases. The VM-specific
// directory is useful when VM files need to be co-located.
//
// # CloudHypervisor Integration
//
// CloudHypervisor mounts the ISO as a read-only disk device. The ISO
// must be formatted correctly for cloud-init to detect it as a NoCloud
// datasource.
//
// Example usage:
//
//	gen := cloudinit.NewISOGenerator("/var/lib/chv")
//
//	// Generate ISO
//	isoPath, err := gen.GenerateISO("vm-123", &cloudinit.Config{
//	    UserData: "#cloud-config\nusers:\n  - name: admin\n",
//	    MetaData: "instance-id: vm-123\nlocal-hostname: myvm\n",
//	})
//
//	// Attach to CloudHypervisor via --disk path=<isoPath>,readonly=on
//
//	// Cleanup when VM is deleted
//	err = gen.DeleteISO("vm-123")
//
// # Cleanup
//
// The generator provides CleanupOrphanedISOs for removing ISOs that
// belong to VMs no longer running. This should be called on agent
// startup to handle crash recovery scenarios.
//
//	activeVMs := []string{"vm-1", "vm-2", "vm-3"}
//	err := gen.CleanupOrphanedISOs(activeVMs)
//
package cloudinit
