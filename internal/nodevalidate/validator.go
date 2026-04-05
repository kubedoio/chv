package nodevalidate

import (
	"context"
	"os"
	"os/exec"
	"regexp"
	"runtime"
	"strconv"
	"strings"
)

// ValidationResult contains validation results.
type ValidationResult struct {
	OK                bool
	Errors            []ValidationError
	Capabilities      map[string]string
	TotalCPUCores     uint32
	TotalRAMMB        uint64
	HypervisorVersion string
}

// ValidationError represents a validation error.
type ValidationError struct {
	Code      string
	Message   string
	Retryable bool
	Hint      string
}

// Validator validates node requirements.
type Validator struct {
	cloudHypervisorPath string
}

// NewValidator creates a new validator.
func NewValidator(cloudHypervisorPath string) *Validator {
	return &Validator{
		cloudHypervisorPath: cloudHypervisorPath,
	}
}

// Validate performs all validations.
func (v *Validator) Validate(ctx context.Context) (*ValidationResult, error) {
	result := &ValidationResult{
		Capabilities: make(map[string]string),
	}
	
	// Check KVM
	if err := v.checkKVM(); err != nil {
		result.Errors = append(result.Errors, ValidationError{
			Code:      "KVM_NOT_AVAILABLE",
			Message:   err.Error(),
			Retryable: false,
			Hint:      "Ensure KVM is enabled in BIOS and kernel modules are loaded",
		})
	} else {
		result.Capabilities["kvm"] = "available"
	}
	
	// Check CPU
	cpuInfo, err := v.getCPUInfo()
	if err != nil {
		result.Errors = append(result.Errors, ValidationError{
			Code:      "CPU_INFO_FAILED",
			Message:   err.Error(),
			Retryable: true,
		})
	} else {
		result.TotalCPUCores = cpuInfo.Cores
		result.Capabilities["cpu_model"] = cpuInfo.Model
		result.Capabilities["cpu_arch"] = runtime.GOARCH
		
		if cpuInfo.VTx {
			result.Capabilities["vmx"] = "supported"
		}
	}
	
	// Check memory
	memInfo, err := v.getMemoryInfo()
	if err != nil {
		result.Errors = append(result.Errors, ValidationError{
			Code:      "MEMORY_INFO_FAILED",
			Message:   err.Error(),
			Retryable: true,
		})
	} else {
		result.TotalRAMMB = memInfo.TotalMB
	}
	
	// Check Cloud Hypervisor
	chvVersion, err := v.checkCloudHypervisor(ctx)
	if err != nil {
		result.Errors = append(result.Errors, ValidationError{
			Code:      "CLOUD_HV_NOT_FOUND",
			Message:   err.Error(),
			Retryable: false,
			Hint:      "Install Cloud Hypervisor",
		})
	} else {
		result.HypervisorVersion = chvVersion
		result.Capabilities["cloud_hypervisor"] = chvVersion
	}
	
	// Check kernel
	kernelVersion, err := v.getKernelVersion()
	if err == nil {
		result.Capabilities["kernel"] = kernelVersion
	}
	
	result.OK = len(result.Errors) == 0
	return result, nil
}

func (v *Validator) checkKVM() error {
	// Check if /dev/kvm exists and is accessible
	info, err := os.Stat("/dev/kvm")
	if err != nil {
		return err
	}
	
	// Check if it's a device
	mode := info.Mode()
	if mode&os.ModeDevice == 0 {
		return os.ErrInvalid
	}
	
	// Try to open it
	f, err := os.Open("/dev/kvm")
	if err != nil {
		return err
	}
	f.Close()
	
	return nil
}

type cpuInfo struct {
	Model string
	Cores uint32
	VTx   bool
}

func (v *Validator) getCPUInfo() (*cpuInfo, error) {
	data, err := os.ReadFile("/proc/cpuinfo")
	if err != nil {
		return nil, err
	}
	
	content := string(data)
	info := &cpuInfo{}
	
	// Count CPUs
	cores := strings.Count(content, "processor\t:")
	if cores == 0 {
		cores = 1
	}
	info.Cores = uint32(cores)
	
	// Get model name
	re := regexp.MustCompile(`model name\s*:\s*(.+)`)
	match := re.FindStringSubmatch(content)
	if len(match) > 1 {
		info.Model = strings.TrimSpace(match[1])
	}
	
	// Check for VMX support
	info.VTx = strings.Contains(content, "vmx")
	
	return info, nil
}

type memoryInfo struct {
	TotalMB uint64
}

func (v *Validator) getMemoryInfo() (*memoryInfo, error) {
	data, err := os.ReadFile("/proc/meminfo")
	if err != nil {
		return nil, err
	}
	
	content := string(data)
	info := &memoryInfo{}
	
	// Parse MemTotal
	re := regexp.MustCompile(`MemTotal:\s*(\d+)\s*kB`)
	match := re.FindStringSubmatch(content)
	if len(match) > 1 {
		kb, _ := strconv.ParseUint(match[1], 10, 64)
		info.TotalMB = kb / 1024
	}
	
	return info, nil
}

func (v *Validator) checkCloudHypervisor(ctx context.Context) (string, error) {
	cmd := exec.CommandContext(ctx, v.cloudHypervisorPath, "--version")
	out, err := cmd.Output()
	if err != nil {
		return "", err
	}
	
	return strings.TrimSpace(string(out)), nil
}

func (v *Validator) getKernelVersion() (string, error) {
	data, err := os.ReadFile("/proc/version")
	if err != nil {
		return "", err
	}
	
	parts := strings.Fields(string(data))
	if len(parts) >= 3 {
		return parts[2], nil
	}
	
	return "", os.ErrInvalid
}
