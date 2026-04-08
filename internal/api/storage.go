package api

import (
	"net/http"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
)

type createStoragePoolRequest struct {
	Name             string `json:"name"`
	PoolType         string `json:"pool_type"`
	Path             string `json:"path"`
	CapacityBytes    int64  `json:"capacity_bytes,omitempty"`
	AllocatableBytes int64  `json:"allocatable_bytes,omitempty"`
}

func (h *Handler) createStoragePool(w http.ResponseWriter, r *http.Request) {
	var req createStoragePoolRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON.",
			Retryable: false,
		})
		return
	}

	if req.Name == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Storage pool name is required.",
			Retryable: false,
		})
		return
	}

	if req.PoolType != h.config.DefaultPoolType {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Only '" + h.config.DefaultPoolType + "' pool type is supported in MVP-1.",
			Retryable: false,
			Hint:      "Use pool_type '" + h.config.DefaultPoolType + "' for local storage.",
		})
		return
	}

	if req.Path == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Storage pool path is required.",
			Retryable: false,
		})
		return
	}

	ctx := requestContext(r)

	existing, err := h.repo.GetStoragePoolByName(ctx, req.Name)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "storage_pool_create_failed",
			Message:   "Could not check for existing storage pool.",
			Retryable: true,
		})
		return
	}
	if existing != nil {
		h.writeError(w, http.StatusConflict, apiError{
			Code:         "already_exists",
			Message:      "A storage pool with this name already exists.",
			ResourceType: "storage_pool",
			ResourceID:   existing.ID,
			Retryable:    false,
		})
		return
	}

	pool := &models.StoragePool{
		ID:               uuid.NewString(),
		Name:             req.Name,
		PoolType:         req.PoolType,
		Path:             req.Path,
		IsDefault:        false,
		Status:           "ready",
		CapacityBytes:    req.CapacityBytes,
		AllocatableBytes: req.AllocatableBytes,
		CreatedAt:        time.Now().UTC().Format(time.RFC3339),
	}

	if err := h.repo.CreateStoragePool(ctx, pool); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "storage_pool_create_failed",
			Message:   "Could not create storage pool.",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusCreated, pool)
}
