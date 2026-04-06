package config

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestDefaultControllerConfig(t *testing.T) {
	cfg := DefaultControllerConfig()

	if cfg.HTTPAddr != ":8080" {
		t.Errorf("expected HTTPAddr ':8080', got '%s'", cfg.HTTPAddr)
	}
	if cfg.GRPCAddr != ":9090" {
		t.Errorf("expected GRPCAddr ':9090', got '%s'", cfg.GRPCAddr)
	}
	if cfg.LogLevel != "info" {
		t.Errorf("expected LogLevel 'info', got '%s'", cfg.LogLevel)
	}
	if len(cfg.CORS.AllowedOrigins) == 0 {
		t.Error("expected default CORS origins")
	}
}

func TestLoadControllerConfigFromFile(t *testing.T) {
	// Create temporary config file
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "controller.yaml")

	configContent := `
http_addr: ":9090"
grpc_addr: ":9091"
database_url: "postgres://test@testhost/chv"
log_level: "debug"
cors:
  allowed_origins:
    - "http://example.com"
    - "http://test.com"
  allow_credentials: false
`
	if err := os.WriteFile(configPath, []byte(configContent), 0644); err != nil {
		t.Fatalf("failed to write config file: %v", err)
	}

	cfg, err := LoadControllerConfig(configPath)
	if err != nil {
		t.Fatalf("failed to load config: %v", err)
	}

	if cfg.HTTPAddr != ":9090" {
		t.Errorf("expected HTTPAddr ':9090', got '%s'", cfg.HTTPAddr)
	}
	if cfg.DatabaseURL != "postgres://test@testhost/chv" {
		t.Errorf("expected custom database URL, got '%s'", cfg.DatabaseURL)
	}
	if cfg.LogLevel != "debug" {
		t.Errorf("expected LogLevel 'debug', got '%s'", cfg.LogLevel)
	}
	if len(cfg.CORS.AllowedOrigins) != 2 {
		t.Errorf("expected 2 CORS origins, got %d", len(cfg.CORS.AllowedOrigins))
	}
	if cfg.CORS.AllowCredentials {
		t.Error("expected AllowCredentials to be false")
	}
}

func TestLoadControllerConfigFromEnv(t *testing.T) {
	// Set environment variables
	os.Setenv("CHV_HTTP_ADDR", ":7070")
	os.Setenv("CHV_LOG_LEVEL", "warn")
	os.Setenv("CHV_CORS_ORIGINS", "http://env1.com,http://env2.com")
	defer func() {
		os.Unsetenv("CHV_HTTP_ADDR")
		os.Unsetenv("CHV_LOG_LEVEL")
		os.Unsetenv("CHV_CORS_ORIGINS")
	}()

	cfg, err := LoadControllerConfig("")
	if err != nil {
		t.Fatalf("failed to load config: %v", err)
	}

	if cfg.HTTPAddr != ":7070" {
		t.Errorf("expected HTTPAddr ':7070', got '%s'", cfg.HTTPAddr)
	}
	if cfg.LogLevel != "warn" {
		t.Errorf("expected LogLevel 'warn', got '%s'", cfg.LogLevel)
	}
	if len(cfg.CORS.AllowedOrigins) != 2 {
		t.Errorf("expected 2 CORS origins from env, got %d", len(cfg.CORS.AllowedOrigins))
	}
}

func TestEnvOverridesFile(t *testing.T) {
	// Create temporary config file
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "controller.yaml")

	configContent := `
http_addr: ":9090"
log_level: "debug"
`
	if err := os.WriteFile(configPath, []byte(configContent), 0644); err != nil {
		t.Fatalf("failed to write config file: %v", err)
	}

	// Set environment variable - should override file
	os.Setenv("CHV_LOG_LEVEL", "error")
	defer os.Unsetenv("CHV_LOG_LEVEL")

	cfg, err := LoadControllerConfig(configPath)
	if err != nil {
		t.Fatalf("failed to load config: %v", err)
	}

	// From file
	if cfg.HTTPAddr != ":9090" {
		t.Errorf("expected HTTPAddr ':9090' from file, got '%s'", cfg.HTTPAddr)
	}
	// From env (overrides file)
	if cfg.LogLevel != "error" {
		t.Errorf("expected LogLevel 'error' from env override, got '%s'", cfg.LogLevel)
	}
}

func TestControllerConfigValidate(t *testing.T) {
	tests := []struct {
		name    string
		cfg     *ControllerConfig
		wantErr bool
	}{
		{
			name: "valid config",
			cfg: &ControllerConfig{
				HTTPAddr:    ":8080",
				GRPCAddr:    ":9090",
				DatabaseURL: "postgres://localhost/chv",
			},
			wantErr: false,
		},
		{
			name: "missing HTTP addr",
			cfg: &ControllerConfig{
				HTTPAddr:    "",
				GRPCAddr:    ":9090",
				DatabaseURL: "postgres://localhost/chv",
			},
			wantErr: true,
		},
		{
			name: "missing database URL",
			cfg: &ControllerConfig{
				HTTPAddr:    ":8080",
				GRPCAddr:    ":9090",
				DatabaseURL: "",
			},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.cfg.Validate()
			if (err != nil) != tt.wantErr {
				t.Errorf("Validate() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestParseOrigins(t *testing.T) {
	tests := []struct {
		input    string
		expected []string
	}{
		{
			input:    "http://localhost,http://example.com",
			expected: []string{"http://localhost", "http://example.com"},
		},
		{
			input:    "http://localhost:3000 , http://example.com:8080",
			expected: []string{"http://localhost:3000", "http://example.com:8080"},
		},
		{
			input:    "http://single.com",
			expected: []string{"http://single.com"},
		},
		{
			input:    "",
			expected: nil,
		},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result := parseOrigins(tt.input)
			if len(result) != len(tt.expected) {
				t.Errorf("expected %d origins, got %d", len(tt.expected), len(result))
				return
			}
			for i, v := range result {
				if v != tt.expected[i] {
					t.Errorf("origin[%d]: expected '%s', got '%s'", i, tt.expected[i], v)
				}
			}
		})
	}
}

func TestDurationParsing(t *testing.T) {
	os.Setenv("CHV_TOKEN_DURATION", "2h30m")
	defer os.Unsetenv("CHV_TOKEN_DURATION")

	cfg, err := LoadControllerConfig("")
	if err != nil {
		t.Fatalf("failed to load config: %v", err)
	}

	expected := 2*time.Hour + 30*time.Minute
	if cfg.TokenDuration != expected {
		t.Errorf("expected duration %v, got %v", expected, cfg.TokenDuration)
	}
}

func TestAgentConfigLoad(t *testing.T) {
	// Create temporary config file
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "agent.yaml")

	configContent := `
node_id: "test-node-1"
listen_addr: ":9090"
controller_addr: "controller.example.com:9090"
cloud_hypervisor: "/opt/chv/cloud-hypervisor"
bridge_name: "br1"
heartbeat_interval: "10s"
`
	if err := os.WriteFile(configPath, []byte(configContent), 0644); err != nil {
		t.Fatalf("failed to write config file: %v", err)
	}

	cfg, err := LoadAgentConfig(configPath)
	if err != nil {
		t.Fatalf("failed to load agent config: %v", err)
	}

	if cfg.NodeID != "test-node-1" {
		t.Errorf("expected NodeID 'test-node-1', got '%s'", cfg.NodeID)
	}
	if cfg.BridgeName != "br1" {
		t.Errorf("expected BridgeName 'br1', got '%s'", cfg.BridgeName)
	}
	if cfg.HeartbeatInterval != 10*time.Second {
		t.Errorf("expected HeartbeatInterval 10s, got %v", cfg.HeartbeatInterval)
	}
}
