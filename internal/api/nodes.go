package api

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/pkg/uuidx"
	"github.com/go-chi/chi/v5"
)

// NodeRequest represents a node registration request.
type NodeRequest struct {
	Hostname            string            `json:"hostname"`
	ManagementIP        string            `json:"management_ip"`
	TotalCPUCores       int32             `json:"total_cpu_cores"`
	TotalRAMMB          int64             `json:"total_ram_mb"`
	Labels              map[string]string `json:"labels"`
	Capabilities        map[string]string `json:"capabilities"`
	AgentVersion        string            `json:"agent_version"`
	HypervisorVersion   string            `json:"hypervisor_version"`
}

func (h *Handler) registerNode(w http.ResponseWriter, r *http.Request) {
	var req NodeRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "Invalid request body")
		return
	}
	
	// Validate required fields
	if req.Hostname == "" || req.ManagementIP == "" {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "hostname and management_ip are required")
		return
	}
	
	// Check if node already exists
	existing, err := h.store.GetNodeByHostname(r.Context(), req.Hostname)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to check existing node")
		return
	}
	
	if existing != nil {
		// Update existing node
		existing.ManagementIP = req.ManagementIP
		existing.TotalCPUcores = req.TotalCPUCores
		existing.TotalRAMMB = req.TotalRAMMB
		existing.AllocatableCPUCores = req.TotalCPUCores
		existing.AllocatableRAMMB = req.TotalRAMMB
		existing.AgentVersion = req.AgentVersion
		existing.HypervisorVersion = req.HypervisorVersion
		existing.Status = models.NodeStateOnline
		now := time.Now()
		existing.LastHeartbeatAt = &now
		
		if labels, err := json.Marshal(req.Labels); err == nil {
			existing.Labels = labels
		}
		if caps, err := json.Marshal(req.Capabilities); err == nil {
			existing.Capabilities = caps
		}
		
		if err := h.store.UpdateNode(r.Context(), existing); err != nil {
			h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to update node")
			return
		}
		
		h.jsonResponse(w, http.StatusOK, existing)
		return
	}
	
	// Create new node
	labelsJSON, _ := json.Marshal(req.Labels)
	capsJSON, _ := json.Marshal(req.Capabilities)
	now := time.Now()
	
	node := &models.Node{
		ID:                  uuidx.New(),
		Hostname:            req.Hostname,
		ManagementIP:        req.ManagementIP,
		Status:              models.NodeStateOnline,
		MaintenanceMode:     false,
		TotalCPUcores:       req.TotalCPUCores,
		TotalRAMMB:          req.TotalRAMMB,
		AllocatableCPUCores: req.TotalCPUCores,
		AllocatableRAMMB:    req.TotalRAMMB,
		Labels:              labelsJSON,
		Capabilities:        capsJSON,
		AgentVersion:        req.AgentVersion,
		HypervisorVersion:   req.HypervisorVersion,
		LastHeartbeatAt:     &now,
		CreatedAt:           now,
		UpdatedAt:           now,
	}
	
	if err := h.store.CreateNode(r.Context(), node); err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to create node")
		return
	}
	
	h.jsonResponse(w, http.StatusCreated, node)
}

func (h *Handler) listNodes(w http.ResponseWriter, r *http.Request) {
	nodes, err := h.store.ListNodes(r.Context())
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to list nodes")
		return
	}
	
	h.jsonResponse(w, http.StatusOK, nodes)
}

func (h *Handler) getNode(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	nodeID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid node ID")
		return
	}
	
	node, err := h.store.GetNode(r.Context(), nodeID)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get node")
		return
	}
	
	if node == nil {
		h.errorResponse(w, http.StatusNotFound, "NOT_FOUND", "Node not found")
		return
	}
	
	h.jsonResponse(w, http.StatusOK, node)
}

// MaintenanceRequest represents a maintenance mode request.
type MaintenanceRequest struct {
	Enabled bool `json:"enabled"`
}

func (h *Handler) setNodeMaintenance(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	nodeID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid node ID")
		return
	}
	
	var req MaintenanceRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "Invalid request body")
		return
	}
	
	if err := h.store.SetNodeMaintenance(r.Context(), nodeID, req.Enabled); err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to set maintenance mode")
		return
	}
	
	// Get updated node
	node, err := h.store.GetNode(r.Context(), nodeID)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get node")
		return
	}
	
	h.jsonResponse(w, http.StatusOK, node)
}
