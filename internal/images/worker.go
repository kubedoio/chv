package images

import (
	"context"
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/chv/chv/internal/agentapi"
	"github.com/chv/chv/internal/agentclient"
	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/logger"
	"github.com/chv/chv/internal/operations"
)

// Worker handles background image download tasks
type Worker struct {
	repo       *db.Repository
	opService  *operations.Service
	agentURL   string
	agentToken string
	stopChan   chan struct{}
	progress   *ProgressTracker
}

// NewWorker creates a new image download worker
func NewWorker(repo *db.Repository, opService *operations.Service, agentURL string) *Worker {
	return &Worker{
		repo:      repo,
		opService: opService,
		agentURL:  agentURL,
		stopChan:  make(chan struct{}),
		progress:  NewProgressTracker(),
	}
}

// NewWorkerWithAuth creates a new image download worker with authentication
func NewWorkerWithAuth(repo *db.Repository, opService *operations.Service, agentURL, agentToken string) *Worker {
	w := NewWorker(repo, opService, agentURL)
	w.agentToken = agentToken
	return w
}

// Start begins the worker loop
func (w *Worker) Start(ctx context.Context) {
	log := logger.L().WithComponent("image-worker")
	log.Info("Image import worker started")
	
	// Process any pending imports on startup (use background context)
	go w.processPendingImports(context.Background())
}

// Stop signals the worker to stop
func (w *Worker) Stop() {
	close(w.stopChan)
}

// QueueImport queues an image for import processing
func (w *Worker) QueueImport(ctx context.Context, imageID string) {
	// Start tracking progress
	w.progress.StartTracking(imageID, 0)
	// Use background context since HTTP request context will be cancelled
	go w.processImport(context.Background(), imageID)
}

// GetProgress retrieves the current import progress for an image
func (w *Worker) GetProgress(imageID string) *ImportProgress {
	return w.progress.GetProgress(imageID)
}

// processPendingImports finds and processes any images stuck in importing state
func (w *Worker) processPendingImports(ctx context.Context) {
	log := logger.L().WithComponent("image-worker")
	images, err := w.repo.ListImages(ctx)
	if err != nil {
		log.Error("Failed to list images for pending import check", logger.ErrorField(err))
		return
	}

	for _, img := range images {
		if img.Status == StatusImporting {
			log.Info("Resuming import", logger.F("image_id", img.ID), logger.F("name", img.Name))
			go w.processImport(ctx, img.ID)
		}
	}
}

// processImport handles the full import lifecycle for an image
func (w *Worker) processImport(ctx context.Context, imageID string) {
	log := logger.L().WithComponent("image-worker")

	// Get image details
	image, err := w.repo.GetImageByID(ctx, imageID)
	if err != nil {
		log.Error("Failed to get image", logger.F("image_id", imageID), logger.ErrorField(err))
		return
	}
	if image == nil {
		log.Error("Image not found", logger.F("image_id", imageID))
		return
	}

	// Skip if not in importing state
	if image.Status != StatusImporting {
		log.Warn("Image not in importing state", logger.F("image_id", imageID), logger.F("status", image.Status))
		return
	}

	// Create agent client
	var agentClient *agentclient.Client
	if w.agentToken != "" {
		agentClient = agentclient.NewClientWithAuth(w.agentURL, w.agentToken)
	} else {
		agentClient = agentclient.NewClient(w.agentURL)
	}

	// Step 1: Download the image
	log.Info("Downloading image", logger.F("image_id", imageID), logger.F("source", image.SourceURL))

	// Initialize progress tracking with estimated total if available
	w.progress.StartTracking(imageID, 0)
	w.progress.SetStatus(imageID, StatusImportDownloading)

	downloadReq := &agentapi.ImageImportRequest{
		ImageID:   image.ID,
		SourceURL: image.SourceURL,
		DestPath:  image.LocalPath,
	}

	downloadResp, err := agentClient.DownloadImage(ctx, downloadReq)
	if err != nil {
		log.Error("Failed to download image", logger.F("image_id", imageID), logger.ErrorField(err))
		w.progress.SetError(imageID, fmt.Errorf("download failed: %w", err))
		w.markFailed(ctx, imageID, fmt.Errorf("download failed: %w", err))
		return
	}

	// Update progress to reflect completed download
	w.progress.UpdateDownloadProgress(imageID, downloadResp.DownloadedBytes, "0 B/s")
	w.progress.SetStatus(imageID, StatusImportDownloading)

	log.Info("Image downloaded successfully",
		logger.F("image_id", imageID),
		logger.F("bytes", downloadResp.DownloadedBytes))

	// Step 2: Validate checksum if provided
	if image.Checksum != "" {
		log.Info("Validating checksum", logger.F("image_id", imageID))
		w.progress.SetStatus(imageID, StatusImportValidating)

		if err := w.validateChecksum(ctx, imageID, downloadResp.LocalPath, image.Checksum); err != nil {
			log.Error("Checksum validation failed", logger.F("image_id", imageID), logger.ErrorField(err))
			// Remove the bad file
			os.Remove(downloadResp.LocalPath)
			w.progress.SetError(imageID, fmt.Errorf("checksum validation failed: %w", err))
			w.markFailed(ctx, imageID, fmt.Errorf("checksum validation failed: %w", err))
			return
		}
		log.Info("Checksum validated", logger.F("image_id", imageID))
	}

	// Step 3: Mark as ready
	if err := w.markReady(ctx, imageID); err != nil {
		log.Error("Failed to mark image as ready", logger.F("image_id", imageID), logger.ErrorField(err))
		w.progress.SetError(imageID, err)
		return
	}

	// Mark progress as complete
	w.progress.Complete(imageID)

	log.Info("Image import completed successfully", logger.F("image_id", imageID))
}

// validateChecksum validates the checksum of a downloaded image
func (w *Worker) validateChecksum(ctx context.Context, imageID, localPath, expectedChecksum string) error {
	// Parse checksum format (supports "sha256:abc123" or just "abc123")
	hash := expectedChecksum
	if strings.Contains(expectedChecksum, ":") {
		parts := strings.SplitN(expectedChecksum, ":", 2)
		if len(parts) == 2 && parts[0] == "sha256" {
			hash = parts[1]
		}
	}

	if err := ValidateChecksum(localPath, hash); err != nil {
		return err
	}

	return nil
}

// markReady marks an image as ready
func (w *Worker) markReady(ctx context.Context, imageID string) error {
	image, err := w.repo.GetImageByID(ctx, imageID)
	if err != nil {
		return err
	}
	if image == nil {
		return fmt.Errorf("image not found: %s", imageID)
	}

	image.Status = StatusReady
	if err := w.repo.UpdateImage(ctx, image); err != nil {
		return err
	}

	// Update operation status if we have an operation tracking this
	// This is best-effort
	ops, _ := w.repo.ListOperations(ctx)
	for _, op := range ops {
		if op.ResourceID == imageID && op.State == operations.StateRunning {
			_ = w.opService.LogImageImportComplete(ctx, op.ID, map[string]string{
				"status": "completed",
				"path":   image.LocalPath,
			})
			break
		}
	}

	return nil
}

// markFailed marks an image as failed
func (w *Worker) markFailed(ctx context.Context, imageID string, importErr error) {
	log := logger.L().WithComponent("image-worker")
	svc := NewService(w.repo, "")
	if err := svc.SetImageFailed(ctx, imageID, importErr); err != nil {
		log.Error("Failed to mark image as failed", logger.F("image_id", imageID), logger.ErrorField(err))
	}

	// Update operation status
	ops, _ := w.repo.ListOperations(ctx)
	for _, op := range ops {
		if op.ResourceID == imageID && (op.State == operations.StatePending || op.State == operations.StateRunning) {
			_ = w.opService.LogImageImportFailed(ctx, op.ID, importErr)
			break
		}
	}
}

// WaitForImport blocks until an image import completes or fails
func (w *Worker) WaitForImport(ctx context.Context, imageID string, timeout time.Duration) error {
	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	ticker := time.NewTicker(2 * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return fmt.Errorf("timeout waiting for import")
		case <-ticker.C:
			image, err := w.repo.GetImageByID(ctx, imageID)
			if err != nil {
				return err
			}
			if image == nil {
				return fmt.Errorf("image not found")
			}

			switch image.Status {
			case StatusReady:
				return nil
			case StatusFailed:
				return fmt.Errorf("image import failed")
			}
		}
	}
}
