// Package integration provides integration tests for Cloud Hypervisor.
package integration

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"github.com/chv/chv/internal/agent/cloudinit"
	"github.com/chv/chv/internal/hypervisor"
	"github.com/chv/chv/internal/network"
	"github.com/chv/chv/internal/storage"
)

const (
	CHBinaryPath = "../../../bin/cloud-hypervisor"
	TestVMTimeout = 60 * time.Second
	DefaultTestVCPUs = 2
	DefaultTestMemoryMB = 512
	DefaultTestDiskSize = 100 * 1024 * 1024
)

type TestEnvironment struct {
	TempDir       string
	StateDir      string
	VMDir         string
	ImagesDir     string
	SocketDir     string
	LogDir        string
	CloudInitDir  string
	StorageMgr   *storage.Manager
	TAPManager   *network.TAPManager
	StateManager *hypervisor.StateManager
	ISOGenerator *cloudinit.ISOGenerator
	Launcher     *hypervisor.Launcher
	CHBinary string
}

func SkipIfCHNotAvailable(t *testing.T) {
	chBinary := os.Getenv("CH_BINARY")
	if chBinary == "" {
		chBinary = CHBinaryPath
	}
	if _, err := os.Stat(chBinary); os.IsNotExist(err) {
		t.Skipf("Cloud Hypervisor binary not found at %s, skipping integration test", chBinary)
	}
	cmd := exec.Command(chBinary, "--version")
	if err := cmd.Run(); err != nil {
		t.Skipf("Cloud Hypervisor binary not executable: %v, skipping integration test", err)
	}
}

func SkipIfNotRoot(t *testing.T) {
	if os.Getuid() != 0 {
		t.Skip("Integration tests require root privileges for TAP device management")
	}
}

func SkipIfNoKVM(t *testing.T) {
	if _, err := os.Stat("/dev/kvm"); os.IsNotExist(err) {
		t.Skip("KVM not available (/dev/kvm not found), skipping integration test")
	}
}

func SetupTestEnvironment(t *testing.T) *TestEnvironment {
	t.Helper()
	tempDir, err := os.MkdirTemp("", "chv-integration-*")
	if err != nil {
		t.Fatalf("Failed to create temp directory: %v", err)
	}
	env := &TestEnvironment{
		TempDir:      tempDir,
		StateDir:     filepath.Join(tempDir, "state"),
		VMDir:        filepath.Join(tempDir, "vms"),
		ImagesDir:    filepath.Join(tempDir, "images"),
		SocketDir:    filepath.Join(tempDir, "sockets"),
		LogDir:       filepath.Join(tempDir, "logs"),
		CloudInitDir: filepath.Join(tempDir, "cloudinit"),
	}
	for _, dir := range []string{env.StateDir, env.VMDir, env.ImagesDir, env.SocketDir, env.LogDir, env.CloudInitDir} {
		if err := os.MkdirAll(dir, 0755); err != nil {
			os.RemoveAll(tempDir)
			t.Fatalf("Failed to create directory %s: %v", dir, err)
		}
	}
	env.StorageMgr = storage.NewManager(env.VMDir)
	env.TAPManager = network.NewTAPManager("")
	env.StateManager = hypervisor.NewStateManager(env.StateDir)
	env.ISOGenerator = cloudinit.NewISOGenerator(env.CloudInitDir)
	chBinary := os.Getenv("CH_BINARY")
	if chBinary == "" {
		chBinary = CHBinaryPath
	}
	env.CHBinary = chBinary
	env.Launcher = hypervisor.NewLauncher(
		env.CHBinary,
		env.StateDir,
		env.LogDir,
		env.SocketDir,
		env.StateManager,
		env.TAPManager,
		env.ISOGenerator,
	)
	if err := env.Launcher.Initialize(); err != nil {
		env.Cleanup()
		t.Fatalf("Failed to initialize launcher: %v", err)
	}
	return env
}

func (e *TestEnvironment) Cleanup() {
	if e.Launcher != nil {
		instances := e.Launcher.ListInstances()
		for _, inst := range instances {
			ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
			_ = e.Launcher.StopVM(inst.VMID, true, "cleanup")
			_ = WaitForVMStopped(ctx, inst)
			cancel()
		}
	}
	if e.TempDir != "" {
		os.RemoveAll(e.TempDir)
	}
}

func (e *TestEnvironment) CreateTestDisk(vmID string, size int64) (string, error) {
	diskPath := filepath.Join(e.VMDir, vmID+".raw")
	if err := e.StorageMgr.CreateRawVolume(diskPath, size); err != nil {
		return "", fmt.Errorf("failed to create disk: %w", err)
	}
	return diskPath, nil
}

func (e *TestEnvironment) CreateTestVMConfig(vmID string) *hypervisor.VMConfig {
	return &hypervisor.VMConfig{
		VMID:       vmID,
		Name:       "test-vm-" + vmID[:8],
		VCPU:       DefaultTestVCPUs,
		MemoryMB:   DefaultTestMemoryMB,
		APIsocket:  filepath.Join(e.SocketDir, vmID+".sock"),
	}
}

func WaitForVMRunning(ctx context.Context, client *hypervisor.CHVClient) error {
	return client.WaitForRunning(ctx, TestVMTimeout)
}

func WaitForVMStopped(ctx context.Context, instance *hypervisor.VMInstance) error {
	client := hypervisor.NewCHVClient(instance.APISocket)
	return client.WaitForStopped(ctx, TestVMTimeout)
}

func GenerateTestVMID(t *testing.T) string {
	return fmt.Sprintf("test-%s-%d", t.Name(), time.Now().UnixNano())
}

func VerifyCHVersion(t *testing.T) string {
	chBinary := os.Getenv("CH_BINARY")
	if chBinary == "" {
		chBinary = CHBinaryPath
	}
	cmd := exec.Command(chBinary, "--version")
	output, err := cmd.Output()
	if err != nil {
		t.Logf("Failed to get CH version: %v", err)
		return ""
	}
	return string(output)
}

func CreateTestCloudInitConfig(hostname string) *cloudinit.Config {
	return &cloudinit.Config{
		MetaData: fmt.Sprintf("instance-id: %s\nlocal-hostname: %s\n", hostname, hostname),
		UserData: "#cloud-config\n",
	}
}
