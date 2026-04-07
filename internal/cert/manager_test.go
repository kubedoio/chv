package cert

import (
	"crypto/x509"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestNewManager(t *testing.T) {
	mgr := NewManager("/tmp/test-certs")
	if mgr == nil {
		t.Fatal("expected manager to be non-nil")
	}
	if mgr.certDir != "/tmp/test-certs" {
		t.Errorf("expected certDir to be /tmp/test-certs, got %s", mgr.certDir)
	}
}

func TestNewManager_DefaultDir(t *testing.T) {
	mgr := NewManager("")
	if mgr.certDir != DefaultCertDir {
		t.Errorf("expected default certDir to be %s, got %s", DefaultCertDir, mgr.certDir)
	}
}

func TestManager_GenerateCA(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	err := mgr.GenerateCA()
	if err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	// Verify CA certificate was saved
	caCertPath := filepath.Join(tmpDir, "ca.crt")
	if _, err := os.Stat(caCertPath); os.IsNotExist(err) {
		t.Errorf("CA certificate file not created")
	}

	// Verify CA key was saved
	caKeyPath := filepath.Join(tmpDir, "ca.key")
	if _, err := os.Stat(caKeyPath); os.IsNotExist(err) {
		t.Errorf("CA key file not created")
	}

	// Verify key has restricted permissions
	info, err := os.Stat(caKeyPath)
	if err != nil {
		t.Fatalf("failed to stat CA key: %v", err)
	}
	if info.Mode().Perm() != 0600 {
		t.Errorf("CA key has incorrect permissions: %v", info.Mode().Perm())
	}

	// Verify CA certificate was loaded
	if mgr.caCert == nil {
		t.Error("CA certificate not loaded into manager")
	}
	if mgr.caKey == nil {
		t.Error("CA key not loaded into manager")
	}
}

func TestManager_LoadCA(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	// Generate CA first
	if err := mgr.GenerateCA(); err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	// Create new manager and load existing CA
	mgr2 := NewManager(tmpDir)
	err := mgr2.LoadCA()
	if err != nil {
		t.Fatalf("failed to load CA: %v", err)
	}

	if mgr2.caCert == nil {
		t.Error("CA certificate not loaded")
	}
	if mgr2.caKey == nil {
		t.Error("CA key not loaded")
	}
}

func TestManager_GenerateServerCert(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	// Generate CA first
	if err := mgr.GenerateCA(); err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	// Generate server certificate
	cert, err := mgr.GenerateServerCert("test-server", []string{"localhost"}, nil)
	if err != nil {
		t.Fatalf("failed to generate server certificate: %v", err)
	}

	if len(cert.Certificate) == 0 {
		t.Error("server certificate is empty")
	}
	if cert.PrivateKey == nil {
		t.Error("server private key is nil")
	}

	// Verify certificate chain
	parsedCert, err := x509.ParseCertificate(cert.Certificate[0])
	if err != nil {
		t.Fatalf("failed to parse certificate: %v", err)
	}

	if parsedCert.Subject.CommonName != "test-server" {
		t.Errorf("expected CN to be 'test-server', got %s", parsedCert.Subject.CommonName)
	}

	// Check extended key usage
	hasServerAuth := false
	for _, usage := range parsedCert.ExtKeyUsage {
		if usage == x509.ExtKeyUsageServerAuth {
			hasServerAuth = true
			break
		}
	}
	if !hasServerAuth {
		t.Error("certificate missing ExtKeyUsageServerAuth")
	}
}

func TestManager_GenerateClientCert(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	// Generate CA first
	if err := mgr.GenerateCA(); err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	// Generate client certificate
	cert, err := mgr.GenerateClientCert("test-client")
	if err != nil {
		t.Fatalf("failed to generate client certificate: %v", err)
	}

	if len(cert.Certificate) == 0 {
		t.Error("client certificate is empty")
	}
	if cert.PrivateKey == nil {
		t.Error("client private key is nil")
	}

	// Verify certificate
	parsedCert, err := x509.ParseCertificate(cert.Certificate[0])
	if err != nil {
		t.Fatalf("failed to parse certificate: %v", err)
	}

	if parsedCert.Subject.CommonName != "test-client" {
		t.Errorf("expected CN to be 'test-client', got %s", parsedCert.Subject.CommonName)
	}

	// Check extended key usage
	hasClientAuth := false
	for _, usage := range parsedCert.ExtKeyUsage {
		if usage == x509.ExtKeyUsageClientAuth {
			hasClientAuth = true
			break
		}
	}
	if !hasClientAuth {
		t.Error("certificate missing ExtKeyUsageClientAuth")
	}
}

func TestManager_GenerateServerCert_WithoutCA(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	_, err := mgr.GenerateServerCert("test-server", nil, nil)
	if err == nil {
		t.Error("expected error when generating server cert without CA")
	}
}

func TestManager_GenerateClientCert_WithoutCA(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	_, err := mgr.GenerateClientCert("test-client")
	if err == nil {
		t.Error("expected error when generating client cert without CA")
	}
}

func TestManager_GenerateCA_WithOptions(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	customOrg := "Custom Org"
	customValidity := 365 * 24 * time.Hour

	err := mgr.GenerateCA(
		WithCAOrganization(customOrg),
		WithCAValidity(customValidity),
		WithCAKeySize(2048),
	)
	if err != nil {
		t.Fatalf("failed to generate CA with options: %v", err)
	}

	if mgr.caCert.Subject.Organization[0] != customOrg {
		t.Errorf("expected organization to be %s, got %s", customOrg, mgr.caCert.Subject.Organization[0])
	}
}

func TestManager_GetCACert(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	// Should return nil before generating CA
	if mgr.GetCACert() != nil {
		t.Error("expected nil CA cert before generation")
	}

	// Generate CA
	if err := mgr.GenerateCA(); err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	// Should return CA cert after generation
	caCertPEM := mgr.GetCACert()
	if caCertPEM == nil {
		t.Error("expected CA cert after generation")
	}
}

func TestManager_SaveCertAndKey(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	// Generate CA
	if err := mgr.GenerateCA(); err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	// Generate server cert
	cert, err := mgr.GenerateServerCert("test-server", nil, nil)
	if err != nil {
		t.Fatalf("failed to generate server cert: %v", err)
	}

	// Save certificate
	err = mgr.SaveCert("test.crt", cert.Certificate[0])
	if err != nil {
		t.Fatalf("failed to save certificate: %v", err)
	}

	// Verify certificate file exists
	certPath := filepath.Join(tmpDir, "test.crt")
	if _, err := os.Stat(certPath); os.IsNotExist(err) {
		t.Error("certificate file not saved")
	}
}

func TestManager_GenerateCertsForController(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	err := mgr.GenerateCertsForController("chv-controller", []string{"localhost"}, nil)
	if err != nil {
		t.Fatalf("failed to generate controller certs: %v", err)
	}

	// Verify files were created
	expectedFiles := []string{"ca.crt", "ca.key", "controller.crt", "controller.key"}
	for _, file := range expectedFiles {
		path := filepath.Join(tmpDir, file)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			t.Errorf("expected file %s to exist", file)
		}
	}
}

func TestManager_GenerateCertsForAgent(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	err := mgr.GenerateCertsForAgent("agent-1", []string{"localhost"}, nil)
	if err != nil {
		t.Fatalf("failed to generate agent certs: %v", err)
	}

	// Verify CA files were created
	if _, err := os.Stat(filepath.Join(tmpDir, "ca.crt")); os.IsNotExist(err) {
		t.Error("expected ca.crt to exist")
	}
}

func TestManager_GenerateECCCerts(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	// Generate CA first
	if err := mgr.GenerateCA(); err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	// Generate ECC server certificate
	cert, err := mgr.GenerateECCCerts("test-ecc-server", true, []string{"localhost"}, nil)
	if err != nil {
		t.Fatalf("failed to generate ECC certificate: %v", err)
	}

	if len(cert.Certificate) == 0 {
		t.Error("ECC certificate is empty")
	}
	if cert.PrivateKey == nil {
		t.Error("ECC private key is nil")
	}

	// Verify certificate
	parsedCert, err := x509.ParseCertificate(cert.Certificate[0])
	if err != nil {
		t.Fatalf("failed to parse ECC certificate: %v", err)
	}

	// Check that it's using ECDSA
	if parsedCert.PublicKeyAlgorithm != x509.ECDSA {
		t.Errorf("expected ECDSA key algorithm, got %v", parsedCert.PublicKeyAlgorithm)
	}
}

func TestManager_ConcurrentAccess(t *testing.T) {
	tmpDir := t.TempDir()
	mgr := NewManager(tmpDir)

	// Generate CA
	if err := mgr.GenerateCA(); err != nil {
		t.Fatalf("failed to generate CA: %v", err)
	}

	// Generate multiple certs concurrently
	done := make(chan bool, 10)
	for i := 0; i < 10; i++ {
		go func(id int) {
			_, err := mgr.GenerateServerCert(
				"concurrent-server",
				[]string{"localhost"},
				nil,
				WithCertValidity(24*time.Hour),
			)
			if err != nil {
				t.Errorf("concurrent cert generation failed: %v", err)
			}
			done <- true
		}(i)
	}

	// Wait for all goroutines
	for i := 0; i < 10; i++ {
		<-done
	}
}
