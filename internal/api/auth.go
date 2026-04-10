package api

import (
	"net/http"
)

type loginRequest struct {
	Username string `json:"username"`
	Password string `json:"password"`
}

type loginResponse struct {
	User      userInfo `json:"user"`
	Token     string   `json:"token"`
	TokenType string   `json:"token_type"`
	ExpiresIn int      `json:"expires_in"`
}

type userInfo struct {
	ID       string `json:"id"`
	Username string `json:"username"`
	Email    string `json:"email,omitempty"`
	Role     string `json:"role"`
	IsActive bool   `json:"is_active"`
}

func (h *Handler) login(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)

	var req loginRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON",
			Retryable: false,
		})
		return
	}

	if req.Username == "" || req.Password == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Username and password are required",
			Retryable: false,
		})
		return
	}

	result, err := h.auth.Login(ctx, req.Username, req.Password)
	if err != nil {
		h.writeError(w, http.StatusUnauthorized, apiError{
			Code:      "invalid_credentials",
			Message:   "Invalid username or password",
			Retryable: false,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, loginResponse{
		User: userInfo{
			ID:       result.User.ID,
			Username: result.User.Username,
			Email:    result.User.Email,
			Role:     result.User.Role,
			IsActive: result.User.IsActive,
		},
		Token:     result.Token,
		TokenType: result.TokenType,
		ExpiresIn: result.ExpiresIn,
	})
}

func (h *Handler) logout(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)

	token := r.Header.Get("Authorization")
	if token == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "missing_token",
			Message:   "Authorization header is required",
			Retryable: false,
		})
		return
	}

	if err := h.auth.Logout(ctx, token); err != nil {
		// Logout is best-effort, still return success
		// The token will expire naturally
	}

	h.writeJSON(w, http.StatusOK, map[string]string{
		"message": "Logged out successfully",
	})
}

func (h *Handler) getCurrentUser(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)

	token := r.Header.Get("Authorization")
	if token == "" {
		h.writeError(w, http.StatusUnauthorized, apiError{
			Code:      "missing_token",
			Message:   "Authorization header is required",
			Retryable: false,
		})
		return
	}

	user, err := h.auth.GetCurrentUser(ctx, token)
	if err != nil {
		h.writeError(w, http.StatusUnauthorized, apiError{
			Code:      "invalid_token",
			Message:   "Invalid or expired token",
			Retryable: false,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, userInfo{
		ID:       user.ID,
		Username: user.Username,
		Email:    user.Email,
		Role:     user.Role,
		IsActive: user.IsActive,
	})
}

// authMiddleware validates user bearer tokens
func (h *Handler) authMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, err := h.auth.ValidateToken(requestContext(r), r.Header.Get("Authorization"))
		if err != nil {
			h.writeError(w, http.StatusUnauthorized, apiError{
				Code:      "unauthorized",
				Message:   "A valid bearer token is required.",
				Retryable: false,
				Hint:      "Create a token with POST /api/v1/tokens and retry with Authorization: Bearer <token>.",
			})
			return
		}

		next.ServeHTTP(w, r)
	})
}

// agentAuthMiddleware validates tokens for agent endpoints
// For now, accepts any valid user token
func (h *Handler) agentAuthMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ctx := requestContext(r)
		authHeader := r.Header.Get("Authorization")

		if authHeader == "" {
			h.writeError(w, http.StatusUnauthorized, apiError{
				Code:      "missing_token",
				Message:   "Authorization header is required",
				Retryable: false,
			})
			return
		}

		// For now, accept any valid user token for agent endpoints
		if _, err := h.auth.ValidateToken(ctx, authHeader); err != nil {
			h.writeError(w, http.StatusUnauthorized, apiError{
				Code:      "unauthorized",
				Message:   "A valid bearer token is required.",
				Retryable: false,
			})
			return
		}

		next.ServeHTTP(w, r)
	})
}
