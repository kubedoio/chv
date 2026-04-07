// chv-certgen is a certificate generation tool for CHV Platform TLS/mTLS.
package main

import (
	"crypto/ecdsa"
	"crypto/rsa"
	"crypto/tls"
	"crypto/x509"
	"encoding/pem"
	"flag"
	"fmt"
	"log"
	"net"
	"os"
	"path/filepath"

	"github.com/chv/chv/internal/cert"
)

func main() {
	var (
		certDir        = flag.String("cert-dir", "/etc/chv/certs", "Directory to store certificates")
		generateCA     = flag.Bool("ca", false, "Generate CA certificate")
		generateCtrl   = flag.Bool("controller", false, "Generate controller certificates")
		generateAgent  = flag.String("agent", "", "Generate agent certificates (specify agent ID)")
		controllerName = flag.String("controller-name", "chv-controller", "Controller hostname for certificate")
		agentName      = flag.String("agent-name", "", "Agent hostname for certificate (defaults to system hostname)")
		outputFormat   = flag.String("format", "files", "Output format: files, bundle")
		showVersion    = flag.Bool("version", false, "Show version")
	)
	flag.Parse()

	if *showVersion {
		fmt.Println("chv-certgen version 0.1.0")
		os.Exit(0)
	}

	// Ensure certificate directory exists
	if err := os.MkdirAll(*certDir, 0750); err != nil {
		log.Fatalf("Failed to create certificate directory: %v", err)
	}

	mgr := cert.NewManager(*certDir)

	// Load existing CA or generate new one
	var caExists bool
	if err := mgr.LoadCA(); err == nil {
		caExists = true
		log.Println("Loaded existing CA certificate")
	}

	// Generate CA if requested or if no CA exists
	if *generateCA || !caExists {
		log.Println("Generating new CA certificate...")
		if err := mgr.GenerateCA(); err != nil {
			log.Fatalf("Failed to generate CA: %v", err)
		}
		log.Printf("CA certificate generated in %s", *certDir)
		caExists = true
	}

	// Generate controller certificates
	if *generateCtrl {
		if err := generateControllerCerts(mgr, *controllerName, *certDir); err != nil {
			log.Fatalf("Failed to generate controller certificates: %v", err)
		}
	}

	// Generate agent certificates
	if *generateAgent != "" {
		agentID := *generateAgent
		if *agentName == "" {
			*agentName, _ = os.Hostname()
			if *agentName == "" {
				*agentName = agentID
			}
		}
		if err := generateAgentCerts(mgr, agentID, *agentName, *certDir); err != nil {
			log.Fatalf("Failed to generate agent certificates: %v", err)
		}
	}

	// If no specific generation requested, show usage
	if !*generateCA && !*generateCtrl && *generateAgent == "" && !caExists {
		flag.Usage()
		os.Exit(1)
	}

	// Output certificate bundle if requested
	if *outputFormat == "bundle" {
		if err := outputBundle(mgr, *certDir); err != nil {
			log.Fatalf("Failed to output bundle: %v", err)
		}
	}

	// Print summary
	printSummary(*certDir)
}

func generateControllerCerts(mgr *cert.Manager, controllerName, certDir string) error {
	log.Printf("Generating controller certificates for %s...", controllerName)

	dnsNames := []string{
		"localhost",
		controllerName,
		"controller",
		"controller.chv.svc.cluster.local",
	}

	// Add IP addresses
	ipAddresses := []net.IP{
		net.ParseIP("127.0.0.1"),
		net.ParseIP("::1"),
	}

	serverCert, err := mgr.GenerateServerCert(controllerName, dnsNames, ipAddresses)
	if err != nil {
		return fmt.Errorf("failed to generate server certificate: %w", err)
	}

	// Save certificates and key
	if err := saveTLSCert(certDir, "controller", serverCert); err != nil {
		return fmt.Errorf("failed to save controller certificate: %w", err)
	}

	log.Printf("Controller certificates saved to %s", certDir)
	return nil
}

func generateAgentCerts(mgr *cert.Manager, agentID, agentName, certDir string) error {
	log.Printf("Generating agent certificates for %s (%s)...", agentID, agentName)

	// Server certificate for agent's gRPC server
	dnsNames := []string{
		"localhost",
		agentName,
		agentID,
	}

	ipAddresses := []net.IP{
		net.ParseIP("127.0.0.1"),
		net.ParseIP("::1"),
	}

	serverCert, err := mgr.GenerateServerCert(agentID, dnsNames, ipAddresses)
	if err != nil {
		return fmt.Errorf("failed to generate agent server certificate: %w", err)
	}

	// Client certificate for mTLS when connecting to controller
	clientCert, err := mgr.GenerateClientCert(agentID)
	if err != nil {
		return fmt.Errorf("failed to generate agent client certificate: %w", err)
	}

	// Save server certificate and key
	prefix := fmt.Sprintf("agent-%s-server", agentID)
	if err := saveTLSCert(certDir, prefix, serverCert); err != nil {
		return fmt.Errorf("failed to save agent server certificate: %w", err)
	}

	// Save client certificate and key
	prefix = fmt.Sprintf("agent-%s-client", agentID)
	if err := saveTLSCert(certDir, prefix, clientCert); err != nil {
		return fmt.Errorf("failed to save agent client certificate: %w", err)
	}

	log.Printf("Agent certificates saved to %s/agent-%s-*.crt", certDir, agentID)
	return nil
}

func saveTLSCert(certDir, name string, cert tls.Certificate) error {
	// Save certificate
	if len(cert.Certificate) == 0 {
		return fmt.Errorf("no certificate data")
	}

	certPath := filepath.Join(certDir, name+".crt")
	certPEM := pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: cert.Certificate[0]})
	if err := os.WriteFile(certPath, certPEM, 0644); err != nil {
		return fmt.Errorf("failed to write certificate: %w", err)
	}

	// Save private key
	keyPath := filepath.Join(certDir, name+".key")
	var keyPEM []byte

	switch key := cert.PrivateKey.(type) {
	case *rsa.PrivateKey:
		keyPEM = pem.EncodeToMemory(&pem.Block{Type: "RSA PRIVATE KEY", Bytes: x509.MarshalPKCS1PrivateKey(key)})
	case *ecdsa.PrivateKey:
		keyBytes, err := x509.MarshalECPrivateKey(key)
		if err != nil {
			return fmt.Errorf("failed to marshal EC key: %w", err)
		}
		keyPEM = pem.EncodeToMemory(&pem.Block{Type: "EC PRIVATE KEY", Bytes: keyBytes})
	default:
		return fmt.Errorf("unsupported private key type")
	}

	if err := os.WriteFile(keyPath, keyPEM, 0600); err != nil {
		return fmt.Errorf("failed to write private key: %w", err)
	}

	return nil
}

func outputBundle(mgr *cert.Manager, certDir string) error {
	// Output a bundle containing CA cert and controller cert for distribution
	caCert := mgr.GetCACert()
	if caCert == nil {
		return fmt.Errorf("CA certificate not available")
	}

	bundleFile := filepath.Join(certDir, "ca-bundle.crt")
	if err := os.WriteFile(bundleFile, caCert, 0644); err != nil {
		return fmt.Errorf("failed to write CA bundle: %w", err)
	}

	log.Printf("CA bundle saved to %s", bundleFile)
	return nil
}

func printSummary(certDir string) {
	fmt.Println("\n=== Certificate Summary ===")
	fmt.Printf("Certificate directory: %s\n\n", certDir)

	files := []string{
		"ca.crt",
		"ca.key",
		"controller.crt",
		"controller.key",
	}

	fmt.Println("Generated files:")
	for _, file := range files {
		path := filepath.Join(certDir, file)
		if info, err := os.Stat(path); err == nil {
			mode := info.Mode().String()
			fmt.Printf("  %-20s %s\n", file, mode)
		}
	}

	fmt.Println("\nAgent certificates (if generated):")
	entries, _ := os.ReadDir(certDir)
	for _, entry := range entries {
		name := entry.Name()
		if len(name) > 6 && name[:6] == "agent-" {
			info, _ := entry.Info()
			mode := info.Mode().String()
			fmt.Printf("  %-30s %s\n", name, mode)
		}
	}

	fmt.Println("\n=== Configuration ===")
	fmt.Println("Controller environment variables:")
	fmt.Printf("  export CHV_TLS_ENABLED=true\n")
	fmt.Printf("  export CHV_TLS_CERT=%s/controller.crt\n", certDir)
	fmt.Printf("  export CHV_TLS_KEY=%s/controller.key\n", certDir)
	fmt.Printf("  export CHV_TLS_CA=%s/ca.crt\n", certDir)

	fmt.Println("\nAgent environment variables:")
	fmt.Printf("  export CHV_TLS_ENABLED=true\n")
	fmt.Printf("  export CHV_TLS_CERT=%s/agent-<id>-server.crt\n", certDir)
	fmt.Printf("  export CHV_TLS_KEY=%s/agent-<id>-server.key\n", certDir)
	fmt.Printf("  export CHV_TLS_CA=%s/ca.crt\n", certDir)
	fmt.Printf("  export CHV_TLS_CLIENT_CERT=%s/agent-<id>-client.crt\n", certDir)
	fmt.Printf("  export CHV_TLS_CLIENT_KEY=%s/agent-<id>-client.key\n", certDir)
	fmt.Printf("  export CHV_TLS_SERVER_NAME=chv-controller\n")
}
