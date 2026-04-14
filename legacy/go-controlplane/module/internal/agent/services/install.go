package services

import (
	"context"
	"net"
	"os"
	"os/exec"

	"github.com/chv/chv/internal/agentapi"
	"github.com/chv/chv/internal/config"
)

type InstallService struct {
	cfg config.AgentConfig
}

func NewInstallService(cfg config.AgentConfig) *InstallService {
	return &InstallService{cfg: cfg}
}

func (s *InstallService) Check(ctx context.Context) (*agentapi.InstallCheckResponse, error) {
	resp := &agentapi.InstallCheckResponse{
		DataRoot:         s.cfg.DataRoot,
		BridgeName:       s.cfg.BridgeName,
		BridgeIPExpected: s.cfg.BridgeCIDR,
		LocaldiskPath:    s.cfg.LocaldiskPath,
	}

	// Check bridge
	resp.BridgeExists = s.bridgeExists(s.cfg.BridgeName)
	if resp.BridgeExists {
		resp.BridgeIPActual = s.getBridgeIP(s.cfg.BridgeName)
		resp.BridgeUp = s.isBridgeUp(s.cfg.BridgeName)
	}

	// Check directories
	resp.LocaldiskReady = s.dirExists(s.cfg.LocaldiskPath)

	// Check cloud-hypervisor
	if path, err := exec.LookPath("cloud-hypervisor"); err == nil {
		resp.CloudHypervisorFound = true
		resp.CloudHypervisorPath = path
	}

	// Check cloud-init tools
	resp.CloudInitSupported = s.hasCloudInitTool()

	// Determine overall state
	resp.OverallState = s.determineState(resp)

	return resp, nil
}

func (s *InstallService) bridgeExists(name string) bool {
	_, err := net.InterfaceByName(name)
	return err == nil
}

func (s *InstallService) getBridgeIP(name string) string {
	iface, err := net.InterfaceByName(name)
	if err != nil {
		return ""
	}
	addrs, err := iface.Addrs()
	if err != nil {
		return ""
	}
	for _, addr := range addrs {
		if ipnet, ok := addr.(*net.IPNet); ok && ipnet.IP.To4() != nil {
			return ipnet.String()
		}
	}
	return ""
}

func (s *InstallService) isBridgeUp(name string) bool {
	iface, err := net.InterfaceByName(name)
	if err != nil {
		return false
	}
	return iface.Flags&net.FlagUp != 0
}

func (s *InstallService) dirExists(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.IsDir()
}

func (s *InstallService) hasCloudInitTool() bool {
	for _, tool := range []string{"xorrisofs", "mkisofs", "genisoimage"} {
		if _, err := exec.LookPath(tool); err == nil {
			return true
		}
	}
	return false
}

func (s *InstallService) determineState(resp *agentapi.InstallCheckResponse) string {
	if !resp.BridgeExists || !resp.BridgeUp || !resp.LocaldiskReady {
		return "missing_prerequisites"
	}
	if !resp.CloudHypervisorFound {
		return "missing_prerequisites"
	}
	return "ready"
}
