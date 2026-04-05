package api

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/worker"
	"github.com/chv/chv/pkg/uuidx"
	"github.com/go-chi/chi/v5"
)

// ImageImportRequest represents an image import request.
type ImageImportRequest struct {
	Name             string `json:"name"`
	OSFamily         string `json:"os_family"`
	SourceURL        string `json:"source_url"`
	SourceFormat     string `json:"source_format"`
	Architecture     string `json:"architecture"`
	Checksum         string `json:"checksum,omitempty"`
	CloudInitSupported bool `json:"cloud_init_supported"`
	DefaultUsername  string `json:"default_username,omitempty"`
}

func (h *Handler) importImage(w http.ResponseWriter, r *http.Request) {
	var req ImageImportRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "Invalid request body")
		return
	}
	
	if req.Name == "" || req.OSFamily == "" || req.SourceURL == "" {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "name, os_family, and source_url are required")
		return
	}
	
	// Default format
	if req.SourceFormat == "" {
		req.SourceFormat = "qcow2"
	}
	if req.Architecture == "" {
		req.Architecture = "x86_64"
	}
	
	sourceFormat := models.ImageFormat(req.SourceFormat)
	if sourceFormat != models.ImageFormatQCOW2 && sourceFormat != models.ImageFormatRaw {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_FORMAT", "source_format must be 'qcow2' or 'raw'")
		return
	}
	
	image := &models.Image{
		ID:                 uuidx.New(),
		Name:               req.Name,
		OSFamily:           req.OSFamily,
		SourceFormat:       sourceFormat,
		NormalizedFormat:   models.ImageFormatRaw,
		Architecture:       req.Architecture,
		CloudInitSupported: req.CloudInitSupported,
		DefaultUsername:    req.DefaultUsername,
		Checksum:           req.Checksum,
		Status:             models.ImageStatusImporting,
		CreatedAt:          time.Now(),
	}
	
	// Add source URL to metadata
	metadata := map[string]interface{}{
		"source_url": req.SourceURL,
	}
	metadataJSON, _ := json.Marshal(metadata)
	image.Metadata = metadataJSON
	
	if err := h.store.CreateImage(r.Context(), image); err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to create image")
		return
	}
	
	// Trigger async import job
	if h.imageImportWorker != nil {
		h.imageImportWorker.Enqueue(worker.ImportJob{
			ImageID:   image.ID.String(),
			SourceURL: req.SourceURL,
		})
	}
	
	h.jsonResponse(w, http.StatusCreated, image)
}

func (h *Handler) listImages(w http.ResponseWriter, r *http.Request) {
	images, err := h.store.ListImages(r.Context())
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to list images")
		return
	}
	
	h.jsonResponse(w, http.StatusOK, images)
}

func (h *Handler) getImage(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	imageID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid image ID")
		return
	}
	
	image, err := h.store.GetImage(r.Context(), imageID)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get image")
		return
	}
	
	if image == nil {
		h.errorResponse(w, http.StatusNotFound, "NOT_FOUND", "Image not found")
		return
	}
	
	h.jsonResponse(w, http.StatusOK, image)
}
