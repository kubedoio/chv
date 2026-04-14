package vm

import (
	"context"
	"fmt"
	"os"
	"path/filepath"

	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/models"
)

// GateCheck represents a single prerequisite check
type GateCheck struct {
	Name    string
	Check   func() error
	ErrCode string
}

// GateResult holds the result of all gate checks
type GateResult struct {
	Passed bool
	Errors []GateError
}

// GateError represents a single gate failure
type GateError struct {
	Gate    string
	Code    string
	Message string
}

func (e GateError) Error() string {
	return fmt.Sprintf("%s: %s", e.Gate, e.Message)
}

// Gatekeeper performs pre-flight checks before VM start
type Gatekeeper struct {
	repo *db.Repository
}

// NewGatekeeper creates a new Gatekeeper instance
func NewGatekeeper(repo *db.Repository) *Gatekeeper {
	return &Gatekeeper{repo: repo}
}

// CheckAll performs all gate checks for a VM
func (g *Gatekeeper) CheckAll(ctx context.Context, vm *models.VirtualMachine) *GateResult {
	var errors []GateError

	checks := []GateCheck{
		{
			Name:    "image_ready",
			ErrCode: "image_not_ready",
			Check: func() error {
				return g.checkImageReady(ctx, vm.ImageID)
			},
		},
		{
			Name:    "storage_ready",
			ErrCode: "storage_not_ready",
			Check: func() error {
				return g.checkStorageReady(ctx, vm.StoragePoolID)
			},
		},
		{
			Name:    "network_ready",
			ErrCode: "network_not_ready",
			Check: func() error {
				return g.checkNetworkReady(ctx, vm.NetworkID)
			},
		},
		{
			Name:    "workspace_exists",
			ErrCode: "workspace_missing",
			Check: func() error {
				return g.checkWorkspaceExists(vm.WorkspacePath)
			},
		},
		{
			Name:    "seed_iso_exists",
			ErrCode: "seed_iso_missing",
			Check: func() error {
				return g.checkSeedISOExists(vm.WorkspacePath)
			},
		},
		{
			Name:    "disk_exists",
			ErrCode: "disk_missing",
			Check: func() error {
				return g.checkDiskExists(vm.WorkspacePath)
			},
		},
	}

	for _, check := range checks {
		if err := check.Check(); err != nil {
			errors = append(errors, GateError{
				Gate:    check.Name,
				Code:    check.ErrCode,
				Message: err.Error(),
			})
		}
	}

	return &GateResult{
		Passed: len(errors) == 0,
		Errors: errors,
	}
}

func (g *Gatekeeper) checkImageReady(ctx context.Context, imageID string) error {
	image, err := g.repo.GetImageByID(ctx, imageID)
	if err != nil {
		return fmt.Errorf("failed to check image: %w", err)
	}
	if image == nil {
		return fmt.Errorf("image not found")
	}
	if image.Status != "ready" {
		return fmt.Errorf("image status is %s, expected ready", image.Status)
	}
	return nil
}

func (g *Gatekeeper) checkStorageReady(ctx context.Context, poolID string) error {
	pool, err := g.repo.GetStoragePoolByID(ctx, poolID)
	if err != nil {
		return fmt.Errorf("failed to check storage pool: %w", err)
	}
	if pool == nil {
		return fmt.Errorf("storage pool not found")
	}
	if pool.Status != "ready" {
		return fmt.Errorf("storage pool status is %s, expected ready", pool.Status)
	}
	return nil
}

func (g *Gatekeeper) checkNetworkReady(ctx context.Context, networkID string) error {
	// For MVP, networks are always "active" if they exist
	// Future: check bridge actually exists on host
	network, err := g.repo.GetNetworkByID(ctx, networkID)
	if err != nil {
		return fmt.Errorf("failed to check network: %w", err)
	}
	if network == nil {
		return fmt.Errorf("network not found")
	}
	return nil
}

func (g *Gatekeeper) checkWorkspaceExists(workspacePath string) error {
	if workspacePath == "" {
		return fmt.Errorf("workspace path not set")
	}
	info, err := os.Stat(workspacePath)
	if err != nil {
		return fmt.Errorf("workspace does not exist: %w", err)
	}
	if !info.IsDir() {
		return fmt.Errorf("workspace path is not a directory")
	}
	return nil
}

func (g *Gatekeeper) checkSeedISOExists(workspacePath string) error {
	seedPath := filepath.Join(workspacePath, "seed.iso")
	if _, err := os.Stat(seedPath); err != nil {
		return fmt.Errorf("seed.iso not found at %s", seedPath)
	}
	return nil
}

func (g *Gatekeeper) checkDiskExists(workspacePath string) error {
	diskPath := filepath.Join(workspacePath, "disk.qcow2")
	if _, err := os.Stat(diskPath); err != nil {
		return fmt.Errorf("disk.qcow2 not found at %s", diskPath)
	}
	return nil
}
