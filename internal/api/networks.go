package api

import (
	"net/http"
	"strconv"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/networking"
	"github.com/go-chi/chi/v5"
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

	// Get local node ID
	localNode, err := h.repo.GetLocalNode(ctx)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "network_create_failed",
			Message:   "Could not determine local node.",
			Retryable: true,
		})
		return
	}
	if localNode == nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "network_create_failed",
			Message:   "Local node not found.",
			Retryable: true,
		})
		return
	}

	network := &models.Network{
		ID:              uuid.NewString(),
		NodeID:          localNode.ID,
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

// VLAN-related request/response types
type createVLANRequest struct {
	VLANID    int    `json:"vlan_id"`
	Name      string `json:"name"`
	CIDR      string `json:"cidr"`
	GatewayIP string `json:"gateway_ip"`
}

// createVLANHandler creates a new VLAN for a network
func (h *Handler) createVLANHandler(w http.ResponseWriter, r *http.Request) {
	networkID := chi.URLParam(r, "id")
	if networkID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Network ID is required.",
			Retryable: false,
		})
		return
	}

	var req createVLANRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON.",
			Retryable: false,
		})
		return
	}

	// Validate VLAN ID
	if req.VLANID < 1 || req.VLANID > 4094 {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "VLAN ID must be between 1 and 4094.",
			Retryable: false,
		})
		return
	}

	if req.Name == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "VLAN name is required.",
			Retryable: false,
		})
		return
	}

	if req.CIDR == "" || req.GatewayIP == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "CIDR and gateway_ip are required.",
			Retryable: false,
		})
		return
	}

	ctx := requestContext(r)

	// Check if network exists
	network, err := h.repo.GetNetworkByID(ctx, networkID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Could not check network.",
			Retryable: true,
		})
		return
	}
	if network == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:         "not_found",
			Message:      "Network not found.",
			ResourceType: "network",
			ResourceID:   networkID,
			Retryable:    false,
		})
		return
	}

	// Check for duplicate VLAN ID on this network
	existing, err := h.repo.GetVLANByNetworkAndVLANID(ctx, networkID, req.VLANID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Could not check existing VLAN.",
			Retryable: true,
		})
		return
	}
	if existing != nil {
		h.writeError(w, http.StatusConflict, apiError{
			Code:         "already_exists",
			Message:      "A VLAN with this ID already exists on this network.",
			ResourceType: "vlan",
			Retryable:    false,
		})
		return
	}

	vlan := &networking.VLANNetwork{
		ID:        uuid.NewString(),
		NetworkID: networkID,
		VLANID:    req.VLANID,
		Name:      req.Name,
		CIDR:      req.CIDR,
		GatewayIP: req.GatewayIP,
		CreatedAt: time.Now().UTC().Format(time.RFC3339),
	}

	if err := h.repo.CreateVLAN(ctx, vlan); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "create_failed",
			Message:   "Could not create VLAN.",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusCreated, vlan)
}

// listVLANsHandler lists all VLANs for a network
func (h *Handler) listVLANsHandler(w http.ResponseWriter, r *http.Request) {
	networkID := chi.URLParam(r, "id")
	if networkID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Network ID is required.",
			Retryable: false,
		})
		return
	}

	ctx := requestContext(r)

	// Check if network exists
	network, err := h.repo.GetNetworkByID(ctx, networkID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Could not check network.",
			Retryable: true,
		})
		return
	}
	if network == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:         "not_found",
			Message:      "Network not found.",
			ResourceType: "network",
			ResourceID:   networkID,
			Retryable:    false,
		})
		return
	}

	vlans, err := h.repo.ListVLANsByNetwork(ctx, networkID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "list_failed",
			Message:   "Could not list VLANs.",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, vlans)
}

// deleteVLANHandler deletes a VLAN
func (h *Handler) deleteVLANHandler(w http.ResponseWriter, r *http.Request) {
	networkID := chi.URLParam(r, "id")
	vlanID := chi.URLParam(r, "vlanId")

	if networkID == "" || vlanID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Network ID and VLAN ID are required.",
			Retryable: false,
		})
		return
	}

	ctx := requestContext(r)

	// Check if VLAN exists
	vlan, err := h.repo.GetVLANByID(ctx, vlanID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Could not check VLAN.",
			Retryable: true,
		})
		return
	}
	if vlan == nil || vlan.NetworkID != networkID {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:         "not_found",
			Message:      "VLAN not found.",
			ResourceType: "vlan",
			ResourceID:   vlanID,
			Retryable:    false,
		})
		return
	}

	if err := h.repo.DeleteVLAN(ctx, vlanID); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "delete_failed",
			Message:   "Could not delete VLAN.",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]bool{"success": true})
}

// DHCP-related request/response types
type dhcpConfigRequest struct {
	RangeStart string `json:"range_start"`
	RangeEnd   string `json:"range_end"`
	LeaseTime  int    `json:"lease_time_seconds"` // in seconds
}

type dhcpServerResponse struct {
	ID                 string `json:"id"`
	NetworkID          string `json:"network_id"`
	RangeStart         string `json:"range_start"`
	RangeEnd           string `json:"range_end"`
	LeaseTimeSeconds   int    `json:"lease_time_seconds"`
	IsRunning          bool   `json:"is_running"`
	CreatedAt          string `json:"created_at"`
	UpdatedAt          string `json:"updated_at"`
}

// configureDHCPHandler configures the DHCP server for a network
func (h *Handler) configureDHCPHandler(w http.ResponseWriter, r *http.Request) {
	networkID := chi.URLParam(r, "id")
	if networkID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Network ID is required.",
			Retryable: false,
		})
		return
	}

	var req dhcpConfigRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON.",
			Retryable: false,
		})
		return
	}

	if req.RangeStart == "" || req.RangeEnd == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "range_start and range_end are required.",
			Retryable: false,
		})
		return
	}

	leaseTime := time.Duration(req.LeaseTime) * time.Second
	if leaseTime == 0 {
		leaseTime = 1 * time.Hour
	}

	ctx := requestContext(r)

	// Check if network exists
	network, err := h.repo.GetNetworkByID(ctx, networkID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Could not check network.",
			Retryable: true,
		})
		return
	}
	if network == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:         "not_found",
			Message:      "Network not found.",
			ResourceType: "network",
			ResourceID:   networkID,
			Retryable:    false,
		})
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)
	server := &networking.DHCPServer{
		ID:         uuid.NewString(),
		NetworkID:  networkID,
		RangeStart: req.RangeStart,
		RangeEnd:   req.RangeEnd,
		LeaseTime:  leaseTime,
		IsRunning:  false,
		CreatedAt:  now,
		UpdatedAt:  now,
	}

	// Check if server already exists
	existing, err := h.repo.GetDHCPServerByNetwork(ctx, networkID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Could not check existing DHCP server.",
			Retryable: true,
		})
		return
	}

	if existing != nil {
		// Update existing
		existing.RangeStart = req.RangeStart
		existing.RangeEnd = req.RangeEnd
		existing.LeaseTime = leaseTime
		existing.UpdatedAt = now
		if err := h.repo.UpdateDHCPServer(ctx, existing); err != nil {
			h.writeError(w, http.StatusInternalServerError, apiError{
				Code:      "update_failed",
				Message:   "Could not update DHCP server.",
				Retryable: true,
			})
			return
		}
		server = existing
	} else {
		// Create new
		if err := h.repo.CreateDHCPServer(ctx, server); err != nil {
			h.writeError(w, http.StatusInternalServerError, apiError{
				Code:      "create_failed",
				Message:   "Could not create DHCP server.",
				Retryable: true,
			})
			return
		}
	}

	resp := dhcpServerResponse{
		ID:               server.ID,
		NetworkID:        server.NetworkID,
		RangeStart:       server.RangeStart,
		RangeEnd:         server.RangeEnd,
		LeaseTimeSeconds: int(server.LeaseTime.Seconds()),
		IsRunning:        server.IsRunning,
		CreatedAt:        server.CreatedAt,
		UpdatedAt:        server.UpdatedAt,
	}

	h.writeJSON(w, http.StatusOK, resp)
}

// startDHCPHandler starts the DHCP server for a network
func (h *Handler) startDHCPHandler(w http.ResponseWriter, r *http.Request) {
	networkID := chi.URLParam(r, "id")
	if networkID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Network ID is required.",
			Retryable: false,
		})
		return
	}

	ctx := requestContext(r)

	// Check if DHCP server exists
	server, err := h.repo.GetDHCPServerByNetwork(ctx, networkID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Could not check DHCP server.",
			Retryable: true,
		})
		return
	}
	if server == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:         "not_found",
			Message:      "DHCP server not configured for this network.",
			ResourceType: "dhcp_server",
			Retryable:    false,
		})
		return
	}

	// Update status to running
	server.IsRunning = true
	server.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	if err := h.repo.UpdateDHCPServer(ctx, server); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "update_failed",
			Message:   "Could not start DHCP server.",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]interface{}{
		"message":    "DHCP server started",
		"is_running": true,
	})
}

// stopDHCPHandler stops the DHCP server for a network
func (h *Handler) stopDHCPHandler(w http.ResponseWriter, r *http.Request) {
	networkID := chi.URLParam(r, "id")
	if networkID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Network ID is required.",
			Retryable: false,
		})
		return
	}

	ctx := requestContext(r)

	// Check if DHCP server exists
	server, err := h.repo.GetDHCPServerByNetwork(ctx, networkID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Could not check DHCP server.",
			Retryable: true,
		})
		return
	}
	if server == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:         "not_found",
			Message:      "DHCP server not configured for this network.",
			ResourceType: "dhcp_server",
			Retryable:    false,
		})
		return
	}

	// Update status to stopped
	server.IsRunning = false
	server.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	if err := h.repo.UpdateDHCPServer(ctx, server); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "update_failed",
			Message:   "Could not stop DHCP server.",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]interface{}{
		"message":    "DHCP server stopped",
		"is_running": false,
	})
}

// getDHCPLeasesHandler returns all DHCP leases for a network
func (h *Handler) getDHCPLeasesHandler(w http.ResponseWriter, r *http.Request) {
	networkID := chi.URLParam(r, "id")
	if networkID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Network ID is required.",
			Retryable: false,
		})
		return
	}

	ctx := requestContext(r)

	leases, err := h.repo.ListDHCPLeases(ctx, networkID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "list_failed",
			Message:   "Could not list DHCP leases.",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, leases)
}

// getDHCPStatusHandler returns the DHCP server status for a network
func (h *Handler) getDHCPStatusHandler(w http.ResponseWriter, r *http.Request) {
	networkID := chi.URLParam(r, "id")
	if networkID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Network ID is required.",
			Retryable: false,
		})
		return
	}

	ctx := requestContext(r)

	server, err := h.repo.GetDHCPServerByNetwork(ctx, networkID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Could not get DHCP server status.",
			Retryable: true,
		})
		return
	}
	if server == nil {
		h.writeJSON(w, http.StatusOK, map[string]interface{}{
			"configured": false,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, dhcpServerResponse{
		ID:               server.ID,
		NetworkID:        server.NetworkID,
		RangeStart:       server.RangeStart,
		RangeEnd:         server.RangeEnd,
		LeaseTimeSeconds: int(server.LeaseTime.Seconds()),
		IsRunning:        server.IsRunning,
		CreatedAt:        server.CreatedAt,
		UpdatedAt:        server.UpdatedAt,
	})
}

// Firewall-related request/response types
type createFirewallRuleRequest struct {
	Direction   string `json:"direction"`
	Protocol    string `json:"protocol"`
	PortRange   string `json:"port_range,omitempty"`
	SourceCIDR  string `json:"source_cidr"`
	Action      string `json:"action"`
	Priority    int    `json:"priority"`
	Description string `json:"description,omitempty"`
}

// createFirewallRuleHandler creates a firewall rule for a VM
func (h *Handler) createFirewallRuleHandler(w http.ResponseWriter, r *http.Request) {
	vmID := chi.URLParam(r, "id")
	if vmID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "VM ID is required.",
			Retryable: false,
		})
		return
	}

	var req createFirewallRuleRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON.",
			Retryable: false,
		})
		return
	}

	// Validate request
	if req.Direction != "ingress" && req.Direction != "egress" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "direction must be 'ingress' or 'egress'.",
			Retryable: false,
		})
		return
	}

	if req.Protocol != "tcp" && req.Protocol != "udp" && req.Protocol != "icmp" && req.Protocol != "all" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "protocol must be 'tcp', 'udp', 'icmp', or 'all'.",
			Retryable: false,
		})
		return
	}

	if req.Action != "allow" && req.Action != "deny" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "action must be 'allow' or 'deny'.",
			Retryable: false,
		})
		return
	}

	if req.Priority < 100 || req.Priority > 999 {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "priority must be between 100 and 999.",
			Retryable: false,
		})
		return
	}

	if req.SourceCIDR == "" {
		req.SourceCIDR = "0.0.0.0/0"
	}

	ctx := requestContext(r)

	// Check if VM exists
	vm, err := h.repo.GetVMByID(ctx, vmID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Could not check VM.",
			Retryable: true,
		})
		return
	}
	if vm == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:         "not_found",
			Message:      "VM not found.",
			ResourceType: "vm",
			ResourceID:   vmID,
			Retryable:    false,
		})
		return
	}

	rule := &networking.FirewallRule{
		ID:          uuid.NewString(),
		VMID:        vmID,
		Direction:   req.Direction,
		Protocol:    req.Protocol,
		PortRange:   req.PortRange,
		SourceCIDR:  req.SourceCIDR,
		Action:      req.Action,
		Priority:    req.Priority,
		Description: req.Description,
		CreatedAt:   time.Now().UTC().Format(time.RFC3339),
	}

	if err := h.repo.CreateFirewallRule(ctx, rule); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "create_failed",
			Message:   "Could not create firewall rule.",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusCreated, rule)
}

// listFirewallRulesHandler lists all firewall rules for a VM
func (h *Handler) listFirewallRulesHandler(w http.ResponseWriter, r *http.Request) {
	vmID := chi.URLParam(r, "id")
	if vmID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "VM ID is required.",
			Retryable: false,
		})
		return
	}

	ctx := requestContext(r)

	// Check if VM exists
	vm, err := h.repo.GetVMByID(ctx, vmID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Could not check VM.",
			Retryable: true,
		})
		return
	}
	if vm == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:         "not_found",
			Message:      "VM not found.",
			ResourceType: "vm",
			ResourceID:   vmID,
			Retryable:    false,
		})
		return
	}

	rules, err := h.repo.ListFirewallRulesByVM(ctx, vmID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "list_failed",
			Message:   "Could not list firewall rules.",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, rules)
}

// deleteFirewallRuleHandler deletes a firewall rule
func (h *Handler) deleteFirewallRuleHandler(w http.ResponseWriter, r *http.Request) {
	vmID := chi.URLParam(r, "id")
	ruleID := chi.URLParam(r, "ruleId")

	if vmID == "" || ruleID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "VM ID and Rule ID are required.",
			Retryable: false,
		})
		return
	}

	ctx := requestContext(r)

	// Check if rule exists and belongs to the VM
	rule, err := h.repo.GetFirewallRuleByID(ctx, ruleID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Could not check firewall rule.",
			Retryable: true,
		})
		return
	}
	if rule == nil || rule.VMID != vmID {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:         "not_found",
			Message:      "Firewall rule not found.",
			ResourceType: "firewall_rule",
			ResourceID:   ruleID,
			Retryable:    false,
		})
		return
	}

	if err := h.repo.DeleteFirewallRule(ctx, ruleID); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "delete_failed",
			Message:   "Could not delete firewall rule.",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]bool{"success": true})
}

// getNetworkHandler returns a single network by ID
func (h *Handler) getNetworkHandler(w http.ResponseWriter, r *http.Request) {
	networkID := chi.URLParam(r, "id")
	if networkID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Network ID is required.",
			Retryable: false,
		})
		return
	}

	ctx := requestContext(r)

	network, err := h.repo.GetNetworkByID(ctx, networkID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Could not get network.",
			Retryable: true,
		})
		return
	}
	if network == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:         "not_found",
			Message:      "Network not found.",
			ResourceType: "network",
			ResourceID:   networkID,
			Retryable:    false,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, network)
}

// deleteNetworkHandler deletes a network
func (h *Handler) deleteNetworkHandler(w http.ResponseWriter, r *http.Request) {
	networkID := chi.URLParam(r, "id")
	if networkID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Network ID is required.",
			Retryable: false,
		})
		return
	}

	ctx := requestContext(r)

	// Check if network exists
	network, err := h.repo.GetNetworkByID(ctx, networkID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "internal_error",
			Message:   "Could not check network.",
			Retryable: true,
		})
		return
	}
	if network == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:         "not_found",
			Message:      "Network not found.",
			ResourceType: "network",
			ResourceID:   networkID,
			Retryable:    false,
		})
		return
	}

	// TODO: Check if network is in use by any VMs
	// For now, we'll allow deletion

	if err := h.repo.DeleteNetwork(ctx, networkID); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "delete_failed",
			Message:   "Could not delete network.",
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]bool{"success": true})
}

// Helper to parse int from string
func parseInt(s string) int {
	i, _ := strconv.Atoi(s)
	return i
}
