// Package cert provides certificate management for TLS/mTLS encryption.
package cert

import (
	"crypto"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/rsa"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"fmt"
	"math/big"
	"net"
	"os"
	"path/filepath"
	"sync"
	"time"
)

const (
	// DefaultCertDir is the default directory for certificates
	DefaultCertDir = "/etc/chv/certs"
	// DefaultKeySize is the default RSA key size
	DefaultKeySize = 4096
	// DefaultCertValidity is the default certificate validity period
	DefaultCertValidity = 365 * 24 * time.Hour // 1 year
	// DefaultCAValidity is the default CA certificate validity period
	DefaultCAValidity = 10 * 365 * 24 * time.Hour // 10 years
)

// Manager handles certificate generation and management.
type Manager struct {
	caCert  *x509.Certificate
	caKey   crypto.PrivateKey
	certDir string
	mu      sync.RWMutex
}

// NewManager creates a new certificate manager.
func NewManager(certDir string) *Manager {
	if certDir == "" {
		certDir = DefaultCertDir
	}
	return &Manager{
		certDir: certDir,
	}
}

// NewManagerWithCA creates a new certificate manager with an existing CA.
func NewManagerWithCA(certDir string, caCert *x509.Certificate, caKey crypto.PrivateKey) *Manager {
	m := NewManager(certDir)
	m.caCert = caCert
	m.caKey = caKey
	return m
}

// LoadCA loads an existing CA certificate and key from files.
func (m *Manager) LoadCA() error {
	m.mu.Lock()
	defer m.mu.Unlock()

	caCertPath := filepath.Join(m.certDir, "ca.crt")
	caKeyPath := filepath.Join(m.certDir, "ca.key")

	// Load CA certificate
	caCertPEM, err := os.ReadFile(caCertPath)
	if err != nil {
		return fmt.Errorf("failed to read CA certificate: %w", err)
	}

	block, _ := pem.Decode(caCertPEM)
	if block == nil {
		return fmt.Errorf("failed to decode CA certificate PEM")
	}

	caCert, err := x509.ParseCertificate(block.Bytes)
	if err != nil {
		return fmt.Errorf("failed to parse CA certificate: %w", err)
	}

	// Load CA key
	caKeyPEM, err := os.ReadFile(caKeyPath)
	if err != nil {
		return fmt.Errorf("failed to read CA key: %w", err)
	}

	block, _ = pem.Decode(caKeyPEM)
	if block == nil {
		return fmt.Errorf("failed to decode CA key PEM")
	}

	var caKey crypto.PrivateKey
	switch block.Type {
	case "RSA PRIVATE KEY":
		caKey, err = x509.ParsePKCS1PrivateKey(block.Bytes)
	case "EC PRIVATE KEY":
		caKey, err = x509.ParseECPrivateKey(block.Bytes)
	case "PRIVATE KEY":
		caKey, err = x509.ParsePKCS8PrivateKey(block.Bytes)
	default:
		return fmt.Errorf("unsupported key type: %s", block.Type)
	}
	if err != nil {
		return fmt.Errorf("failed to parse CA key: %w", err)
	}

	m.caCert = caCert
	m.caKey = caKey
	return nil
}

// GenerateCA generates a new Certificate Authority.
func (m *Manager) GenerateCA(opts ...CAOption) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	options := &caOptions{
		organization: "CHV Platform",
		validity:     DefaultCAValidity,
		keySize:      DefaultKeySize,
	}
	for _, opt := range opts {
		opt(options)
	}

	// Generate RSA key pair for CA
	caKey, err := rsa.GenerateKey(rand.Reader, options.keySize)
	if err != nil {
		return fmt.Errorf("failed to generate CA key: %w", err)
	}

	// Create CA certificate template
	caTemplate := &x509.Certificate{
		SerialNumber: big.NewInt(1),
		Subject: pkix.Name{
			Organization:       []string{options.organization},
			OrganizationalUnit: []string{"Certificate Authority"},
			CommonName:         "CHV CA",
		},
		NotBefore:             time.Now().Add(-time.Hour), // Valid from 1 hour ago to avoid clock skew issues
		NotAfter:              time.Now().Add(options.validity),
		IsCA:                  true,
		KeyUsage:              x509.KeyUsageKeyEncipherment | x509.KeyUsageDigitalSignature | x509.KeyUsageCertSign | x509.KeyUsageCRLSign,
		ExtKeyUsage:           []x509.ExtKeyUsage{x509.ExtKeyUsageAny},
		BasicConstraintsValid: true,
		MaxPathLen:            0,
		MaxPathLenZero:        true,
	}

	// Self-sign the CA certificate
	caCertBytes, err := x509.CreateCertificate(rand.Reader, caTemplate, caTemplate, &caKey.PublicKey, caKey)
	if err != nil {
		return fmt.Errorf("failed to create CA certificate: %w", err)
	}

	// Parse the generated certificate
	caCert, err := x509.ParseCertificate(caCertBytes)
	if err != nil {
		return fmt.Errorf("failed to parse generated CA certificate: %w", err)
	}

	// Ensure cert directory exists
	if err := os.MkdirAll(m.certDir, 0750); err != nil {
		return fmt.Errorf("failed to create cert directory: %w", err)
	}

	// Save CA certificate
	caCertPath := filepath.Join(m.certDir, "ca.crt")
	if err := m.savePEM(caCertPath, "CERTIFICATE", caCertBytes, 0644); err != nil {
		return fmt.Errorf("failed to save CA certificate: %w", err)
	}

	// Save CA key with restricted permissions
	caKeyPath := filepath.Join(m.certDir, "ca.key")
	caKeyBytes := x509.MarshalPKCS1PrivateKey(caKey)
	if err := m.savePEM(caKeyPath, "RSA PRIVATE KEY", caKeyBytes, 0600); err != nil {
		return fmt.Errorf("failed to save CA key: %w", err)
	}

	m.caCert = caCert
	m.caKey = caKey
	return nil
}

// GenerateServerCert generates a server certificate signed by the CA.
func (m *Manager) GenerateServerCert(commonName string, dnsNames []string, ipAddresses []net.IP, opts ...CertOption) (tls.Certificate, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	if m.caCert == nil || m.caKey == nil {
		return tls.Certificate{}, fmt.Errorf("CA not initialized, call GenerateCA or LoadCA first")
	}

	options := &certOptions{
		organization: "CHV Platform",
		validity:     DefaultCertValidity,
		keySize:      DefaultKeySize,
	}
	for _, opt := range opts {
		opt(options)
	}

	// Generate server key pair
	certKey, err := rsa.GenerateKey(rand.Reader, options.keySize)
	if err != nil {
		return tls.Certificate{}, fmt.Errorf("failed to generate server key: %w", err)
	}

	// Create server certificate template
	serialNumber, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	if err != nil {
		return tls.Certificate{}, fmt.Errorf("failed to generate serial number: %w", err)
	}

	certTemplate := &x509.Certificate{
		SerialNumber: serialNumber,
		Subject: pkix.Name{
			Organization:       []string{options.organization},
			OrganizationalUnit: []string{"Server"},
			CommonName:         commonName,
		},
		DNSNames:    dnsNames,
		IPAddresses: ipAddresses,
		NotBefore:   time.Now().Add(-time.Hour),
		NotAfter:    time.Now().Add(options.validity),
		KeyUsage:    x509.KeyUsageKeyEncipherment | x509.KeyUsageDigitalSignature,
		ExtKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
	}

	// Sign the server certificate with CA
	certBytes, err := x509.CreateCertificate(rand.Reader, certTemplate, m.caCert, &certKey.PublicKey, m.caKey)
	if err != nil {
		return tls.Certificate{}, fmt.Errorf("failed to create server certificate: %w", err)
	}

	// Create TLS certificate
	certPEM := pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: certBytes})
	keyPEM := pem.EncodeToMemory(&pem.Block{Type: "RSA PRIVATE KEY", Bytes: x509.MarshalPKCS1PrivateKey(certKey)})

	return tls.X509KeyPair(certPEM, keyPEM)
}

// GenerateClientCert generates a client certificate for mTLS.
func (m *Manager) GenerateClientCert(commonName string, opts ...CertOption) (tls.Certificate, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	if m.caCert == nil || m.caKey == nil {
		return tls.Certificate{}, fmt.Errorf("CA not initialized, call GenerateCA or LoadCA first")
	}

	options := &certOptions{
		organization: "CHV Platform",
		validity:     DefaultCertValidity,
		keySize:      DefaultKeySize,
	}
	for _, opt := range opts {
		opt(options)
	}

	// Generate client key pair
	certKey, err := rsa.GenerateKey(rand.Reader, options.keySize)
	if err != nil {
		return tls.Certificate{}, fmt.Errorf("failed to generate client key: %w", err)
	}

	// Create client certificate template
	serialNumber, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	if err != nil {
		return tls.Certificate{}, fmt.Errorf("failed to generate serial number: %w", err)
	}

	certTemplate := &x509.Certificate{
		SerialNumber: serialNumber,
		Subject: pkix.Name{
			Organization:       []string{options.organization},
			OrganizationalUnit: []string{"Client"},
			CommonName:         commonName,
		},
		NotBefore:   time.Now().Add(-time.Hour),
		NotAfter:    time.Now().Add(options.validity),
		KeyUsage:    x509.KeyUsageKeyEncipherment | x509.KeyUsageDigitalSignature,
		ExtKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageClientAuth},
	}

	// Sign the client certificate with CA
	certBytes, err := x509.CreateCertificate(rand.Reader, certTemplate, m.caCert, &certKey.PublicKey, m.caKey)
	if err != nil {
		return tls.Certificate{}, fmt.Errorf("failed to create client certificate: %w", err)
	}

	// Create TLS certificate
	certPEM := pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: certBytes})
	keyPEM := pem.EncodeToMemory(&pem.Block{Type: "RSA PRIVATE KEY", Bytes: x509.MarshalPKCS1PrivateKey(certKey)})

	return tls.X509KeyPair(certPEM, keyPEM)
}

// SaveCert saves a certificate to a file.
func (m *Manager) SaveCert(filename string, cert []byte) error {
	path := filepath.Join(m.certDir, filename)
	return m.savePEM(path, "CERTIFICATE", cert, 0644)
}

// SaveKey saves a private key to a file.
func (m *Manager) SaveKey(filename string, key []byte) error {
	path := filepath.Join(m.certDir, filename)
	return m.savePEM(path, "RSA PRIVATE KEY", key, 0600)
}

// savePEM saves PEM-encoded data to a file with specified permissions.
func (m *Manager) savePEM(path, blockType string, data []byte, perm os.FileMode) error {
	pemData := pem.EncodeToMemory(&pem.Block{Type: blockType, Bytes: data})
	return os.WriteFile(path, pemData, perm)
}

// GetCACert returns the CA certificate PEM bytes.
func (m *Manager) GetCACert() []byte {
	m.mu.RLock()
	defer m.mu.RUnlock()

	if m.caCert == nil {
		return nil
	}
	return pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: m.caCert.Raw})
}

// GetCertDir returns the certificate directory.
func (m *Manager) GetCertDir() string {
	return m.certDir
}

// CA options
type caOptions struct {
	organization string
	validity     time.Duration
	keySize      int
}

// CAOption is a functional option for CA generation.
type CAOption func(*caOptions)

// WithCAOrganization sets the organization for the CA.
func WithCAOrganization(org string) CAOption {
	return func(o *caOptions) {
		o.organization = org
	}
}

// WithCAValidity sets the validity period for the CA.
func WithCAValidity(d time.Duration) CAOption {
	return func(o *caOptions) {
		o.validity = d
	}
}

// WithCAKeySize sets the RSA key size for the CA.
func WithCAKeySize(size int) CAOption {
	return func(o *caOptions) {
		o.keySize = size
	}
}

// Certificate options
type certOptions struct {
	organization string
	validity     time.Duration
	keySize      int
}

// CertOption is a functional option for certificate generation.
type CertOption func(*certOptions)

// WithCertOrganization sets the organization for the certificate.
func WithCertOrganization(org string) CertOption {
	return func(o *certOptions) {
		o.organization = org
	}
}

// WithCertValidity sets the validity period for the certificate.
func WithCertValidity(d time.Duration) CertOption {
	return func(o *certOptions) {
		o.validity = d
	}
}

// WithCertKeySize sets the RSA key size for the certificate.
func WithCertKeySize(size int) CertOption {
	return func(o *certOptions) {
		o.keySize = size
	}
}

// GenerateCertsForController generates all certificates needed for the controller.
func (m *Manager) GenerateCertsForController(serverName string, dnsNames []string, ipAddresses []net.IP) error {
	// Generate CA if not already done
	if m.caCert == nil {
		if err := m.GenerateCA(); err != nil {
			return fmt.Errorf("failed to generate CA: %w", err)
		}
	}

	// Generate controller server certificate
	serverCert, err := m.GenerateServerCert(serverName, dnsNames, ipAddresses)
	if err != nil {
		return fmt.Errorf("failed to generate server certificate: %w", err)
	}

	// Save server certificate
	if err := m.saveTLSCert("controller", serverCert); err != nil {
		return fmt.Errorf("failed to save controller certificate: %w", err)
	}

	return nil
}

// GenerateCertsForAgent generates all certificates needed for an agent.
func (m *Manager) GenerateCertsForAgent(agentID string, dnsNames []string, ipAddresses []net.IP) error {
	// Generate CA if not already done
	if m.caCert == nil {
		if err := m.GenerateCA(); err != nil {
			return fmt.Errorf("failed to generate CA: %w", err)
		}
	}

	// Generate agent server certificate
	serverCert, err := m.GenerateServerCert(agentID, dnsNames, ipAddresses)
	if err != nil {
		return fmt.Errorf("failed to generate agent server certificate: %w", err)
	}

	// Generate agent client certificate for mTLS
	clientCert, err := m.GenerateClientCert(agentID)
	if err != nil {
		return fmt.Errorf("failed to generate agent client certificate: %w", err)
	}

	// Use agent-specific file names to avoid conflicts
	serverPrefix := "agent-" + agentID + "-server"
	clientPrefix := "agent-" + agentID + "-client"

	// Save certificates
	if err := m.saveTLSCert(serverPrefix, serverCert); err != nil {
		return fmt.Errorf("failed to save agent server certificate: %w", err)
	}
	if err := m.saveTLSCert(clientPrefix, clientCert); err != nil {
		return fmt.Errorf("failed to save agent client certificate: %w", err)
	}

	return nil
}

// saveTLSCert saves a TLS certificate and its private key to files.
func (m *Manager) saveTLSCert(name string, cert tls.Certificate) error {
	// Save certificate
	if len(cert.Certificate) == 0 {
		return fmt.Errorf("no certificate data")
	}

	certPath := filepath.Join(m.certDir, name+".crt")
	if err := m.savePEM(certPath, "CERTIFICATE", cert.Certificate[0], 0644); err != nil {
		return err
	}

	// Parse private key
	var keyBytes []byte
	var keyType string

	switch key := cert.PrivateKey.(type) {
	case *rsa.PrivateKey:
		keyBytes = x509.MarshalPKCS1PrivateKey(key)
		keyType = "RSA PRIVATE KEY"
	case *ecdsa.PrivateKey:
		keyBytes, _ = x509.MarshalECPrivateKey(key)
		keyType = "EC PRIVATE KEY"
	default:
		return fmt.Errorf("unsupported private key type")
	}

	keyPath := filepath.Join(m.certDir, name+".key")
	return m.savePEM(keyPath, keyType, keyBytes, 0600)
}

// GenerateECCCerts generates ECDSA certificates (more efficient than RSA).
func (m *Manager) GenerateECCCerts(commonName string, isServer bool, dnsNames []string, ipAddresses []net.IP) (tls.Certificate, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	if m.caCert == nil || m.caKey == nil {
		return tls.Certificate{}, fmt.Errorf("CA not initialized")
	}

	// Generate ECDSA key pair (P-256 for good balance of security and performance)
	certKey, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return tls.Certificate{}, fmt.Errorf("failed to generate ECC key: %w", err)
	}

	// Create certificate template
	serialNumber, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	if err != nil {
		return tls.Certificate{}, fmt.Errorf("failed to generate serial number: %w", err)
	}

	var extKeyUsage []x509.ExtKeyUsage
	if isServer {
		extKeyUsage = []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth}
	} else {
		extKeyUsage = []x509.ExtKeyUsage{x509.ExtKeyUsageClientAuth}
	}

	certTemplate := &x509.Certificate{
		SerialNumber: serialNumber,
		Subject: pkix.Name{
			Organization:       []string{"CHV Platform"},
			OrganizationalUnit: []string{"Auto-Generated"},
			CommonName:         commonName,
		},
		DNSNames:    dnsNames,
		IPAddresses: ipAddresses,
		NotBefore:   time.Now().Add(-time.Hour),
		NotAfter:    time.Now().Add(DefaultCertValidity),
		KeyUsage:    x509.KeyUsageDigitalSignature,
		ExtKeyUsage: extKeyUsage,
	}

	// Sign the certificate
	certBytes, err := x509.CreateCertificate(rand.Reader, certTemplate, m.caCert, &certKey.PublicKey, m.caKey)
	if err != nil {
		return tls.Certificate{}, fmt.Errorf("failed to create certificate: %w", err)
	}

	// Create TLS certificate
	certPEM := pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: certBytes})

	keyBytes, err := x509.MarshalECPrivateKey(certKey)
	if err != nil {
		return tls.Certificate{}, fmt.Errorf("failed to marshal ECC key: %w", err)
	}
	keyPEM := pem.EncodeToMemory(&pem.Block{Type: "EC PRIVATE KEY", Bytes: keyBytes})

	return tls.X509KeyPair(certPEM, keyPEM)
}
