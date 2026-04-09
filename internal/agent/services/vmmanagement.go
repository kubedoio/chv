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
	args := []string{
		"--kernel", kernelPath,
		"--disk", fmt.Sprintf("path=%s", req.DiskPath),
	}

	if req.SeedISOPath != "" {
		args = append(args, "--disk", fmt.Sprintf("path=%s,readonly=on", req.SeedISOPath))
	}

	// Build network config (only include IP/mask if IP is provided)
	netConfig := fmt.Sprintf("tap=%s", tapDev)
	if req.MACAddress != "" {
		netConfig += fmt.Sprintf(",mac=%s", req.MACAddress)
	}
	if req.IPAddress != "" && req.Netmask != "" {
		netConfig += fmt.Sprintf(",ip=%s,mask=%s", req.IPAddress, req.Netmask)
	}
	args = append(args, "--net", netConfig)
	
	args = append(args,
		"--cpus", fmt.Sprintf("boot=%d", req.VCPU),
		"--memory", fmt.Sprintf("size=%dM", req.MemoryMB),
		"--api-socket", filepath.Join(req.WorkspacePath, "api.sock"),
		"--console", "off",
		"--serial", "tty",
	)

	// Create command
	cmd := exec.CommandContext(ctx, chPath, args...)
	cmd.SysProcAttr = &syscall.SysProcAttr{
		Setsid: true, // Create new session so CH survives parent
	}

	// Capture stdout/stderr to parse PTY path
	stdoutPipe, err := cmd.StdoutPipe()
	if err != nil {
		return nil, fmt.Errorf("failed to create stdout pipe: %w", err)
	}
	stderrPipe, err := cmd.StderrPipe()
	if err != nil {
		return nil, fmt.Errorf("failed to create stderr pipe: %w", err)
	}

	// Start the process
	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("failed to start cloud-hypervisor: %w", err)
	}

	pid := cmd.Process.Pid

	// Store process info
	s.processes[req.VMID] = cmd
	s.pids[req.VMID] = pid

	// Parse PTY path from stdout/stderr
	ptyPath, err := s.parsePtyPath(stdoutPipe, stderrPipe, req.WorkspacePath)
	if err != nil {
		// Log but don't fail - console may still work via API
		fmt.Fprintf(os.Stderr, "Warning: could not capture PTY path: %v\n", err)
	} else if ptyPath != "" {
		// Store PTY path for console access
		ptyFile := filepath.Join(req.WorkspacePath, "serial.ptty")
		if writeErr := os.WriteFile(ptyFile, []byte(ptyPath), 0644); writeErr != nil {
			fmt.Fprintf(os.Stderr, "Warning: could not write PTY path to file: %v\n", writeErr)
		}
	}

	// Wait a moment to ensure process didn't immediately exit
	time.Sleep(500 * time.Millisecond)

	if !s.isProcessRunning(pid) {
		// Process exited quickly, likely an error
		delete(s.processes, req.VMID)
		delete(s.pids, req.VMID)
		return nil, fmt.Errorf("cloud-hypervisor process exited immediately")
	}

	// Start background waiter to clean up when process exits
	go s.waitForProcess(req.VMID, cmd)

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
		
		// cmdline is null-separated
		content := string(data)
		if !strings.Contains(content, "cloud-hypervisor") {
			continue
		}

		if strings.Contains(content, socketPath) {
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
	return 0, false
}

// waitForProcess waits for a process to exit and cleans up
func (s *VMManagementService) waitForProcess(vmID string, cmd *exec.Cmd) {
	cmd.Wait()
	delete(s.pids, vmID)
	delete(s.processes, vmID)
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
			Hostname:          req.VMID, // Use ID as default hostname
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
