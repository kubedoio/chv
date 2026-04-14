package api

import (
	"net/http"

	"github.com/chv/chv/internal/models"
)

// Stub implementations for Phase 2B - will be fully implemented in Phase 2D

// listUsers handles GET /api/v1/users
func (h *Handler) listUsers(w http.ResponseWriter, r *http.Request) {
	h.writeJSON(w, http.StatusOK, []models.User{})
}

// createUser handles POST /api/v1/users
func (h *Handler) createUser(w http.ResponseWriter, r *http.Request) {
	h.writeError(w, http.StatusNotImplemented, apiError{
		Code:      "not_implemented",
		Message:   "User management not yet implemented",
		Retryable: false,
	})
}

// getUser handles GET /api/v1/users/{id}
func (h *Handler) getUser(w http.ResponseWriter, r *http.Request) {
	h.writeError(w, http.StatusNotImplemented, apiError{
		Code:      "not_implemented",
		Message:   "User management not yet implemented",
		Retryable: false,
	})
}

// updateUser handles PATCH /api/v1/users/{id}
func (h *Handler) updateUser(w http.ResponseWriter, r *http.Request) {
	h.writeError(w, http.StatusNotImplemented, apiError{
		Code:      "not_implemented",
		Message:   "User management not yet implemented",
		Retryable: false,
	})
}

// deleteUser handles DELETE /api/v1/users/{id}
func (h *Handler) deleteUser(w http.ResponseWriter, r *http.Request) {
	h.writeError(w, http.StatusNotImplemented, apiError{
		Code:      "not_implemented",
		Message:   "User management not yet implemented",
		Retryable: false,
	})
}

// resetPassword handles POST /api/v1/users/{id}/reset-password
func (h *Handler) resetPassword(w http.ResponseWriter, r *http.Request) {
	h.writeError(w, http.StatusNotImplemented, apiError{
		Code:      "not_implemented",
		Message:   "User management not yet implemented",
		Retryable: false,
	})
}

// listRoles handles GET /api/v1/roles
func (h *Handler) listRoles(w http.ResponseWriter, r *http.Request) {
	h.writeJSON(w, http.StatusOK, []map[string]string{
		{"id": "admin", "name": "Administrator"},
		{"id": "user", "name": "User"},
	})
}

// listAuditLogs handles GET /api/v1/audit-logs
func (h *Handler) listAuditLogs(w http.ResponseWriter, r *http.Request) {
	h.writeJSON(w, http.StatusOK, []map[string]string{})
}
