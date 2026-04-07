package api

import (
	"context"
	"encoding/json"
	"net/http"
	"time"
)

// HealthResponse represents the health check response.
type HealthResponse struct {
	Status     string            `json:"status"`
	Version    string            `json:"version"`
	Components map[string]string `json:"components"`
	Timestamp  time.Time         `json:"timestamp"`
}

// handleHealthV1 returns a detailed health status for API v1.
func (h *Handler) handleHealthV1(w http.ResponseWriter, r *http.Request) {
	health := HealthResponse{
		Status:     "healthy",
		Version:    "v0.1.0",
		Components: make(map[string]string),
		Timestamp:  time.Now().UTC(),
	}

	// Check database
	if err := h.checkDatabaseConn(r.Context()); err != nil {
		health.Status = "unhealthy"
		health.Components["database"] = "disconnected"
		w.WriteHeader(http.StatusServiceUnavailable)
	} else {
		health.Components["database"] = "connected"
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(health)
}

// checkDatabaseConn checks database connectivity.
func (h *Handler) checkDatabaseConn(ctx context.Context) error {
	// Try to list nodes as a health check
	_, err := h.store.ListNodes(ctx)
	return err
}
