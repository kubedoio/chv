package api

import (
	"encoding/json"
	"log"
	"net/http"
	"time"
)

// CreateTokenRequest represents a token creation request.
type CreateTokenRequest struct {
	Name    string `json:"name"`
	RoleID  string `json:"role_id,omitempty"`
	Expires string `json:"expires_in,omitempty"` // Duration string, e.g., "24h"
}

// CreateTokenResponse represents a token creation response.
type CreateTokenResponse struct {
	Token     string    `json:"token"`
	ID        string    `json:"id"`
	Name      string    `json:"name"`
	CreatedAt time.Time `json:"created_at"`
	ExpiresAt *time.Time `json:"expires_at,omitempty"`
}

// TokenResponse represents a token in the list
type TokenResponse struct {
	ID        string     `json:"id"`
	Name      string     `json:"name"`
	RoleID    *string    `json:"role_id,omitempty"`
	CreatedAt time.Time  `json:"created_at"`
	ExpiresAt *time.Time `json:"expires_at,omitempty"`
	RevokedAt *time.Time `json:"revoked_at,omitempty"`
}

func (h *Handler) createToken(w http.ResponseWriter, r *http.Request) {
	var req CreateTokenRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "Invalid request body")
		return
	}
	
	if req.Name == "" {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "name is required")
		return
	}
	
	var expiresIn *time.Duration
	if req.Expires != "" {
		d, err := time.ParseDuration(req.Expires)
		if err != nil {
			h.errorResponse(w, http.StatusBadRequest, "INVALID_DURATION", "Invalid expires_in duration")
			return
		}
		expiresIn = &d
	}
	
	result, err := h.auth.CreateToken(r.Context(), req.Name, req.RoleID, expiresIn)
	if err != nil {
		log.Printf("CreateToken error: %v", err)
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to create token")
		return
	}
	
	resp := CreateTokenResponse{
		Token: result.Token,
		ID:    result.ID,
		Name:  req.Name,
	}
	
	if expiresIn != nil {
		t := time.Now().Add(*expiresIn)
		resp.ExpiresAt = &t
	}
	
	h.jsonResponse(w, http.StatusCreated, resp)
}

// listTokens returns all API tokens
func (h *Handler) listTokens(w http.ResponseWriter, r *http.Request) {
	tokens, err := h.store.ListAPITokens(r.Context())
	if err != nil {
		log.Printf("ListTokens error: %v", err)
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to list tokens")
		return
	}

	// Convert to response format (hide sensitive fields)
	var resp []TokenResponse
	for _, t := range tokens {
		tr := TokenResponse{
			ID:        t.ID.String(),
			Name:      t.Name,
			CreatedAt: t.CreatedAt,
		}
		if t.RoleID != nil {
			roleID := t.RoleID.String()
			tr.RoleID = &roleID
		}
		if t.ExpiresAt != nil {
			tr.ExpiresAt = t.ExpiresAt
		}
		if t.RevokedAt != nil {
			tr.RevokedAt = t.RevokedAt
		}
		resp = append(resp, tr)
	}

	h.jsonResponse(w, http.StatusOK, resp)
}

// authMiddleware validates the bearer token.
func (h *Handler) authMiddleware(next http.Handler) http.Handler {
	log.Printf("authMiddleware called")
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		token := r.Header.Get("Authorization")
		if token == "" {
			h.errorResponse(w, http.StatusUnauthorized, "UNAUTHORIZED", "Missing authorization header")
			return
		}
		
		_, err := h.auth.ValidateToken(r.Context(), token)
		if err != nil {
			// Log the actual error for debugging
			log.Printf("Token validation failed: %v", err)
			h.errorResponse(w, http.StatusUnauthorized, "UNAUTHORIZED", "Invalid or expired token")
			return
		}
		
		next.ServeHTTP(w, r)
	})
}
