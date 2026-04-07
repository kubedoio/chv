package bootstrap

import (
	"context"
	"errors"
	"path/filepath"
	"testing"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/network"
)

type fakeBridgeManager struct {
	status  network.BridgeStatus
	actions []string
	err     error
}

func (f *fakeBridgeManager) Inspect(_ context.Context, _ string, _ string) (network.BridgeStatus, error) {
	return f.status, f.err
}

func (f *fakeBridgeManager) Ensure(_ context.Context, _ string, _ string) ([]string, error) {
	return f.actions, f.err
}

func (f *fakeBridgeManager) Repair(_ context.Context, _ string, _ string) ([]string, error) {
	return f.actions, f.err
}

type fakePrereqChecker struct {
	hypervisorPath string
	hypervisorErr  error
	isoToolPath    string
	isoToolErr     error
}

func (f fakePrereqChecker) FindCloudHypervisor() (string, error) {
	return f.hypervisorPath, f.hypervisorErr
}

func (f fakePrereqChecker) FindCloudInitTool() (string, error) {
	return f.isoToolPath, f.isoToolErr
}

func TestServiceBootstrapIsIdempotent(t *testing.T) {
	root := t.TempDir()
	repoPath := filepath.Join(root, "chv.db")

	service, err := NewService(Config{
		DataRoot:         root,
		DatabasePath:     repoPath,
		BridgeName:       "chvbr0",
		BridgeCIDR:       "10.0.0.1/24",
		LocaldiskPath:    filepath.Join(root, "storage", "localdisk"),
		BridgeManager:    &fakeBridgeManager{actions: []string{"created_bridge", "assigned_bridge_ip", "brought_bridge_up"}},
		PrereqChecker:    fakePrereqChecker{hypervisorPath: "/usr/bin/cloud-hypervisor", isoToolPath: "/usr/bin/xorrisofs"},
		SkipDBConnection: true,
	})
	if err != nil {
		t.Fatalf("NewService() error = %v", err)
	}

	first, err := service.Bootstrap(context.Background())
	if err != nil {
		t.Fatalf("Bootstrap() first error = %v", err)
	}
	second, err := service.Bootstrap(context.Background())
	if err != nil {
		t.Fatalf("Bootstrap() second error = %v", err)
	}

	if first.OverallState != models.InstallStateReady || second.OverallState != models.InstallStateReady {
		t.Fatalf("expected ready state after bootstrap, got %q and %q", first.OverallState, second.OverallState)
	}
}

func TestServiceCheckReportsMissingPrerequisites(t *testing.T) {
	root := t.TempDir()
	service, err := NewService(Config{
		DataRoot:         root,
		DatabasePath:     filepath.Join(root, "chv.db"),
		BridgeName:       "chvbr0",
		BridgeCIDR:       "10.0.0.1/24",
		LocaldiskPath:    filepath.Join(root, "storage", "localdisk"),
		BridgeManager:    &fakeBridgeManager{status: network.BridgeStatus{}},
		PrereqChecker:    fakePrereqChecker{hypervisorErr: errors.New("missing"), isoToolErr: errors.New("missing")},
		SkipDBConnection: true,
	})
	if err != nil {
		t.Fatalf("NewService() error = %v", err)
	}

	status, err := service.Check(context.Background())
	if err != nil {
		t.Fatalf("Check() error = %v", err)
	}

	if status.OverallState != models.InstallStateMissingPrerequisites {
		t.Fatalf("expected missing_prerequisites, got %q", status.OverallState)
	}
}
