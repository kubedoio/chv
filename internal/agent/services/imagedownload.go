package services

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
)

type ImageDownloadService struct {
	httpClient *http.Client
}

func NewImageDownloadService() *ImageDownloadService {
	return &ImageDownloadService{
		httpClient: &http.Client{
			Timeout: 0, // No timeout - large files take time
		},
	}
}

type DownloadResult struct {
	DownloadedBytes int64
	LocalPath       string
}

func (s *ImageDownloadService) Download(ctx context.Context, url, destPath string) (*DownloadResult, error) {
	// Create parent directory if needed
	if err := os.MkdirAll(filepath.Dir(destPath), 0755); err != nil {
		return nil, fmt.Errorf("failed to create directory: %w", err)
	}

	// Create temp file for atomic download
	tmpPath := destPath + ".tmp"
	file, err := os.Create(tmpPath)
	if err != nil {
		return nil, fmt.Errorf("failed to create file: %w", err)
	}
	defer file.Close()

	// Create HTTP request with context
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		os.Remove(tmpPath)
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	// Execute request
	resp, err := s.httpClient.Do(req)
	if err != nil {
		os.Remove(tmpPath)
		return nil, fmt.Errorf("failed to download: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		os.Remove(tmpPath)
		return nil, fmt.Errorf("download failed with status: %d", resp.StatusCode)
	}

	// Stream copy to file
	downloaded, err := io.Copy(file, resp.Body)
	if err != nil {
		os.Remove(tmpPath)
		return nil, fmt.Errorf("failed to write file: %w", err)
	}

	// Close file before rename
	file.Close()

	// Atomic rename
	if err := os.Rename(tmpPath, destPath); err != nil {
		os.Remove(tmpPath)
		return nil, fmt.Errorf("failed to finalize file: %w", err)
	}

	return &DownloadResult{
		DownloadedBytes: downloaded,
		LocalPath:       destPath,
	}, nil
}
