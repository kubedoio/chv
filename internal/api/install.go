package api

import (
	"net/http"

	"github.com/chv/chv/internal/bootstrap"
	"github.com/chv/chv/internal/logger"
)

type createTokenRequest struct {
	Name string `json:"name"`
}

func (h *Handler) createToken(w http.ResponseWriter, r *http.Request) {
	var req createTokenRequest
	if err := decodeJSON(r, &req); err != nil || req.Name == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Token creation requires a non-empty name.",
			Retryable: false,
		})
		return
	}

	result, err := h.auth.CreateToken(requestContext(r), req.Name)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "token_create_failed",
			Message:   "CHV could not create an API token.",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusCreated, map[string]any{
		"id":      result.ID,
		"token":   result.Token,
		"message": "Store this token securely. It will not be shown again.",
	})
}

func (h *Handler) loginValidate(w http.ResponseWriter, _ *http.Request) {
	h.writeJSON(w, http.StatusOK, map[string]any{"ok": true})
}

func (h *Handler) installStatus(w http.ResponseWriter, r *http.Request) {
	status, err := h.bootstrap.Check(requestContext(r))
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "install_status_failed",
			Message:   "CHV could not inspect install status.",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"overall_state": status.OverallState,
		"data_root":     status.DataRoot,
		"database_path": status.DatabasePath,
		"bridge": map[string]any{
			"name":        status.BridgeName,
			"exists":      status.BridgeExists,
			"expected_ip": status.BridgeIPExpected,
			"actual_ip":   status.BridgeIPActual,
			"up":          status.BridgeUp,
		},
		"localdisk": map[string]any{
			"path":  status.LocaldiskPath,
			"ready": status.LocaldiskReady,
		},
		"cloud_hypervisor": map[string]any{
			"path":  status.CloudHypervisorPath,
			"found": status.CloudHypervisorFound,
		},
		"cloudinit": map[string]any{
			"supported": status.CloudInitSupported,
		},
		"checks":   []string{},
		"warnings": []string{},
		"errors":   []string{},
	})
}

func (h *Handler) installBootstrap(w http.ResponseWriter, r *http.Request) {
	result, err := h.bootstrap.Bootstrap(requestContext(r))
	if err != nil {
		logger.L().Error("Bootstrap failed", logger.ErrorField(err))
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "bootstrap_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}
	h.writeJSON(w, http.StatusOK, result)
}

func (h *Handler) installRepair(w http.ResponseWriter, r *http.Request) {
	var req bootstrap.RepairRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Repair request body must be valid JSON.",
			Retryable: false,
		})
		return
	}

	result, err := h.bootstrap.Repair(requestContext(r), req)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "repair_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}
	h.writeJSON(w, http.StatusOK, result)
}

func (h *Handler) listNetworks(w http.ResponseWriter, r *http.Request) {
	items, err := h.repo.ListNetworks(requestContext(r))
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{Code: "networks_failed", Message: "CHV could not list networks.", Retryable: true})
		return
	}
	h.writeJSON(w, http.StatusOK, items)
}

func (h *Handler) listStoragePools(w http.ResponseWriter, r *http.Request) {
	items, err := h.repo.ListStoragePools(requestContext(r))
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{Code: "storage_pools_failed", Message: "CHV could not list storage pools.", Retryable: true})
		return
	}
	h.writeJSON(w, http.StatusOK, items)
}

func (h *Handler) listImages(w http.ResponseWriter, r *http.Request) {
	items, err := h.repo.ListImages(requestContext(r))
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{Code: "images_failed", Message: "CHV could not list images.", Retryable: true})
		return
	}
	h.writeJSON(w, http.StatusOK, items)
}

func (h *Handler) listVMs(w http.ResponseWriter, r *http.Request) {
	items, err := h.repo.ListVMs(requestContext(r))
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{Code: "vms_failed", Message: "CHV could not list virtual machines.", Retryable: true})
		return
	}
	h.writeJSON(w, http.StatusOK, items)
}

func (h *Handler) listOperations(w http.ResponseWriter, r *http.Request) {
	items, err := h.repo.ListOperations(requestContext(r))
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{Code: "operations_failed", Message: "CHV could not list operations.", Retryable: true})
		return
	}
	h.writeJSON(w, http.StatusOK, items)
}
