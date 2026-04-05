package api

import (
	"net/http"

	"github.com/chv/chv/pkg/uuidx"
	"github.com/go-chi/chi/v5"
	"github.com/google/uuid"
)

// listOperations handles GET /api/v1/operations
func (h *Handler) listOperations(w http.ResponseWriter, r *http.Request) {
	// Parse optional filters from query params
	filters := make(map[string]interface{})

	if resourceType := r.URL.Query().Get("resource_type"); resourceType != "" {
		filters["resource_type"] = resourceType
	}

	if status := r.URL.Query().Get("status"); status != "" {
		filters["status"] = status
	}

	if opType := r.URL.Query().Get("operation_type"); opType != "" {
		filters["operation_type"] = opType
	}

	if resourceID := r.URL.Query().Get("resource_id"); resourceID != "" {
		if id, err := uuid.Parse(resourceID); err == nil {
			filters["resource_id"] = id
		}
	}

	if nodeID := r.URL.Query().Get("node_id"); nodeID != "" {
		if id, err := uuid.Parse(nodeID); err == nil {
			filters["node_id"] = id
		}
	}

	operations, err := h.store.ListOperations(r.Context(), filters)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to list operations")
		return
	}

	h.jsonResponse(w, http.StatusOK, operations)
}

// getOperation handles GET /api/v1/operations/:id
func (h *Handler) getOperation(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	opID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid operation ID")
		return
	}

	operation, err := h.store.GetOperation(r.Context(), opID)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get operation")
		return
	}

	if operation == nil {
		h.errorResponse(w, http.StatusNotFound, "NOT_FOUND", "Operation not found")
		return
	}

	h.jsonResponse(w, http.StatusOK, operation)
}

// getOperationLogs handles GET /api/v1/operations/:id/logs
func (h *Handler) getOperationLogs(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	opID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid operation ID")
		return
	}

	// First verify operation exists
	operation, err := h.store.GetOperation(r.Context(), opID)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get operation")
		return
	}

	if operation == nil {
		h.errorResponse(w, http.StatusNotFound, "NOT_FOUND", "Operation not found")
		return
	}

	logs, err := h.store.GetOperationLogs(r.Context(), opID)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get operation logs")
		return
	}

	h.jsonResponse(w, http.StatusOK, logs)
}
