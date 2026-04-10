package handlers

import (
	"net/http"

	"github.com/chv/chv/internal/agent/services"
)

type InstallHandler struct {
	service *services.InstallService
}

func NewInstallHandler(service *services.InstallService) *InstallHandler {
	return &InstallHandler{service: service}
}

func (h *InstallHandler) Check(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	resp, err := h.service.Check(ctx)
	if err != nil {
		respondError(w, http.StatusInternalServerError, "install_check_failed", "Failed to check installation status", true)
		return
	}

	respondJSON(w, http.StatusOK, resp)
}
