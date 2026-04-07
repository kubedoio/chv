package api

import (
	"encoding/json"
	"net/http"

	"github.com/chv/chv/internal/agent"
	"github.com/chv/chv/internal/auth"
	"github.com/chv/chv/internal/config"
	"github.com/chv/chv/internal/hypervisor"
	"github.com/chv/chv/internal/operations"
	"github.com/chv/chv/internal/quota"
	"github.com/chv/chv/internal/reconcile"
	"github.com/chv/chv/internal/scheduler"
	"github.com/chv/chv/internal/store"
	"github.com/chv/chv/internal/worker"
	"github.com/go-chi/chi/v5"
)

// Handler holds API handlers.
type Handler struct {
	store             store.Store
	auth              *auth.Service
	scheduler         *scheduler.Service
	reconciler        *reconcile.Service
	quota             *quota.Service
	imageImportWorker *worker.ImageImportWorker
	operations        *operations.Service
	consoleSessions   *hypervisor.SessionManager
	agentClient       agent.Client
	config            *config.ControllerConfig
}

// NewHandler creates a new API handler.
func NewHandler(store store.Store, auth *auth.Service, scheduler *scheduler.Service, reconciler *reconcile.Service, agentClient agent.Client, cfg *config.ControllerConfig) *Handler {
	if agentClient == nil {
		agentClient = agent.NewClient()
	}
	if cfg == nil {
		cfg = config.DefaultControllerConfig()
	}
	return &Handler{
		store:           store,
		auth:            auth,
		scheduler:       scheduler,
		reconciler:      reconciler,
		quota:           quota.NewService(store),
		operations:      operations.NewService(store),
		consoleSessions: hypervisor.NewSessionManager(),
		agentClient:     agentClient,
		config:          cfg,
	}
}

// SetImageImportWorker sets the image import worker.
func (h *Handler) SetImageImportWorker(w *worker.ImageImportWorker) {
	h.imageImportWorker = w
}

// RegisterRoutes registers all API routes.
func (h *Handler) RegisterRoutes(r chi.Router) {
	// Health and metrics (public)
	r.Get("/health", h.healthCheck)
	r.Get("/metrics", h.metrics)
	r.Get("/metrics/prometheus", h.prometheusMetrics)
	
	// API v1 routes
	r.Route("/api/v1", func(r chi.Router) {
		// Public routes
		r.Get("/health", h.handleHealthV1)
		r.Post("/tokens", h.createToken)
		
		// VM Console (public - uses query param token for WebSocket)
		r.Get("/vms/{id}/console", h.vmConsole)
		
		// Protected routes
		r.Group(func(r chi.Router) {
			r.Use(h.authMiddleware)
			
			// Tokens
			r.Get("/tokens", h.listTokens)
			
			// Nodes
			r.Post("/nodes/register", h.registerNode)
			r.Get("/nodes", h.listNodes)
			r.Get("/nodes/{id}", h.getNode)
			r.Post("/nodes/{id}/maintenance", h.setNodeMaintenance)
			
			// Networks
			r.Post("/networks", h.createNetwork)
			r.Get("/networks", h.listNetworks)
			r.Get("/networks/{id}", h.getNetwork)
			r.Delete("/networks/{id}", h.deleteNetwork)
			
			// Storage Pools
			r.Post("/storage-pools", h.createStoragePool)
			r.Get("/storage-pools", h.listStoragePools)
			r.Get("/storage-pools/{id}", h.getStoragePool)
			r.Delete("/storage-pools/{id}", h.deleteStoragePool)
			
			// Images
			r.Post("/images/import", h.importImage)
			r.Get("/images", h.listImages)
			r.Get("/images/{id}", h.getImage)
			r.Delete("/images/{id}", h.deleteImage)
			
			// VMs
			r.Post("/vms", h.createVM)
			r.Get("/vms", h.listVMs)
			r.Get("/vms/{id}", h.getVM)
			r.Put("/vms/{id}", h.updateVM)
			r.Post("/vms/{id}/start", h.startVM)
			r.Post("/vms/{id}/stop", h.stopVM)
			r.Post("/vms/{id}/reboot", h.rebootVM)
			r.Post("/vms/{id}/resize-disk", h.resizeDisk)
			r.Delete("/vms/{id}", h.deleteVM)

			// VM Snapshots
			r.Post("/vms/{id}/snapshots", h.createSnapshot)
			r.Get("/vms/{id}/snapshots", h.listSnapshots)
			r.Delete("/vms/{id}/snapshots/{snapshot_id}", h.deleteSnapshot)

			// Volume Clone
			r.Post("/volumes/{id}/clone", h.cloneVolume)

			// Operations
			r.Get("/operations", h.listOperations)
			r.Get("/operations/{id}", h.getOperation)
			r.Get("/operations/{id}/logs", h.getOperationLogs)
			
			// Settings and User Info
			r.Get("/settings", h.getSettings)
			r.Get("/me", h.getMe)

			// Resource Quotas
			r.Get("/quota", h.getQuota)
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



// updateVM handles VM updates
func (h *Handler) updateVM(w http.ResponseWriter, r *http.Request) {
	// TODO: Implement VM update
	h.errorResponse(w, http.StatusNotImplemented, "NOT_IMPLEMENTED", "VM update not yet implemented")
}

// createSnapshot handles snapshot creation
func (h *Handler) createSnapshot(w http.ResponseWriter, r *http.Request) {
	// TODO: Implement snapshot creation
	h.errorResponse(w, http.StatusNotImplemented, "NOT_IMPLEMENTED", "Snapshot creation not yet implemented")
}

// listSnapshots handles listing snapshots
func (h *Handler) listSnapshots(w http.ResponseWriter, r *http.Request) {
	// TODO: Implement snapshot listing
	h.errorResponse(w, http.StatusNotImplemented, "NOT_IMPLEMENTED", "Snapshot listing not yet implemented")
}

// deleteSnapshot handles snapshot deletion
func (h *Handler) deleteSnapshot(w http.ResponseWriter, r *http.Request) {
	// TODO: Implement snapshot deletion
	h.errorResponse(w, http.StatusNotImplemented, "NOT_IMPLEMENTED", "Snapshot deletion not yet implemented")
}
