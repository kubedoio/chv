package network

import (
	"context"
	"errors"
	"os/exec"
	"strings"
)

type Runner interface {
	Run(ctx context.Context, name string, args ...string) ([]byte, error)
	LookPath(file string) (string, error)
}

type OSRunner struct{}

func (OSRunner) Run(ctx context.Context, name string, args ...string) ([]byte, error) {
	return exec.CommandContext(ctx, name, args...).CombinedOutput()
}

func (OSRunner) LookPath(file string) (string, error) {
	return exec.LookPath(file)
}

type BridgeStatus struct {
	Name     string
	Exists   bool
	ActualIP string
	Up       bool
	Drift    bool
}

type BridgeManager struct {
	runner Runner
}

func NewBridgeManager(runner Runner) *BridgeManager {
	if runner == nil {
		runner = OSRunner{}
	}
	return &BridgeManager{runner: runner}
}

func (m *BridgeManager) Inspect(ctx context.Context, bridgeName string, expectedCIDR string) (BridgeStatus, error) {
	status := BridgeStatus{Name: bridgeName}

	linkOut, err := m.runner.Run(ctx, "ip", "link", "show", bridgeName)
	if err != nil {
		return status, nil
	}

	status.Exists = true
	status.Up = strings.Contains(string(linkOut), "UP")

	addrOut, err := m.runner.Run(ctx, "ip", "-4", "addr", "show", "dev", bridgeName)
	if err != nil {
		return status, nil
	}

	for _, line := range strings.Split(string(addrOut), "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "inet ") {
			fields := strings.Fields(line)
			if len(fields) >= 2 {
				status.ActualIP = fields[1]
				break
			}
		}
	}

	status.Drift = status.Exists && status.ActualIP != "" && status.ActualIP != expectedCIDR
	return status, nil
}

func (m *BridgeManager) Ensure(ctx context.Context, bridgeName string, expectedCIDR string) ([]string, error) {
	return m.apply(ctx, bridgeName, expectedCIDR, false)
}

func (m *BridgeManager) Repair(ctx context.Context, bridgeName string, expectedCIDR string) ([]string, error) {
	return m.apply(ctx, bridgeName, expectedCIDR, true)
}

func (m *BridgeManager) apply(ctx context.Context, bridgeName string, expectedCIDR string, allowRepair bool) ([]string, error) {
	status, err := m.Inspect(ctx, bridgeName, expectedCIDR)
	if err != nil {
		return nil, err
	}

	var actions []string
	if !status.Exists {
		if _, err := m.runner.Run(ctx, "ip", "link", "add", bridgeName, "type", "bridge"); err != nil {
			return nil, err
		}
		actions = append(actions, "created_bridge")
		status.Exists = true
	}

	if status.ActualIP != "" && status.ActualIP != expectedCIDR {
		return actions, errors.New("bridge drift detected")
	}

	if status.ActualIP == "" {
		if _, err := m.runner.Run(ctx, "ip", "addr", "add", expectedCIDR, "dev", bridgeName); err != nil {
			return nil, err
		}
		actions = append(actions, "assigned_bridge_ip")
	}

	if !status.Up || allowRepair {
		if _, err := m.runner.Run(ctx, "ip", "link", "set", bridgeName, "up"); err != nil {
			return nil, err
		}
		actions = append(actions, "brought_bridge_up")
	}

	return dedupe(actions), nil
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
