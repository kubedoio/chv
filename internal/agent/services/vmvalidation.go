package services

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"

	"github.com/chv/chv/internal/agentapi"
)

// VMValidationService validates running VMs against expected state
type VMValidationService struct {
	vmService *VMManagementService
}

// NewVMValidationService creates a new VM validation service
func NewVMValidationService(vmService *VMManagementService) *VMValidationService {
	return &VMValidationService{
		vmService: vmService,
	}
}

// ValidateRunningVMs scans the system for running cloud-hypervisor processes
// and validates them against the expected list of VMs
func (s *VMValidationService) ValidateRunningVMs(req *agentapi.VMValidationRequest) (*agentapi.VMValidationResponse, error) {
	// Create a set of expected VM IDs for quick lookup
	expectedSet := make(map[string]bool)
	for _, vmID := range req.ExpectedVMIDs {
		expectedSet[vmID] = true
	}

	// Scan for all running cloud-hypervisor processes
	runningVMs, err := s.scanRunningVMs(req.DataRoot)
	if err != nil {
		return nil, fmt.Errorf("failed to scan running VMs: %w", err)
	}

	// Categorize VMs
	var validVMs, orphanVMs []agentapi.RunningVMInfo
	foundSet := make(map[string]bool)

	for _, vm := range runningVMs {
		foundSet[vm.VMID] = true
		if expectedSet[vm.VMID] {
			vm.IsManaged = true
			validVMs = append(validVMs, vm)
		} else {
			vm.IsManaged = false
			orphanVMs = append(orphanVMs, vm)
		}
	}

	// Find missing VMs (expected but not running)
	var missingVMs []string
	for _, vmID := range req.ExpectedVMIDs {
		if !foundSet[vmID] {
			missingVMs = append(missingVMs, vmID)
		}
	}

	return &agentapi.VMValidationResponse{
		RunningVMs: runningVMs,
		OrphanVMs:  orphanVMs,
		MissingVMs: missingVMs,
		ValidVMs:   validVMs,
		Summary: agentapi.ValidationSummary{
			TotalRunning: len(runningVMs),
			Valid:        len(validVMs),
			Orphans:      len(orphanVMs),
			Missing:      len(missingVMs),
		},
	}, nil
}

// scanRunningVMs scans /proc for all running cloud-hypervisor processes
func (s *VMValidationService) scanRunningVMs(dataRoot string) ([]agentapi.RunningVMInfo, error) {
	var vms []agentapi.RunningVMInfo

	// Look through /proc for all processes
	matches, err := filepath.Glob("/proc/[0-9]*/cmdline")
	if err != nil {
		return nil, fmt.Errorf("failed to glob /proc: %w", err)
	}

	for _, match := range matches {
		data, err := os.ReadFile(match)
		if err != nil {
			continue
		}

		// cmdline is null-separated
		args := strings.Split(string(data), "\x00")
		if len(args) == 0 {
			continue
		}

		// Check if this is a cloud-hypervisor process
		isCH := false
		for _, arg := range args {
			if strings.Contains(arg, "cloud-hypervisor") {
				isCH = true
				break
			}
		}
		if !isCH {
			continue
		}
		
		// Reconstruct the full command line for reference
		cmdLine := strings.Join(args, " ")

		// Extract PID from path /proc/<PID>/cmdline
		parts := strings.Split(match, "/")
		if len(parts) < 3 {
			continue
		}
		pid, err := strconv.Atoi(parts[2])
		if err != nil {
			continue
		}

		// Verify process is actually running
		if !s.isProcessRunning(pid) {
			continue
		}

		// Parse the command line arguments
		vmInfo := s.parseCommandLine(args, pid, cmdLine)
		
		// If dataRoot is provided, check if VM is managed
		if dataRoot != "" && vmInfo.SocketPath != "" {
			absDataRoot, _ := filepath.Abs(dataRoot)
			absSocket, _ := filepath.Abs(vmInfo.SocketPath)
			vmInfo.IsManaged = strings.HasPrefix(absSocket, absDataRoot)
		}

		vms = append(vms, vmInfo)
	}

	return vms, nil
}

// parseCommandLine extracts VM information from cloud-hypervisor command line arguments
func (s *VMValidationService) parseCommandLine(args []string, pid int, cmdLine string) agentapi.RunningVMInfo {
	info := agentapi.RunningVMInfo{
		PID:         pid,
		CommandLine: cmdLine,
		VCPU:        1,     // Default
		MemoryMB:    512,   // Default
	}

	for i := 0; i < len(args); i++ {
		arg := args[i]

		switch arg {
		case "--api-socket":
			if i+1 < len(args) {
				info.SocketPath = args[i+1]
				// Try to extract VM ID from socket path
				// Expected format: /path/to/vms/<vmid>/api.sock
				info.VMID = s.extractVMIDFromSocketPath(info.SocketPath)
				info.WorkspacePath = filepath.Dir(info.SocketPath)
			}

		case "--disk":
			if i+1 < len(args) {
				diskArg := args[i+1]
				// Parse disk path, handling format like "path=/path/to/disk.qcow2"
				if strings.HasPrefix(diskArg, "path=") {
					diskPath := strings.TrimPrefix(diskArg, "path=")
					// Handle multiple disks (comma-separated or multiple --disk args)
					diskParts := strings.Split(diskPath, ",")
					for _, part := range diskParts {
						if strings.HasPrefix(part, "path=") {
							part = strings.TrimPrefix(part, "path=")
						}
						// Skip if it's a seed ISO (read-only)
						if strings.Contains(part, "seed.iso") || strings.Contains(part, "readonly=on") {
							// This might be the seed ISO, extract it
							if !strings.Contains(part, "readonly=on") && strings.Contains(part, ".iso") {
								info.SeedISOPath = part
							} else if strings.Contains(part, "seed.iso") {
								// Extract just the path part
								pathParts := strings.Split(part, ",")
								if len(pathParts) > 0 {
									info.SeedISOPath = pathParts[0]
								}
							}
						} else if part != "" && !strings.Contains(part, "=") {
							// This is likely the main disk
							info.DiskPath = part
						}
					}
				} else {
					info.DiskPath = diskArg
				}
			}

		case "--cpus":
			if i+1 < len(args) {
				cpuArg := args[i+1]
				// Parse "boot=N" format
				if strings.HasPrefix(cpuArg, "boot=") {
					cpuArg = strings.TrimPrefix(cpuArg, "boot=")
				}
				if vcpu, err := strconv.Atoi(cpuArg); err == nil {
					info.VCPU = vcpu
				}
			}

		case "--memory":
			if i+1 < len(args) {
				memArg := args[i+1]
				// Parse "size=XM" format
				if strings.HasPrefix(memArg, "size=") {
					memArg = strings.TrimPrefix(memArg, "size=")
				}
				// Remove trailing M or G
				memArg = strings.TrimSuffix(memArg, "M")
				memArg = strings.TrimSuffix(memArg, "m")
				memArg = strings.TrimSuffix(memArg, "G")
				memArg = strings.TrimSuffix(memArg, "g")
				if mem, err := strconv.Atoi(memArg); err == nil {
					info.MemoryMB = mem
					// Convert GB to MB if needed
					if strings.Contains(args[i+1], "G") || strings.Contains(args[i+1], "g") {
						info.MemoryMB = mem * 1024
					}
				}
			}

		case "--net":
			if i+1 < len(args) {
				netArg := args[i+1]
				// Parse "tap=<name>,mac=<mac>,ip=<ip>" format
				netParts := strings.Split(netArg, ",")
				for _, part := range netParts {
					if strings.HasPrefix(part, "tap=") {
						info.TAPDevice = strings.TrimPrefix(part, "tap=")
					} else if strings.HasPrefix(part, "mac=") {
						info.MACAddress = strings.TrimPrefix(part, "mac=")
					} else if strings.HasPrefix(part, "ip=") {
						info.IPAddress = strings.TrimPrefix(part, "ip=")
					}
				}
			}

		case "--kernel":
			if i+1 < len(args) {
				info.KernelPath = args[i+1]
			}
		}
	}

	// If VMID is still empty, try to extract from disk path or socket path
	if info.VMID == "" {
		if info.DiskPath != "" {
			info.VMID = s.extractVMIDFromPath(info.DiskPath)
		}
		if info.VMID == "" && info.SocketPath != "" {
			info.VMID = s.extractVMIDFromPath(info.SocketPath)
		}
		// Last resort: use PID as identifier
		if info.VMID == "" {
			info.VMID = fmt.Sprintf("unknown-%d", pid)
		}
	}

	return info
}

// extractVMIDFromSocketPath extracts VM ID from socket path
// Expected format: /path/to/vms/<vmid>/api.sock
func (s *VMValidationService) extractVMIDFromSocketPath(socketPath string) string {
	// Get the directory containing api.sock
	dir := filepath.Dir(socketPath)
	if dir == "." || dir == "/" {
		return ""
	}

	// Get the base name (should be the VM ID directory)
	vmID := filepath.Base(dir)
	
	// Validate it looks like a VM ID
	if isValidVMID(vmID) {
		return vmID
	}

	// Try going up one more level if this looks like a workspace structure
	parent := filepath.Base(filepath.Dir(dir))
	if isValidVMID(parent) {
		return parent
	}

	return ""
}

// extractVMIDFromPath extracts VM ID from a path containing vms/<vmid>/
func (s *VMValidationService) extractVMIDFromPath(path string) string {
	// Look for pattern /vms/<vmid>/ in the path
	parts := strings.Split(path, string(filepath.Separator))
	for i, part := range parts {
		if part == "vms" && i+1 < len(parts) {
			vmID := parts[i+1]
			if isValidVMID(vmID) {
				return vmID
			}
		}
	}
	return ""
}

// isProcessRunning checks if a process with the given PID is running
func (s *VMValidationService) isProcessRunning(pid int) bool {
	if pid <= 0 {
		return false
	}
	process, err := os.FindProcess(pid)
	if err != nil {
		return false
	}
	// On Unix, FindProcess always succeeds, so we need to send signal 0
	err = process.Signal(syscall.Signal(0))
	return err == nil
}

// GetVMByPID returns information about a specific VM by its PID
func (s *VMValidationService) GetVMByPID(pid int) (*agentapi.RunningVMInfo, error) {
	cmdlinePath := filepath.Join("/proc", strconv.Itoa(pid), "cmdline")
	data, err := os.ReadFile(cmdlinePath)
	if err != nil {
		return nil, fmt.Errorf("failed to read process cmdline: %w", err)
	}

	args := strings.Split(string(data), "\x00")
	if len(args) == 0 {
		return nil, fmt.Errorf("empty command line")
	}

	// Verify this is a cloud-hypervisor process
	isCH := false
	for _, arg := range args {
		if strings.Contains(arg, "cloud-hypervisor") {
			isCH = true
			break
		}
	}
	if !isCH {
		return nil, fmt.Errorf("process %d is not a cloud-hypervisor process", pid)
	}

	cmdLine := strings.Join(args, " ")
	info := s.parseCommandLine(args, pid, cmdLine)
	
	return &info, nil
}

// GetVMByVMID returns information about a specific VM by its VM ID
func (s *VMValidationService) GetVMByVMID(vmID string) (*agentapi.RunningVMInfo, error) {
	// Scan all running VMs and find the one with matching ID
	vms, err := s.scanRunningVMs("")
	if err != nil {
		return nil, err
	}

	for _, vm := range vms {
		if vm.VMID == vmID {
			return &vm, nil
		}
	}

	return nil, fmt.Errorf("VM %s not found running", vmID)
}
