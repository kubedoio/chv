package api

import (
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"net/http"
	"time"

	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/health"
	"github.com/chv/chv/internal/logger"
	"github.com/chv/chv/internal/models"
	"github.com/go-chi/chi/v5"
	"github.com/google/uuid"
)

// nodeResourcesResponse is the response structure for node-scoped resource lists
type nodeResourcesResponse struct {
	NodeID    string      `json:"node_id"`
	NodeName  string      `json:"node_name"`
	Resources interface{} `json:"resources"`
	Count     int         `json:"count"`
}

// createNodeRequest is the request body for creating a new node
type createNodeRequest struct {
	Name      string `json:"name"`
	Hostname  string `json:"hostname"`
	IPAddress string `json:"ip_address"`
	AgentURL  string `json:"agent_url"`
}

// createNodeResponse is the response for node creation, includes the agent token
type createNodeResponse struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Hostname    string `json:"hostname"`
	IPAddress   string `json:"ip_address"`
	Status      string `json:"status"`
	AgentURL    string `json:"agent_url,omitempty"`
	AgentToken  string `json:"agent_token"`
	CreatedAt   string `json:"created_at"`
}

// updateNodeRequest is the request body for updating a node
type updateNodeRequest struct {
	Name      string `json:"name,omitempty"`
	Hostname  string `json:"hostname,omitempty"`
	IPAddress string `json:"ip_address,omitempty"`
	AgentURL  string `json:"agent_url,omitempty"`
}

// agentRegisterRequest is the request body for agent registration
type agentRegisterRequest struct {
	NodeID    string `json:"node_id"`
	Hostname  string `json:"hostname"`
	Token     string `json:"token"`
}

// agentHeartbeatRequest is the request body for agent heartbeat
type agentHeartbeatRequest struct {
	NodeID    string `json:"node_id"`
	Timestamp string `json:"timestamp"`
}

// listNodes handles GET /api/v1/nodes
// Returns all nodes in the cluster.
func (h *Handler) listNodes(w http.ResponseWriter, r *http.Request) {
	nodes, err := h.repo.ListNodes(requestContext(r))
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "nodes_failed",
			Message:   "CHV could not list nodes",
			Retryable: true,
		})
		return
	}

	// Build response with resource counts
	var response []models.NodeWithResources
	for _, node := range nodes {
		counts, err := h.getNodeResourceCounts(r, node.ID)
		if err != nil {
			h.writeError(w, http.StatusInternalServerError, apiError{
				Code:      "node_resources_failed",
				Message:   "CHV could not get resource counts for node",
				Retryable: true,
			})
			return
		}
		response = append(response, models.NodeWithResources{
			Node:      node,
			Resources: counts,
		})
	}

	h.writeJSON(w, http.StatusOK, response)
}

// createNode handles POST /api/v1/nodes
// Creates a new remote node with agent token.
func (h *Handler) createNode(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)

	var req createNodeRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON",
			Retryable: false,
		})
		return
	}

	// Validation
	if req.Name == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Node name is required",
			Retryable: false,
		})
		return
	}

	if req.Hostname == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Hostname is required",
			Retryable: false,
		})
		return
	}

	if req.IPAddress == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "IP address is required",
			Retryable: false,
		})
		return
	}

	// Generate agent token
	agentToken, tokenHash, err := generateAgentToken()
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "token_generation_failed",
			Message:   "Failed to generate agent token",
			Retryable: true,
		})
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)
	node := &models.Node{
		ID:             uuid.NewString(),
		Name:           req.Name,
		Hostname:       req.Hostname,
		IPAddress:      req.IPAddress,
		Status:         models.NodeStatusOffline, // Start as offline until agent registers
		IsLocal:        false,
		AgentURL:       req.AgentURL,
		AgentToken:     agentToken,
		AgentTokenHash: tokenHash,
		CreatedAt:      now,
		UpdatedAt:      now,
	}

	if err := h.repo.CreateNode(ctx, node); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "node_create_failed",
			Message:   "Failed to create node: " + err.Error(),
			Retryable: true,
		})
		return
	}

	// Return the node with the plaintext token (only shown once)
	h.writeJSON(w, http.StatusCreated, createNodeResponse{
		ID:         node.ID,
		Name:       node.Name,
		Hostname:   node.Hostname,
		IPAddress:  node.IPAddress,
		Status:     node.Status,
		AgentURL:   node.AgentURL,
		AgentToken: agentToken,
		CreatedAt:  node.CreatedAt,
	})
}

// updateNode handles PATCH /api/v1/nodes/{id}
// Updates an existing node.
func (h *Handler) updateNode(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	nodeID := chi.URLParam(r, "id")

	if nodeID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Node ID is required",
			Retryable: false,
		})
		return
	}

	node, err := h.repo.GetNode(ctx, nodeID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "node_failed",
			Message:   "CHV could not get node",
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

	var req updateNodeRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON",
			Retryable: false,
		})
		return
	}

	// Update fields if provided
	if req.Name != "" {
		node.Name = req.Name
	}
	if req.Hostname != "" {
		node.Hostname = req.Hostname
	}
	if req.IPAddress != "" {
		node.IPAddress = req.IPAddress
	}
	if req.AgentURL != "" {
		node.AgentURL = req.AgentURL
	}

	node.UpdatedAt = time.Now().UTC().Format(time.RFC3339)

	if err := h.repo.UpdateNode(ctx, node); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "node_update_failed",
			Message:   "Failed to update node: " + err.Error(),
			Retryable: true,
		})
		return
	}

	counts, _ := h.getNodeResourceCounts(r, nodeID)
	h.writeJSON(w, http.StatusOK, models.NodeWithResources{
		Node:      *node,
		Resources: counts,
	})
}

// deleteNode handles DELETE /api/v1/nodes/{id}
// Deletes a node.
func (h *Handler) deleteNode(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	nodeID := chi.URLParam(r, "id")

	if nodeID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Node ID is required",
			Retryable: false,
		})
		return
	}

	node, err := h.repo.GetNode(ctx, nodeID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "node_failed",
			Message:   "CHV could not get node",
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

	// Prevent deletion of local node
	if node.IsLocal {
		h.writeError(w, http.StatusForbidden, apiError{
			Code:      "cannot_delete_local_node",
			Message:   "Cannot delete the local node",
			Retryable: false,
		})
		return
	}

	if err := h.repo.DeleteNode(ctx, nodeID); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "node_delete_failed",
			Message:   "Failed to delete node: " + err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]string{
		"message": "Node deleted successfully",
	})
}

// setNodeMaintenance handles POST /api/v1/nodes/{id}/maintenance
// Sets a node to maintenance mode or brings it back online.
func (h *Handler) setNodeMaintenance(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	nodeID := chi.URLParam(r, "id")

	if nodeID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Node ID is required",
			Retryable: false,
		})
		return
	}

	node, err := h.repo.GetNode(ctx, nodeID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "node_failed",
			Message:   "CHV could not get node",
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

	// Parse request body for enabled flag
	var req struct {
		Enabled bool `json:"enabled"`
	}
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON",
			Retryable: false,
		})
		return
	}

	// Update status
	newStatus := models.NodeStatusOnline
	if req.Enabled {
		newStatus = models.NodeStatusMaintenance
	}

	if err := h.repo.UpdateNodeStatus(ctx, nodeID, newStatus); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "node_maintenance_failed",
			Message:   "Failed to update node status: " + err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"message":      "Node maintenance status updated",
		"maintenance":  req.Enabled,
		"status":       newStatus,
	})
}

// registerAgent handles POST /api/v1/agents/register
// Agent registers with the controller on startup.
func (h *Handler) registerAgent(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)

	var req agentRegisterRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON",
			Retryable: false,
		})
		return
	}

	if req.NodeID == "" || req.Token == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "node_id and token are required",
			Retryable: false,
		})
		return
	}

	// Get node and validate token
	node, err := h.repo.GetNode(ctx, req.NodeID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "node_failed",
			Message:   "CHV could not get node",
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

	// Validate token using constant time comparison
	tokenHash := hashAgentToken(req.Token)
	if !constantTimeCompare(tokenHash, node.AgentTokenHash) {
		h.writeError(w, http.StatusUnauthorized, apiError{
			Code:      "invalid_token",
			Message:   "Invalid agent token",
			Retryable: false,
		})
		return
	}

	// Update node status to online
	now := time.Now().UTC().Format(time.RFC3339)
	if err := h.repo.UpdateNodeStatus(ctx, req.NodeID, models.NodeStatusOnline); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "status_update_failed",
			Message:   "Failed to update node status",
			Retryable: true,
		})
		return
	}

	if err := h.repo.UpdateNodeLastSeen(ctx, req.NodeID); err != nil {
		logger.L().Warn("Failed to update node last seen", logger.F("node_id", req.NodeID), logger.ErrorField(err))
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"message":      "Agent registered successfully",
		"node_id":      req.NodeID,
		"status":       models.NodeStatusOnline,
		"registered_at": now,
	})
}

// agentHeartbeat handles POST /api/v1/agents/heartbeat
// Agent sends periodic heartbeats to update status.
func (h *Handler) agentHeartbeat(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)

	var req agentHeartbeatRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON",
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

	// Update last seen timestamp
	if err := h.repo.UpdateNodeLastSeen(ctx, req.NodeID); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "heartbeat_failed",
			Message:   "Failed to record heartbeat",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"message":    "Heartbeat recorded",
		"node_id":    req.NodeID,
		"timestamp":  time.Now().UTC().Format(time.RFC3339),
	})
}

// getNode returns information about a specific node
func (h *Handler) getNode(w http.ResponseWriter, r *http.Request) {
	nodeID := chi.URLParam(r, "id")
	if nodeID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Node ID is required",
			Retryable: false,
		})
		return
	}

	node, err := h.repo.GetNode(requestContext(r), nodeID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "node_failed",
			Message:   "CHV could not get node",
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

	counts, err := h.getNodeResourceCounts(r, nodeID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "node_resources_failed",
			Message:   "CHV could not get resource counts",
			Retryable: true,
		})
		return
	}

	response := models.NodeWithResources{
		Node:      *node,
		Resources: counts,
	}

	h.writeJSON(w, http.StatusOK, response)
}

// getNodeResourceCounts returns resource counts for a node
func (h *Handler) getNodeResourceCounts(r *http.Request, nodeID string) (models.NodeResourceCount, error) {
	ctx := requestContext(r)
	var counts models.NodeResourceCount

	vms, err := h.repo.CountVMsByNode(ctx, nodeID)
	if err != nil {
		return counts, err
	}
	counts.VMs = vms

	images, err := h.repo.CountImagesByNode(ctx, nodeID)
	if err != nil {
		return counts, err
	}
	counts.Images = images

	pools, err := h.repo.CountStoragePoolsByNode(ctx, nodeID)
	if err != nil {
		return counts, err
	}
	counts.StoragePools = pools

	networks, err := h.repo.CountNetworksByNode(ctx, nodeID)
	if err != nil {
		return counts, err
	}
	counts.Networks = networks

	return counts, nil
}

// validateNodeExists checks if a node exists and returns 404 if not
func (h *Handler) validateNodeExists(w http.ResponseWriter, r *http.Request, nodeID string) bool {
	if nodeID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Node ID is required",
			Retryable: false,
		})
		return false
	}

	node, err := h.repo.GetNode(requestContext(r), nodeID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "node_validation_failed",
			Message:   "CHV could not validate node",
			Retryable: true,
		})
		return false
	}

	if node == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:      "node_not_found",
			Message:   "Node not found",
			Retryable: false,
		})
		return false
	}

	return true
}

// listNodeVMs handles GET /api/v1/nodes/{id}/vms
// Returns all VMs associated with the specified node.
func (h *Handler) listNodeVMs(w http.ResponseWriter, r *http.Request) {
	nodeID := chi.URLParam(r, "id")
	if !h.validateNodeExists(w, r, nodeID) {
		return
	}

	items, err := h.repo.ListVMsByNode(requestContext(r), nodeID)
	if err != nil {
		logger.L().Error("ListVMsByNode failed", logger.ErrorField(err), logger.F("node_id", nodeID))
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "vms_failed",
			Message:   "CHV could not list virtual machines for this node",
			Retryable: true,
		})
		return
	}

	node, _ := h.repo.GetNode(requestContext(r), nodeID)
	nodeName := "Unknown"
	if node != nil {
		nodeName = node.Name
	}

	response := nodeResourcesResponse{
		NodeID:    nodeID,
		NodeName:  nodeName,
		Resources: items,
		Count:     len(items),
	}

	h.writeJSON(w, http.StatusOK, response)
}

// listNodeImages handles GET /api/v1/nodes/{id}/images
// Returns all images available on the specified node.
func (h *Handler) listNodeImages(w http.ResponseWriter, r *http.Request) {
	nodeID := chi.URLParam(r, "id")
	if !h.validateNodeExists(w, r, nodeID) {
		return
	}

	items, err := h.repo.ListImagesByNode(requestContext(r), nodeID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "images_failed",
			Message:   "CHV could not list images for this node",
			Retryable: true,
		})
		return
	}

	node, _ := h.repo.GetNode(requestContext(r), nodeID)
	nodeName := "Unknown"
	if node != nil {
		nodeName = node.Name
	}

	response := nodeResourcesResponse{
		NodeID:    nodeID,
		NodeName:  nodeName,
		Resources: items,
		Count:     len(items),
	}

	h.writeJSON(w, http.StatusOK, response)
}

// listNodeStoragePools handles GET /api/v1/nodes/{id}/storage
// Returns all storage pools on the specified node.
func (h *Handler) listNodeStoragePools(w http.ResponseWriter, r *http.Request) {
	nodeID := chi.URLParam(r, "id")
	if !h.validateNodeExists(w, r, nodeID) {
		return
	}

	items, err := h.repo.ListStoragePoolsByNode(requestContext(r), nodeID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "storage_pools_failed",
			Message:   "CHV could not list storage pools for this node",
			Retryable: true,
		})
		return
	}

	node, _ := h.repo.GetNode(requestContext(r), nodeID)
	nodeName := "Unknown"
	if node != nil {
		nodeName = node.Name
	}

	response := nodeResourcesResponse{
		NodeID:    nodeID,
		NodeName:  nodeName,
		Resources: items,
		Count:     len(items),
	}

	h.writeJSON(w, http.StatusOK, response)
}

// listNodeNetworks handles GET /api/v1/nodes/{id}/networks
// Returns all networks available on the specified node.
func (h *Handler) listNodeNetworks(w http.ResponseWriter, r *http.Request) {
	nodeID := chi.URLParam(r, "id")
	if !h.validateNodeExists(w, r, nodeID) {
		return
	}

	items, err := h.repo.ListNetworksByNode(requestContext(r), nodeID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "networks_failed",
			Message:   "CHV could not list networks for this node",
			Retryable: true,
		})
		return
	}

	node, _ := h.repo.GetNode(requestContext(r), nodeID)
	nodeName := "Unknown"
	if node != nil {
		nodeName = node.Name
	}

	response := nodeResourcesResponse{
		NodeID:    nodeID,
		NodeName:  nodeName,
		Resources: items,
		Count:     len(items),
	}

	h.writeJSON(w, http.StatusOK, response)
}

// getNodeHealth handles GET /api/v1/nodes/{id}/health
// Returns health information for a specific node.
func (h *Handler) getNodeHealth(w http.ResponseWriter, r *http.Request) {
	nodeID := chi.URLParam(r, "id")
	if !h.validateNodeExists(w, r, nodeID) {
		return
	}

	hc := health.NewService(h.repo)
	nodeHealth, err := hc.GetNodeHealth(requestContext(r), nodeID)
	if err != nil {
		logger.L().Error("Failed to get node health", logger.ErrorField(err), logger.F("node_id", nodeID))
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "health_failed",
			Message:   "CHV could not get node health",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, nodeHealth)
}

// getAllNodesHealth handles GET /api/v1/nodes/health
// Returns health information for all nodes.
func (h *Handler) getAllNodesHealth(w http.ResponseWriter, r *http.Request) {
	hc := health.NewService(h.repo)
	healths, err := hc.GetAllNodesHealth(requestContext(r))
	if err != nil {
		logger.L().Error("Failed to get all nodes health", logger.ErrorField(err))
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "health_failed",
			Message:   "CHV could not get nodes health",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, healths)
}

// getNodeMetrics handles GET /api/v1/nodes/{id}/metrics
// Returns metrics history for a specific node.
func (h *Handler) getNodeMetrics(w http.ResponseWriter, r *http.Request) {
	nodeID := chi.URLParam(r, "id")
	if !h.validateNodeExists(w, r, nodeID) {
		return
	}

	// Parse range parameter (default to last hour)
	rangeParam := r.URL.Query().Get("range")
	since := time.Now().UTC().Add(-1 * time.Hour).Format(time.RFC3339)
	
	switch rangeParam {
	case "24h":
		since = time.Now().UTC().Add(-24 * time.Hour).Format(time.RFC3339)
	case "7d":
		since = time.Now().UTC().Add(-7 * 24 * time.Hour).Format(time.RFC3339)
	case "30d":
		since = time.Now().UTC().Add(-30 * 24 * time.Hour).Format(time.RFC3339)
	}

	metrics, err := h.repo.GetNodeMetrics(requestContext(r), nodeID, since)
	if err != nil {
		logger.L().Error("Failed to get node metrics", logger.ErrorField(err), logger.F("node_id", nodeID))
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "metrics_failed",
			Message:   "CHV could not get node metrics",
			Retryable: true,
		})
		return
	}

	response := struct {
		NodeID  string               `json:"node_id"`
		Metrics []db.NodeMetricsRecord `json:"metrics"`
	}{
		NodeID:  nodeID,
		Metrics: metrics,
	}

	h.writeJSON(w, http.StatusOK, response)
}

// generateAgentToken generates a secure agent token and its hash
func generateAgentToken() (token, tokenHash string, err error) {
	// Generate 32 random bytes
	secret := make([]byte, 32)
	if _, err := rand.Read(secret); err != nil {
		return "", "", err
	}

	// Create token with prefix
	token = "chv_agent_" + hex.EncodeToString(secret)
	tokenHash = hashAgentToken(token)

	return token, tokenHash, nil
}

// hashAgentToken creates a SHA-256 hash of the agent token
func hashAgentToken(token string) string {
	sum := sha256.Sum256([]byte(token))
	return hex.EncodeToString(sum[:])
}

// constantTimeCompare performs constant time comparison to prevent timing attacks
func constantTimeCompare(a, b string) bool {
	if len(a) != len(b) {
		return false
	}
	var result byte
	for i := 0; i < len(a); i++ {
		result |= a[i] ^ b[i]
	}
	return result == 0
}
