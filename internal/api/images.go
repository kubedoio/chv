package api

import (
	"net/http"
	"os"

	"github.com/chv/chv/internal/images"
	"github.com/chv/chv/internal/operations"
	"github.com/go-chi/chi/v5"
)

type createImageRequest struct {
	Name               string `json:"name"`
	OSFamily           string `json:"os_family"`
	Architecture       string `json:"architecture"`
	Format             string `json:"format"`
	SourceURL          string `json:"source_url"`
	Checksum           string `json:"checksum,omitempty"`
	CloudInitSupported bool   `json:"cloud_init_supported"`
}

func (h *Handler) createImage(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)

	var req createImageRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON.",
			Retryable: false,
		})
		return
	}

	// Validation
	if req.Name == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Image name is required.",
			Retryable: false,
		})
		return
	}

	if req.SourceURL == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Source URL is required.",
			Retryable: false,
		})
		return
	}

	if req.Format == "" {
		req.Format = "qcow2" // Default for MVP
	}

	// Create services
	imgService := images.NewService(h.repo, h.config.DataRoot)
	opService := operations.NewService(h.repo)

	// Create image record
	image, err := imgService.ImportImage(ctx, images.ImportInput{
		Name:               req.Name,
		OSFamily:           req.OSFamily,
		Architecture:       req.Architecture,
		Format:             req.Format,
		SourceURL:          req.SourceURL,
		Checksum:           req.Checksum,
		CloudInitSupported: req.CloudInitSupported,
	})
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "image_create_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	// Log operation start (pending state)
	op, err := opService.LogImageImportStart(ctx, image.ID, req)
	if err != nil {
		// Log error but don't fail the request
		// Operation logging is best-effort
		_ = err
	} else {
		// Update to running state
		_ = opService.LogImageImportRunning(ctx, op.ID)
	}

	// Trigger async download via agent
	if h.imageWorker != nil {
		h.imageWorker.QueueImport(ctx, image.ID)
	} else {
		// Fallback: set status to failed since we can't download
		image.Status = images.StatusFailed
		_ = os.Remove(image.LocalPath)
	}

	h.writeJSON(w, http.StatusCreated, image)
}

func (h *Handler) getImageProgress(w http.ResponseWriter, r *http.Request) {
	imageID := chi.URLParam(r, "id")

	if h.imageWorker == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:      "service_unavailable",
			Message:   "Image worker is not available.",
			Retryable: true,
		})
		return
	}

	progress := h.imageWorker.GetProgress(imageID)
	if progress == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:      "not_found",
			Message:   "No progress found for this image. It may not be in the importing state.",
			Retryable: false,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, progress)
}
