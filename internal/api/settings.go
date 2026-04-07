package api

import (
	"net/http"
	"runtime"
)

// SettingsResponse represents platform settings
type SettingsResponse struct {
	Version       string            `json:"version"`
	Platform      string            `json:"platform"`
	Features      []string          `json:"features"`
	ConsoleConfig ConsoleConfig     `json:"console"`
	Limits        PlatformLimits    `json:"limits"`
}

// ConsoleConfig represents console-related settings
type ConsoleConfig struct {
	Enabled       bool   `json:"enabled"`
	WebSocketPath string `json:"websocket_path"`
	RequiresToken bool   `json:"requires_token"`
}

// PlatformLimits represents platform resource limits
type PlatformLimits struct {
	MaxVCPUPerVM     int32 `json:"max_vcpus_per_vm"`
	MaxMemoryMBPerVM int64 `json:"max_memory_mb_per_vm"`
	MaxDiskGBPerVM   int64 `json:"max_disk_gb_per_vm"`
	MaxVMsPerNode    int   `json:"max_vms_per_node"`
}

// MeResponse represents current user info
type MeResponse struct {
	ID        string   `json:"id"`
	TokenID   string   `json:"token_id"`
	Role      string   `json:"role"`
	ExpiresAt *string  `json:"expires_at,omitempty"`
}

// getSettings returns platform settings
func (h *Handler) getSettings(w http.ResponseWriter, r *http.Request) {
	// These could be loaded from config in the future
	settings := SettingsResponse{
		Version:  "0.1.0-mvp1",
		Platform: runtime.GOOS + "/" + runtime.GOARCH,
		Features: []string{
			"vm-management",
			"console",
			"cloud-init",
			"networking",
			"storage",
		},
		ConsoleConfig: ConsoleConfig{
			Enabled:       true,
			WebSocketPath: "/api/v1/vms/{id}/console",
			RequiresToken: true,
		},
		Limits: PlatformLimits{
			MaxVCPUPerVM:     16,
			MaxMemoryMBPerVM: 65536,
			MaxDiskGBPerVM:   500,
			MaxVMsPerNode:    20,
		},
	}

	h.jsonResponse(w, http.StatusOK, settings)
}

// getMe returns current user info from the validated token
func (h *Handler) getMe(w http.ResponseWriter, r *http.Request) {
	// Get token from context (set by auth middleware)
	tokenStr := r.Header.Get("Authorization")
	
	// Validate token to get details
	tokenModel, err := h.auth.ValidateToken(r.Context(), tokenStr)
	if err != nil {
		h.errorResponse(w, http.StatusUnauthorized, "UNAUTHORIZED", "Invalid token")
		return
	}

	resp := MeResponse{
		ID:      tokenModel.ID.String(),
		TokenID: tokenModel.ID.String(),
		Role:    "admin", // Default role for MVP-1
	}

	if tokenModel.ExpiresAt != nil {
		expires := tokenModel.ExpiresAt.Format(http.TimeFormat)
		resp.ExpiresAt = &expires
	}

	h.jsonResponse(w, http.StatusOK, resp)
}
