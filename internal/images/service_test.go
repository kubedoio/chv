package images

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/chv/chv/internal/db"
)

func TestService_ImportImage(t *testing.T) {
	// Create temp directory for test data
	tempDir := t.TempDir()

	// Create test repository
	dbPath := filepath.Join(tempDir, "test.db")
	repo, err := db.Open(dbPath)
	if err != nil {
		t.Fatalf("failed to open db: %v", err)
	}
	defer repo.Close()

	service := NewService(repo, tempDir)

	tests := []struct {
		name    string
		input   ImportInput
		wantErr bool
	}{
		{
			name: "valid image import",
			input: ImportInput{
				Name:               "ubuntu-22.04",
				OSFamily:           "ubuntu",
				Architecture:       "x86_64",
				Format:             "qcow2",
				SourceURL:          "https://example.com/ubuntu.qcow2",
				Checksum:           "sha256:abc123",
				CloudInitSupported: true,
			},
			wantErr: false,
		},
		{
			name: "minimal image import",
			input: ImportInput{
				Name:         "alpine",
				OSFamily:     "alpine",
				Architecture: "x86_64",
				Format:       "raw",
				SourceURL:    "https://example.com/alpine.raw",
			},
			wantErr: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ctx := context.Background()
			image, err := service.ImportImage(ctx, tt.input)

			if (err != nil) != tt.wantErr {
				t.Errorf("ImportImage() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if err != nil {
				return
			}

			// Verify image was created with expected fields
			if image.ID == "" {
				t.Error("expected image ID to be set")
			}
			if image.Name != tt.input.Name {
				t.Errorf("expected name %q, got %q", tt.input.Name, image.Name)
			}
			if image.OSFamily != tt.input.OSFamily {
				t.Errorf("expected os_family %q, got %q", tt.input.OSFamily, image.OSFamily)
			}
			if image.Architecture != tt.input.Architecture {
				t.Errorf("expected architecture %q, got %q", tt.input.Architecture, image.Architecture)
			}
			if image.Format != tt.input.Format {
				t.Errorf("expected format %q, got %q", tt.input.Format, image.Format)
			}
			if image.SourceURL != tt.input.SourceURL {
				t.Errorf("expected source_url %q, got %q", tt.input.SourceURL, image.SourceURL)
			}
			if image.Checksum != tt.input.Checksum {
				t.Errorf("expected checksum %q, got %q", tt.input.Checksum, image.Checksum)
			}
			if image.CloudInitSupported != tt.input.CloudInitSupported {
				t.Errorf("expected cloud_init_supported %v, got %v", tt.input.CloudInitSupported, image.CloudInitSupported)
			}
			if image.Status != StatusImporting {
				t.Errorf("expected status %q, got %q", StatusImporting, image.Status)
			}
			if image.CreatedAt == "" {
				t.Error("expected created_at to be set")
			}

			// Verify local path was generated
			if image.LocalPath == "" {
				t.Error("expected local_path to be set")
			}
			if !strings.HasPrefix(image.LocalPath, tempDir) {
				t.Errorf("expected local_path to start with %q, got %q", tempDir, image.LocalPath)
			}
			if !strings.HasSuffix(image.LocalPath, "."+tt.input.Format) {
				t.Errorf("expected local_path to end with .%s, got %q", tt.input.Format, image.LocalPath)
			}

			// Verify images directory was created
			imagesDir := filepath.Join(tempDir, "images")
			if _, err := os.Stat(imagesDir); os.IsNotExist(err) {
				t.Error("expected images directory to be created")
			}

			// Verify we can retrieve the image from the database
			retrieved, err := repo.GetImageByID(ctx, image.ID)
			if err != nil {
				t.Errorf("failed to retrieve image from db: %v", err)
			}
			if retrieved == nil {
				t.Error("expected to retrieve image from db, got nil")
			} else if retrieved.ID != image.ID {
				t.Errorf("expected retrieved image ID %q, got %q", image.ID, retrieved.ID)
			}
		})
	}
}

func TestService_UpdateImageStatus(t *testing.T) {
	tempDir := t.TempDir()
	dbPath := filepath.Join(tempDir, "test.db")
	repo, err := db.Open(dbPath)
	if err != nil {
		t.Fatalf("failed to open db: %v", err)
	}
	defer repo.Close()

	service := NewService(repo, tempDir)
	ctx := context.Background()

	// Create a test image
	image, err := service.ImportImage(ctx, ImportInput{
		Name:         "test-image",
		OSFamily:     "ubuntu",
		Architecture: "x86_64",
		Format:       "qcow2",
		SourceURL:    "https://example.com/test.qcow2",
	})
	if err != nil {
		t.Fatalf("failed to create test image: %v", err)
	}

	// Test state transitions
	transitions := []struct {
		status string
		valid  bool
	}{
		{StatusValidating, true},
		{StatusReady, true},
		{StatusFailed, true},
	}

	for _, tt := range transitions {
		t.Run("transition_to_"+tt.status, func(t *testing.T) {
			err := service.UpdateImageStatus(ctx, image.ID, tt.status)
			if (err != nil) == tt.valid {
				t.Errorf("UpdateImageStatus() error = %v", err)
			}

			if err == nil {
				// Verify status was updated
				updated, err := repo.GetImageByID(ctx, image.ID)
				if err != nil {
					t.Errorf("failed to get updated image: %v", err)
					return
				}
				if updated.Status != tt.status {
					t.Errorf("expected status %q, got %q", tt.status, updated.Status)
				}
			}
		})
	}

	t.Run("nonexistent_image", func(t *testing.T) {
		err := service.UpdateImageStatus(ctx, "nonexistent-id", StatusReady)
		if err == nil {
			t.Error("expected error for nonexistent image")
		}
	})
}

func TestService_SetImageFailed(t *testing.T) {
	tempDir := t.TempDir()
	dbPath := filepath.Join(tempDir, "test.db")
	repo, err := db.Open(dbPath)
	if err != nil {
		t.Fatalf("failed to open db: %v", err)
	}
	defer repo.Close()

	service := NewService(repo, tempDir)
	ctx := context.Background()

	// Create a test image
	image, err := service.ImportImage(ctx, ImportInput{
		Name:         "failing-image",
		OSFamily:     "ubuntu",
		Architecture: "x86_64",
		Format:       "qcow2",
		SourceURL:    "https://example.com/fail.qcow2",
	})
	if err != nil {
		t.Fatalf("failed to create test image: %v", err)
	}

	// Set image as failed
	testErr := fmt.Errorf("download failed: connection timeout")
	err = service.SetImageFailed(ctx, image.ID, testErr)
	if err != nil {
		t.Errorf("SetImageFailed() error = %v", err)
	}

	// Verify status is failed
	updated, err := repo.GetImageByID(ctx, image.ID)
	if err != nil {
		t.Fatalf("failed to get updated image: %v", err)
	}
	if updated.Status != StatusFailed {
		t.Errorf("expected status %q, got %q", StatusFailed, updated.Status)
	}
}

func TestService_generateImagePath(t *testing.T) {
	service := NewService(nil, "/var/lib/chv")

	path := service.generateImagePath("ubuntu-22.04", "qcow2")

	// Verify path structure
	if !strings.HasPrefix(path, "/var/lib/chv/images/") {
		t.Errorf("expected path to start with /var/lib/chv/images/, got %q", path)
	}
	if !strings.HasSuffix(path, ".qcow2") {
		t.Errorf("expected path to end with .qcow2, got %q", path)
	}
}
