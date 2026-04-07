package bootstrap

import (
	"context"
	"os"
	"path/filepath"
	"time"

	"github.com/chv/chv/internal/config"
	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/installstatus"
	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/network"
)

type BridgeManager interface {
	Inspect(ctx context.Context, bridgeName string, expectedCIDR string) (network.BridgeStatus, error)
	Ensure(ctx context.Context, bridgeName string, expectedCIDR string) ([]string, error)
	Repair(ctx context.Context, bridgeName string, expectedCIDR string) ([]string, error)
}

type PrereqChecker interface {
	FindCloudHypervisor() (string, error)
	FindCloudInitTool() (string, error)
}

type Config struct {
	DataRoot         string
	DatabasePath     string
	BridgeName       string
	BridgeCIDR       string
	LocaldiskPath    string
	Repository       *db.Repository
	BridgeManager    BridgeManager
	PrereqChecker    PrereqChecker
	SkipDBConnection bool
}

type RepairRequest struct {
	RepairBridge      bool `json:"repair_bridge"`
	RepairDirectories bool `json:"repair_directories"`
	RepairLocaldisk   bool `json:"repair_localdisk"`
}

type Service struct {
	cfg Config
}

func NewService(cfg Config) (*Service, error) {
	if cfg.DataRoot == "" {
		cfg.DataRoot = config.DefaultDataRoot
	}
	if cfg.DatabasePath == "" {
		cfg.DatabasePath = filepath.Join(cfg.DataRoot, "chv.db")
	}
	if cfg.BridgeName == "" {
		cfg.BridgeName = config.DefaultBridgeName
	}
	if cfg.BridgeCIDR == "" {
		cfg.BridgeCIDR = config.DefaultBridgeCIDR
	}
	if cfg.LocaldiskPath == "" {
		cfg.LocaldiskPath = filepath.Join(cfg.DataRoot, "storage", "localdisk")
	}
	if cfg.BridgeManager == nil {
		cfg.BridgeManager = network.NewBridgeManager(network.OSRunner{})
	}
	if cfg.PrereqChecker == nil {
		cfg.PrereqChecker = defaultPrereqChecker{runner: network.OSRunner{}}
	}
	return &Service{cfg: cfg}, nil
}

func (s *Service) Check(ctx context.Context) (*models.InstallStatus, error) {
	status := &models.InstallStatus{
		ID:               "singleton",
		DataRoot:         s.cfg.DataRoot,
		DatabasePath:     s.cfg.DatabasePath,
		BridgeName:       s.cfg.BridgeName,
		BridgeIPExpected: s.cfg.BridgeCIDR,
		LocaldiskPath:    s.cfg.LocaldiskPath,
		LastCheckedAt:    time.Now().UTC().Format(time.RFC3339),
	}

	bridgeStatus, err := s.cfg.BridgeManager.Inspect(ctx, s.cfg.BridgeName, s.cfg.BridgeCIDR)
	if err != nil {
		status.LastError = err.Error()
	}
	status.BridgeExists = bridgeStatus.Exists
	status.BridgeIPActual = bridgeStatus.ActualIP
	status.BridgeUp = bridgeStatus.Up

	if _, err := os.Stat(s.cfg.LocaldiskPath); err == nil {
		status.LocaldiskReady = true
	}

	if hypervisorPath, err := s.cfg.PrereqChecker.FindCloudHypervisor(); err == nil {
		status.CloudHypervisorFound = true
		status.CloudHypervisorPath = hypervisorPath
	}

	if _, err := s.cfg.PrereqChecker.FindCloudInitTool(); err == nil {
		status.CloudInitSupported = true
	}

	missingDirs := !pathExists(s.cfg.DataRoot) || !pathExists(filepath.Join(s.cfg.DataRoot, "images")) || !pathExists(filepath.Join(s.cfg.DataRoot, "cloudinit")) || !pathExists(filepath.Join(s.cfg.DataRoot, "storage")) || !pathExists(filepath.Join(s.cfg.DataRoot, "vms")) || !pathExists(filepath.Join(s.cfg.DataRoot, "tmp"))
	status.OverallState = installstatus.Evaluate(status, bridgeStatus.Drift, missingDirs)

	if s.cfg.Repository != nil {
		_ = s.cfg.Repository.UpsertInstallStatus(ctx, status)
	}

	return status, nil
}

func (s *Service) Bootstrap(ctx context.Context) (*models.InstallActionResult, error) {
	actions, err := s.ensureDirectories()
	if err != nil {
		return nil, err
	}

	if err := touchFile(s.cfg.DatabasePath); err != nil {
		return nil, err
	}

	bridgeActions, err := s.cfg.BridgeManager.Ensure(ctx, s.cfg.BridgeName, s.cfg.BridgeCIDR)
	if err != nil {
		return nil, err
	}
	actions = append(actions, bridgeActions...)

	if s.cfg.Repository != nil {
		if err := s.cfg.Repository.EnsureDefaultNetwork(ctx); err != nil {
			return nil, err
		}
		if err := s.cfg.Repository.EnsureDefaultStoragePool(ctx, s.cfg.LocaldiskPath); err != nil {
			return nil, err
		}
	}

	status, err := s.Check(ctx)
	if err != nil {
		return nil, err
	}
	status.LastBootstrappedAt = time.Now().UTC().Format(time.RFC3339)
	status.OverallState = models.InstallStateReady
	if s.cfg.Repository != nil {
		if err := s.cfg.Repository.UpsertInstallStatus(ctx, status); err != nil {
			return nil, err
		}
	}

	return &models.InstallActionResult{
		Status:       status,
		OverallState: status.OverallState,
		ActionsTaken: dedupe(actions),
		Warnings:     nil,
		Errors:       nil,
	}, nil
}

func (s *Service) Repair(ctx context.Context, req RepairRequest) (*models.InstallActionResult, error) {
	var actions []string
	if req.RepairDirectories {
		dirActions, err := s.ensureDirectories()
		if err != nil {
			return nil, err
		}
		actions = append(actions, dirActions...)
	}
	if req.RepairBridge {
		bridgeActions, err := s.cfg.BridgeManager.Repair(ctx, s.cfg.BridgeName, s.cfg.BridgeCIDR)
		if err != nil {
			return nil, err
		}
		actions = append(actions, bridgeActions...)
	}
	if req.RepairLocaldisk {
		if err := os.MkdirAll(s.cfg.LocaldiskPath, 0o755); err != nil {
			return nil, err
		}
		actions = append(actions, "created_localdisk_pool")
		if s.cfg.Repository != nil {
			if err := s.cfg.Repository.EnsureDefaultStoragePool(ctx, s.cfg.LocaldiskPath); err != nil {
				return nil, err
			}
		}
	}

	status, err := s.Check(ctx)
	if err != nil {
		return nil, err
	}
	if len(actions) > 0 {
		status.LastBootstrappedAt = time.Now().UTC().Format(time.RFC3339)
		status.OverallState = models.InstallStateReady
		if s.cfg.Repository != nil {
			if err := s.cfg.Repository.UpsertInstallStatus(ctx, status); err != nil {
				return nil, err
			}
		}
	}

	return &models.InstallActionResult{
		Status:       status,
		OverallState: status.OverallState,
		ActionsTaken: dedupe(actions),
		Warnings:     nil,
		Errors:       nil,
	}, nil
}

func (s *Service) ensureDirectories() ([]string, error) {
	required := []string{
		s.cfg.DataRoot,
		filepath.Join(s.cfg.DataRoot, "images"),
		filepath.Join(s.cfg.DataRoot, "cloudinit"),
		filepath.Join(s.cfg.DataRoot, "storage", "localdisk"),
		filepath.Join(s.cfg.DataRoot, "vms"),
		filepath.Join(s.cfg.DataRoot, "tmp"),
	}

	var actions []string
	for _, dir := range required {
		if !pathExists(dir) {
			if err := os.MkdirAll(dir, 0o755); err != nil {
				return nil, err
			}
			actions = append(actions, "created_directories")
		}
	}
	return dedupe(actions), nil
}

func pathExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

func touchFile(path string) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	file, err := os.OpenFile(path, os.O_CREATE, 0o644)
	if err != nil {
		return err
	}
	return file.Close()
}

func dedupe(values []string) []string {
	seen := map[string]struct{}{}
	var out []string
	for _, value := range values {
		if _, ok := seen[value]; ok {
			continue
		}
		seen[value] = struct{}{}
		out = append(out, value)
	}
	return out
}

type defaultPrereqChecker struct {
	runner network.Runner
}

func (c defaultPrereqChecker) FindCloudHypervisor() (string, error) {
	return c.runner.LookPath("cloud-hypervisor")
}

func (c defaultPrereqChecker) FindCloudInitTool() (string, error) {
	for _, tool := range []string{"xorrisofs", "mkisofs", "genisoimage"} {
		if path, err := c.runner.LookPath(tool); err == nil {
			return path, nil
		}
	}
	return "", os.ErrNotExist
}
