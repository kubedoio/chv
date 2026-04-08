package services

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"

	"github.com/chv/chv/internal/agentapi"
)

type BootstrapService struct{}

func NewBootstrapService() *BootstrapService {
	return &BootstrapService{}
}

func (s *BootstrapService) Bootstrap(ctx context.Context, req *agentapi.BootstrapRequest) (*agentapi.BootstrapResponse, error) {
	var actions []string
	var warnings []string

	// Create directories (idempotent - only report if created)
	dirs := []string{
		req.DataRoot,
		filepath.Join(req.DataRoot, "images"),
		filepath.Join(req.DataRoot, "cloudinit"),
		filepath.Join(req.DataRoot, "storage", "localdisk"),
		filepath.Join(req.DataRoot, "vms"),
		filepath.Join(req.DataRoot, "tmp"),
	}

	for _, dir := range dirs {
		created, err := s.ensureDirectory(dir)
		if err != nil {
			return nil, fmt.Errorf("failed to create directory %s: %w", dir, err)
		}
		if created {
			actions = append(actions, "created_directory:"+dir)
		}
	}

	// Create/ensure bridge
	bridgeActions, err := s.ensureBridge(req.BridgeName, req.BridgeCIDR)
	if err != nil {
		return nil, fmt.Errorf("failed to ensure bridge: %w", err)
	}
	actions = append(actions, bridgeActions...)

	return &agentapi.BootstrapResponse{
		ActionsTaken: actions,
		Warnings:     warnings,
	}, nil
}

func (s *BootstrapService) ensureDirectory(dir string) (bool, error) {
	if _, err := os.Stat(dir); err == nil {
		// Directory already exists
		return false, nil
	}
	if err := os.MkdirAll(dir, 0755); err != nil {
		return false, err
	}
	return true, nil
}

func (s *BootstrapService) ensureBridge(name, cidr string) ([]string, error) {
	var actions []string

	// Check if ip command exists
	if _, err := exec.LookPath("ip"); err != nil {
		return nil, fmt.Errorf("ip command not found")
	}

	// Check if bridge exists
	cmd := exec.Command("ip", "link", "show", name)
	bridgeExists := cmd.Run() == nil

	if !bridgeExists {
		// Bridge doesn't exist, create it
		createCmd := exec.Command("ip", "link", "add", name, "type", "bridge")
		if out, err := createCmd.CombinedOutput(); err != nil {
			return nil, fmt.Errorf("failed to create bridge: %w (output: %s)", err, out)
		}
		actions = append(actions, "created_bridge:"+name)
	}

	// Bring bridge up (idempotent - safe to call multiple times)
	upCmd := exec.Command("ip", "link", "set", name, "up")
	if out, err := upCmd.CombinedOutput(); err != nil {
		return nil, fmt.Errorf("failed to bring bridge up: %w (output: %s)", err, out)
	}

	// Only report if we actually changed the state (bridge was just created)
	if !bridgeExists {
		actions = append(actions, "brought_bridge_up:"+name)
	}

	// Assign IP if provided
	if cidr != "" {
		addrCmd := exec.Command("ip", "addr", "add", cidr, "dev", name)
		out, err := addrCmd.CombinedOutput()
		if err != nil {
			// IP might already be assigned, that's ok
			if !isAddrExistsError(out) {
				return nil, fmt.Errorf("failed to assign IP: %w (output: %s)", err, out)
			}
		} else {
			actions = append(actions, "assigned_bridge_ip:"+cidr)
		}
	}

	return actions, nil
}

func isAddrExistsError(output []byte) bool {
	// "RTNETLINK answers: File exists" means IP already assigned
	return bytes.Contains(output, []byte("File exists"))
}
