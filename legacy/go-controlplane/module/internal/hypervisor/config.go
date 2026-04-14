package hypervisor

// Config holds Cloud Hypervisor configuration
type Config struct {
	BinaryPath  string // Path to cloud-hypervisor binary
	KernelPath  string // Path to vmlinux kernel
	VMLinuxPath string // Alternative kernel path
}

func DefaultConfig() *Config {
	return &Config{
		BinaryPath:  "/usr/bin/cloud-hypervisor",
		KernelPath:  "/usr/share/cloud-hypervisor/vmlinux",
		VMLinuxPath: "/var/lib/chv/vmlinux",
	}
}
