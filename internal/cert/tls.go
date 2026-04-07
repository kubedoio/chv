// Package cert provides TLS configuration helpers for CHV services.
package cert

import (
	"crypto/tls"
	"crypto/x509"
	"fmt"
	"net"
	"os"

	"github.com/chv/chv/internal/config"
	"google.golang.org/grpc/credentials"
)

// ServerTLSConfig creates a TLS configuration for the server (controller or agent).
// It supports both TLS and mTLS modes based on the provided configuration.
func ServerTLSConfig(cfg config.TLSConfig) (*tls.Config, error) {
	if !cfg.Enabled {
		return nil, fmt.Errorf("TLS is not enabled")
	}

	// If auto-generate is enabled, generate certificates
	if cfg.AutoGenerate {
		return autoGenerateServerTLS(cfg)
	}

	// Load server certificate
	if cfg.Cert == "" || cfg.Key == "" {
		return nil, fmt.Errorf("server certificate and key paths are required when auto-generate is disabled")
	}

	cert, err := tls.LoadX509KeyPair(cfg.Cert, cfg.Key)
	if err != nil {
		return nil, fmt.Errorf("failed to load server certificate: %w", err)
	}

	tlsConfig := &tls.Config{
		Certificates: []tls.Certificate{cert},
		MinVersion:   tls.VersionTLS13,
		CipherSuites: []uint16{
			tls.TLS_AES_256_GCM_SHA384,
			tls.TLS_CHACHA20_POLY1305_SHA256,
			tls.TLS_AES_128_GCM_SHA256,
		},
		CurvePreferences: []tls.CurveID{
			tls.X25519,
			tls.CurveP256,
		},
		PreferServerCipherSuites: true,
	}

	// Configure mTLS if CA is provided
	if cfg.CA != "" {
		caCert, err := os.ReadFile(cfg.CA)
		if err != nil {
			return nil, fmt.Errorf("failed to load CA certificate: %w", err)
		}

		caCertPool := x509.NewCertPool()
		if !caCertPool.AppendCertsFromPEM(caCert) {
			return nil, fmt.Errorf("failed to parse CA certificate")
		}

		tlsConfig.ClientCAs = caCertPool
		tlsConfig.ClientAuth = tls.RequireAndVerifyClientCert
	}

	return tlsConfig, nil
}

// ClientTLSConfig creates a TLS configuration for the client (agent connecting to controller).
// It supports mTLS when client certificates are provided.
func ClientTLSConfig(cfg config.TLSConfig) (*tls.Config, error) {
	if !cfg.Enabled {
		return nil, fmt.Errorf("TLS is not enabled")
	}

	// If auto-generate is enabled, generate or load certificates
	if cfg.AutoGenerate {
		return autoGenerateClientTLS(cfg)
	}

	tlsConfig := &tls.Config{
		MinVersion: tls.VersionTLS13,
		CipherSuites: []uint16{
			tls.TLS_AES_256_GCM_SHA384,
			tls.TLS_CHACHA20_POLY1305_SHA256,
			tls.TLS_AES_128_GCM_SHA256,
		},
		CurvePreferences: []tls.CurveID{
			tls.X25519,
			tls.CurveP256,
		},
		InsecureSkipVerify: cfg.InsecureSkipVerify,
	}

	// Set server name for SNI (required for proper certificate verification)
	if cfg.ServerName != "" {
		tlsConfig.ServerName = cfg.ServerName
	}

	// Load CA certificate for server verification
	if cfg.CA != "" {
		caCert, err := os.ReadFile(cfg.CA)
		if err != nil {
			return nil, fmt.Errorf("failed to load CA certificate: %w", err)
		}

		caCertPool := x509.NewCertPool()
		if !caCertPool.AppendCertsFromPEM(caCert) {
			return nil, fmt.Errorf("failed to parse CA certificate")
		}

		tlsConfig.RootCAs = caCertPool
	} else if !cfg.InsecureSkipVerify {
		return nil, fmt.Errorf("CA certificate is required for secure connections (or set InsecureSkipVerify for development only)")
	}

	// Load client certificate for mTLS
	if cfg.ClientCert != "" && cfg.ClientKey != "" {
		cert, err := tls.LoadX509KeyPair(cfg.ClientCert, cfg.ClientKey)
		if err != nil {
			return nil, fmt.Errorf("failed to load client certificate: %w", err)
		}
		tlsConfig.Certificates = []tls.Certificate{cert}
	}

	return tlsConfig, nil
}

// GRPCServerCredentials creates gRPC transport credentials for the server.
func GRPCServerCredentials(cfg config.TLSConfig) (credentials.TransportCredentials, error) {
	tlsConfig, err := ServerTLSConfig(cfg)
	if err != nil {
		return nil, err
	}
	return credentials.NewTLS(tlsConfig), nil
}

// GRPCClientCredentials creates gRPC transport credentials for the client.
func GRPCClientCredentials(cfg config.TLSConfig) (credentials.TransportCredentials, error) {
	tlsConfig, err := ClientTLSConfig(cfg)
	if err != nil {
		return nil, err
	}
	return credentials.NewTLS(tlsConfig), nil
}

// HTTPSTLSConfig creates a TLS configuration for HTTPS servers.
// This is similar to ServerTLSConfig but with HTTP-specific optimizations.
func HTTPSTLSConfig(cfg config.TLSConfig) (*tls.Config, error) {
	return ServerTLSConfig(cfg)
}

// autoGenerateServerTLS auto-generates certificates for server TLS.
func autoGenerateServerTLS(cfg config.TLSConfig) (*tls.Config, error) {
	certDir := cfg.CertDir
	if certDir == "" {
		certDir = DefaultCertDir
	}

	mgr := NewManager(certDir)

	// Try to load existing CA
	if err := mgr.LoadCA(); err != nil {
		// Generate new CA if loading fails
		if err := mgr.GenerateCA(); err != nil {
			return nil, fmt.Errorf("failed to generate CA: %w", err)
		}
	}

	// Generate server certificate
	serverName := "chv-server"
	dnsNames := []string{"localhost", serverName}
	ipAddresses := []net.IP(nil) // Will auto-detect

	serverCert, err := mgr.GenerateServerCert(serverName, dnsNames, ipAddresses)
	if err != nil {
		return nil, fmt.Errorf("failed to generate server certificate: %w", err)
	}

	// Get CA certificate for mTLS
	caCert := mgr.GetCACert()
	caCertPool := x509.NewCertPool()
	caCertPool.AppendCertsFromPEM(caCert)

	tlsConfig := &tls.Config{
		Certificates: []tls.Certificate{serverCert},
		ClientCAs:    caCertPool,
		ClientAuth:   tls.RequireAndVerifyClientCert,
		MinVersion:   tls.VersionTLS13,
		CipherSuites: []uint16{
			tls.TLS_AES_256_GCM_SHA384,
			tls.TLS_CHACHA20_POLY1305_SHA256,
			tls.TLS_AES_128_GCM_SHA256,
		},
		CurvePreferences: []tls.CurveID{
			tls.X25519,
			tls.CurveP256,
		},
		PreferServerCipherSuites: true,
	}

	return tlsConfig, nil
}

// autoGenerateClientTLS auto-generates certificates for client TLS.
func autoGenerateClientTLS(cfg config.TLSConfig) (*tls.Config, error) {
	certDir := cfg.CertDir
	if certDir == "" {
		certDir = DefaultCertDir
	}

	mgr := NewManager(certDir)

	// Try to load existing CA
	if err := mgr.LoadCA(); err != nil {
		return nil, fmt.Errorf("CA certificate not found, please generate certificates first: %w", err)
	}

	// Load or generate client certificate
	var clientCert tls.Certificate
	clientCertPath := cfg.ClientCert
	clientKeyPath := cfg.ClientKey

	if clientCertPath != "" && clientKeyPath != "" {
		// Load existing client certificate
		cert, err := tls.LoadX509KeyPair(clientCertPath, clientKeyPath)
		if err != nil {
			return nil, fmt.Errorf("failed to load client certificate: %w", err)
		}
		clientCert = cert
	} else {
		// Generate new client certificate
		cert, err := mgr.GenerateClientCert("chv-client")
		if err != nil {
			return nil, fmt.Errorf("failed to generate client certificate: %w", err)
		}
		clientCert = cert
	}

	// Load CA certificate
	caCert := mgr.GetCACert()
	caCertPool := x509.NewCertPool()
	caCertPool.AppendCertsFromPEM(caCert)

	tlsConfig := &tls.Config{
		Certificates:       []tls.Certificate{clientCert},
		RootCAs:            caCertPool,
		ServerName:         cfg.ServerName,
		InsecureSkipVerify: cfg.InsecureSkipVerify,
		MinVersion:         tls.VersionTLS13,
		CipherSuites: []uint16{
			tls.TLS_AES_256_GCM_SHA384,
			tls.TLS_CHACHA20_POLY1305_SHA256,
			tls.TLS_AES_128_GCM_SHA256,
		},
		CurvePreferences: []tls.CurveID{
			tls.X25519,
			tls.CurveP256,
		},
	}

	return tlsConfig, nil
}

// ValidateTLSConfig validates the TLS configuration.
func ValidateTLSConfig(cfg config.TLSConfig, isServer bool) error {
	if !cfg.Enabled {
		return nil // TLS is disabled, nothing to validate
	}

	if cfg.AutoGenerate {
		return nil // Auto-generation handles its own validation
	}

	// Validate server certificate paths for servers
	if isServer {
		if cfg.Cert == "" {
			return fmt.Errorf("server certificate path is required when TLS is enabled")
		}
		if cfg.Key == "" {
			return fmt.Errorf("server key path is required when TLS is enabled")
		}
		if _, err := os.Stat(cfg.Cert); err != nil {
			return fmt.Errorf("server certificate file not found: %w", err)
		}
		if _, err := os.Stat(cfg.Key); err != nil {
			return fmt.Errorf("server key file not found: %w", err)
		}
	}

	// Validate CA certificate for mTLS
	if cfg.CA != "" {
		if _, err := os.Stat(cfg.CA); err != nil {
			return fmt.Errorf("CA certificate file not found: %w", err)
		}
	}

	// Validate client certificate for mTLS
	if cfg.ClientCert != "" {
		if cfg.ClientKey == "" {
			return fmt.Errorf("client key path is required when client certificate is provided")
		}
		if _, err := os.Stat(cfg.ClientCert); err != nil {
			return fmt.Errorf("client certificate file not found: %w", err)
		}
		if _, err := os.Stat(cfg.ClientKey); err != nil {
			return fmt.Errorf("client key file not found: %w", err)
		}
	}

	// Validate server name for clients
	if !isServer && cfg.ServerName == "" && !cfg.InsecureSkipVerify {
		// This is a warning condition, not necessarily an error
		// Some deployments may use IP addresses without DNS names
	}

	return nil
}

// IsTLSEnabled checks if TLS is enabled and properly configured.
func IsTLSEnabled(cfg config.TLSConfig) bool {
	if !cfg.Enabled {
		return false
	}

	if cfg.AutoGenerate {
		return true
	}

	// Check if certificate files exist
	if cfg.Cert != "" && cfg.Key != "" {
		if _, err := os.Stat(cfg.Cert); err == nil {
			if _, err := os.Stat(cfg.Key); err == nil {
				return true
			}
		}
	}

	return false
}
