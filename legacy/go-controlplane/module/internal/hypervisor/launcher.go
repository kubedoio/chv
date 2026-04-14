package hypervisor

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

// VMParams holds VM launch parameters
type VMParams struct {
	ID        string
	Name      string
	VCPU      int
	MemoryMB  int
	DiskPath  string
	SeedISO   string
	TapDevice string
	MacAddr   string
	IPAddr    string
	Netmask   string
	Workspace string
}

// Launcher builds Cloud Hypervisor commands
type Launcher struct {
	config *Config
}

func NewLauncher(config *Config) *Launcher {
	if config == nil {
		config = DefaultConfig()
	}
	return &Launcher{config: config}
}

// BuildCommand constructs the cloud-hypervisor command
func (l *Launcher) BuildCommand(params VMParams) (*exec.Cmd, error) {
	// Find kernel
	kernelPath := l.findKernel()
	if kernelPath == "" {
		return nil, fmt.Errorf("kernel not found (tried: %s, %s)", l.config.KernelPath, l.config.VMLinuxPath)
	}

	args := []string{
		"--kernel", kernelPath,
	}

	// Disks
	args = append(args, "--disk", fmt.Sprintf("path=%s", params.DiskPath))
	if params.SeedISO != "" {
		args = append(args, "--disk", fmt.Sprintf("path=%s,readonly=on", params.SeedISO))
	}

	// Network
	if params.TapDevice != "" {
		netConfig := fmt.Sprintf("tap=%s", params.TapDevice)
		if params.MacAddr != "" {
			netConfig += fmt.Sprintf(",mac=%s", params.MacAddr)
		}
		if params.IPAddr != "" {
			netConfig += fmt.Sprintf(",ip=%s", params.IPAddr)
		}
		if params.Netmask != "" {
			netConfig += fmt.Sprintf(",mask=%s", params.Netmask)
		}
		args = append(args, "--net", netConfig)
	}

	// CPU
	args = append(args, "--cpus", fmt.Sprintf("boot=%d", params.VCPU))

	// Memory
	args = append(args, "--memory", fmt.Sprintf("size=%dM", params.MemoryMB))

	// API socket for management
	if params.Workspace != "" {
		apiSocket := filepath.Join(params.Workspace, "ch-api.sock")
		args = append(args, "--api-socket", apiSocket)

		// Log file
		logFile := filepath.Join(params.Workspace, "chv.log")
		args = append(args, "--log-file", logFile)
	}

	cmd := exec.Command(l.config.BinaryPath, args...)
	return cmd, nil
}

// BuildCommandString returns the command as a string (for logging/debugging)
func (l *Launcher) BuildCommandString(params VMParams) (string, error) {
	cmd, err := l.BuildCommand(params)
	if err != nil {
		return "", err
	}

	return cmd.String(), nil
}

func (l *Launcher) findKernel() string {
	// Try configured paths
	paths := []string{
		l.config.KernelPath,
		l.config.VMLinuxPath,
		"/usr/share/cloud-hypervisor/vmlinux",
		"/boot/vmlinux",
		"/var/lib/chv/vmlinux",
	}

	for _, path := range paths {
		if fileExists(path) {
			return path
		}
	}

	return ""
}

func fileExists(path string) bool {
	_, err := exec.LookPath(path)
	if err == nil {
		return true
	}
	// Try as regular file
	info, err := os.Stat(path)
	return err == nil && !info.IsDir()
}
