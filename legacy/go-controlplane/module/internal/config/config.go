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
	LogDir              string
	AgentURL            string
	AgentToken          string // Token for authenticating with agent
	BridgeName          string
	BridgeCIDR          string
	BridgeGateway       string
	NetworkCIDR         string
	LocaldiskPath       string
	CloudHypervisorPath string
	DefaultPoolType     string
}

type AgentConfig struct {
	DataRoot            string
	BridgeName          string
	BridgeCIDR          string
	LocaldiskPath       string
	CloudHypervisorPath string
	AuthToken           string // Bearer token for controller authentication
}

func LoadController() ControllerConfig {
	dataRoot := getenv("CHV_DATA_ROOT", DefaultDataRoot)
	return ControllerConfig{
		HTTPAddr:            getenv("CHV_HTTP_ADDR", ":8080"),
		DataRoot:            dataRoot,
		DatabasePath:        getenv("CHV_DATABASE_PATH", filepath.Join(dataRoot, "chv.db")),
		LogDir:              getenv("CHV_LOG_DIR", filepath.Join(dataRoot, "logs")),
		AgentURL:            getenv("CHV_AGENT_URL", ""),
		AgentToken:          os.Getenv("CHV_AGENT_TOKEN"),
		BridgeName:          getenv("CHV_BRIDGE_NAME", DefaultBridgeName),
		BridgeCIDR:          getenv("CHV_BRIDGE_CIDR", DefaultBridgeCIDR),
		BridgeGateway:       getenv("CHV_BRIDGE_GATEWAY", DefaultBridgeGateway),
		NetworkCIDR:         getenv("CHV_NETWORK_CIDR", DefaultNetworkCIDR),
		LocaldiskPath:       getenv("CHV_LOCALDISK_PATH", filepath.Join(dataRoot, "storage", "localdisk")),
		CloudHypervisorPath: getenv("CHV_CLOUD_HYPERVISOR", DefaultCloudHypervisor),
		DefaultPoolType:     getenv("CHV_DEFAULT_POOL_TYPE", "localdisk"),
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
		AuthToken:           os.Getenv("CHV_AGENT_TOKEN"),
	}
}

func getenv(key, fallback string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return fallback
}
