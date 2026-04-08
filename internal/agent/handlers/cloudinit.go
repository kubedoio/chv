package handlers

import (
	"encoding/json"
	"net/http"

	"github.com/chv/chv/internal/agent/services"
)

type CloudInitHandler struct {
	seedService *services.SeedISOService
}

func NewCloudInitHandler(seedService *services.SeedISOService) *CloudInitHandler {
	return &CloudInitHandler{seedService: seedService}
}

type generateSeedISORequest struct {
	VMID         string `json:"vm_id"`
	CloudinitDir string `json:"cloudinit_dir"`
	OutputDir    string `json:"output_dir"`
}

type generateSeedISOResponse struct {
	Tool    string `json:"tool"`
	ISOPath string `json:"iso_path"`
}

func (h *CloudInitHandler) GenerateSeedISO(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req generateSeedISORequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	if req.VMID == "" || req.CloudinitDir == "" || req.OutputDir == "" {
		respondError(w, http.StatusBadRequest, "invalid_request", "vm_id, cloudinit_dir, and output_dir are required", false)
		return
	}

	result, err := h.seedService.Generate(ctx, services.GenerateRequest{
		VMID:         req.VMID,
		CloudinitDir: req.CloudinitDir,
		OutputDir:    req.OutputDir,
	})
	if err != nil {
		respondError(w, http.StatusInternalServerError, "seed_iso_failed", err.Error(), true)
		return
	}

	respondJSON(w, http.StatusOK, generateSeedISOResponse{
		Tool:    result.ISOTool,
		ISOPath: result.ISOPath,
	})
}

func (h *CloudInitHandler) CheckISOSupport(w http.ResponseWriter, r *http.Request) {
	tool, err := h.seedService.FindISOTool()
	if err != nil {
		respondJSON(w, http.StatusOK, map[string]any{
			"supported": false,
			"error":     err.Error(),
		})
		return
	}

	respondJSON(w, http.StatusOK, map[string]any{
		"supported": true,
		"tool":      tool.Name,
	})
}
