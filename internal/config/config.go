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
	DatabasePath string `yaml:"database_path" env:"CHV_DATABASE_PATH"`

	// Logging
	LogLevel  string `yaml:"log_level" env:"CHV_LOG_LEVEL"`
	LogFormat string `yaml:"log_format" env:"CHV_LOG_FORMAT"`

	// CORS configuration
	CORS CORSConfig `yaml:"cors"`

	// Security
	JWTSecret     string        `yaml:"jwt_secret" env:"CHV_JWT_SECRET"`
	TokenDuration time.Duration `yaml:"token_duration" env:"CHV_TOKEN_DURATION"`

	// TLS configuration
	TLS TLSConfig `yaml:"tls"`

	// Rate limiting
	RateLimit RateLimitConfig `yaml:"rate_limit"`
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
	BridgeName     string `yaml:"bridge_name" env:"CHV_BRIDGE_NAME"`
	BridgeUplink   string `yaml:"bridge_uplink" env:"CHV_BRIDGE_UPLINK"`
	BridgeGatewayIP string `yaml:"bridge_gateway_ip" env:"CHV_BRIDGE_GATEWAY_IP"`

	// Logging
	LogLevel  string `yaml:"log_level" env:"CHV_LOG_LEVEL"`
	LogFormat string `yaml:"log_format" env:"CHV_LOG_FORMAT"`

	// Heartbeat
	HeartbeatInterval time.Duration `yaml:"heartbeat_interval" env:"CHV_HEARTBEAT_INTERVAL"`

	// Console
	ConsolePort int `yaml:"console_port" env:"CHV_CONSOLE_PORT"`

	// CORS for console WebSocket
	CORS CORSConfig `yaml:"cors"`

	// TLS configuration
	TLS TLSConfig `yaml:"tls"`
}

// TLSConfig holds TLS/mTLS configuration.
type TLSConfig struct {
	// Enabled enables TLS/mTLS for all connections
	Enabled bool `yaml:"enabled" env:"CHV_TLS_ENABLED"`
	// Cert is the path to the server certificate (PEM encoded)
	Cert string `yaml:"cert" env:"CHV_TLS_CERT"`
	// Key is the path to the server private key (PEM encoded)
	Key string `yaml:"key" env:"CHV_TLS_KEY"`
	// CA is the path to the CA certificate for client verification (mTLS)
	CA string `yaml:"ca" env:"CHV_TLS_CA"`
	// ClientCert is the path to the client certificate for mTLS (agent only)
	ClientCert string `yaml:"client_cert" env:"CHV_TLS_CLIENT_CERT"`
	// ClientKey is the path to the client private key for mTLS (agent only)
	ClientKey string `yaml:"client_key" env:"CHV_TLS_CLIENT_KEY"`
	// ServerName is the expected server hostname for certificate verification
	ServerName string `yaml:"server_name" env:"CHV_TLS_SERVER_NAME"`
	// InsecureSkipVerify skips server certificate verification (dev only, dangerous!)
	InsecureSkipVerify bool `yaml:"insecure_skip_verify" env:"CHV_TLS_INSECURE_SKIP_VERIFY"`
	// AutoGenerate enables automatic certificate generation (dev/testing only)
	AutoGenerate bool `yaml:"auto_generate" env:"CHV_TLS_AUTO_GENERATE"`
	// CertDir is the directory to store auto-generated certificates
	CertDir string `yaml:"cert_dir" env:"CHV_TLS_CERT_DIR"`
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

// RateLimitConfig holds rate limiting configuration.
type RateLimitConfig struct {
	// Enabled enables or disables rate limiting
	Enabled bool `yaml:"enabled" env:"CHV_RATE_LIMIT_ENABLED"`

	// IPBased configures per-IP rate limiting
	IPBased IPRateLimitConfig `yaml:"ip_based"`

	// UserBased configures per-user rate limiting for authenticated requests
	UserBased UserRateLimitConfig `yaml:"user_based"`

	// Endpoints configures endpoint-specific rate limits
	Endpoints EndpointRateLimitConfig `yaml:"endpoints"`
}

// IPRateLimitConfig holds per-IP rate limiting configuration.
type IPRateLimitConfig struct {
	// Enabled enables per-IP rate limiting
	Enabled bool `yaml:"enabled" env:"CHV_RATE_LIMIT_IP_ENABLED"`
	// RequestsPerMinute is the number of requests allowed per minute
	RequestsPerMinute int `yaml:"requests_per_minute" env:"CHV_RATE_LIMIT_IP_RPM"`
	// Burst is the maximum burst size
	Burst int `yaml:"burst" env:"CHV_RATE_LIMIT_IP_BURST"`
}

// UserRateLimitConfig holds per-user rate limiting configuration.
type UserRateLimitConfig struct {
	// Enabled enables per-user rate limiting
	Enabled bool `yaml:"enabled" env:"CHV_RATE_LIMIT_USER_ENABLED"`
	// RequestsPerMinute is the number of requests allowed per minute
	RequestsPerMinute int `yaml:"requests_per_minute" env:"CHV_RATE_LIMIT_USER_RPM"`
	// Burst is the maximum burst size
	Burst int `yaml:"burst" env:"CHV_RATE_LIMIT_USER_BURST"`
}

// EndpointRateLimitConfig holds endpoint-specific rate limit configuration.
type EndpointRateLimitConfig struct {
	// StrictRPM is the rate limit for expensive operations (VM create, delete, etc.)
	StrictRPM int `yaml:"strict_rpm" env:"CHV_RATE_LIMIT_STRICT_RPM"`
	// StrictBurst is the burst size for expensive operations
	StrictBurst int `yaml:"strict_burst" env:"CHV_RATE_LIMIT_STRICT_BURST"`
	// StandardRPM is the rate limit for normal operations
	StandardRPM int `yaml:"standard_rpm" env:"CHV_RATE_LIMIT_STANDARD_RPM"`
	// StandardBurst is the burst size for normal operations
	StandardBurst int `yaml:"standard_burst" env:"CHV_RATE_LIMIT_STANDARD_BURST"`
	// RelaxedRPM is the rate limit for health checks
	RelaxedRPM int `yaml:"relaxed_rpm" env:"CHV_RATE_LIMIT_RELAXED_RPM"`
	// RelaxedBurst is the burst size for health checks
	RelaxedBurst int `yaml:"relaxed_burst" env:"CHV_RATE_LIMIT_RELAXED_BURST"`
}

// DefaultControllerConfig returns default controller configuration.
func DefaultControllerConfig() *ControllerConfig {
	return &ControllerConfig{
		HTTPAddr:          ":8080",
		GRPCAddr:          ":9090",
		DatabasePath:      "/var/lib/chv/chv.db",
		LogLevel:          "info",
		LogFormat:         "json",
		TokenDuration:     24 * time.Hour,
		CORS:              DefaultCORSConfig(),
		RateLimit:         DefaultRateLimitConfig(),
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
		BridgeName:        "chvbr0",
		BridgeUplink:      "ens19",
		BridgeGatewayIP:   "10.0.0.1",
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

// DefaultRateLimitConfig returns default rate limiting configuration.
func DefaultRateLimitConfig() RateLimitConfig {
	return RateLimitConfig{
		Enabled: true,
		IPBased: IPRateLimitConfig{
			Enabled:           true,
			RequestsPerMinute: 60,
			Burst:             10,
		},
		UserBased: UserRateLimitConfig{
			Enabled:           true,
			RequestsPerMinute: 120,
			Burst:             20,
		},
		Endpoints: EndpointRateLimitConfig{
			StrictRPM:     10,
			StrictBurst:   5,
			StandardRPM:   60,
			StandardBurst: 10,
			RelaxedRPM:    300,
			RelaxedBurst:  50,
		},
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
	if c.DatabasePath == "" {
		return fmt.Errorf("database path is required")
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
	if v := os.Getenv("CHV_DATABASE_PATH"); v != "" {
		cfg.DatabasePath = v
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
	// TLS environment overrides
	if v := os.Getenv("CHV_TLS_ENABLED"); v != "" {
		cfg.TLS.Enabled = parseBool(v)
	}
	if v := os.Getenv("CHV_TLS_CERT"); v != "" {
		cfg.TLS.Cert = v
	}
	if v := os.Getenv("CHV_TLS_KEY"); v != "" {
		cfg.TLS.Key = v
	}
	if v := os.Getenv("CHV_TLS_CA"); v != "" {
		cfg.TLS.CA = v
	}
	if v := os.Getenv("CHV_TLS_AUTO_GENERATE"); v != "" {
		cfg.TLS.AutoGenerate = parseBool(v)
	}
	if v := os.Getenv("CHV_TLS_CERT_DIR"); v != "" {
		cfg.TLS.CertDir = v
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
	if v := os.Getenv("CHV_BRIDGE_UPLINK"); v != "" {
		cfg.BridgeUplink = v
	}
	if v := os.Getenv("CHV_BRIDGE_GATEWAY_IP"); v != "" {
		cfg.BridgeGatewayIP = v
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
	// TLS environment overrides
	if v := os.Getenv("CHV_TLS_ENABLED"); v != "" {
		cfg.TLS.Enabled = parseBool(v)
	}
	if v := os.Getenv("CHV_TLS_CLIENT_CERT"); v != "" {
		cfg.TLS.ClientCert = v
	}
	if v := os.Getenv("CHV_TLS_CLIENT_KEY"); v != "" {
		cfg.TLS.ClientKey = v
	}
	if v := os.Getenv("CHV_TLS_CA"); v != "" {
		cfg.TLS.CA = v
	}
	if v := os.Getenv("CHV_TLS_SERVER_NAME"); v != "" {
		cfg.TLS.ServerName = v
	}
	if v := os.Getenv("CHV_TLS_INSECURE_SKIP_VERIFY"); v != "" {
		cfg.TLS.InsecureSkipVerify = parseBool(v)
	}
	if v := os.Getenv("CHV_TLS_CERT"); v != "" {
		cfg.TLS.Cert = v
	}
	if v := os.Getenv("CHV_TLS_KEY"); v != "" {
		cfg.TLS.Key = v
	}
	if v := os.Getenv("CHV_TLS_AUTO_GENERATE"); v != "" {
		cfg.TLS.AutoGenerate = parseBool(v)
	}
	if v := os.Getenv("CHV_TLS_CERT_DIR"); v != "" {
		cfg.TLS.CertDir = v
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

// parseBool parses a string as a boolean value.
func parseBool(s string) bool {
	s = strings.ToLower(strings.TrimSpace(s))
	return s == "true" || s == "1" || s == "yes" || s == "on"
}

// getHostname returns the system hostname or "unknown".
func getHostname() string {
	hostname, err := os.Hostname()
	if err != nil {
		return "unknown"
	}
	return hostname
}
