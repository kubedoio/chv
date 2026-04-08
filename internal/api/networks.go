package api

import (
	"net/http"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
)

type createNetworkRequest struct {
	Name       string `json:"name"`
	Mode       string `json:"mode"`
	BridgeName string `json:"bridge_name"`
	CIDR       string `json:"cidr"`
	GatewayIP  string `json:"gateway_ip"`
}

func (h *Handler) createNetwork(w http.ResponseWriter, r *http.Request) {
	var req createNetworkRequest
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
			Message:   "Network name is required.",
			Retryable: false,
		})
		return
	}

	if req.Mode != "bridge" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Only 'bridge' mode is supported in MVP-1.",
			Retryable: false,
			Hint:      "Use mode 'bridge' with bridge_name '" + h.config.BridgeName + "'.",
		})
		return
	}

	if req.BridgeName == "" {
		req.BridgeName = h.config.BridgeName
	}

	if req.CIDR == "" {
		req.CIDR = h.config.NetworkCIDR
	}

	if req.GatewayIP == "" {
		req.GatewayIP = h.config.BridgeGateway
	}

	ctx := requestContext(r)

	existing, err := h.repo.GetNetworkByName(ctx, req.Name)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "network_create_failed",
			Message:   "Could not check for existing network.",
			Retryable: true,
		})
		return
	}
	if existing != nil {
		h.writeError(w, http.StatusConflict, apiError{
			Code:         "already_exists",
			Message:      "A network with this name already exists.",
			ResourceType: "network",
			ResourceID:   existing.ID,
			Retryable:    false,
		})
		return
	}

	network := &models.Network{
		ID:              uuid.NewString(),
		Name:            req.Name,
		Mode:            req.Mode,
		BridgeName:      req.BridgeName,
		CIDR:            req.CIDR,
		GatewayIP:       req.GatewayIP,
		IsSystemManaged: false,
		Status:          "active",
		CreatedAt:       time.Now().UTC().Format(time.RFC3339),
	}

	if err := h.repo.CreateNetwork(ctx, network); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "network_create_failed",
			Message:   "Could not create network.",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusCreated, network)
}
