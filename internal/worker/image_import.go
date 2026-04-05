// Package worker provides background job processing for CHV.
package worker

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/store"
	"github.com/google/uuid"
)

// ImageImportWorker handles asynchronous image import jobs.
type ImageImportWorker struct {
	store     store.Store
	imageDir  string
	workQueue chan ImportJob
	quit      chan struct{}
}

// ImportJob represents an image import job.
type ImportJob struct {
	ImageID   string
	SourceURL string
}

// NewImageImportWorker creates a new image import worker.
func NewImageImportWorker(store store.Store, imageDir string) *ImageImportWorker {
	return &ImageImportWorker{
		store:     store,
		imageDir:  imageDir,
		workQueue: make(chan ImportJob, 100),
		quit:      make(chan struct{}),
	}
}

// Start begins the worker goroutines.
func (w *ImageImportWorker) Start(numWorkers int) {
	for i := 0; i < numWorkers; i++ {
		go w.worker(i)
	}
}

// Stop shuts down the worker.
func (w *ImageImportWorker) Stop() {
	close(w.quit)
}

// Enqueue adds a job to the queue.
func (w *ImageImportWorker) Enqueue(job ImportJob) {
	select {
	case w.workQueue <- job:
		log.Printf("Enqueued image import job for %s", job.ImageID)
	default:
		log.Printf("Warning: image import queue full, dropping job for %s", job.ImageID)
	}
}

// worker processes import jobs.
func (w *ImageImportWorker) worker(id int) {
	log.Printf("Image import worker %d started", id)
	for {
		select {
		case <-w.quit:
			log.Printf("Image import worker %d stopped", id)
			return
		case job := <-w.workQueue:
			if err := w.processJob(context.Background(), job); err != nil {
				log.Printf("Worker %d: failed to process job %s: %v", id, job.ImageID, err)
			}
		}
	}
}

// processJob processes a single import job.
func (w *ImageImportWorker) processJob(ctx context.Context, job ImportJob) error {
	log.Printf("Processing image import job: %s from %s", job.ImageID, job.SourceURL)
	
	// Get image from database
	imageID, err := uuid.Parse(job.ImageID)
	if err != nil {
		return fmt.Errorf("invalid image ID: %w", err)
	}
	
	image, err := w.store.GetImage(ctx, imageID)
	if err != nil {
		return fmt.Errorf("failed to get image: %w", err)
	}
	if image == nil {
		return fmt.Errorf("image not found: %s", job.ImageID)
	}
	
	// Update status to importing
	image.Status = models.ImageStatusImporting
	if err := w.store.UpdateImage(ctx, image); err != nil {
		return fmt.Errorf("failed to update image status: %w", err)
	}
	
	// Download the image
	tempPath := filepath.Join(w.imageDir, job.ImageID+".tmp")
	finalPath := filepath.Join(w.imageDir, job.ImageID+".raw")
	
	if err := w.downloadImage(ctx, job.SourceURL, tempPath, image.Checksum); err != nil {
		w.markFailed(ctx, image, fmt.Sprintf("Download failed: %v", err))
		return err
	}
	
	// Convert to raw format if needed
	sourceFormat := image.SourceFormat
	if sourceFormat == models.ImageFormatQCOW2 {
		if err := w.convertToRaw(tempPath, finalPath); err != nil {
			os.Remove(tempPath)
			w.markFailed(ctx, image, fmt.Sprintf("Conversion failed: %v", err))
			return err
		}
		os.Remove(tempPath) // Remove original qcow2
	} else {
		// Already raw, just rename
		if err := os.Rename(tempPath, finalPath); err != nil {
			w.markFailed(ctx, image, fmt.Sprintf("Rename failed: %v", err))
			return err
		}
	}
	
	// Get image size
	info, err := os.Stat(finalPath)
	if err != nil {
		w.markFailed(ctx, image, fmt.Sprintf("Stat failed: %v", err))
		return err
	}
	
	// Update image as ready
	image.Status = models.ImageStatusReady
	sizeBytes := uint64(info.Size())
	image.SizeBytes = sizeBytes
	now := time.Now()
	image.ImportedAt = &now
	
	if err := w.store.UpdateImage(ctx, image); err != nil {
		return fmt.Errorf("failed to mark image as ready: %w", err)
	}
	
	log.Printf("Image import completed: %s (%d bytes)", job.ImageID, sizeBytes)
	return nil
}

// downloadImage downloads an image from URL to path.
func (w *ImageImportWorker) downloadImage(ctx context.Context, url, destPath, expectedChecksum string) error {
	// Create temp file
	out, err := os.Create(destPath)
	if err != nil {
		return fmt.Errorf("failed to create file: %w", err)
	}
	defer out.Close()
	
	// Create HTTP request
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return fmt.Errorf("failed to create request: %w", err)
	}
	
	// Download
	client := &http.Client{Timeout: 30 * time.Minute}
	resp, err := client.Do(req)
	if err != nil {
		return fmt.Errorf("failed to download: %w", err)
	}
	defer resp.Body.Close()
	
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("bad status: %s", resp.Status)
	}
	
	// Calculate checksum while downloading
	hasher := sha256.New()
	writer := io.MultiWriter(out, hasher)
	
	written, err := io.Copy(writer, resp.Body)
	if err != nil {
		return fmt.Errorf("failed to write: %w", err)
	}
	
	log.Printf("Downloaded %d bytes from %s", written, url)
	
	// Verify checksum if provided
	if expectedChecksum != "" {
		actualChecksum := hex.EncodeToString(hasher.Sum(nil))
		if actualChecksum != expectedChecksum {
			return fmt.Errorf("checksum mismatch: expected %s, got %s", expectedChecksum, actualChecksum)
		}
		log.Printf("Checksum verified: %s", actualChecksum)
	}
	
	return nil
}

// convertToRaw converts a QCOW2 image to raw format.
func (w *ImageImportWorker) convertToRaw(sourcePath, destPath string) error {
	log.Printf("Converting %s to raw format: %s", sourcePath, destPath)
	
	cmd := exec.Command("qemu-img", "convert", "-f", "qcow2", "-O", "raw", sourcePath, destPath)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("qemu-img failed: %v (output: %s)", err, string(output))
	}
	
	return nil
}

// markFailed marks an image as failed.
func (w *ImageImportWorker) markFailed(ctx context.Context, image *models.Image, reason string) {
	image.Status = models.ImageStatusFailed
	// Could add failure reason to metadata
	if err := w.store.UpdateImage(ctx, image); err != nil {
		log.Printf("Failed to mark image as failed: %v", err)
	}
	log.Printf("Image import failed: %s - %s", image.ID, reason)
}
