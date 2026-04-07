package cert

import (
	"path/filepath"
	"testing"

	"github.com/chv/chv/internal/config"
)

func TestServerTLSConfig_Disabled(t *testing.T) {
	cfg := config.TLSConfig{
		Enabled: false,
	}

	_, err := ServerTLSConfig(cfg)
	if err == nil {
		t.Error("expected error when TLS is disabled")
	}
}

func TestServerTLSConfig_AutoGenerate(t *testing.T) {
	tmpDir := t.TempDir()
	cfg := config.TLSConfig{
		Enabled:      true,
		AutoGenerate: true,
		CertDir:      tmpDir,
	}

	tlsConfig, err := ServerTLSConfig(cfg)
	if err != nil {
		t.Fatalf("failed to create server TLS config: %v", err)
	}

	if tlsConfig == nil {
		t.Fatal("expected non-nil TLS config")
	}

	if tlsConfig.MinVersion != 0x0304 { // TLS 1.3
		t.Error("expected TLS 1.3")
	}

	if tlsConfig.ClientAuth == 0 {
		t.Error("expected mTLS to be enabled")
	}
}

func TestServerTLSConfig_WithCertificates(t *testing.T) {
	tmpDir := t.TempDir()

	// Generate test certificates
	mgr := NewManager(tmpDir)
	if err := mgr.GenerateCA(); err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	// Generate and save server certificate
	if err := mgr.GenerateCertsForController("test-server", []string{"localhost"}, nil); err != nil {
		t.Fatalf("failed to generate controller certs: %v", err)
	}

	cfg := config.TLSConfig{
		Enabled: true,
		Cert:    filepath.Join(tmpDir, "controller.crt"),
		Key:     filepath.Join(tmpDir, "controller.key"),
	}

	tlsConfig, err := ServerTLSConfig(cfg)
	if err != nil {
		t.Fatalf("failed to create server TLS config: %v", err)
	}

	if len(tlsConfig.Certificates) == 0 {
		t.Error("expected at least one certificate")
	}
}

func TestServerTLSConfig_WithMTLS(t *testing.T) {
	tmpDir := t.TempDir()

	// Generate test certificates
	mgr := NewManager(tmpDir)
	if err := mgr.GenerateCA(); err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	if err := mgr.GenerateCertsForController("test-server", []string{"localhost"}, nil); err != nil {
		t.Fatalf("failed to generate controller certs: %v", err)
	}

	cfg := config.TLSConfig{
		Enabled: true,
		Cert:    filepath.Join(tmpDir, "controller.crt"),
		Key:     filepath.Join(tmpDir, "controller.key"),
		CA:      filepath.Join(tmpDir, "ca.crt"),
	}

	tlsConfig, err := ServerTLSConfig(cfg)
	if err != nil {
		t.Fatalf("failed to create server TLS config: %v", err)
	}

	if tlsConfig.ClientCAs == nil {
		t.Error("expected client CA pool to be set")
	}

	if tlsConfig.ClientAuth == 0 {
		t.Error("expected client auth to be required")
	}
}

func TestServerTLSConfig_MissingCertFiles(t *testing.T) {
	cfg := config.TLSConfig{
		Enabled: true,
		Cert:    "/nonexistent/server.crt",
		Key:     "/nonexistent/server.key",
	}

	_, err := ServerTLSConfig(cfg)
	if err == nil {
		t.Error("expected error for missing certificate files")
	}
}

func TestClientTLSConfig_Disabled(t *testing.T) {
	cfg := config.TLSConfig{
		Enabled: false,
	}

	_, err := ClientTLSConfig(cfg)
	if err == nil {
		t.Error("expected error when TLS is disabled")
	}
}

func TestClientTLSConfig_WithCA(t *testing.T) {
	tmpDir := t.TempDir()

	// Generate test CA
	mgr := NewManager(tmpDir)
	if err := mgr.GenerateCA(); err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	cfg := config.TLSConfig{
		Enabled:    true,
		CA:         filepath.Join(tmpDir, "ca.crt"),
		ServerName: "test-server",
	}

	tlsConfig, err := ClientTLSConfig(cfg)
	if err != nil {
		t.Fatalf("failed to create client TLS config: %v", err)
	}

	if tlsConfig.RootCAs == nil {
		t.Error("expected root CA pool to be set")
	}

	if tlsConfig.ServerName != "test-server" {
		t.Errorf("expected server name 'test-server', got %s", tlsConfig.ServerName)
	}
}

func TestClientTLSConfig_WithMTLS(t *testing.T) {
	tmpDir := t.TempDir()

	// Generate test certificates
	mgr := NewManager(tmpDir)
	if err := mgr.GenerateCA(); err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	if err := mgr.GenerateCertsForAgent("agent-1", []string{"localhost"}, nil); err != nil {
		t.Fatalf("failed to generate agent certs: %v", err)
	}

	cfg := config.TLSConfig{
		Enabled:    true,
		CA:         filepath.Join(tmpDir, "ca.crt"),
		ServerName: "test-server",
		ClientCert: filepath.Join(tmpDir, "agent-agent-1-client.crt"),
		ClientKey:  filepath.Join(tmpDir, "agent-agent-1-client.key"),
	}

	tlsConfig, err := ClientTLSConfig(cfg)
	if err != nil {
		t.Fatalf("failed to create client TLS config: %v", err)
	}

	if len(tlsConfig.Certificates) == 0 {
		t.Error("expected client certificate to be set")
	}
}

func TestClientTLSConfig_InsecureSkipVerify(t *testing.T) {
	cfg := config.TLSConfig{
		Enabled:            true,
		InsecureSkipVerify: true,
	}

	tlsConfig, err := ClientTLSConfig(cfg)
	if err != nil {
		t.Fatalf("failed to create client TLS config: %v", err)
	}

	if !tlsConfig.InsecureSkipVerify {
		t.Error("expected InsecureSkipVerify to be true")
	}
}

func TestGRPCServerCredentials(t *testing.T) {
	tmpDir := t.TempDir()

	// Generate test certificates
	mgr := NewManager(tmpDir)
	if err := mgr.GenerateCA(); err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	if err := mgr.GenerateCertsForController("test-server", []string{"localhost"}, nil); err != nil {
		t.Fatalf("failed to generate controller certs: %v", err)
	}

	cfg := config.TLSConfig{
		Enabled: true,
		Cert:    filepath.Join(tmpDir, "controller.crt"),
		Key:     filepath.Join(tmpDir, "controller.key"),
	}

	creds, err := GRPCServerCredentials(cfg)
	if err != nil {
		t.Fatalf("failed to create gRPC server credentials: %v", err)
	}

	if creds == nil {
		t.Error("expected non-nil credentials")
	}
}

func TestGRPCClientCredentials(t *testing.T) {
	tmpDir := t.TempDir()

	// Generate test CA
	mgr := NewManager(tmpDir)
	if err := mgr.GenerateCA(); err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	cfg := config.TLSConfig{
		Enabled:    true,
		CA:         filepath.Join(tmpDir, "ca.crt"),
		ServerName: "test-server",
	}

	creds, err := GRPCClientCredentials(cfg)
	if err != nil {
		t.Fatalf("failed to create gRPC client credentials: %v", err)
	}

	if creds == nil {
		t.Error("expected non-nil credentials")
	}
}

func TestHTTPSTLSConfig(t *testing.T) {
	tmpDir := t.TempDir()

	// Generate test certificates
	mgr := NewManager(tmpDir)
	if err := mgr.GenerateCA(); err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	if err := mgr.GenerateCertsForController("test-server", []string{"localhost"}, nil); err != nil {
		t.Fatalf("failed to generate controller certs: %v", err)
	}

	cfg := config.TLSConfig{
		Enabled: true,
		Cert:    filepath.Join(tmpDir, "controller.crt"),
		Key:     filepath.Join(tmpDir, "controller.key"),
	}

	tlsConfig, err := HTTPSTLSConfig(cfg)
	if err != nil {
		t.Fatalf("failed to create HTTPS TLS config: %v", err)
	}

	if tlsConfig == nil {
		t.Fatal("expected non-nil TLS config")
	}
}

func TestValidateTLSConfig_Server(t *testing.T) {
	tests := []struct {
		name     string
		cfg      config.TLSConfig
		isServer bool
		wantErr  bool
	}{
		{
			name:     "disabled",
			cfg:      config.TLSConfig{Enabled: false},
			isServer: true,
			wantErr:  false,
		},
		{
			name: "auto-generate",
			cfg: config.TLSConfig{
				Enabled:      true,
				AutoGenerate: true,
			},
			isServer: true,
			wantErr:  false,
		},
		{
			name: "missing cert",
			cfg: config.TLSConfig{
				Enabled: true,
				Cert:    "",
				Key:     "/path/to/key",
			},
			isServer: true,
			wantErr:  true,
		},
		{
			name: "missing key",
			cfg: config.TLSConfig{
				Enabled: true,
				Cert:    "/path/to/cert",
				Key:     "",
			},
			isServer: true,
			wantErr:  true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateTLSConfig(tt.cfg, tt.isServer)
			if (err != nil) != tt.wantErr {
				t.Errorf("ValidateTLSConfig() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestIsTLSEnabled(t *testing.T) {
	tests := []struct {
		name string
		cfg  config.TLSConfig
		want bool
	}{
		{
			name: "disabled",
			cfg:  config.TLSConfig{Enabled: false},
			want: false,
		},
		{
			name: "enabled but no cert",
			cfg:  config.TLSConfig{Enabled: true},
			want: false,
		},
		{
			name: "auto-generate",
			cfg: config.TLSConfig{
				Enabled:      true,
				AutoGenerate: true,
			},
			want: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := IsTLSEnabled(tt.cfg); got != tt.want {
				t.Errorf("IsTLSEnabled() = %v, want %v", got, tt.want)
			}
		})
	}
}
