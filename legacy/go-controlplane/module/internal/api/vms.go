package api

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/chv/chv/internal/agentapi"
	"github.com/chv/chv/internal/logger"
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

// updateVMRequest represents the fields that can be updated on a VM
type updateVMRequest struct {
	Name     *string `json:"name,omitempty"`
	VCPU     *int    `json:"vcpu,omitempty"`
	MemoryMB *int    `json:"memory_mb,omitempty"`
}

func (h *Handler) updateVM(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	vmID := chi.URLParam(r, "id")

	var req updateVMRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON",
			Retryable: false,
		})
		return
	}

	// Get existing VM
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

	// Check if VM is running - some changes require the VM to be stopped
	isRunning := vm.ActualState == "running"

	// Apply updates
	if req.Name != nil && *req.Name != "" {
		vm.Name = *req.Name
	}

	// VCPU and Memory changes require the VM to be stopped
	if req.VCPU != nil {
		if isRunning {
			h.writeError(w, http.StatusConflict, apiError{
				Code:      "vm_running",
				Message:   "Cannot change VCPU while VM is running. Please stop the VM first.",
				Retryable: false,
			})
			return
		}
		if *req.VCPU < 1 {
			h.writeError(w, http.StatusBadRequest, apiError{
				Code:      "invalid_request",
				Message:   "VCPU must be at least 1",
				Retryable: false,
			})
			return
		}
		vm.VCPU = *req.VCPU
	}

	if req.MemoryMB != nil {
		if isRunning {
			h.writeError(w, http.StatusConflict, apiError{
				Code:      "vm_running",
				Message:   "Cannot change memory while VM is running. Please stop the VM first.",
				Retryable: false,
			})
			return
		}
		if *req.MemoryMB < 64 {
			h.writeError(w, http.StatusBadRequest, apiError{
				Code:      "invalid_request",
				Message:   "Memory must be at least 64 MB",
				Retryable: false,
			})
			return
		}
		vm.MemoryMB = *req.MemoryMB
	}

	vm.UpdatedAt = time.Now().UTC().Format(time.RFC3339)

	// Save changes
	if err := h.repo.UpdateVM(ctx, vm); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "vm_update_failed",
			Message:   err.Error(),
			Retryable: true,
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

	// Parse query parameters for graceful restart
	graceful := r.URL.Query().Get("graceful") == "true"
	timeoutStr := r.URL.Query().Get("timeout")
	timeout := 60 // default 60 seconds
	if timeoutStr != "" {
		if t, err := strconv.Atoi(timeoutStr); err == nil && t > 0 {
			timeout = t
		}
	}

	var err error
	if graceful {
		err = h.vmService.RestartVMWithOptions(ctx, vmID, true, time.Duration(timeout)*time.Second)
	} else {
		err = h.vmService.RestartVM(ctx, vmID)
	}

	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "vm_restart_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"message": "VM restart initiated",
		"graceful": graceful,
		"timeout": timeout,
	})
}

func (h *Handler) shutdownVM(w http.ResponseWriter, r *http.Request) {
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

	// Parse timeout parameter
	timeoutStr := r.URL.Query().Get("timeout")
	timeout := 60 // default 60 seconds
	if timeoutStr != "" {
		if t, err := strconv.Atoi(timeoutStr); err == nil && t > 0 {
			timeout = t
		}
	}

	if err := h.vmService.ShutdownVM(ctx, vmID, time.Duration(timeout)*time.Second); err != nil {
		status := http.StatusInternalServerError
		if strings.Contains(err.Error(), "timed out") {
			status = http.StatusRequestTimeout
		}
		h.writeError(w, status, apiError{
			Code:      "vm_shutdown_failed",
			Message:   err.Error(),
			Retryable: status != http.StatusRequestTimeout,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"message": "VM shutdown completed",
		"timeout": timeout,
	})
}

func (h *Handler) forceStopVM(w http.ResponseWriter, r *http.Request) {
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

	if err := h.vmService.ForceStopVM(ctx, vmID); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "vm_force_stop_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"message": "VM force stopped",
	})
}

func (h *Handler) resetVM(w http.ResponseWriter, r *http.Request) {
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

	if err := h.vmService.ResetVM(ctx, vmID); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "vm_reset_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"message": "VM reset initiated",
	})
}

func (h *Handler) getVMBootLogs(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	vmID := chi.URLParam(r, "id")

	// Parse lines parameter
	linesStr := r.URL.Query().Get("lines")
	lines := 100 // default 100 lines
	if linesStr != "" {
		if n, err := strconv.Atoi(linesStr); err == nil && n > 0 {
			lines = n
		}
	}

	// Get boot logs from repository
	logEntries, err := h.repo.GetVMBootLogs(ctx, vmID, lines)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "boot_logs_fetch_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	// Convert to response format
	var linesResponse []map[string]any
	for _, entry := range logEntries {
		linesResponse = append(linesResponse, map[string]any{
			"line_number": entry.LineNumber,
			"content":     entry.Content,
			"timestamp":   entry.Timestamp.Format(time.RFC3339),
		})
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"vm_id": vmID,
		"lines": linesResponse,
		"count": len(linesResponse),
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
	// Uses Unix Domain Socket via WebSocket for reliable bidirectional communication
	wsURL := scheme + "://" + host + "/api/v1/vms/console/ws?vm_id=" + vmID + "&workspace_path=" + vm.WorkspacePath
	if token != "" {
		wsURL += "&token=" + token
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"ws_url":   wsURL,
		"type":     "serial",
		"message":  "Use WebSocket URL to connect to serial console",
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
		m, err := h.vmService.GetVMMetrics(ctx, vm.ID, vm.CloudHypervisorPID, vm.WorkspacePath)
		if err != nil {
			logger.L().Warn("Failed to get VM metrics", logger.F("vm_id", vm.ID), logger.ErrorField(err))
		} else {
			metrics = m
		}
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

// validateVMs validates running VMs against the expected state
func (h *Handler) validateVMs(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)

	// Get agent client
	agentClient := h.vmService.GetAgentClient()
	if agentClient == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:      "service_unavailable",
			Message:   "Agent client not available",
			Retryable: true,
		})
		return
	}

	// Get all VMs from database that should be running
	vms, err := h.repo.ListVMsByDesiredState(ctx, "running")
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   fmt.Sprintf("Failed to list VMs: %v", err),
			Retryable: true,
		})
		return
	}

	// Build list of expected VM IDs
	var expectedVMIDs []string
	for _, vm := range vms {
		expectedVMIDs = append(expectedVMIDs, vm.ID)
	}

	// Call agent to validate running VMs
	validationReq := &agentapi.VMValidationRequest{
		ExpectedVMIDs: expectedVMIDs,
	}

	validationResp, err := agentClient.ValidateRunningVMs(ctx, validationReq)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "validation_failed",
			Message:   fmt.Sprintf("Failed to validate VMs: %v", err),
			Retryable: true,
		})
		return
	}

	// Return the validation results
	h.writeJSON(w, http.StatusOK, map[string]any{
		"validation": validationResp,
		"expected":   expectedVMIDs,
	})
}
