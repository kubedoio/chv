package agentapi

// VMConsoleRequest requests console access
type VMConsoleRequest struct {
	VMID      string `json:"vm_id"`
	APISocket string `json:"api_socket"`
}

// VMConsoleResponse provides console connection info
type VMConsoleResponse struct {
	WSURL   string `json:"ws_url"`
	Message string `json:"message"`
}
