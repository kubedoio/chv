package api

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/pkg/uuidx"
	"github.com/go-chi/chi/v5"
)

// StoragePoolCreateRequest represents a storage pool creation request.
type StoragePoolCreateRequest struct {
	Name             string `json:"name"`
	NodeID           string `json:"node_id,omitempty"`
	PoolType         string `json:"pool_type"`
	PathOrExport     string `json:"path_or_export"`
	SupportsResize   bool   `json:"supports_online_resize"`
}

func (h *Handler) createStoragePool(w http.ResponseWriter, r *http.Request) {
	var req StoragePoolCreateRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "Invalid request body")
		return
	}
	
	if req.Name == "" || req.PoolType == "" || req.PathOrExport == "" {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "name, pool_type, and path_or_export are required")
		return
	}
	
	poolType := models.StoragePoolType(req.PoolType)
	if poolType != models.StoragePoolTypeLocal && poolType != models.StoragePoolTypeNFS {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_TYPE", "pool_type must be 'local' or 'nfs'")
		return
	}
	
	pool := &models.StoragePool{
		ID:                   uuidx.New(),
		Name:                 req.Name,
		PoolType:             poolType,
		PathOrExport:         req.PathOrExport,
		Status:               models.StoragePoolStatusActive,
		SupportsOnlineResize: req.SupportsResize,
		SupportsClone:        false,
		SupportsSnapshot:     false,
		CreatedAt:            time.Now(),
	}
	
	if req.NodeID != "" {
		nodeID, err := uuidx.Parse(req.NodeID)
		if err != nil {
			h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid node ID")
			return
		}
		pool.NodeID = &nodeID
	}
	
	if err := h.store.CreateStoragePool(r.Context(), pool); err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to create storage pool")
		return
	}
	
	h.jsonResponse(w, http.StatusCreated, pool)
}

func (h *Handler) listStoragePools(w http.ResponseWriter, r *http.Request) {
	pools, err := h.store.ListStoragePools(r.Context())
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to list storage pools")
		return
	}
	
	h.jsonResponse(w, http.StatusOK, pools)
}

func (h *Handler) getStoragePool(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	poolID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid storage pool ID")
		return
	}
	
	pool, err := h.store.GetStoragePool(r.Context(), poolID)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get storage pool")
		return
	}
	
	if pool == nil {
		h.errorResponse(w, http.StatusNotFound, "NOT_FOUND", "Storage pool not found")
		return
	}
	
	h.jsonResponse(w, http.StatusOK, pool)
}

func (h *Handler) deleteStoragePool(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	poolID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid storage pool ID")
		return
	}

	// Check if storage pool exists
	pool, err := h.store.GetStoragePool(r.Context(), poolID)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get storage pool")
		return
	}
	if pool == nil {
		h.errorResponse(w, http.StatusNotFound, "NOT_FOUND", "Storage pool not found")
		return
	}

	// Delete the storage pool
	if err := h.store.DeleteStoragePool(r.Context(), poolID); err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to delete storage pool")
		return
	}

	w.WriteHeader(http.StatusNoContent)
}
