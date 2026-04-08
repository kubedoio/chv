package handlers

import (
	"encoding/json"
	"net/http"

	"github.com/chv/chv/internal/agent/services"
	"github.com/chv/chv/internal/agentapi"
)

type BootstrapHandler struct {
	service *services.BootstrapService
}

func NewBootstrapHandler(service *services.BootstrapService) *BootstrapHandler {
	return &BootstrapHandler{service: service}
}

func (h *BootstrapHandler) Bootstrap(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req agentapi.BootstrapRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	resp, err := h.service.Bootstrap(ctx, &req)
	if err != nil {
		respondError(w, http.StatusInternalServerError, "bootstrap_failed", err.Error(), true)
		return
	}

	respondJSON(w, http.StatusOK, resp)
}
