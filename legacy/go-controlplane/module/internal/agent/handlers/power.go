package handlers

import (
	"encoding/json"
	"net/http"

	"github.com/chv/chv/internal/agentapi"
)

// ShutdownVM handles graceful VM shutdown via ACPI
func (h *VMHandler) ShutdownVM(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req agentapi.VMShutdownRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	if req.VMID == "" {
		respondError(w, http.StatusBadRequest, "invalid_request", "vm_id is required", false)
		return
	}

	resp, err := h.vmService.ShutdownVM(ctx, &req)
	if err != nil {
		respondError(w, http.StatusInternalServerError, "vm_shutdown_failed", err.Error(), false)
		return
	}

	respondJSON(w, http.StatusOK, resp)
}

// ForceStopVM handles immediate VM termination
func (h *VMHandler) ForceStopVM(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req agentapi.VMForceStopRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	if req.VMID == "" {
		respondError(w, http.StatusBadRequest, "invalid_request", "vm_id is required", false)
		return
	}

	resp, err := h.vmService.ForceStopVM(ctx, &req)
	if err != nil {
		respondError(w, http.StatusInternalServerError, "vm_force_stop_failed", err.Error(), false)
		return
	}

	respondJSON(w, http.StatusOK, resp)
}

// RebootVM handles VM reboot
func (h *VMHandler) RebootVM(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req agentapi.VMResetRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	if req.VMID == "" {
		respondError(w, http.StatusBadRequest, "invalid_request", "vm_id is required", false)
		return
	}

	resp, err := h.vmService.RebootVM(ctx, &req)
	if err != nil {
		respondError(w, http.StatusInternalServerError, "vm_reboot_failed", err.Error(), false)
		return
	}

	respondJSON(w, http.StatusOK, resp)
}

// PauseVM handles pausing a VM
func (h *VMHandler) PauseVM(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req agentapi.VMPauseRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	if req.VMID == "" {
		respondError(w, http.StatusBadRequest, "invalid_request", "vm_id is required", false)
		return
	}

	if err := h.vmService.PauseVM(ctx, req.VMID); err != nil {
		respondError(w, http.StatusInternalServerError, "vm_pause_failed", err.Error(), false)
		return
	}

	respondJSON(w, http.StatusOK, agentapi.VMPauseResponse{
		Success: true,
		Message: "VM paused successfully",
	})
}

// ResumeVM handles resuming a paused VM
func (h *VMHandler) ResumeVM(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req agentapi.VMResumeRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	if req.VMID == "" {
		respondError(w, http.StatusBadRequest, "invalid_request", "vm_id is required", false)
		return
	}

	if err := h.vmService.ResumeVM(ctx, req.VMID); err != nil {
		respondError(w, http.StatusInternalServerError, "vm_resume_failed", err.Error(), false)
		return
	}

	respondJSON(w, http.StatusOK, agentapi.VMResumeResponse{
		Success: true,
		Message: "VM resumed successfully",
	})
}

// ResizeVM handles VM resize (CPU/memory hot-plug)
func (h *VMHandler) ResizeVM(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req agentapi.VMResizeRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	if req.VMID == "" {
		respondError(w, http.StatusBadRequest, "invalid_request", "vm_id is required", false)
		return
	}

	if req.VCPUs == 0 && req.MemoryMB == 0 {
		respondError(w, http.StatusBadRequest, "invalid_request", "At least one of vcpus or memory_mb must be specified", false)
		return
	}

	resp, err := h.vmService.ResizeVM(ctx, &req)
	if err != nil {
		respondError(w, http.StatusInternalServerError, "vm_resize_failed", err.Error(), false)
		return
	}

	respondJSON(w, http.StatusOK, resp)
}

// GetVMState returns the current state of a VM
func (h *VMHandler) GetVMState(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req agentapi.VMStateRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "invalid_request", "Request body must be valid JSON", false)
		return
	}

	if req.VMID == "" {
		respondError(w, http.StatusBadRequest, "invalid_request", "vm_id is required", false)
		return
	}

	state, err := h.vmService.GetVMState(ctx, req.VMID)
	if err != nil {
		respondError(w, http.StatusInternalServerError, "vm_state_failed", err.Error(), true)
		return
	}

	respondJSON(w, http.StatusOK, agentapi.VMStateResponse{
		State:   state,
		Running: state == "Running",
	})
}
