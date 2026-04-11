package services

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/chv/chv/internal/agentapi"
	"github.com/chv/chv/internal/cloudinit"
)

// VMManagementService handles VM lifecycle via Cloud Hypervisor
type VMManagementService struct {
	processes   map[string]*exec.Cmd // vmID -> cmd
	pids        map[string]int       // vmID -> pid
	tapService  *TAPDeviceService
	fwService   *FirewallService
	seedService *SeedISOService
	cloudInit   *cloudinit.Renderer
}

// NewVMManagementService creates a new VM management service
func NewVMManagementService(dataRoot string, bridgeName string, fw *FirewallService, seed *SeedISOService, ci *cloudinit.Renderer) *VMManagementService {
	return &VMManagementService{
		processes:  make(map[string]*exec.Cmd),
		pids:       make(map[string]int),
		tapService: NewTAPDeviceService(bridgeName),
		fwService:  fw,
		seedService: seed,
		cloudInit:   ci,
	}
}

// NewVMManagementServiceWithBridge creates a VM management service with custom bridge
func NewVMManagementServiceWithBridge(bridgeName string) *VMManagementService {
	return &VMManagementService{
		processes:  make(map[string]*exec.Cmd),
		pids:       make(map[string]int),
		tapService: NewTAPDeviceService(bridgeName),
	}
}

// StartVM launches a VM using Cloud Hypervisor
func (s *VMManagementService) StartVM(ctx context.Context, req *agentapi.VMStartRequest) (*agentapi.VMStartResponse, error) {
	// Check if already running in memory
	if pid, exists := s.pids[req.VMID]; exists {
		if s.isProcessRunning(pid) {
			return nil, fmt.Errorf("VM %s is already running with PID %d", req.VMID, pid)
		}
		// Clean up stale entry
		delete(s.pids, req.VMID)
		delete(s.processes, req.VMID)
	}

	// Check for orphan process on system
	socketPath := filepath.Join(req.WorkspacePath, "api.sock")
	if pid, running := s.isProcessRunningBySocket(socketPath); running {
		s.pids[req.VMID] = pid
		return nil, fmt.Errorf("VM %s is already running as an orphan process with PID %d", req.VMID, pid)
	}

	// Build CH command
	chPath := req.CloudHypervisorPath
	if chPath == "" {
		chPath = "/usr/bin/cloud-hypervisor"
	}

	// Find kernel if not specified
	kernelPath := req.KernelPath
	if kernelPath == "" {
		var err error
		kernelPath, err = s.findKernel()
		if err != nil {
			return nil, err
		}
	}

	// Create TAP device
	tapDev := req.TapDevice
	if tapDev == "" {
		tapDev = GenerateTAPName(req.VMID)
	}
	
	if err := s.tapService.CreateTAP(tapDev); err != nil {
		return nil, fmt.Errorf("failed to create TAP device: %w", err)
	}

	// 5. Create Firewall rules (anti-spoofing)
	if s.fwService != nil {
		if err := s.fwService.AddVMRules(req.VMID, tapDev, req.IPAddress, req.MACAddress); err != nil {
			// Log but don't fail for now - firewall might be missing
			fmt.Fprintf(os.Stderr, "Warning: failed to add firewall rules: %v\n", err)
		}
	}

	// Build command arguments
	// Note: cloud-hypervisor requires all disks in a single --disk argument
	// Multiple disks are separated by commas, not spaces
	diskArg := fmt.Sprintf("path=%s", req.DiskPath)
	if req.SeedISOPath != "" {
		diskArg += fmt.Sprintf(",path=%s,readonly=on", req.SeedISOPath)
	}

	args := []string{
		"--kernel", kernelPath,
		"--disk", diskArg,
	}

	// Network config
	netConfig := fmt.Sprintf("tap=%s", tapDev)
	if req.MACAddress != "" {
		netConfig += fmt.Sprintf(",mac=%s", req.MACAddress)
	}
	if req.IPAddress != "" && req.Netmask != "" {
		netConfig += fmt.Sprintf(",ip=%s,mask=%s", req.IPAddress, req.Netmask)
	}
	args = append(args, "--net", netConfig)

	// Console configuration
	// Use Unix socket for serial console (more reliable than PTY for WebSocket bridging)
	consoleSocket := filepath.Join(req.WorkspacePath, "console.sock")
	args = append(args, "--console", "off", "--serial", "socket="+consoleSocket)

	args = append(args,
		"--cpus", fmt.Sprintf("boot=%d", req.VCPU),
		"--memory", fmt.Sprintf("size=%dM", req.MemoryMB),
		"--api-socket", filepath.Join(req.WorkspacePath, "api.sock"),
		"--cmdline", "root=/dev/vda1 console=ttyS0",
	)

	// Create command - use background context so VM doesn't get killed when HTTP request completes
	cmd := exec.Command(chPath, args...)
	cmd.SysProcAttr = &syscall.SysProcAttr{
		Setsid: true, // Create new session so CH survives parent
	}

	// Redirect stdout/stderr to log files to prevent blocking
	// Note: Don't use defer Close() here - the files need to stay open
	// for the child process to write to them
	stdoutFile := filepath.Join(req.WorkspacePath, "chv.stdout.log")
	stderrFile := filepath.Join(req.WorkspacePath, "chv.stderr.log")
	
	stdout, err := os.Create(stdoutFile)
	if err != nil {
		return nil, fmt.Errorf("failed to create stdout log: %w", err)
	}
	
	stderr, err := os.Create(stderrFile)
	if err != nil {
		stdout.Close()
		return nil, fmt.Errorf("failed to create stderr log: %w", err)
	}
	
	cmd.Stdout = stdout
	cmd.Stderr = stderr
	
	// Files will be closed when the command finishes via waitForProcess goroutine

	// Start the process
	fmt.Fprintf(os.Stderr, "Starting cloud-hypervisor with args: %v\n", args)
	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("failed to start cloud-hypervisor: %w", err)
	}

	pid := cmd.Process.Pid
	fmt.Fprintf(os.Stderr, "Cloud-hypervisor started with PID: %d\n", pid)

	// Store process info
	s.processes[req.VMID] = cmd
	s.pids[req.VMID] = pid

	// Wait a moment to ensure process didn't immediately exit
	time.Sleep(500 * time.Millisecond)

	if !s.isProcessRunning(pid) {
		// Process exited quickly, likely an error
		stdout.Close()
		stderr.Close()
		delete(s.processes, req.VMID)
		delete(s.pids, req.VMID)
		return nil, fmt.Errorf("cloud-hypervisor process exited immediately")
	}

	// Give CH time to output the PTY path, then try to capture it
	go s.capturePtyPath(req.VMID, req.WorkspacePath, stdoutFile, stdout)

	// Start background waiter to clean up when process exits
	go s.waitForProcess(req.VMID, cmd, stdout, stderr)

	return &agentapi.VMStartResponse{
		PID: pid,
	}, nil
}

// StopVM stops a running VM
func (s *VMManagementService) StopVM(ctx context.Context, req *agentapi.VMStopRequest) (*agentapi.VMStopResponse, error) {
	pid, exists := s.pids[req.VMID]
	if !exists {
		// Even if not in memory, check if it's running on system if we have workspace info (not in stop request though)
		// For now, assume it's not running if not in pids map, or caller provides PID
		if req.PID == 0 {
			return nil, fmt.Errorf("VM %s is not running", req.VMID)
		}
		pid = req.PID
	}

	// Use requested PID if provided
	if req.PID != 0 {
		pid = req.PID
	}

	// Check if process is running
	if !s.isProcessRunning(pid) {
		delete(s.pids, req.VMID)
		delete(s.processes, req.VMID)
		return &agentapi.VMStopResponse{Stopped: true}, nil
	}

	// Hard stop if requested
	if req.Force {
		syscall.Kill(pid, syscall.SIGKILL)
		// Wait short time
		time.Sleep(200 * time.Millisecond)
		if !s.isProcessRunning(pid) {
			delete(s.pids, req.VMID)
			delete(s.processes, req.VMID)
			return &agentapi.VMStopResponse{Stopped: true}, nil
		}
	}

	// Try graceful shutdown via SIGTERM
	if err := syscall.Kill(pid, syscall.SIGTERM); err != nil {
		// If SIGTERM fails and we didn't already try SIGKILL, try SIGKILL
		if err := syscall.Kill(pid, syscall.SIGKILL); err != nil {
			return nil, fmt.Errorf("failed to stop VM: %w", err)
		}
	}

	// Wait for process to exit
	for i := 0; i < 30; i++ {
		if !s.isProcessRunning(pid) {
			break
		}
		time.Sleep(100 * time.Millisecond)
	}

	// Final force kill if still running
	if s.isProcessRunning(pid) {
		syscall.Kill(pid, syscall.SIGKILL)
	}

	delete(s.pids, req.VMID)
	delete(s.processes, req.VMID)

	return &agentapi.VMStopResponse{Stopped: true}, nil
}

// DestroyVM stops a VM and cleans up its resources
func (s *VMManagementService) DestroyVM(ctx context.Context, req *agentapi.VMDestroyRequest) (*agentapi.VMDestroyResponse, error) {
	// 1. Stop the VM if it's running
	pid, exists := s.pids[req.VMID]
	if !exists {
		// Check system processes just in case
		socketPath := filepath.Join(req.WorkspacePath, "api.sock")
		if p, running := s.isProcessRunningBySocket(socketPath); running {
			pid = p
			exists = true
		}
	}

	if exists && s.isProcessRunning(pid) {
		// Hard stop
		s.StopVM(ctx, &agentapi.VMStopRequest{
			VMID:  req.VMID,
			PID:   pid,
			Force: true,
		})
	}

	// 2. Clean up TAP device
	tapDev := GenerateTAPName(req.VMID)
	if err := s.tapService.DeleteTAP(tapDev); err != nil {
		// Log error but continue
		fmt.Fprintf(os.Stderr, "Warning: failed to delete TAP device %s: %v\n", tapDev, err)
	}

	// 3. Clean up Firewall rules
	if s.fwService != nil {
		if err := s.fwService.RemoveVMRules(req.VMID); err != nil {
			fmt.Fprintf(os.Stderr, "Warning: failed to remove firewall rules: %v\n", err)
		}
	}

	// 3. Clean up Cloud-init seed ISO
	seedISO := filepath.Join(req.WorkspacePath, "seed.iso")
	if _, err := os.Stat(seedISO); err == nil {
		if err := os.Remove(seedISO); err != nil {
			fmt.Fprintf(os.Stderr, "Warning: failed to remove seed ISO %s: %v\n", seedISO, err)
		}
	}

	// 4. Clean up serial ptty file if exists
	serialPtty := filepath.Join(req.WorkspacePath, "serial.ptty")
	if _, err := os.Stat(serialPtty); err == nil {
		os.Remove(serialPtty)
	}

	delete(s.pids, req.VMID)
	delete(s.processes, req.VMID)

	return &agentapi.VMDestroyResponse{Destroyed: true}, nil
}

// GetVMStatus returns the status of a VM
func (s *VMManagementService) GetVMStatus(ctx context.Context, req *agentapi.VMStatusRequest) (*agentapi.VMStatusResponse, error) {
	pid, exists := s.pids[req.VMID]
	if !exists {
		return &agentapi.VMStatusResponse{Running: false}, nil
	}

	running := s.isProcessRunning(pid)
	if !running {
		delete(s.pids, req.VMID)
		delete(s.processes, req.VMID)
	}

	return &agentapi.VMStatusResponse{
		Running: running,
		PID:     pid,
	}, nil
}

// isProcessRunning checks if a process with the given PID is running
func (s *VMManagementService) isProcessRunning(pid int) bool {
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

// isProcessRunningBySocket checks if any cloud-hypervisor process is using the given socket
func (s *VMManagementService) isProcessRunningBySocket(socketPath string) (int, bool) {
	// Normalize the socket path for comparison
	absSocketPath, err := filepath.Abs(socketPath)
	if err != nil {
		absSocketPath = socketPath
	}

	// Look through /proc for cloud-hypervisor processes with matching --api-socket
	matches, err := filepath.Glob("/proc/[0-9]*/cmdline")
	if err != nil {
		return 0, false
	}

	for _, match := range matches {
		data, err := os.ReadFile(match)
		if err != nil {
			continue
		}

		// cmdline is null-separated - convert to args slice for proper parsing
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

		// Look for --api-socket argument
		for i, arg := range args {
			if arg == "--api-socket" && i+1 < len(args) {
				// Found socket path argument, compare it
				procSocketPath := args[i+1]
				procAbsPath, err := filepath.Abs(procSocketPath)
				if err != nil {
					procAbsPath = procSocketPath
				}

				// Also check raw string containment for robustness
				if procAbsPath == absSocketPath || strings.Contains(procAbsPath, absSocketPath) || strings.Contains(absSocketPath, procAbsPath) {
					// Found it. Extract PID from path /proc/<PID>/cmdline
					parts := strings.Split(match, "/")
					if len(parts) >= 3 {
						pid, _ := strconv.Atoi(parts[2])
						if s.isProcessRunning(pid) {
							return pid, true
						}
					}
				}
			}
		}
	}
	return 0, false
}

// ScanAndRecoverOrphans scans for existing cloud-hypervisor processes and rebuilds internal state
// This should be called on agent startup to detect VMs that were running before a restart
func (s *VMManagementService) ScanAndRecoverOrphans(dataRoot string) ([]string, error) {
	var recovered []string

	// Look through /proc for all cloud-hypervisor processes
	matches, err := filepath.Glob("/proc/[0-9]*/cmdline")
	if err != nil {
		return nil, fmt.Errorf("failed to glob /proc: %w", err)
	}

	for _, match := range matches {
		data, err := os.ReadFile(match)
		if err != nil {
			continue
		}

		// cmdline is null-separated - convert to args slice
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

		// Extract PID
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

		// Look for --api-socket argument to identify VM
		var socketPath string
		for i, arg := range args {
			if arg == "--api-socket" && i+1 < len(args) {
				socketPath = args[i+1]
				break
			}
		}

		if socketPath == "" {
			// No socket path found, can't identify this VM
			continue
		}

		// Try to extract VM ID from socket path
		// Expected: /path/to/data/root/vms/<vmid>/api.sock
		vmID := s.extractVMIDFromSocketPath(socketPath, dataRoot)
		if vmID == "" {
			// Socket path doesn't match our data root, skip
			continue
		}

		// Check if we already have this VM tracked
		if existingPID, exists := s.pids[vmID]; exists {
			if existingPID == pid {
				// Already tracked correctly
				recovered = append(recovered, vmID)
				continue
			}
			// Different PID tracked, update it
			fmt.Fprintf(os.Stderr, "Orphan recovery: Updating PID for VM %s from %d to %d\n", vmID, existingPID, pid)
		}

		// Store the PID - we can't recreate the exec.Cmd but we can track the PID
		s.pids[vmID] = pid
		// Note: s.processes[vmID] will remain nil for orphans, which is handled gracefully

		fmt.Fprintf(os.Stderr, "Orphan recovery: Recovered VM %s with PID %d (socket: %s)\n", vmID, pid, socketPath)
		recovered = append(recovered, vmID)
	}

	return recovered, nil
}

// extractVMIDFromSocketPath extracts the VM ID from a socket path
// Expected format: /data/root/vms/<vmid>/api.sock or similar
func (s *VMManagementService) extractVMIDFromSocketPath(socketPath, dataRoot string) string {
	// Normalize paths
	absSocket, err := filepath.Abs(socketPath)
	if err != nil {
		absSocket = socketPath
	}
	absDataRoot, err := filepath.Abs(dataRoot)
	if err != nil {
		absDataRoot = dataRoot
	}

	// Check if socket is within our data root
	if !strings.HasPrefix(absSocket, absDataRoot) {
		return ""
	}

	// Extract the relative path from data root
	relPath, err := filepath.Rel(absDataRoot, absSocket)
	if err != nil {
		return ""
	}

	// Parse: vms/<vmid>/api.sock or just <vmid>/api.sock
	parts := strings.Split(relPath, string(filepath.Separator))

	// Look for api.sock and extract VM ID from parent directory
	for i, part := range parts {
		if part == "api.sock" && i > 0 {
			return parts[i-1]
		}
	}

	// Alternative: look for any path component that could be a VM ID
	// (alphanumeric with dashes, typical UUID or name format)
	for _, part := range parts {
		if part != "" && part != "vms" && part != "workspace" {
			// Validate it looks like a VM ID (not a generic directory)
			if isValidVMID(part) {
				return part
			}
		}
	}

	return ""
}

// isValidVMID checks if a string looks like a valid VM ID
func isValidVMID(s string) bool {
	if s == "" || len(s) < 3 {
		return false
	}
	// Allow alphanumeric, dashes, underscores
	for _, r := range s {
		if !((r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || (r >= '0' && r <= '9') || r == '-' || r == '_') {
			return false
		}
	}
	return true
}

// waitForProcess waits for a process to exit and cleans up
func (s *VMManagementService) waitForProcess(vmID string, cmd *exec.Cmd, stdout, stderr *os.File) {
	err := cmd.Wait()
	if err != nil {
		fmt.Fprintf(os.Stderr, "VM %s exited with error: %v\n", vmID, err)
	} else {
		fmt.Fprintf(os.Stderr, "VM %s exited normally\n", vmID)
	}
	
	// Close log files
	if stdout != nil {
		stdout.Close()
	}
	if stderr != nil {
		stderr.Close()
	}
	
	delete(s.pids, vmID)
	delete(s.processes, vmID)
}

// capturePtyPath reads the stdout log to find and save the PTY path
// Note: does NOT close stdoutFileHandle - it's also cmd.Stdout and will be
// closed by waitForProcess when the VM exits
func (s *VMManagementService) capturePtyPath(vmID, workspacePath, stdoutFile string, stdoutFileHandle *os.File) {
	// Wait for CH to output the PTY path (usually happens within 1-2 seconds)
	time.Sleep(2 * time.Second)

	// Sync the file to ensure all data is written to disk before reading
	if stdoutFileHandle != nil {
		stdoutFileHandle.Sync()
	}

	// Read the stdout log
	data, err := os.ReadFile(stdoutFile)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Warning: could not read stdout log for PTY: %v\n", err)
		return
	}

	// Parse PTY path from output
	// Cloud Hypervisor outputs: "PTY path: /dev/pts/X" when started with --serial tty
	ptyPattern := regexp.MustCompile(`PTY path:\s*(/dev/pts/\d+)`)
	matches := ptyPattern.FindSubmatch(data)

	if matches != nil && len(matches) > 1 {
		ptyPath := string(matches[1])
		ptyFile := filepath.Join(workspacePath, "serial.ptty")
		if err := os.WriteFile(ptyFile, []byte(ptyPath), 0644); err != nil {
			fmt.Fprintf(os.Stderr, "Warning: could not write PTY path to file: %v\n", err)
		} else {
			fmt.Fprintf(os.Stderr, "Captured PTY path for VM %s: %s\n", vmID, ptyPath)
		}
	} else {
		fmt.Fprintf(os.Stderr, "Warning: PTY path not found in stdout for VM %s\n", vmID)
	}
}

// findKernel searches for a vmlinux kernel
func (s *VMManagementService) findKernel() (string, error) {
	candidates := []string{
		"/usr/share/cloud-hypervisor/vmlinux",
		"/var/lib/chv/vmlinux",
		"/boot/vmlinux",
		"/usr/lib/cloud-hypervisor/vmlinux",
	}

	for _, path := range candidates {
		if _, err := os.Stat(path); err == nil {
			return path, nil
		}
	}

	return "", fmt.Errorf("no kernel found in standard locations")
}



// ListRunningVMs returns a list of running VM IDs
func (s *VMManagementService) ListRunningVMs() []string {
	var running []string
	for vmID, pid := range s.pids {
		if s.isProcessRunning(pid) {
			running = append(running, vmID)
		} else {
			delete(s.pids, vmID)
			delete(s.processes, vmID)
		}
	}
	return running
}

// GetVMPID returns the PID of a running VM
func (s *VMManagementService) GetVMPID(vmID string) (int, bool) {
	pid, exists := s.pids[vmID]
	if !exists {
		return 0, false
	}
	if !s.isProcessRunning(pid) {
		delete(s.pids, vmID)
		delete(s.processes, vmID)
		return 0, false
	}
	return pid, true
}

// CreateSnapshot creates an internal qcow2 snapshot
func (s *VMManagementService) CreateSnapshot(ctx context.Context, req *agentapi.VMSnapshotCreateRequest) (*agentapi.VMSnapshotActionResponse, error) {
	// 1. Enforce non-live (VM must be stopped)
	if _, running := s.GetVMPID(req.VMID); running {
		return nil, fmt.Errorf("VM must be stopped before creating a snapshot")
	}

	// 2. Execute qemu-img snapshot -c
	cmd := exec.CommandContext(ctx, "qemu-img", "snapshot", "-c", req.Name, req.DiskPath)
	if out, err := cmd.CombinedOutput(); err != nil {
		return nil, fmt.Errorf("failed to create snapshot: %w (output: %s)", err, out)
	}

	return &agentapi.VMSnapshotActionResponse{Success: true}, nil
}

// ListSnapshots returns a list of internal snapshots from the qcow2 header
func (s *VMManagementService) ListSnapshots(ctx context.Context, req *agentapi.VMSnapshotListRequest) ([]agentapi.VMSnapshotInfo, error) {
	cmd := exec.CommandContext(ctx, "qemu-img", "snapshot", "-l", req.DiskPath)
	out, err := cmd.Output()
	if err != nil {
		// qemu-img snapshot -l returns error if no snapshots exist occasionally or if file is missing
		return []agentapi.VMSnapshotInfo{}, nil
	}

	lines := strings.Split(string(out), "\n")
	var snapshots []agentapi.VMSnapshotInfo
	
	// Skip header lines
	// ID        TAG               VM SIZE                DATE       VM CLOCK
	for _, line := range lines {
		fields := strings.Fields(line)
		if len(fields) >= 6 && fields[0] != "ID" && fields[0] != "Snapshot" {
			snapshots = append(snapshots, agentapi.VMSnapshotInfo{
				ID:   fields[0],
				Tag:  fields[1],
				Name: fields[1], // We use Tag as Name
				Date: fields[4] + " " + fields[5],
				VMID: req.VMID,
			})
		}
	}

	return snapshots, nil
}

// RestoreSnapshot reverts a disk to a snapshot
func (s *VMManagementService) RestoreSnapshot(ctx context.Context, req *agentapi.VMSnapshotRestoreRequest) (*agentapi.VMSnapshotActionResponse, error) {
	// 1. Enforce non-live (VM must be stopped)
	if _, running := s.GetVMPID(req.VMID); running {
		return nil, fmt.Errorf("VM must be stopped before restoring a snapshot")
	}

	// 2. Execute qemu-img snapshot -a
	cmd := exec.CommandContext(ctx, "qemu-img", "snapshot", "-a", req.Name, req.DiskPath)
	if out, err := cmd.CombinedOutput(); err != nil {
		return nil, fmt.Errorf("failed to restore snapshot: %w (output: %s)", err, out)
	}

	return &agentapi.VMSnapshotActionResponse{Success: true}, nil
}

// DeleteSnapshot removes an internal snapshot
func (s *VMManagementService) DeleteSnapshot(ctx context.Context, req *agentapi.VMSnapshotDeleteRequest) (*agentapi.VMSnapshotActionResponse, error) {
	// Execute qemu-img snapshot -d
	cmd := exec.CommandContext(ctx, "qemu-img", "snapshot", "-d", req.Name, req.DiskPath)
	if out, err := cmd.CombinedOutput(); err != nil {
		return nil, fmt.Errorf("failed to delete snapshot: %w (output: %s)", err, out)
	}

	return &agentapi.VMSnapshotActionResponse{Success: true}, nil
}

// Helper to parse int
func parseInt(s string) int {
	i, _ := strconv.Atoi(s)
	return i
}

// parsePtyPath reads from stdout and stderr pipes to find the PTY path
// Cloud Hypervisor outputs: "PTY path: /dev/pts/X" when started with --serial tty
func (s *VMManagementService) parsePtyPath(stdout, stderr io.Reader, workspacePath string) (string, error) {
	ptyPattern := regexp.MustCompile(`PTY path:\s*(/dev/pts/\d+)`)
	resultChan := make(chan string, 1)
	var wg sync.WaitGroup
	wg.Add(2)

	// Read from stdout
	go func() {
		defer wg.Done()
		scanner := bufio.NewScanner(stdout)
		for scanner.Scan() {
			line := scanner.Text()
			// Also write to stdout for logging
			fmt.Println(line)
			if matches := ptyPattern.FindStringSubmatch(line); matches != nil {
				select {
				case resultChan <- matches[1]:
				default:
				}
			}
		}
	}()

	// Read from stderr
	go func() {
		defer wg.Done()
		scanner := bufio.NewScanner(stderr)
		for scanner.Scan() {
			line := scanner.Text()
			// Also write to stderr for logging
			fmt.Fprintln(os.Stderr, line)
			if matches := ptyPattern.FindStringSubmatch(line); matches != nil {
				select {
				case resultChan <- matches[1]:
				default:
				}
			}
		}
	}()

	// Close resultChan when both goroutines finish
	go func() {
		wg.Wait()
		close(resultChan)
	}()

	// Wait for result with timeout
	select {
	case ptyPath := <-resultChan:
		if ptyPath != "" {
			return ptyPath, nil
		}
		// Channel closed without finding PTY
		return "", fmt.Errorf("PTY path not found in output")
	case <-time.After(5 * time.Second):
		return "", fmt.Errorf("timeout waiting for PTY path")
	}
}

// ProvisionVM handles the creation of a VM workspace, disk cloning, and cloud-init generation
func (s *VMManagementService) ProvisionVM(ctx context.Context, req *agentapi.VMProvisionRequest) error {
	// 1. Create workspace
	if err := os.MkdirAll(req.WorkspacePath, 0755); err != nil {
		return fmt.Errorf("failed to create workspace: %w", err)
	}

	// 2. Render cloud-init if requested
	if s.cloudInit != nil && s.seedService != nil {
		ciCfg := cloudinit.Config{
			VMID:              req.VMID,
			VMName:            req.VMName,
			Hostname:          req.VMName, // Use VM name as hostname
			Username:          req.Username,
			Password:          req.Password,
			SSHAuthorizedKeys: req.SSHAuthorizedKeys,
			UserData:          req.UserData,
		}

		renderResult, err := s.cloudInit.Render(ctx, req.VMID, ciCfg)
		if err != nil {
			return fmt.Errorf("failed to render cloud-init: %w", err)
		}

		// 3. Generate seed ISO
		_, err = s.seedService.Generate(ctx, GenerateRequest{
			VMID:         req.VMID,
			CloudinitDir: renderResult.CloudinitDir,
			OutputDir:    req.WorkspacePath,
		})
		if err != nil {
			return fmt.Errorf("failed to generate seed ISO: %w", err)
		}
	}

	// 4. Clone disk from image if applicable
	if req.ImagePath != "" && req.DiskPath != "" {
		if err := s.cloneDisk(ctx, req.ImagePath, req.DiskPath); err != nil {
			return fmt.Errorf("failed to clone disk: %w", err)
		}
	}

	return nil
}

func (s *VMManagementService) cloneDisk(ctx context.Context, src, dst string) error {
	// Ensure destination directory exists
	if err := os.MkdirAll(filepath.Dir(dst), 0755); err != nil {
		return err
	}

	// Use qemu-img to clone (and convert if source is raw/other)
	cmd := exec.CommandContext(ctx, "qemu-img", "convert", "-f", "qcow2", "-O", "qcow2", src, dst)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("qemu-img failed: %w (output: %s)", err, output)
	}

	return nil
}


// ShutdownVM gracefully shuts down a VM via the Cloud Hypervisor API (ACPI shutdown)
func (s *VMManagementService) ShutdownVM(ctx context.Context, req *agentapi.VMShutdownRequest) (*agentapi.VMShutdownResponse, error) {
	// Get API socket path
	apiSocket := filepath.Join(s.getWorkspacePath(req.VMID), "api.sock")
	
	// Verify the API socket exists
	if _, err := os.Stat(apiSocket); err != nil {
		return nil, fmt.Errorf("VM API socket not found at %s: %w", apiSocket, err)
	}

	// Create CH API client
	client := NewCHAPIClient(apiSocket)
	
	// Try graceful shutdown first
	if err := client.Shutdown(ctx); err != nil {
		// Fallback to power button if shutdown endpoint not available
		if err := client.PowerButton(ctx); err != nil {
			return nil, fmt.Errorf("failed to initiate graceful shutdown: %w", err)
		}
	}

	// Wait for VM to stop if timeout is specified
	if req.Timeout > 0 {
		timeoutCtx, cancel := context.WithTimeout(ctx, time.Duration(req.Timeout)*time.Second)
		defer cancel()
		
		for {
			select {
			case <-timeoutCtx.Done():
				return &agentapi.VMShutdownResponse{
					Success: false,
					Message: "shutdown timeout reached, VM may still be stopping",
				}, nil
			default:
				// Check if process is still running
				if pid, exists := s.pids[req.VMID]; exists {
					if !s.isProcessRunning(pid) {
						delete(s.pids, req.VMID)
						delete(s.processes, req.VMID)
						return &agentapi.VMShutdownResponse{
							Success: true,
							Message: "VM shutdown completed",
						}, nil
					}
				} else {
					// Not in our tracked processes
					return &agentapi.VMShutdownResponse{
						Success: true,
						Message: "VM not tracked locally",
					}, nil
				}
				time.Sleep(500 * time.Millisecond)
			}
		}
	}

	return &agentapi.VMShutdownResponse{
		Success: true,
		Message: "shutdown signal sent to VM",
	}, nil
}

// RebootVM reboots a running VM via the Cloud Hypervisor API
func (s *VMManagementService) RebootVM(ctx context.Context, req *agentapi.VMResetRequest) (*agentapi.VMResetResponse, error) {
	// Get API socket path
	apiSocket := filepath.Join(s.getWorkspacePath(req.VMID), "api.sock")
	
	// Verify the API socket exists
	if _, err := os.Stat(apiSocket); err != nil {
		return nil, fmt.Errorf("VM API socket not found at %s: %w", apiSocket, err)
	}

	// Verify VM is running
	if pid, exists := s.pids[req.VMID]; exists {
		if !s.isProcessRunning(pid) {
			return nil, fmt.Errorf("VM %s is not running", req.VMID)
		}
	}

	// Create CH API client and send reboot request
	client := NewCHAPIClient(apiSocket)
	if err := client.Reboot(ctx); err != nil {
		return nil, fmt.Errorf("failed to reboot VM: %w", err)
	}

	return &agentapi.VMResetResponse{
		Success: true,
		Message: "reboot signal sent to VM",
	}, nil
}

// PauseVM pauses a running VM
func (s *VMManagementService) PauseVM(ctx context.Context, vmID string) error {
	// Get API socket path
	apiSocket := filepath.Join(s.getWorkspacePath(vmID), "api.sock")
	
	// Verify the API socket exists
	if _, err := os.Stat(apiSocket); err != nil {
		return fmt.Errorf("VM API socket not found at %s: %w", apiSocket, err)
	}

	// Verify VM is running
	if pid, exists := s.pids[vmID]; exists {
		if !s.isProcessRunning(pid) {
			return fmt.Errorf("VM %s is not running", vmID)
		}
	}

	// Create CH API client and send pause request
	client := NewCHAPIClient(apiSocket)
	if err := client.Pause(ctx); err != nil {
		return fmt.Errorf("failed to pause VM: %w", err)
	}

	return nil
}

// ResumeVM resumes a paused VM
func (s *VMManagementService) ResumeVM(ctx context.Context, vmID string) error {
	// Get API socket path
	apiSocket := filepath.Join(s.getWorkspacePath(vmID), "api.sock")
	
	// Verify the API socket exists
	if _, err := os.Stat(apiSocket); err != nil {
		return fmt.Errorf("VM API socket not found at %s: %w", apiSocket, err)
	}

	// Verify VM is running
	if pid, exists := s.pids[vmID]; exists {
		if !s.isProcessRunning(pid) {
			return fmt.Errorf("VM %s is not running", vmID)
		}
	}

	// Create CH API client and send resume request
	client := NewCHAPIClient(apiSocket)
	if err := client.Resume(ctx); err != nil {
		return fmt.Errorf("failed to resume VM: %w", err)
	}

	return nil
}

// ResizeVM resizes a running VM (CPU/memory hot-plug)
func (s *VMManagementService) ResizeVM(ctx context.Context, req *agentapi.VMResizeRequest) (*agentapi.VMResizeResponse, error) {
	// Get API socket path
	apiSocket := filepath.Join(s.getWorkspacePath(req.VMID), "api.sock")
	
	// Verify the API socket exists
	if _, err := os.Stat(apiSocket); err != nil {
		return nil, fmt.Errorf("VM API socket not found at %s: %w", apiSocket, err)
	}

	// Verify VM is running
	if pid, exists := s.pids[req.VMID]; exists {
		if !s.isProcessRunning(pid) {
			return nil, fmt.Errorf("VM %s is not running", req.VMID)
		}
	}

	// Create CH API client
	client := NewCHAPIClient(apiSocket)
	
	// Get current VM info to validate resize parameters
	info, err := client.GetInfo(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to get VM info: %w", err)
	}

	// Validate resize request
	if req.VCPUs > 0 && req.VCPUs < info.Config.Cpus.BootVcpus {
		return nil, fmt.Errorf("cannot decrease vCPUs below current count (%d)", info.Config.Cpus.BootVcpus)
	}
	if req.MemoryMB > 0 && req.MemoryMB < info.Config.Memory.Size/(1024*1024) {
		return nil, fmt.Errorf("cannot decrease memory below current size (%d MB)", info.Config.Memory.Size/(1024*1024))
	}

	// Calculate desired resources
	desiredVcpus := req.VCPUs
	if desiredVcpus == 0 {
		desiredVcpus = info.Config.Cpus.BootVcpus
	}
	desiredRam := req.MemoryMB * 1024 * 1024 // Convert MB to bytes
	if desiredRam == 0 {
		desiredRam = info.Config.Memory.Size
	}

	// Send resize request
	resizeResp, err := client.Resize(ctx, desiredVcpus, desiredRam)
	if err != nil {
		return nil, fmt.Errorf("failed to resize VM: %w", err)
	}

	return &agentapi.VMResizeResponse{
		VCPUs:    resizeResp.Vcpus,
		MemoryMB: resizeResp.Ram / (1024 * 1024),
	}, nil
}

// GetVMState returns the current state of a VM
func (s *VMManagementService) GetVMState(ctx context.Context, vmID string) (string, error) {
	// Get API socket path
	apiSocket := filepath.Join(s.getWorkspacePath(vmID), "api.sock")
	
	// Verify the API socket exists
	if _, err := os.Stat(apiSocket); err != nil {
		return "", fmt.Errorf("VM API socket not found at %s: %w", apiSocket, err)
	}

	// Verify VM is running
	if pid, exists := s.pids[vmID]; exists {
		if !s.isProcessRunning(pid) {
			return "ShutDown", nil
		}
	}

	// Create CH API client and get state
	client := NewCHAPIClient(apiSocket)
	info, err := client.GetInfo(ctx)
	if err != nil {
		return "", fmt.Errorf("failed to get VM state: %w", err)
	}

	return info.State, nil
}

// getWorkspacePath returns the expected workspace path for a VM
func (s *VMManagementService) getWorkspacePath(vmID string) string {
	// First try to get from processes map (if we started it)
	if cmd, exists := s.processes[vmID]; exists && cmd != nil {
		// Extract from args - look for --api-socket
		for i, arg := range cmd.Args {
			if arg == "--api-socket" && i+1 < len(cmd.Args) {
				return filepath.Dir(cmd.Args[i+1])
			}
		}
	}

	// Default to standard location
	return filepath.Join("/var/lib/chv/vms", vmID)
}

// ForceStopVM immediately terminates a VM process
func (s *VMManagementService) ForceStopVM(ctx context.Context, req *agentapi.VMForceStopRequest) (*agentapi.VMForceStopResponse, error) {
	pid, exists := s.pids[req.VMID]
	if !exists {
		// Try to find by system scan
		socketPath := filepath.Join(s.getWorkspacePath(req.VMID), "api.sock")
		if p, running := s.isProcessRunningBySocket(socketPath); running {
			pid = p
			exists = true
		} else if req.PID != 0 {
			pid = req.PID
			exists = true
		} else {
			return nil, fmt.Errorf("VM %s is not running", req.VMID)
		}
	}

	// Verify process exists
	if !s.isProcessRunning(pid) {
		delete(s.pids, req.VMID)
		delete(s.processes, req.VMID)
		return &agentapi.VMForceStopResponse{
			Success: true,
			Message: "VM was not running",
		}, nil
	}

	// Send SIGKILL
	if err := syscall.Kill(pid, syscall.SIGKILL); err != nil {
		return nil, fmt.Errorf("failed to kill VM process: %w", err)
	}

	// Wait briefly for process to die
	time.Sleep(200 * time.Millisecond)
	
	// Verify it's dead
	if s.isProcessRunning(pid) {
		return nil, fmt.Errorf("VM process did not terminate after SIGKILL")
	}

	delete(s.pids, req.VMID)
	delete(s.processes, req.VMID)

	return &agentapi.VMForceStopResponse{
		Success: true,
		Message: "VM terminated immediately",
	}, nil
}
