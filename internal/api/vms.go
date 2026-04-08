package api

import (
	"net/http"
	"strings"
	"time"

	"github.com/chv/chv/internal/vm"
	"github.com/go-chi/chi/v5"
)

type createVMRequest struct {
	Name              string   `json:"name"`
	ImageID           string   `json:"image_id"`
	StoragePoolID     string   `json:"storage_pool_id"`
	NetworkID         string   `json:"network_id"`
	VCPU              int      `json:"vcpu"`
	MemoryMB          int      `json:"memory_mb"`
	UserData          string   `json:"user_data,omitempty"`
	Username          string   `json:"username,omitempty"`
	Password          string   `json:"password,omitempty"`
	SSHAuthorizedKeys []string `json:"ssh_authorized_keys,omitempty"`
}

func (h *Handler) createVM(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)

	var req createVMRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON",
			Retryable: false,
		})
		return
	}

	// Validation
	if req.Name == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "VM name is required",
			Retryable: false,
		})
		return
	}

	if req.ImageID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "image_id is required",
			Retryable: false,
		})
		return
	}

	if req.StoragePoolID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "storage_pool_id is required",
			Retryable: false,
		})
		return
	}

	if req.NetworkID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "network_id is required",
			Retryable: false,
		})
		return
	}

	// Set defaults
	if req.VCPU == 0 {
		req.VCPU = 2
	}
	if req.MemoryMB == 0 {
		req.MemoryMB = 2048
	}

	if h.vmService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:      "service_unavailable",
			Message:   "VM service not available",
			Retryable: true,
		})
		return
	}

	result, err := h.vmService.CreateVM(ctx, vm.CreateVMInput{
		Name:              req.Name,
		ImageID:           req.ImageID,
		StoragePoolID:     req.StoragePoolID,
		NetworkID:         req.NetworkID,
		VCPU:              req.VCPU,
		MemoryMB:          req.MemoryMB,
		UserData:          req.UserData,
		Username:          req.Username,
		Password:          req.Password,
		SSHAuthorizedKeys: req.SSHAuthorizedKeys,
	})
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "vm_create_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusCreated, result)
}

func (h *Handler) getVM(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	vmID := chi.URLParam(r, "id")

	vm, err := h.repo.GetVMByID(ctx, vmID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "vm_get_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	if vm == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:      "not_found",
			Message:   "VM not found",
			Retryable: false,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, vm)
}

func (h *Handler) startVM(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	vmID := chi.URLParam(r, "id")

	if h.vmService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:      "service_unavailable",
			Message:   "VM service not available",
			Retryable: true,
		})
		return
	}

	if err := h.vmService.StartVM(ctx, vmID); err != nil {
		status := http.StatusInternalServerError
		if strings.Contains(err.Error(), "boot gate failed") {
			status = http.StatusPreconditionFailed
		}
		h.writeError(w, status, apiError{
			Code:      "vm_start_failed",
			Message:   err.Error(),
			Retryable: status != http.StatusPreconditionFailed,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"message": "VM started successfully",
	})
}

func (h *Handler) stopVM(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	vmID := chi.URLParam(r, "id")

	if h.vmService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:      "service_unavailable",
			Message:   "VM service not available",
			Retryable: true,
		})
		return
	}

	if err := h.vmService.StopVM(ctx, vmID); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "vm_stop_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"message": "VM stopped successfully",
	})
}

func (h *Handler) restartVM(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	vmID := chi.URLParam(r, "id")

	if h.vmService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:      "service_unavailable",
			Message:   "VM service not available",
			Retryable: true,
		})
		return
	}

	if err := h.vmService.RestartVM(ctx, vmID); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "vm_restart_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"message": "VM restart initiated",
	})
}

func (h *Handler) deleteVM(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	vmID := chi.URLParam(r, "id")

	if h.vmService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:      "service_unavailable",
			Message:   "VM service not available",
			Retryable: true,
		})
		return
	}

	if err := h.vmService.DeleteVM(ctx, vmID); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "vm_delete_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"message": "VM deleted successfully",
	})
}

func (h *Handler) getVMConsole(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	vmID := chi.URLParam(r, "id")

	// Get VM details
	vm, err := h.repo.GetVMByID(ctx, vmID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "vm_get_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}
	if vm == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:    "not_found",
			Message: "VM not found",
		})
		return
	}

	// Check if VM is running
	if vm.ActualState != "running" {
		h.writeError(w, http.StatusPreconditionFailed, apiError{
			Code:    "vm_not_running",
			Message: "VM must be running to access console",
		})
		return
	}

	// Return WebSocket URL for console
	// The actual WebSocket connection goes directly to agent
	agentURL := h.config.AgentURL
	if agentURL == "" {
		agentURL = "ws://localhost:9090"
	}

	// Convert http to ws
	wsURL := agentURL + "/v1/vms/console?vm_id=" + vmID + "&api_socket=" + vm.WorkspacePath + "/api.sock"

	h.writeJSON(w, http.StatusOK, map[string]any{
		"ws_url":  wsURL,
		"message": "Use WebSocket URL to connect to console",
	})
}

func (h *Handler) getVMStatus(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	vmID := chi.URLParam(r, "id")

	vm, err := h.repo.GetVMByID(ctx, vmID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "vm_get_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	if vm == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:      "not_found",
			Message:   "VM not found",
			Retryable: false,
		})
		return
	}

	// Calculate uptime if VM is running
	var uptime int64
	if vm.ActualState == "running" && vm.UpdatedAt != "" {
		// Parse the updated at time and calculate duration
		if updatedAt, err := time.Parse(time.RFC3339, vm.UpdatedAt); err == nil {
			uptime = int64(time.Since(updatedAt).Seconds())
		}
	}

	// Return lightweight status
	h.writeJSON(w, http.StatusOK, map[string]any{
		"id":              vm.ID,
		"actual_state":    vm.ActualState,
		"desired_state":   vm.DesiredState,
		"pid":             vm.CloudHypervisorPID,
		"uptime":          uptime,
		"last_error":      vm.LastError,
		"updated_at":      vm.UpdatedAt,
	})
}

func (h *Handler) getVMMetrics(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	vmID := chi.URLParam(r, "id")

	vm, err := h.repo.GetVMByID(ctx, vmID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "vm_get_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	if vm == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:      "not_found",
			Message:   "VM not found",
			Retryable: false,
		})
		return
	}

	// Check if VM is running
	if vm.ActualState != "running" {
		h.writeError(w, http.StatusPreconditionFailed, apiError{
			Code:      "vm_not_running",
			Message:   "VM must be running to retrieve metrics",
			Retryable: false,
		})
		return
	}

	// Check if VM service is available
	if h.vmService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:      "service_unavailable",
			Message:   "VM service not available",
			Retryable: true,
		})
		return
	}

	// Get metrics from agent via vm service
	metrics, err := h.vmService.GetVMMetrics(ctx, vmID, vm.CloudHypervisorPID, vm.WorkspacePath)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "metrics_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, metrics)
}
