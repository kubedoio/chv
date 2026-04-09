package api

import (
	"encoding/json"
	"net/http"
	"strings"
	"time"

	"github.com/chv/chv/internal/agentapi"
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

	// Return WebSocket URL for console (via controller proxy)
	// Derive WebSocket URL from request
	scheme := "ws"
	if r.TLS != nil {
		scheme = "wss"
	}
	host := r.Host
	if host == "" {
		host = r.Header.Get("Host")
	}

	// Extract token from Authorization header to pass in WebSocket URL
	// (browsers can't send custom headers when opening WebSocket connections)
	authHeader := r.Header.Get("Authorization")
	token := ""
	if len(authHeader) > 7 && strings.ToLower(authHeader[:7]) == "bearer " {
		token = authHeader[7:]
	}

	// Build WebSocket URL pointing to controller's proxy endpoint
	wsURL := scheme + "://" + host + "/api/v1/vms/console/ws?vm_id=" + vmID + "&api_socket=" + vm.WorkspacePath + "/api.sock"
	if token != "" {
		wsURL += "&token=" + token
	}

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

// getVMMetrics retrieves VM metrics and historical data
func (h *Handler) getVMMetrics(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	vmID := chi.URLParam(r, "id")

	vm, err := h.repo.GetVMByID(ctx, vmID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code: "vm_get_failed", Message: err.Error(), Retryable: true,
		})
		return
	}
	if vm == nil {
		h.writeError(w, http.StatusNotFound, apiError{Code: "not_found", Message: "VM not found"})
		return
	}

	var metrics *agentapi.VMMetricsResponse
	if vm.ActualState == "running" && vm.CloudHypervisorPID > 0 && h.vmService != nil {
		metrics, _ = h.vmService.GetVMMetrics(ctx, vm.ID, vm.CloudHypervisorPID, vm.WorkspacePath)
	}

	history := []agentapi.VMMetricsResponse{}
	if h.vmService != nil {
		history = h.vmService.GetVMMetricsHistory(vmID)
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"id":      vmID,
		"current": metrics,
		"history": history,
	})
}

type bulkVMRequest struct {
	IDs []string `json:"ids"`
}

type bulkVMResponse struct {
	Results map[string]string `json:"results"` // ID -> Message or "OK"
}

// bulkStartVMs starts multiple VMs
func (h *Handler) bulkStartVMs(w http.ResponseWriter, r *http.Request) {
	var req bulkVMRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{Code: "invalid_request", Message: err.Error()})
		return
	}

	ctx := requestContext(r)
	results := make(map[string]string)

	for _, id := range req.IDs {
		if err := h.vmService.StartVM(ctx, id); err != nil {
			results[id] = err.Error()
		} else {
			results[id] = "OK"
		}
	}

	h.writeJSON(w, http.StatusOK, bulkVMResponse{Results: results})
}

// bulkStopVMs stops multiple VMs
func (h *Handler) bulkStopVMs(w http.ResponseWriter, r *http.Request) {
	var req bulkVMRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{Code: "invalid_request", Message: err.Error()})
		return
	}

	ctx := requestContext(r)
	results := make(map[string]string)

	for _, id := range req.IDs {
		if err := h.vmService.StopVM(ctx, id); err != nil {
			results[id] = err.Error()
		} else {
			results[id] = "OK"
		}
	}

	h.writeJSON(w, http.StatusOK, bulkVMResponse{Results: results})
}

// bulkDeleteVMs deletes multiple VMs
func (h *Handler) bulkDeleteVMs(w http.ResponseWriter, r *http.Request) {
	var req bulkVMRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{Code: "invalid_request", Message: err.Error()})
		return
	}

	ctx := requestContext(r)
	results := make(map[string]string)

	for _, id := range req.IDs {
		if err := h.vmService.DeleteVM(ctx, id); err != nil {
			results[id] = err.Error()
		} else {
			results[id] = "OK"
		}
	}

	h.writeJSON(w, http.StatusOK, bulkVMResponse{Results: results})
}
