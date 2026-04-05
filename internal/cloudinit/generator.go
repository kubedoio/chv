package cloudinit

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"fmt"
	"io"
	"os"
	"path/filepath"
)

// Generator generates cloud-init configuration disks.
type Generator struct {
	outputDir string
}

// NewGenerator creates a new cloud-init generator.
func NewGenerator(outputDir string) *Generator {
	return &Generator{outputDir: outputDir}
}

// Config represents cloud-init configuration.
type Config struct {
	UserData      string
	MetaData      string
	NetworkConfig string
}

// Generate creates a cloud-init ISO or nocloud drive.
func (g *Generator) Generate(vmID string, config *Config) (string, error) {
	// For MVP-1, we create a simple nocloud directory
	nocloudDir := filepath.Join(g.outputDir, vmID+"-cloudinit")
	
	if err := os.MkdirAll(nocloudDir, 0755); err != nil {
		return "", fmt.Errorf("failed to create cloudinit dir: %w", err)
	}
	
	// Write user-data
	userData := config.UserData
	if userData == "" {
		userData = "#cloud-config\n"
	}
	if err := os.WriteFile(filepath.Join(nocloudDir, "user-data"), []byte(userData), 0644); err != nil {
		return "", err
	}
	
	// Write meta-data
	metaData := config.MetaData
	if metaData == "" {
		metaData = fmt.Sprintf("instance-id: %s\nlocal-hostname: %s\n", vmID, vmID)
	}
	if err := os.WriteFile(filepath.Join(nocloudDir, "meta-data"), []byte(metaData), 0644); err != nil {
		return "", err
	}
	
	// Write network-config if provided
	if config.NetworkConfig != "" {
		if err := os.WriteFile(filepath.Join(nocloudDir, "network-config"), []byte(config.NetworkConfig), 0644); err != nil {
			return "", err
		}
	}
	
	return nocloudDir, nil
}

// GenerateISO creates a cloud-init ISO image.
func (g *Generator) GenerateISO(vmID string, config *Config) (string, error) {
	isoPath := filepath.Join(g.outputDir, vmID+"-cloudinit.iso")
	nocloudDir, err := g.Generate(vmID, config)
	if err != nil {
		return "", err
	}
	defer os.RemoveAll(nocloudDir)
	
	// Use mkisofs or xorriso to create ISO
	// For MVP-1, we create a tarball as a placeholder
	// In production, would use actual ISO creation
	
	var buf bytes.Buffer
	gw := gzip.NewWriter(&buf)
	tw := tar.NewWriter(gw)
	
	files := []struct {
		Name    string
		Content string
	}{
		{"user-data", config.UserData},
		{"meta-data", config.MetaData},
		{"network-config", config.NetworkConfig},
	}
	
	for _, file := range files {
		if file.Content == "" {
			continue
		}
		hdr := &tar.Header{
			Name: file.Name,
			Mode: 0644,
			Size: int64(len(file.Content)),
		}
		if err := tw.WriteHeader(hdr); err != nil {
			return "", err
		}
		if _, err := io.WriteString(tw, file.Content); err != nil {
			return "", err
		}
	}
	
	tw.Close()
	gw.Close()
	
	if err := os.WriteFile(isoPath+".tar.gz", buf.Bytes(), 0644); err != nil {
		return "", err
	}
	
	return isoPath, nil
}
