// Package config provides configuration management for CHV services.
// Supports YAML config files, environment variables, and .env files.
package config

import (
	"fmt"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/joho/godotenv"
	"gopkg.in/yaml.v3"
)

// ControllerConfig holds the controller service configuration.
type ControllerConfig struct {
	// Server configuration
	HTTPAddr string `yaml:"http_addr" env:"CHV_HTTP_ADDR"`
	GRPCAddr string `yaml:"grpc_addr" env:"CHV_GRPC_ADDR"`

	// Database configuration
	DatabaseURL string `yaml:"database_url" env:"CHV_DATABASE_URL"`

	// Logging
	LogLevel  string `yaml:"log_level" env:"CHV_LOG_LEVEL"`
	LogFormat string `yaml:"log_format" env:"CHV_LOG_FORMAT"`

	// CORS configuration
	CORS CORSConfig `yaml:"cors"`

	// Security
	JWTSecret     string        `yaml:"jwt_secret" env:"CHV_JWT_SECRET"`
	TokenDuration time.Duration `yaml:"token_duration" env:"CHV_TOKEN_DURATION"`
}

// AgentConfig holds the agent service configuration.
type AgentConfig struct {
	// Server configuration
	NodeID         string `yaml:"node_id" env:"CHV_NODE_ID"`
	ListenAddr     string `yaml:"listen_addr" env:"CHV_AGENT_LISTEN_ADDR"`
	ControllerAddr string `yaml:"controller_addr" env:"CHV_CONTROLLER_ADDR"`

	// Directories
	DataDir   string `yaml:"data_dir" env:"CHV_DATA_DIR"`
	ImageDir  string `yaml:"image_dir" env:"CHV_IMAGE_DIR"`
	VolumeDir string `yaml:"volume_dir" env:"CHV_VOLUME_DIR"`

	// Cloud Hypervisor
	CloudHypervisor string `yaml:"cloud_hypervisor" env:"CHV_CLOUD_HYPERVISOR"`

	// Network
	BridgeName string `yaml:"bridge_name" env:"CHV_BRIDGE_NAME"`

	// Logging
	LogLevel  string `yaml:"log_level" env:"CHV_LOG_LEVEL"`
	LogFormat string `yaml:"log_format" env:"CHV_LOG_FORMAT"`

	// Heartbeat
	HeartbeatInterval time.Duration `yaml:"heartbeat_interval" env:"CHV_HEARTBEAT_INTERVAL"`

	// Console
	ConsolePort int `yaml:"console_port" env:"CHV_CONSOLE_PORT"`

	// CORS for console WebSocket
	CORS CORSConfig `yaml:"cors"`
}

// CORSConfig holds CORS configuration.
type CORSConfig struct {
	// AllowedOrigins is a list of origins a cross-domain request can be executed from.
	// An origin may contain a wildcard (*) to replace 0 or more characters
	// (i.e., http://*.domain.com). Use "*" to allow all origins.
	AllowedOrigins []string `yaml:"allowed_origins" env:"CHV_CORS_ORIGINS"`

	// AllowedMethods is a list of methods the client is allowed to use with
	// cross-domain requests.
	AllowedMethods []string `yaml:"allowed_methods"`

	// AllowedHeaders is list of non-simple headers the client is allowed to use with
	// cross-domain requests.
	AllowedHeaders []string `yaml:"allowed_headers"`

	// ExposedHeaders indicates which headers are safe to expose to the API of a CORS
	// API specification.
	ExposedHeaders []string `yaml:"exposed_headers"`

	// AllowCredentials indicates whether the request can include user credentials like
	// cookies, HTTP authentication or client side SSL certificates.
	AllowCredentials bool `yaml:"allow_credentials"`

	// MaxAge indicates how long (in seconds) the results of a preflight request
	// can be cached.
	MaxAge int `yaml:"max_age"`
}

// DefaultControllerConfig returns default controller configuration.
func DefaultControllerConfig() *ControllerConfig {
	return &ControllerConfig{
		HTTPAddr:          ":8080",
		GRPCAddr:          ":9090",
		DatabaseURL:       "postgres://chv:chv@localhost:5432/chv?sslmode=disable",
		LogLevel:          "info",
		LogFormat:         "json",
		TokenDuration:     24 * time.Hour,
		CORS:              DefaultCORSConfig(),
	}
}

// DefaultAgentConfig returns default agent configuration.
func DefaultAgentConfig() *AgentConfig {
	return &AgentConfig{
		NodeID:            getHostname(),
		ListenAddr:        ":8081",
		ControllerAddr:    "localhost:9090",
		DataDir:           "/var/lib/chv-agent",
		ImageDir:          "/var/lib/chv-agent/images",
		VolumeDir:         "/var/lib/chv-agent/volumes",
		CloudHypervisor:   "/usr/bin/cloud-hypervisor",
		BridgeName:        "br0",
		LogLevel:          "info",
		LogFormat:         "json",
		HeartbeatInterval: 30 * time.Second,
		ConsolePort:       8090,
		CORS:              DefaultCORSConfig(),
	}
}

// DefaultCORSConfig returns default CORS configuration.
func DefaultCORSConfig() CORSConfig {
	return CORSConfig{
		AllowedOrigins: []string{
			"http://10.5.199.83",
			"http://10.5.199.83:3000",
			"http://localhost",
			"http://localhost:3000",
			"http://127.0.0.1",
			"http://127.0.0.1:3000",
		},
		AllowedMethods:   []string{"GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"},
		AllowedHeaders:   []string{"Accept", "Authorization", "Content-Type", "X-Requested-With"},
		ExposedHeaders:   []string{"Link"},
		AllowCredentials: true,
		MaxAge:           300,
	}
}

// LoadControllerConfig loads controller configuration from file and environment.
func LoadControllerConfig(configPath string) (*ControllerConfig, error) {
	cfg := DefaultControllerConfig()

	// Load .env file if it exists
	_ = godotenv.Load()

	// Load from YAML file if provided
	if configPath != "" {
		data, err := os.ReadFile(configPath)
		if err != nil {
			return nil, fmt.Errorf("failed to read config file: %w", err)
		}
		if err := yaml.Unmarshal(data, cfg); err != nil {
			return nil, fmt.Errorf("failed to parse config file: %w", err)
		}
	}

	// Override with environment variables
	applyEnvOverrides(cfg)

	// Validate configuration
	if err := cfg.Validate(); err != nil {
		return nil, fmt.Errorf("invalid configuration: %w", err)
	}

	return cfg, nil
}

// LoadAgentConfig loads agent configuration from file and environment.
func LoadAgentConfig(configPath string) (*AgentConfig, error) {
	cfg := DefaultAgentConfig()

	// Load .env file if it exists
	_ = godotenv.Load()

	// Load from YAML file if provided
	if configPath != "" {
		data, err := os.ReadFile(configPath)
		if err != nil {
			return nil, fmt.Errorf("failed to read config file: %w", err)
		}
		if err := yaml.Unmarshal(data, cfg); err != nil {
			return nil, fmt.Errorf("failed to parse config file: %w", err)
		}
	}

	// Override with environment variables
	applyAgentEnvOverrides(cfg)

	// Validate configuration
	if err := cfg.Validate(); err != nil {
		return nil, fmt.Errorf("invalid configuration: %w", err)
	}

	return cfg, nil
}

// Validate validates controller configuration.
func (c *ControllerConfig) Validate() error {
	if c.HTTPAddr == "" {
		return fmt.Errorf("HTTP address is required")
	}
	if c.GRPCAddr == "" {
		return fmt.Errorf("gRPC address is required")
	}
	if c.DatabaseURL == "" {
		return fmt.Errorf("database URL is required")
	}
	return nil
}

// Validate validates agent configuration.
func (c *AgentConfig) Validate() error {
	if c.NodeID == "" {
		return fmt.Errorf("node ID is required")
	}
	if c.ListenAddr == "" {
		return fmt.Errorf("listen address is required")
	}
	if c.ControllerAddr == "" {
		return fmt.Errorf("controller address is required")
	}
	if c.CloudHypervisor == "" {
		return fmt.Errorf("cloud hypervisor path is required")
	}
	return nil
}

// applyEnvOverrides applies environment variable overrides to controller config.
func applyEnvOverrides(cfg *ControllerConfig) {
	if v := os.Getenv("CHV_HTTP_ADDR"); v != "" {
		cfg.HTTPAddr = v
	}
	if v := os.Getenv("CHV_GRPC_ADDR"); v != "" {
		cfg.GRPCAddr = v
	}
	if v := os.Getenv("CHV_DATABASE_URL"); v != "" {
		cfg.DatabaseURL = v
	}
	if v := os.Getenv("CHV_LOG_LEVEL"); v != "" {
		cfg.LogLevel = v
	}
	if v := os.Getenv("CHV_LOG_FORMAT"); v != "" {
		cfg.LogFormat = v
	}
	if v := os.Getenv("CHV_JWT_SECRET"); v != "" {
		cfg.JWTSecret = v
	}
	if v := os.Getenv("CHV_TOKEN_DURATION"); v != "" {
		if d, err := time.ParseDuration(v); err == nil {
			cfg.TokenDuration = d
		}
	}
	if v := os.Getenv("CHV_CORS_ORIGINS"); v != "" {
		cfg.CORS.AllowedOrigins = parseOrigins(v)
	}
}

// applyAgentEnvOverrides applies environment variable overrides to agent config.
func applyAgentEnvOverrides(cfg *AgentConfig) {
	if v := os.Getenv("CHV_NODE_ID"); v != "" {
		cfg.NodeID = v
	}
	if v := os.Getenv("CHV_AGENT_LISTEN_ADDR"); v != "" {
		cfg.ListenAddr = v
	}
	if v := os.Getenv("CHV_CONTROLLER_ADDR"); v != "" {
		cfg.ControllerAddr = v
	}
	if v := os.Getenv("CHV_DATA_DIR"); v != "" {
		cfg.DataDir = v
	}
	if v := os.Getenv("CHV_IMAGE_DIR"); v != "" {
		cfg.ImageDir = v
	}
	if v := os.Getenv("CHV_VOLUME_DIR"); v != "" {
		cfg.VolumeDir = v
	}
	if v := os.Getenv("CHV_CLOUD_HYPERVISOR"); v != "" {
		cfg.CloudHypervisor = v
	}
	if v := os.Getenv("CHV_BRIDGE_NAME"); v != "" {
		cfg.BridgeName = v
	}
	if v := os.Getenv("CHV_LOG_LEVEL"); v != "" {
		cfg.LogLevel = v
	}
	if v := os.Getenv("CHV_LOG_FORMAT"); v != "" {
		cfg.LogFormat = v
	}
	if v := os.Getenv("CHV_HEARTBEAT_INTERVAL"); v != "" {
		if d, err := time.ParseDuration(v); err == nil {
			cfg.HeartbeatInterval = d
		}
	}
	if v := os.Getenv("CHV_CONSOLE_PORT"); v != "" {
		if p, err := strconv.Atoi(v); err == nil {
			cfg.ConsolePort = p
		}
	}
	if v := os.Getenv("CHV_CORS_ORIGINS"); v != "" {
		cfg.CORS.AllowedOrigins = parseOrigins(v)
	}
}

// parseOrigins parses a comma-separated list of origins.
func parseOrigins(s string) []string {
	var origins []string
	for _, o := range strings.Split(s, ",") {
		o = strings.TrimSpace(o)
		if o != "" {
			origins = append(origins, o)
		}
	}
	return origins
}

// getHostname returns the system hostname or "unknown".
func getHostname() string {
	hostname, err := os.Hostname()
	if err != nil {
		return "unknown"
	}
	return hostname
}
