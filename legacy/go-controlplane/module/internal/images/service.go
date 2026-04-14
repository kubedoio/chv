package images

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
)

// Status constants
const (
	StatusImporting  = "importing"
	StatusValidating = "validating"
	StatusReady      = "ready"
	StatusFailed     = "failed"
)

type Service struct {
	repo     *db.Repository
	dataRoot string
}

func NewService(repo *db.Repository, dataRoot string) *Service {
	return &Service{
		repo:     repo,
		dataRoot: dataRoot,
	}
}

// ImportImage creates an image record and initiates the import process
func (s *Service) ImportImage(ctx context.Context, input ImportInput) (*models.Image, error) {
	// Get local node ID
	localNode, err := s.repo.GetLocalNode(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to get local node: %w", err)
	}
	if localNode == nil {
		return nil, fmt.Errorf("local node not found")
	}

	// Determine source format and normalized format
	sourceFormat := input.Format
	if sourceFormat == "" {
		sourceFormat = "qcow2" // default
	}
	normalizedFormat := "raw" // We always convert to raw

	// Create image record
	image := &models.Image{
		ID:                 uuid.NewString(),
		NodeID:             localNode.ID,
		Name:               input.Name,
		OSFamily:           input.OSFamily,
		Architecture:       input.Architecture,
		Format:             input.Format,
		SourceFormat:       sourceFormat,
		NormalizedFormat:   normalizedFormat,
		SourceURL:          input.SourceURL,
		Checksum:           input.Checksum,
		LocalPath:          s.generateImagePath(input.Name, input.Format),
		CloudInitSupported: input.CloudInitSupported,
		Status:             StatusImporting,
		CreatedAt:          time.Now().UTC().Format(time.RFC3339),
	}

	if err := s.repo.CreateImage(ctx, image); err != nil {
		return nil, fmt.Errorf("failed to create image record: %w", err)
	}

	// Ensure images directory exists
	imagesDir := filepath.Join(s.dataRoot, "images")
	if err := os.MkdirAll(imagesDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create images directory: %w", err)
	}

	return image, nil
}

// UpdateImageStatus updates the status of an image
func (s *Service) UpdateImageStatus(ctx context.Context, imageID, status string) error {
	image, err := s.repo.GetImageByID(ctx, imageID)
	if err != nil {
		return err
	}
	if image == nil {
		return fmt.Errorf("image not found: %s", imageID)
	}

	image.Status = status
	return s.repo.UpdateImage(ctx, image)
}

// SetImageFailed marks an image as failed with error info
func (s *Service) SetImageFailed(ctx context.Context, imageID string, err error) error {
	// For MVP, just update status to failed
	// Future: store error message in a separate field or log
	return s.UpdateImageStatus(ctx, imageID, StatusFailed)
}

// ValidateImageChecksum validates the checksum of a downloaded image
func (s *Service) ValidateImageChecksum(ctx context.Context, imageID string) error {
	image, err := s.repo.GetImageByID(ctx, imageID)
	if err != nil {
		return err
	}
	if image == nil {
		return fmt.Errorf("image not found: %s", imageID)
	}

	if image.Checksum == "" {
		return nil // No checksum to validate
	}

	if err := ValidateChecksum(image.LocalPath, image.Checksum); err != nil {
		// Mark as failed
		_ = s.SetImageFailed(ctx, imageID, err)
		return err
	}

	return nil
}

func (s *Service) generateImagePath(name, format string) string {
	// Generate unique filename: /var/lib/chv/images/{uuid}.qcow2
	return filepath.Join(s.dataRoot, "images", fmt.Sprintf("%s.%s", uuid.NewString(), format))
}

type ImportInput struct {
	Name               string
	OSFamily           string
	Architecture       string
	Format             string
	SourceURL          string
	Checksum           string
	CloudInitSupported bool
}
