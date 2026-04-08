package handlers

import (
	"encoding/json"
	"net/http"

	"github.com/chv/chv/internal/agent/services"
	"github.com/chv/chv/internal/agentapi"
)

type VMHandler struct {
	vmService     *services.VMManagementService
	healthService *services.VMHealthService
	consoleService *services.VMConsoleService
}

func NewVMHandler(vmService *services.VMManagementService, healthService *services.VMHealthService, consoleService *services.VMConsoleService) *VMHandler {
	return &VMHandler{
		vmService:      vmService,
		healthService:  healthService,
		consoleService: consoleService,
	}
}

func (h *VMHandler) StartVM(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req agentapi.VMStartRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	if req.VMID == "" || req.DiskPath == "" {
		respondError(w, http.StatusBadRequest, "invalid_request", "vm_id and disk_path are required", false)
		return
	}

	resp, err := h.vmService.StartVM(ctx, &req)
	if err != nil {
		respondError(w, http.StatusInternalServerError, "vm_start_failed", err.Error(), false)
		return
	}

	respondJSON(w, http.StatusOK, resp)
}

func (h *VMHandler) StopVM(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req agentapi.VMStopRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	if req.VMID == "" {
		respondError(w, http.StatusBadRequest, "invalid_request", "vm_id is required", false)
		return
	}

	resp, err := h.vmService.StopVM(ctx, &req)
	if err != nil {
		respondError(w, http.StatusInternalServerError, "vm_stop_failed", err.Error(), false)
		return
	}

	respondJSON(w, http.StatusOK, resp)
}

func (h *VMHandler) GetVMStatus(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req agentapi.VMStatusRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	if req.VMID == "" {
		respondError(w, http.StatusBadRequest, "invalid_request", "vm_id is required", false)
		return
	}

	resp, err := h.vmService.GetVMStatus(ctx, &req)
	if err != nil {
		respondError(w, http.StatusInternalServerError, "vm_status_failed", err.Error(), true)
		return
	}

	respondJSON(w, http.StatusOK, resp)
}

func (h *VMHandler) ListRunningVMs(w http.ResponseWriter, r *http.Request) {
	vms := h.vmService.ListRunningVMs()
	respondJSON(w, http.StatusOK, map[string]any{
		"vms": vms,
	})
}

func (h *VMHandler) GetVMMetrics(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req agentapi.VMMetricsRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	if req.VMID == "" || req.APISocket == "" {
		respondError(w, http.StatusBadRequest, "invalid_request", "vm_id and api_socket are required", false)
		return
	}

	resp, err := h.healthService.GetMetrics(ctx, &req)
	if err != nil {
		respondError(w, http.StatusInternalServerError, "metrics_failed", err.Error(), true)
		return
	}

	respondJSON(w, http.StatusOK, resp)
}

func (h *VMHandler) HealthCheck(w http.ResponseWriter, r *http.Request) {
	var req struct {
		APISocket string `json:"api_socket"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	if req.APISocket == "" {
		respondError(w, http.StatusBadRequest, "invalid_request", "api_socket is required", false)
		return
	}

	status := h.healthService.HealthCheck(req.APISocket)
	respondJSON(w, http.StatusOK, status)
}

func (h *VMHandler) Console(w http.ResponseWriter, r *http.Request) {
	// Get VM ID from query params
	vmID := r.URL.Query().Get("vm_id")
	apiSocket := r.URL.Query().Get("api_socket")

	if vmID == "" || apiSocket == "" {
		http.Error(w, "vm_id and api_socket query params required", http.StatusBadRequest)
		return
	}

	// Handle WebSocket upgrade
	if err := h.consoleService.HandleWebSocket(w, r, vmID, apiSocket); err != nil {
		// Error already handled by upgrade or sent to client
		return
	}
}
