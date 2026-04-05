// Package config provides configuration loading and management.
package config

import (
	"fmt"
	"os"
	"strconv"
	"strings"

	"gopkg.in/yaml.v3"
)

// Loader handles configuration loading from files and environment variables.
type Loader struct {
	prefix string
}

// NewLoader creates a new config loader.
func NewLoader(envPrefix string) *Loader {
	return &Loader{prefix: envPrefix}
}

// LoadFromFile loads configuration from a YAML file.
func (l *Loader) LoadFromFile(path string, target interface{}) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("failed to read config file: %w", err)
	}

	if err := yaml.Unmarshal(data, target); err != nil {
		return fmt.Errorf("failed to parse config file: %w", err)
	}

	return nil
}

// LoadFromEnv loads configuration from environment variables.
func (l *Loader) LoadFromEnv(target interface{}) error {
	// This is a simplified implementation
	// In production, use a library like envconfig or viper
	return nil
}

// GetEnv gets an environment variable with a prefix.
func (l *Loader) GetEnv(key, defaultValue string) string {
	fullKey := l.prefix + key
	if value := os.Getenv(fullKey); value != "" {
		return value
	}
	return defaultValue
}

// GetEnvInt gets an integer environment variable.
func (l *Loader) GetEnvInt(key string, defaultValue int) int {
	value := l.GetEnv(key, "")
	if value == "" {
		return defaultValue
	}
	
	i, err := strconv.Atoi(value)
	if err != nil {
		return defaultValue
	}
	return i
}

// GetEnvBool gets a boolean environment variable.
func (l *Loader) GetEnvBool(key string, defaultValue bool) bool {
	value := l.GetEnv(key, "")
	if value == "" {
		return defaultValue
	}
	
	b, err := strconv.ParseBool(value)
	if err != nil {
		return defaultValue
	}
	return b
}

// Load loads configuration from file and environment.
func (l *Loader) Load(path string, target interface{}) error {
	// First load from file if it exists
	if _, err := os.Stat(path); err == nil {
		if err := l.LoadFromFile(path, target); err != nil {
			return err
		}
	}

	// Then override with environment variables
	if err := l.LoadFromEnv(target); err != nil {
		return err
	}

	return nil
}

// ControllerConfig represents controller configuration.
type ControllerConfig struct {
	DatabaseURL  string `yaml:"database_url" env:"DATABASE_URL"`
	HTTPAddr     string `yaml:"http_addr" env:"HTTP_ADDR"`
	GRPCAddr     string `yaml:"grpc_addr" env:"GRPC_ADDR"`
	LogLevel     string `yaml:"log_level" env:"LOG_LEVEL"`
	ImageDir     string `yaml:"image_dir" env:"IMAGE_DIR"`
	VolumeDir    string `yaml:"volume_dir" env:"VOLUME_DIR"`
}

// LoadControllerConfig loads controller configuration.
func LoadControllerConfig(path string) (*ControllerConfig, error) {
	loader := NewLoader("CHV_")
	
	config := &ControllerConfig{
		DatabaseURL:  "postgres://chv:chv@localhost:5432/chv?sslmode=disable",
		HTTPAddr:     ":8080",
		GRPCAddr:     ":9090",
		LogLevel:     "info",
		ImageDir:     "/var/lib/chv/images",
		VolumeDir:    "/var/lib/chv/volumes",
	}

	// Load from file if provided
	if path != "" {
		if err := loader.LoadFromFile(path, config); err != nil && !os.IsNotExist(err) {
			return nil, err
		}
	}

	// Override with environment variables
	if v := loader.GetEnv("DATABASE_URL", ""); v != "" {
		config.DatabaseURL = v
	}
	if v := loader.GetEnv("HTTP_ADDR", ""); v != "" {
		config.HTTPAddr = v
	}
	if v := loader.GetEnv("GRPC_ADDR", ""); v != "" {
		config.GRPCAddr = v
	}
	if v := loader.GetEnv("LOG_LEVEL", ""); v != "" {
		config.LogLevel = v
	}
	if v := loader.GetEnv("IMAGE_DIR", ""); v != "" {
		config.ImageDir = v
	}
	if v := loader.GetEnv("VOLUME_DIR", ""); v != "" {
		config.VolumeDir = v
	}

	return config, nil
}

// AgentConfig represents agent configuration.
type AgentConfig struct {
	NodeID           string `yaml:"node_id" env:"NODE_ID"`
	ControllerAddr   string `yaml:"controller_addr" env:"CONTROLLER_ADDR"`
	ListenAddr       string `yaml:"listen_addr" env:"LISTEN_ADDR"`
	DataDir          string `yaml:"data_dir" env:"DATA_DIR"`
	ImageDir         string `yaml:"image_dir" env:"IMAGE_DIR"`
	VolumeDir        string `yaml:"volume_dir" env:"VOLUME_DIR"`
	CloudHypervisor  string `yaml:"cloud_hypervisor_path" env:"CLOUD_HV_PATH"`
	LogLevel         string `yaml:"log_level" env:"LOG_LEVEL"`
}

// LoadAgentConfig loads agent configuration.
func LoadAgentConfig(path string) (*AgentConfig, error) {
	loader := NewLoader("CHV_")
	
	config := &AgentConfig{
		NodeID:          "",
		ControllerAddr:  "localhost:9090",
		ListenAddr:      ":9091",
		DataDir:         "/var/lib/chv",
		ImageDir:        "/var/lib/chv/images",
		VolumeDir:       "/var/lib/chv/volumes",
		CloudHypervisor: "/usr/local/bin/cloud-hypervisor",
		LogLevel:        "info",
	}

	// Load from file if provided
	if path != "" {
		if err := loader.LoadFromFile(path, config); err != nil && !os.IsNotExist(err) {
			return nil, err
		}
	}

	// Override with environment variables
	if v := loader.GetEnv("NODE_ID", ""); v != "" {
		config.NodeID = v
	}
	if v := loader.GetEnv("CONTROLLER_ADDR", ""); v != "" {
		config.ControllerAddr = v
	}
	if v := loader.GetEnv("LISTEN_ADDR", ""); v != "" {
		config.ListenAddr = v
	}
	if v := loader.GetEnv("DATA_DIR", ""); v != "" {
		config.DataDir = v
	}
	if v := loader.GetEnv("IMAGE_DIR", ""); v != "" {
		config.ImageDir = v
	}
	if v := loader.GetEnv("VOLUME_DIR", ""); v != "" {
		config.VolumeDir = v
	}
	if v := loader.GetEnv("CLOUD_HV_PATH", ""); v != "" {
		config.CloudHypervisor = v
	}
	if v := loader.GetEnv("LOG_LEVEL", ""); v != "" {
		config.LogLevel = v
	}

	return config, nil
}

// ExpandPath expands ~ to home directory and environment variables.
func ExpandPath(path string) string {
	if strings.HasPrefix(path, "~/") {
		home, _ := os.UserHomeDir()
		path = home + path[1:]
	}
	return os.ExpandEnv(path)
}
