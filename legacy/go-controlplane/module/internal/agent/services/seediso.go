package services

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

// SeedISOService generates seed ISOs for cloud-init
type SeedISOService struct{}

func NewSeedISOService() *SeedISOService {
	return &SeedISOService{}
}

// ISOTool represents available ISO generation tools
type ISOTool struct {
	Name string
	Cmd  string
	Args func(inputDir, outputPath string) []string
}

// FindISOTool detects available ISO generation tool
func (s *SeedISOService) FindISOTool() (*ISOTool, error) {
	tools := []ISOTool{
		{
			Name: "xorrisofs",
			Cmd:  "xorrisofs",
			Args: func(inputDir, outputPath string) []string {
				return []string{
					"-output", outputPath,
					"-volid", "cidata",
					"-joliet",
					"-rock",
					"-graft-points",
					"/" + filepath.Join(inputDir, "user-data") + "=/user-data",
					"/" + filepath.Join(inputDir, "meta-data") + "=/meta-data",
					"/" + filepath.Join(inputDir, "network-config") + "=/network-config",
				}
			},
		},
		{
			Name: "mkisofs",
			Cmd:  "mkisofs",
			Args: func(inputDir, outputPath string) []string {
				return []string{
					"-o", outputPath,
					"-V", "cidata",
					"-J", "-R",
					inputDir,
				}
			},
		},
		{
			Name: "genisoimage",
			Cmd:  "genisoimage",
			Args: func(inputDir, outputPath string) []string {
				return []string{
					"-o", outputPath,
					"-V", "cidata",
					"-J", "-R",
					inputDir,
				}
			},
		},
	}

	for _, tool := range tools {
		if _, err := exec.LookPath(tool.Cmd); err == nil {
			return &tool, nil
		}
	}

	return nil, fmt.Errorf("no ISO generation tool found (tried: xorrisofs, mkisofs, genisoimage)")
}

// GenerateRequest holds ISO generation parameters
type GenerateRequest struct {
	VMID         string
	CloudinitDir string // Directory containing user-data, meta-data, network-config
	OutputDir    string // Where to write seed.iso
}

// GenerateResult holds ISO generation result
type GenerateResult struct {
	ISOTool string // Which tool was used
	ISOPath string // Full path to generated ISO
}

// Generate creates a seed ISO from cloud-init files
func (s *SeedISOService) Generate(ctx context.Context, req GenerateRequest) (*GenerateResult, error) {
	// Find available ISO tool
	tool, err := s.FindISOTool()
	if err != nil {
		return nil, err
	}

	// Verify input files exist
	requiredFiles := []string{"user-data", "meta-data", "network-config"}
	for _, file := range requiredFiles {
		path := filepath.Join(req.CloudinitDir, file)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			return nil, fmt.Errorf("required file missing: %s", path)
		}
	}

	// Ensure output directory exists
	if err := os.MkdirAll(req.OutputDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create output directory: %w", err)
	}

	// Generate ISO
	outputPath := filepath.Join(req.OutputDir, "seed.iso")

	args := tool.Args(req.CloudinitDir, outputPath)
	cmd := exec.CommandContext(ctx, tool.Cmd, args...)

	output, err := cmd.CombinedOutput()
	if err != nil {
		return nil, fmt.Errorf("failed to generate ISO: %w (output: %s)", err, output)
	}

	// Verify ISO was created
	if _, err := os.Stat(outputPath); os.IsNotExist(err) {
		return nil, fmt.Errorf("ISO file not created")
	}

	return &GenerateResult{
		ISOTool: tool.Name,
		ISOPath: outputPath,
	}, nil
}
