package api

import (
	"encoding/json"
	"net/http"

	"github.com/chv/chv/internal/auth"
	"github.com/chv/chv/internal/operations"
	"github.com/chv/chv/internal/reconcile"
	"github.com/chv/chv/internal/scheduler"
	"github.com/chv/chv/internal/store"
	"github.com/chv/chv/internal/worker"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/cors"
)

// CORSConfig holds CORS configuration
type CORSConfig struct {
	Enabled        bool
	AllowedOrigins []string
}

// Handler holds API handlers.
type Handler struct {
	store              store.Store
	auth               *auth.Service
	scheduler          *scheduler.Service
	reconciler         *reconcile.Service
	imageImportWorker  *worker.ImageImportWorker
	operations         *operations.Service
	corsConfig         CORSConfig
}

// NewHandler creates a new API handler.
func NewHandler(store store.Store, auth *auth.Service, scheduler *scheduler.Service, reconciler *reconcile.Service) *Handler {
	return &Handler{
		store:      store,
		auth:       auth,
		scheduler:  scheduler,
		reconciler: reconciler,
		operations: operations.NewService(store),
	}
}

// SetImageImportWorker sets the image import worker.
func (h *Handler) SetImageImportWorker(w *worker.ImageImportWorker) {
	h.imageImportWorker = w
}

// SetCORSConfig sets the CORS configuration.
func (h *Handler) SetCORSConfig(config CORSConfig) {
	h.corsConfig = config
}

// getAllowedOrigins returns allowed origins with defaults.
func (h *Handler) getAllowedOrigins() []string {
	if len(h.corsConfig.AllowedOrigins) > 0 {
		return h.corsConfig.AllowedOrigins
	}
	return []string{"http://localhost:3000", "http://localhost:5173"}
}

// RegisterRoutes registers all API routes.
func (h *Handler) RegisterRoutes(r chi.Router) {
	// Add CORS middleware if enabled
	if h.corsConfig.Enabled {
		r.Use(cors.Handler(cors.Options{
			AllowedOrigins:   h.getAllowedOrigins(),
			AllowedMethods:   []string{"GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"},
			AllowedHeaders:   []string{"Authorization", "Content-Type"},
			AllowCredentials: true,
			MaxAge:           300,
		}))
	}

	// Health and metrics (public)
	r.Get("/health", h.healthCheck)
	r.Get("/metrics", h.metrics)
	
	// API v1 routes
	r.Route("/api/v1", func(r chi.Router) {
		// Public routes
		r.Post("/tokens", h.createToken)
		
		// Protected routes
		r.Group(func(r chi.Router) {
			r.Use(h.authMiddleware)
			
			// Nodes
			r.Post("/nodes/register", h.registerNode)
			r.Get("/nodes", h.listNodes)
			r.Get("/nodes/{id}", h.getNode)
			r.Post("/nodes/{id}/maintenance", h.setNodeMaintenance)
			
			// Networks
			r.Post("/networks", h.createNetwork)
			r.Get("/networks", h.listNetworks)
			r.Get("/networks/{id}", h.getNetwork)
			
			// Storage Pools
			r.Post("/storage-pools", h.createStoragePool)
			r.Get("/storage-pools", h.listStoragePools)
			r.Get("/storage-pools/{id}", h.getStoragePool)
			
			// Images
			r.Post("/images/import", h.importImage)
			r.Get("/images", h.listImages)
			r.Get("/images/{id}", h.getImage)
			
			// VMs
			r.Post("/vms", h.createVM)
			r.Get("/vms", h.listVMs)
			r.Get("/vms/{id}", h.getVM)
			r.Post("/vms/{id}/start", h.startVM)
			r.Post("/vms/{id}/stop", h.stopVM)
			r.Post("/vms/{id}/reboot", h.rebootVM)
			r.Post("/vms/{id}/resize-disk", h.resizeDisk)
			r.Delete("/vms/{id}", h.deleteVM)

			// Operations
			r.Get("/operations", h.listOperations)
			r.Get("/operations/{id}", h.getOperation)
			r.Get("/operations/{id}/logs", h.getOperationLogs)
		})
	})
}

// ErrorResponse represents an error response.
type ErrorResponse struct {
	Code    string `json:"code"`
	Message string `json:"message"`
}

func (h *Handler) jsonResponse(w http.ResponseWriter, status int, data interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(data)
}

func (h *Handler) errorResponse(w http.ResponseWriter, status int, code, message string) {
	h.jsonResponse(w, status, ErrorResponse{Code: code, Message: message})
}


