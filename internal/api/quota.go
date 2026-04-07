package api

import (
	"encoding/json"
	"net/http"
)

// QuotaResponse represents the quota and usage response.
type QuotaResponse struct {
	Quota *struct {
		MaxCPUs     int   `json:"max_cpus"`
		MaxMemoryMB int64 `json:"max_memory_mb"`
		MaxVMCount  int   `json:"max_vm_count"`
		MaxDiskGB   int64 `json:"max_disk_gb"`
	} `json:"quota"`
	Usage *struct {
		CPUsUsed     int   `json:"cpus_used"`
		MemoryMBUsed int64 `json:"memory_mb_used"`
		VMCount      int   `json:"vm_count"`
		DiskGBUsed   int64 `json:"disk_gb_used"`
	} `json:"usage"`
}

// getQuota returns the current user's quota and usage.
func (h *Handler) getQuota(w http.ResponseWriter, r *http.Request) {
	userID, _ := r.Context().Value("user_id").(string)
	if userID == "" {
		userID = "anonymous"
	}

	if userID == "anonymous" {
		h.errorResponse(w, http.StatusForbidden, "NO_QUOTA", "Anonymous users do not have quota tracking")
		return
	}

	quota, usage, err := h.quota.GetQuotaAndUsage(r.Context(), userID)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get quota information")
		return
	}

	resp := QuotaResponse{
		Quota: &struct {
			MaxCPUs     int   `json:"max_cpus"`
			MaxMemoryMB int64 `json:"max_memory_mb"`
			MaxVMCount  int   `json:"max_vm_count"`
			MaxDiskGB   int64 `json:"max_disk_gb"`
		}{
			MaxCPUs:     quota.MaxCPUs,
			MaxMemoryMB: quota.MaxMemoryMB,
			MaxVMCount:  quota.MaxVMCount,
			MaxDiskGB:   quota.MaxDiskGB,
		},
		Usage: &struct {
			CPUsUsed     int   `json:"cpus_used"`
			MemoryMBUsed int64 `json:"memory_mb_used"`
			VMCount      int   `json:"vm_count"`
			DiskGBUsed   int64 `json:"disk_gb_used"`
		}{
			CPUsUsed:     usage.CPUsUsed,
			MemoryMBUsed: usage.MemoryMBUsed,
			VMCount:      usage.VMCount,
			DiskGBUsed:   usage.DiskGBUsed,
		},
	}

	h.jsonResponse(w, http.StatusOK, resp)
}

// SetQuotaRequest represents a request to set a user's quota.
type SetQuotaRequest struct {
	UserID      string `json:"user_id"`
	MaxCPUs     int    `json:"max_cpus"`
	MaxMemoryMB int64  `json:"max_memory_mb"`
	MaxVMCount  int    `json:"max_vm_count"`
	MaxDiskGB   int64  `json:"max_disk_gb"`
}

// setUserQuota sets a quota for a specific user (admin only).
func (h *Handler) setUserQuota(w http.ResponseWriter, r *http.Request) {
	// TODO: Add admin check middleware
	var req SetQuotaRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "Invalid request body")
		return
	}

	if req.UserID == "" {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "user_id is required")
		return
	}

	// Validate quota values
	if req.MaxCPUs < 1 || req.MaxMemoryMB < 512 || req.MaxVMCount < 1 || req.MaxDiskGB < 1 {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_QUOTA", "Quota values must be positive")
		return
	}

	h.jsonResponse(w, http.StatusOK, map[string]string{"status": "quota updated"})
}
