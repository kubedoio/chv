package api

import (
	"net/http"

	"github.com/chv/chv/internal/quota"
	"github.com/go-chi/chi/v5"
)

// QuotaResponse represents a quota in API responses
type QuotaResponse struct {
	ID           string `json:"id"`
	UserID       string `json:"user_id"`
	MaxVMs       int    `json:"max_vms"`
	MaxCPUs      int    `json:"max_cpu"`
	MaxMemoryGB  int    `json:"max_memory_gb"`
	MaxStorageGB int    `json:"max_storage_gb"`
	MaxNetworks  int    `json:"max_networks"`
	CreatedAt    string `json:"created_at,omitempty"`
	UpdatedAt    string `json:"updated_at,omitempty"`
}

// UsageResponse represents usage in API responses
type UsageResponse struct {
	VMs       int `json:"vms"`
	CPUs      int `json:"cpus"`
	MemoryGB  int `json:"memory_gb"`
	StorageGB int `json:"storage_gb"`
	Networks  int `json:"networks"`
}

// UsageWithQuotaResponse combines usage and quota
type UsageWithQuotaResponse struct {
	Quota QuotaResponse `json:"quota"`
	Usage UsageResponse   `json:"usage"`
}

// SetQuotaRequest represents a request to set quota
type SetQuotaRequest struct {
	UserID       string `json:"user_id"`
	MaxVMs       *int   `json:"max_vms,omitempty"`
	MaxCPUs      *int   `json:"max_cpu,omitempty"`
	MaxMemoryGB  *int   `json:"max_memory_gb,omitempty"`
	MaxStorageGB *int   `json:"max_storage_gb,omitempty"`
	MaxNetworks  *int   `json:"max_networks,omitempty"`
}

// UpdateQuotaRequest represents a request to update quota
type UpdateQuotaRequest struct {
	MaxVMs       *int `json:"max_vms,omitempty"`
	MaxCPUs      *int `json:"max_cpu,omitempty"`
	MaxMemoryGB  *int `json:"max_memory_gb,omitempty"`
	MaxStorageGB *int `json:"max_storage_gb,omitempty"`
	MaxNetworks  *int `json:"max_networks,omitempty"`
}

// CheckQuotaRequest represents a quota check request
type CheckQuotaRequest struct {
	Resource string `json:"resource"`
	Amount   int    `json:"amount"`
}

// CheckQuotaResponse represents a quota check response
type CheckQuotaResponse struct {
	Allowed   bool   `json:"allowed"`
	Resource  string `json:"resource"`
	Requested int    `json:"requested"`
	Current   int    `json:"current"`
	Limit     int    `json:"limit"`
	Message   string `json:"message,omitempty"`
}

// quotaToResponse converts a quota.Quota to QuotaResponse
func quotaToResponse(q *quota.Quota) QuotaResponse {
	return QuotaResponse{
		ID:           q.ID,
		UserID:       q.UserID,
		MaxVMs:       q.MaxVMs,
		MaxCPUs:      q.MaxCPUs,
		MaxMemoryGB:  q.MaxMemoryGB,
		MaxStorageGB: q.MaxStorageGB,
		MaxNetworks:  q.MaxNetworks,
		CreatedAt:    q.CreatedAt,
		UpdatedAt:    q.UpdatedAt,
	}
}

// usageToResponse converts a quota.Usage to UsageResponse
func usageToResponse(u *quota.Usage) UsageResponse {
	return UsageResponse{
		VMs:       u.VMs,
		CPUs:      u.CPUs,
		MemoryGB:  u.MemoryGB,
		StorageGB: u.StorageGB,
		Networks:  u.Networks,
	}
}

// listQuotas handles GET /api/v1/quotas
func (h *Handler) listQuotas(w http.ResponseWriter, r *http.Request) {
	quotas, err := h.repo.ListQuotas(r.Context())
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "list_failed",
			Message: "Failed to list quotas: " + err.Error(),
		})
		return
	}

	response := make([]QuotaResponse, len(quotas))
	for i, q := range quotas {
		response[i] = quotaToResponse(&q)
	}

	h.writeJSON(w, http.StatusOK, response)
}

// createQuota handles POST /api/v1/quotas
func (h *Handler) createQuota(w http.ResponseWriter, r *http.Request) {
	var req SetQuotaRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:    "invalid_request",
			Message: "Invalid request body: " + err.Error(),
		})
		return
	}

	if req.UserID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:    "missing_user_id",
			Message: "user_id is required",
		})
		return
	}

	// Create quota with defaults
	q := quota.DefaultQuota(req.UserID)

	// Apply overrides
	if req.MaxVMs != nil {
		q.MaxVMs = *req.MaxVMs
	}
	if req.MaxCPUs != nil {
		q.MaxCPUs = *req.MaxCPUs
	}
	if req.MaxMemoryGB != nil {
		q.MaxMemoryGB = *req.MaxMemoryGB
	}
	if req.MaxStorageGB != nil {
		q.MaxStorageGB = *req.MaxStorageGB
	}
	if req.MaxNetworks != nil {
		q.MaxNetworks = *req.MaxNetworks
	}

	// Create quota service and set quota
	quotaService := quota.NewService(h.repo)
	if err := quotaService.SetQuota(r.Context(), q); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "create_failed",
			Message: "Failed to create quota: " + err.Error(),
		})
		return
	}

	h.writeJSON(w, http.StatusCreated, quotaToResponse(q))
}

// getQuota handles GET /api/v1/quotas/{userId}
func (h *Handler) getQuota(w http.ResponseWriter, r *http.Request) {
	userID := chi.URLParam(r, "userId")
	if userID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:    "missing_user_id",
			Message: "User ID is required",
		})
		return
	}

	quotaService := quota.NewService(h.repo)
	q, err := quotaService.GetQuota(r.Context(), userID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "get_failed",
			Message: "Failed to get quota: " + err.Error(),
		})
		return
	}

	h.writeJSON(w, http.StatusOK, quotaToResponse(q))
}

// updateQuota handles PATCH /api/v1/quotas/{userId}
func (h *Handler) updateQuota(w http.ResponseWriter, r *http.Request) {
	userID := chi.URLParam(r, "userId")
	if userID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:    "missing_user_id",
			Message: "User ID is required",
		})
		return
	}

	var req UpdateQuotaRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:    "invalid_request",
			Message: "Invalid request body: " + err.Error(),
		})
		return
	}

	quotaService := quota.NewService(h.repo)

	// Get existing quota or create default
	q, err := quotaService.GetQuota(r.Context(), userID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "get_failed",
			Message: "Failed to get quota: " + err.Error(),
		})
		return
	}

	// Apply updates
	if req.MaxVMs != nil {
		q.MaxVMs = *req.MaxVMs
	}
	if req.MaxCPUs != nil {
		q.MaxCPUs = *req.MaxCPUs
	}
	if req.MaxMemoryGB != nil {
		q.MaxMemoryGB = *req.MaxMemoryGB
	}
	if req.MaxStorageGB != nil {
		q.MaxStorageGB = *req.MaxStorageGB
	}
	if req.MaxNetworks != nil {
		q.MaxNetworks = *req.MaxNetworks
	}

	if err := quotaService.SetQuota(r.Context(), q); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "update_failed",
			Message: "Failed to update quota: " + err.Error(),
		})
		return
	}

	h.writeJSON(w, http.StatusOK, quotaToResponse(q))
}

// getMyQuota handles GET /api/v1/quotas/me
func (h *Handler) getMyQuota(w http.ResponseWriter, r *http.Request) {
	// Get user ID from context (set by auth middleware)
	userID := r.Context().Value("user_id")
	if userID == nil {
		h.writeError(w, http.StatusUnauthorized, apiError{
			Code:    "unauthorized",
			Message: "User not authenticated",
		})
		return
	}

	quotaService := quota.NewService(h.repo)
	q, err := quotaService.GetQuota(r.Context(), userID.(string))
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "get_failed",
			Message: "Failed to get quota: " + err.Error(),
		})
		return
	}

	h.writeJSON(w, http.StatusOK, quotaToResponse(q))
}

// getUsage handles GET /api/v1/usage
func (h *Handler) getUsage(w http.ResponseWriter, r *http.Request) {
	// Get user ID from context
	userID := r.Context().Value("user_id")
	if userID == nil {
		h.writeError(w, http.StatusUnauthorized, apiError{
			Code:    "unauthorized",
			Message: "User not authenticated",
		})
		return
	}

	quotaService := quota.NewService(h.repo)
	usageWithQuota, err := quotaService.GetUsageWithQuota(r.Context(), userID.(string))
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "get_failed",
			Message: "Failed to get usage: " + err.Error(),
		})
		return
	}

	response := UsageWithQuotaResponse{
		Quota: quotaToResponse(&usageWithQuota.Quota),
		Usage: usageToResponse(&usageWithQuota.Usage),
	}

	h.writeJSON(w, http.StatusOK, response)
}

// getUserUsage handles GET /api/v1/quotas/{userId}/usage
func (h *Handler) getUserUsage(w http.ResponseWriter, r *http.Request) {
	userID := chi.URLParam(r, "userId")
	if userID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:    "missing_user_id",
			Message: "User ID is required",
		})
		return
	}

	quotaService := quota.NewService(h.repo)
	usageWithQuota, err := quotaService.GetUsageWithQuota(r.Context(), userID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "get_failed",
			Message: "Failed to get usage: " + err.Error(),
		})
		return
	}

	response := UsageWithQuotaResponse{
		Quota: quotaToResponse(&usageWithQuota.Quota),
		Usage: usageToResponse(&usageWithQuota.Usage),
	}

	h.writeJSON(w, http.StatusOK, response)
}

// checkQuota handles POST /api/v1/quotas/check
func (h *Handler) checkQuota(w http.ResponseWriter, r *http.Request) {
	var req CheckQuotaRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:    "invalid_request",
			Message: "Invalid request body: " + err.Error(),
		})
		return
	}

	if req.Resource == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:    "missing_resource",
			Message: "resource is required",
		})
		return
	}

	// Get user ID from context
	userID := r.Context().Value("user_id")
	if userID == nil {
		h.writeError(w, http.StatusUnauthorized, apiError{
			Code:    "unauthorized",
			Message: "User not authenticated",
		})
		return
	}

	quotaService := quota.NewService(h.repo)
	result, err := quotaService.CheckQuotaDetailed(r.Context(), userID.(string), req.Resource, req.Amount)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "check_failed",
			Message: "Failed to check quota: " + err.Error(),
		})
		return
	}

	response := CheckQuotaResponse{
		Allowed:   result.Allowed,
		Resource:  result.Resource,
		Requested: result.Requested,
		Current:   result.Current,
		Limit:     result.Limit,
		Message:   result.Message,
	}

	status := http.StatusOK
	if !result.Allowed {
		status = http.StatusForbidden
	}

	h.writeJSON(w, status, response)
}
