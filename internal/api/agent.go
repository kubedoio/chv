package api

import (
	"encoding/json"
	"net/http"
	"strings"

	"github.com/chv/chv/internal/health"
	"github.com/chv/chv/internal/logger"
)

// HeartbeatRequest represents an agent heartbeat request
type HeartbeatRequest struct {
	NodeID    string                `json:"node_id"`
	Timestamp string                `json:"timestamp"`
	Resources HeartbeatResources    `json:"resources"`
}

// HeartbeatResources represents resource metrics in a heartbeat
type HeartbeatResources struct {
	CPUPercent     float64 `json:"cpu_percent"`
	MemoryUsedMB   int     `json:"memory_used_mb"`
	MemoryTotalMB  int     `json:"memory_total_mb"`
	DiskUsedGB     int     `json:"disk_used_gb"`
	DiskTotalGB    int     `json:"disk_total_gb"`
}

// HeartbeatResponse represents a heartbeat response
type HeartbeatResponse struct {
	Status  string `json:"status"`
	Message string `json:"message,omitempty"`
}

// agentHeartbeatHandler handles POST /api/v1/agents/heartbeat
// This endpoint is called by agents to report their health status
func (h *Handler) agentHeartbeatHandler(w http.ResponseWriter, r *http.Request) {
	log := logger.L()

	// Validate agent token from Authorization header
	authHeader := r.Header.Get("Authorization")
	if authHeader == "" {
		h.writeError(w, http.StatusUnauthorized, apiError{
			Code:      "unauthorized",
			Message:   "Authorization header is required",
			Retryable: false,
		})
		return
	}

	// Extract bearer token
	var token string
	if strings.HasPrefix(authHeader, "Bearer ") {
		token = strings.TrimPrefix(authHeader, "Bearer ")
	} else {
		h.writeError(w, http.StatusUnauthorized, apiError{
			Code:      "unauthorized",
			Message:   "Invalid authorization format. Expected: Bearer {token}",
			Retryable: false,
		})
		return
	}

	// Decode request body
	var req HeartbeatRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Invalid JSON in request body",
			Retryable: false,
		})
		return
	}

	if req.NodeID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "node_id is required",
			Retryable: false,
		})
		return
	}

	// Validate the agent token against the node's stored token
	node, err := h.repo.GetNode(requestContext(r), req.NodeID)
	if err != nil {
		log.Error("Failed to get node for heartbeat validation", logger.ErrorField(err), logger.F("node_id", req.NodeID))
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Failed to validate node",
			Retryable: true,
		})
		return
	}

	if node == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:      "node_not_found",
			Message:   "Node not found",
			Retryable: false,
		})
		return
	}

	// Validate agent token (simple comparison for now)
	// In production, this should use proper token hashing/comparison
	if node.AgentToken != "" && node.AgentToken != token {
		h.writeError(w, http.StatusUnauthorized, apiError{
			Code:      "unauthorized",
			Message:   "Invalid agent token",
			Retryable: false,
		})
		return
	}

	// Convert heartbeat request to node metrics
	metrics := health.NodeMetrics{
		CPUPercent:     req.Resources.CPUPercent,
		MemoryUsedMB:   req.Resources.MemoryUsedMB,
		MemoryTotalMB:  req.Resources.MemoryTotalMB,
		DiskUsedGB:     req.Resources.DiskUsedGB,
		DiskTotalGB:    req.Resources.DiskTotalGB,
		Timestamp:      req.Timestamp,
	}

	// Create a heartbeat service to record the heartbeat
	hbService := health.NewHeartbeatService(h.repo, 0)
	if err := hbService.RecordHeartbeat(requestContext(r), req.NodeID, metrics); err != nil {
		log.Error("Failed to record heartbeat", logger.ErrorField(err), logger.F("node_id", req.NodeID))
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Failed to record heartbeat",
			Retryable: true,
		})
		return
	}

	response := HeartbeatResponse{
		Status:  "ok",
		Message: "Heartbeat recorded successfully",
	}

	h.writeJSON(w, http.StatusOK, response)
}

// AgentRegisterRequest represents an agent registration request
type AgentRegisterRequest struct {
	NodeID   string `json:"node_id"`
	Hostname string `json:"hostname"`
	Version  string `json:"version"`
}

// AgentRegisterResponse represents an agent registration response
type AgentRegisterResponse struct {
	Status    string `json:"status"`
	Message   string `json:"message,omitempty"`
	AgentToken string `json:"agent_token,omitempty"`
}

// agentRegisterHandler handles POST /api/v1/agents/register
// This endpoint is called by agents to register with the controller
func (h *Handler) agentRegisterHandler(w http.ResponseWriter, r *http.Request) {
	var req AgentRegisterRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Invalid JSON in request body",
			Retryable: false,
		})
		return
	}

	// For now, return a simple acknowledgment
	// Full registration flow will be implemented in Phase 2A
	response := AgentRegisterResponse{
		Status:  "ok",
		Message: "Agent registered successfully",
	}

	h.writeJSON(w, http.StatusOK, response)
}
