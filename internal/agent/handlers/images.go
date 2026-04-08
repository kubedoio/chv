package handlers

import (
	"encoding/json"
	"net/http"

	"github.com/chv/chv/internal/agent/services"
	"github.com/chv/chv/internal/agentapi"
)

type ImageHandler struct {
	downloadService *services.ImageDownloadService
}

func NewImageHandler(downloadService *services.ImageDownloadService) *ImageHandler {
	return &ImageHandler{downloadService: downloadService}
}

func (h *ImageHandler) Download(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req agentapi.ImageImportRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	if req.SourceURL == "" || req.DestPath == "" {
		respondError(w, http.StatusBadRequest, "invalid_request", "source_url and dest_path are required", false)
		return
	}

	result, err := h.downloadService.Download(ctx, req.SourceURL, req.DestPath)
	if err != nil {
		respondError(w, http.StatusInternalServerError, "download_failed", err.Error(), true)
		return
	}

	resp := agentapi.ImageImportResponse{
		DownloadedBytes: result.DownloadedBytes,
		LocalPath:       result.LocalPath,
	}

	respondJSON(w, http.StatusOK, resp)
}
