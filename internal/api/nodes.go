package api

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/pkg/uuidx"
	"github.com/go-chi/chi/v5"
	"github.com/google/uuid"
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

	// Get actor info for audit trail (node or user)
	actorID, _ := r.Context().Value("node_id").(string)
	actorType := models.ActorTypeSystem
	if actorID == "" {
		actorID, _ = r.Context().Value("user_id").(string)
		actorType = models.ActorTypeUser
		if actorID == "" {
			actorID = "system"
		}
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

	var opID *uuid.UUID
	nodeID := uuidx.New()
	if existing != nil {
		nodeID = existing.ID
	}
	
	if existing != nil {
		// Start operation tracking for update
		op, _ := h.operations.Start(r.Context(), models.OpNodeRegister, models.OpCategorySync,
			"node", &nodeID, actorType, actorID, req)
		opID = &op.ID

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
			h.operations.Fail(r.Context(), *opID, err)
			h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to update node")
			return
		}

		h.operations.Complete(r.Context(), *opID, existing)
		h.jsonResponse(w, http.StatusOK, existing)
		return
	}

	// Start operation tracking for create
	op, _ := h.operations.Start(r.Context(), models.OpNodeRegister, models.OpCategorySync,
		"node", &nodeID, actorType, actorID, req)
	opID = &op.ID
	
	// Create new node
	labelsJSON, _ := json.Marshal(req.Labels)
	capsJSON, _ := json.Marshal(req.Capabilities)
	now := time.Now()
	
	node := &models.Node{
		ID:                  nodeID,
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
		h.operations.Fail(r.Context(), *opID, err)
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to create node")
		return
	}

	h.operations.Complete(r.Context(), *opID, node)
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

	// Get user ID for audit trail
	userID, _ := r.Context().Value("user_id").(string)
	if userID == "" {
		userID = "anonymous"
	}

	// Start operation tracking
	op, _ := h.operations.Start(r.Context(), models.OpNodeRegister, models.OpCategorySync,
		"node", &nodeID, models.ActorTypeUser, userID, req)

	if err := h.store.SetNodeMaintenance(r.Context(), nodeID, req.Enabled); err != nil {
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to set maintenance mode")
		return
	}
	
	// Get updated node
	node, err := h.store.GetNode(r.Context(), nodeID)
	if err != nil {
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get node")
		return
	}

	h.operations.Complete(r.Context(), op.ID, node)
	h.jsonResponse(w, http.StatusOK, node)
}
