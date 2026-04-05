package api

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/pkg/uuidx"
	"github.com/go-chi/chi/v5"
)

// NetworkCreateRequest represents a network creation request.
type NetworkCreateRequest struct {
	Name       string   `json:"name"`
	BridgeName string   `json:"bridge_name"`
	CIDR       string   `json:"cidr"`
	GatewayIP  string   `json:"gateway_ip"`
	DNSServers []string `json:"dns_servers"`
	MTU        int32    `json:"mtu"`
}

func (h *Handler) createNetwork(w http.ResponseWriter, r *http.Request) {
	var req NetworkCreateRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "Invalid request body")
		return
	}
	
	if req.Name == "" || req.BridgeName == "" || req.CIDR == "" || req.GatewayIP == "" {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "name, bridge_name, cidr, and gateway_ip are required")
		return
	}
	
	if req.MTU == 0 {
		req.MTU = 1500
	}
	
	dnsJSON, _ := json.Marshal(req.DNSServers)
	
	network := &models.Network{
		ID:         uuidx.New(),
		Name:       req.Name,
		BridgeName: req.BridgeName,
		CIDR:       req.CIDR,
		GatewayIP:  req.GatewayIP,
		DNSServers: dnsJSON,
		MTU:        req.MTU,
		Mode:       models.NetworkModeBridge,
		Status:     models.NetworkStatusActive,
		CreatedAt:  time.Now(),
	}
	
	if err := h.store.CreateNetwork(r.Context(), network); err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to create network")
		return
	}
	
	h.jsonResponse(w, http.StatusCreated, network)
}

func (h *Handler) listNetworks(w http.ResponseWriter, r *http.Request) {
	networks, err := h.store.ListNetworks(r.Context())
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to list networks")
		return
	}
	
	h.jsonResponse(w, http.StatusOK, networks)
}

func (h *Handler) getNetwork(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	networkID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid network ID")
		return
	}
	
	network, err := h.store.GetNetwork(r.Context(), networkID)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get network")
		return
	}
	
	if network == nil {
		h.errorResponse(w, http.StatusNotFound, "NOT_FOUND", "Network not found")
		return
	}
	
	h.jsonResponse(w, http.StatusOK, network)
}
