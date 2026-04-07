package config

import (
	"os"
	"path/filepath"
)

const (
	DefaultDataRoot        = "/var/lib/chv"
	DefaultDatabasePath    = "/var/lib/chv/chv.db"
	DefaultBridgeName      = "chvbr0"
	DefaultBridgeCIDR      = "10.0.0.1/24"
	DefaultBridgeGateway   = "10.0.0.1"
	DefaultNetworkCIDR     = "10.0.0.0/24"
	DefaultLocaldiskPath   = "/var/lib/chv/storage/localdisk"
	DefaultCloudHypervisor = "/usr/bin/cloud-hypervisor"
)

type ControllerConfig struct {
	HTTPAddr            string
	DataRoot            string
	DatabasePath        string
	BridgeName          string
	BridgeCIDR          string
	LocaldiskPath       string
	CloudHypervisorPath string
}

type AgentConfig struct {
	DataRoot            string
	BridgeName          string
	BridgeCIDR          string
	LocaldiskPath       string
	CloudHypervisorPath string
}

func LoadController() ControllerConfig {
	dataRoot := getenv("CHV_DATA_ROOT", DefaultDataRoot)
	return ControllerConfig{
		HTTPAddr:            getenv("CHV_HTTP_ADDR", ":8080"),
		DataRoot:            dataRoot,
		DatabasePath:        getenv("CHV_DATABASE_PATH", filepath.Join(dataRoot, "chv.db")),
		BridgeName:          getenv("CHV_BRIDGE_NAME", DefaultBridgeName),
		BridgeCIDR:          getenv("CHV_BRIDGE_CIDR", DefaultBridgeCIDR),
		LocaldiskPath:       getenv("CHV_LOCALDISK_PATH", filepath.Join(dataRoot, "storage", "localdisk")),
		CloudHypervisorPath: getenv("CHV_CLOUD_HYPERVISOR", DefaultCloudHypervisor),
	}
}

func LoadAgent() AgentConfig {
	dataRoot := getenv("CHV_DATA_ROOT", DefaultDataRoot)
	return AgentConfig{
		DataRoot:            dataRoot,
		BridgeName:          getenv("CHV_BRIDGE_NAME", DefaultBridgeName),
		BridgeCIDR:          getenv("CHV_BRIDGE_CIDR", DefaultBridgeCIDR),
		LocaldiskPath:       getenv("CHV_LOCALDISK_PATH", filepath.Join(dataRoot, "storage", "localdisk")),
		CloudHypervisorPath: getenv("CHV_CLOUD_HYPERVISOR", DefaultCloudHypervisor),
	}
}

func getenv(key, fallback string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return fallback
}
