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
	"sync"
	"syscall"
	"time"

	"github.com/chv/chv/internal/agentapi"
)

// VMManagementService handles VM lifecycle via Cloud Hypervisor
type VMManagementService struct {
	processes   map[string]*exec.Cmd // vmID -> cmd
	pids        map[string]int       // vmID -> pid
	tapService  *TAPDeviceService
}

// NewVMManagementService creates a new VM management service
func NewVMManagementService() *VMManagementService {
	return &VMManagementService{
		processes:  make(map[string]*exec.Cmd),
		pids:       make(map[string]int),
		tapService: NewTAPDeviceService("chvbr0"),
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
	// Check if already running
	if pid, exists := s.pids[req.VMID]; exists {
		if s.isProcessRunning(pid) {
			return nil, fmt.Errorf("VM %s is already running with PID %d", req.VMID, pid)
		}
		// Clean up stale entry
		delete(s.pids, req.VMID)
		delete(s.processes, req.VMID)
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
		return nil, fmt.Errorf("VM %s is not running", req.VMID)
	}

	// Use the stored PID or the one from request
	if req.PID != 0 && req.PID != pid {
		pid = req.PID
	}

	// Check if process is running
	if !s.isProcessRunning(pid) {
		delete(s.pids, req.VMID)
		delete(s.processes, req.VMID)
		// Clean up TAP device
		tapDev := GenerateTAPName(req.VMID)
		s.tapService.DeleteTAP(tapDev)
		return &agentapi.VMStopResponse{Stopped: true}, nil
	}

	// Try graceful shutdown via CH API socket first (future enhancement)
	// For now, send SIGTERM
	if err := syscall.Kill(pid, syscall.SIGTERM); err != nil {
		// If SIGTERM fails, try SIGKILL
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

	// Force kill if still running
	if s.isProcessRunning(pid) {
		syscall.Kill(pid, syscall.SIGKILL)
	}

	delete(s.pids, req.VMID)
	delete(s.processes, req.VMID)

	// Clean up TAP device
	tapDev := GenerateTAPName(req.VMID)
	s.tapService.DeleteTAP(tapDev)

	return &agentapi.VMStopResponse{Stopped: true}, nil
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
	process, err := os.FindProcess(pid)
	if err != nil {
		return false
	}

	// On Unix, FindProcess always succeeds, so we need to send signal 0
	err = process.Signal(syscall.Signal(0))
	return err == nil
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
