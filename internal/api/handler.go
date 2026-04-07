package api

import (
	"context"
	"encoding/json"
	"net/http"

	"github.com/chv/chv/internal/auth"
	"github.com/chv/chv/internal/bootstrap"
	"github.com/chv/chv/internal/db"
	"github.com/go-chi/chi/v5"
)

type Handler struct {
	repo      *db.Repository
	auth      *auth.Service
	bootstrap *bootstrap.Service
	router    chi.Router
}

type errorEnvelope struct {
	Error apiError `json:"error"`
}

type apiError struct {
	Code         string `json:"code"`
	Message      string `json:"message"`
	ResourceType string `json:"resource_type,omitempty"`
	ResourceID   string `json:"resource_id,omitempty"`
	Retryable    bool   `json:"retryable"`
	Hint         string `json:"hint,omitempty"`
}

func NewHandler(repo *db.Repository, authService *auth.Service, bootstrapService *bootstrap.Service) *Handler {
	handler := &Handler{
		repo:      repo,
		auth:      authService,
		bootstrap: bootstrapService,
		router:    chi.NewRouter(),
	}
	handler.registerRoutes()
	return handler
}

func (h *Handler) Router() http.Handler {
	return h.router
}

func (h *Handler) registerRoutes() {
	h.router.Get("/health", func(w http.ResponseWriter, _ *http.Request) {
		h.writeJSON(w, http.StatusOK, map[string]any{"ok": true})
	})

	h.router.Route("/api/v1", func(r chi.Router) {
		r.Post("/tokens", h.createToken)
		r.Get("/install/status", h.installStatus)
		r.Post("/install/bootstrap", h.installBootstrap)
		r.Post("/install/repair", h.installRepair)

		r.Group(func(r chi.Router) {
			r.Use(h.authMiddleware)
			r.Post("/login/validate", h.loginValidate)
			r.Get("/networks", h.listNetworks)
			r.Get("/storage-pools", h.listStoragePools)
			r.Get("/images", h.listImages)
			r.Get("/vms", h.listVMs)
			r.Get("/operations", h.listOperations)
		})
	})
}

func (h *Handler) authMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if _, err := h.auth.ValidateToken(r.Context(), r.Header.Get("Authorization")); err != nil {
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

func (h *Handler) writeJSON(w http.ResponseWriter, status int, payload any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(payload)
}

func (h *Handler) writeError(w http.ResponseWriter, status int, payload apiError) {
	h.writeJSON(w, status, errorEnvelope{Error: payload})
}

func decodeJSON[T any](r *http.Request, dst *T) error {
	if r.Body == nil {
		return nil
	}
	return json.NewDecoder(r.Body).Decode(dst)
}

func requestContext(r *http.Request) context.Context {
	if r == nil {
		return context.Background()
	}
	return r.Context()
}
