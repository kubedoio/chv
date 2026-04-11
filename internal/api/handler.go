package api

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"

	"github.com/chv/chv/internal/audit"
	"github.com/chv/chv/internal/auth"
	"github.com/chv/chv/internal/backup"
	"github.com/chv/chv/internal/bootstrap"
	"github.com/chv/chv/internal/config"
	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/images"
	"github.com/chv/chv/internal/logger"
	"github.com/chv/chv/internal/vm"
	"github.com/go-chi/chi/v5"
	"github.com/prometheus/client_golang/prometheus/promhttp"
)

type Handler struct {
	repo          *db.Repository
	auth          *auth.Service
	bootstrap     *bootstrap.Service
	config        config.ControllerConfig
	router        chi.Router
	imageWorker   *images.Worker
	vmService     *vm.Service
	backupService *backup.Service
	reconciler    *ReconciliationLoop
	auditLogger   *audit.Logger
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

func NewHandler(repo *db.Repository, authService *auth.Service, bootstrapService *bootstrap.Service, cfg config.ControllerConfig, imageWorker *images.Worker, vmService *vm.Service, backupService *backup.Service) *Handler {
	handler := &Handler{
		repo:          repo,
		auth:          authService,
		bootstrap:     bootstrapService,
		config:        cfg,
		router:        chi.NewRouter(),
		imageWorker:   imageWorker,
		vmService:     vmService,
		backupService: backupService,
	}
	handler.registerRoutes()
	return handler
}

// StartReconciliationLoop starts the background VM state reconciliation loop
func (h *Handler) StartReconciliationLoop(ctx context.Context) {
	if h.vmService == nil {
		logger.L().Warn("Cannot start reconciliation loop: vmService is nil")
		return
	}
	logger.L().Info("Starting VM state reconciliation loop")
	h.reconciler = NewReconciliationLoop(h)
	h.reconciler.Start(ctx)
}

// StopReconciliationLoop stops the background VM state reconciliation loop
func (h *Handler) StopReconciliationLoop() {
	if h.reconciler != nil {
		h.reconciler.Stop()
	}
}

func (h *Handler) Router() http.Handler {
	return h.router
}

// corsMiddleware handles CORS headers and preflight requests
// Only allows configured origins; defaults to same-origin (no external CORS)
func corsMiddleware(next http.Handler) http.Handler {
	// Parse allowed origins from env var (comma-separated)
	// e.g., CHV_CORS_ORIGINS=https://chv.example.com,https://admin.chv.example.com
	var allowedOrigins []string
	if env := os.Getenv("CHV_CORS_ORIGINS"); env != "" {
		for _, o := range strings.Split(env, ",") {
			o = strings.TrimSpace(o)
			if o != "" {
				allowedOrigins = append(allowedOrigins, o)
			}
		}
	}

	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		origin := r.Header.Get("Origin")

		// Check if origin is allowed
		originAllowed := false
		if origin == "" {
			// Same-origin request (no Origin header)
			originAllowed = true
		} else {
			for _, allowed := range allowedOrigins {
				if origin == allowed {
					originAllowed = true
					break
				}
			}
		}

		if originAllowed && origin != "" {
			w.Header().Set("Access-Control-Allow-Origin", origin)
			w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS, PATCH")
			w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")
			w.Header().Set("Access-Control-Max-Age", "86400")
		}

		// Handle preflight OPTIONS requests
		if r.Method == "OPTIONS" {
			w.WriteHeader(http.StatusOK)
			return
		}

		next.ServeHTTP(w, r)
	})
}

func (h *Handler) registerRoutes() {
	// CORS middleware
	h.router.Use(corsMiddleware)

	// Metrics middleware - records API request metrics
	h.router.Use(MetricsMiddleware)

	// Serve static files for UI (SPA support)
	workDir, _ := os.Getwd()
	filesDir := http.Dir(filepath.Join(workDir, "ui", "build"))
	spaFileServer(h.router, "/", filesDir)

	h.router.Route("/api/v1", func(r chi.Router) {
		r.Get("/health", h.healthHandler)
		r.Post("/tokens", h.createToken)
		r.Post("/auth/login", h.login)
		r.Post("/auth/logout", h.logout)
		r.Get("/auth/me", h.getCurrentUser)
		r.Get("/install/status", h.installStatus)
		r.Post("/install/bootstrap", h.installBootstrap)
		r.Post("/install/repair", h.installRepair)

		// Agent endpoints — agent auth is handled at the agent level via CHV_AGENT_TOKEN
		r.Route("/agents", func(r chi.Router) {
			r.Use(h.authMiddleware)
			r.Post("/register", h.registerAgent)
			r.Post("/heartbeat", h.agentHeartbeat)
		})

		r.Group(func(r chi.Router) {
			r.Use(h.authMiddleware)
			r.Post("/login/validate", h.loginValidate)
			r.Get("/networks", h.listNetworks)
			r.Post("/networks", h.createNetwork)
			r.Route("/networks/{id}", func(r chi.Router) {
				r.Get("/", h.getNetworkHandler)
				r.Delete("/", h.deleteNetworkHandler)
				r.Route("/vlans", func(r chi.Router) {
					r.Get("/", h.listVLANsHandler)
					r.Post("/", h.createVLANHandler)
					r.Delete("/{vlanId}", h.deleteVLANHandler)
				})
				r.Route("/dhcp", func(r chi.Router) {
					r.Get("/", h.getDHCPStatusHandler)
					r.Post("/", h.configureDHCPHandler)
					r.Post("/start", h.startDHCPHandler)
					r.Post("/stop", h.stopDHCPHandler)
					r.Get("/leases", h.getDHCPLeasesHandler)
				})
			})
			r.Get("/storage-pools", h.listStoragePools)
			r.Post("/storage-pools", h.createStoragePool)
			r.Get("/images", h.listImages)
			r.Post("/images/import", h.createImage)
			r.Post("/images/upload", h.uploadImage)
			r.Get("/images/{id}/progress", h.getImageProgress)
			r.Get("/events", h.listEvents)
			r.Route("/vms", func(r chi.Router) {
				r.Get("/", h.listVMs)
				r.Post("/", h.createVM)
				r.Post("/bulk/start", h.bulkStartVMs)
				r.Post("/bulk/stop", h.bulkStopVMs)
				r.Post("/bulk/delete", h.bulkDeleteVMs)
				r.Route("/{id}", func(r chi.Router) {
					r.Get("/", h.getVM)
					r.Get("/status", h.getVMStatus)
					r.Get("/metrics", h.getVMMetrics)
					r.Get("/boot-logs", h.getVMBootLogs)
					r.Post("/start", h.startVM)
					r.Post("/stop", h.stopVM)
					r.Post("/shutdown", h.shutdownVM)
					r.Post("/force-stop", h.forceStopVM)
					r.Post("/reset", h.resetVM)
					r.Post("/restart", h.restartVM)
					r.Delete("/", h.deleteVM)
					r.Get("/console", h.getVMConsole)
					r.Route("/snapshots", func(r chi.Router) {
						r.Get("/", h.listVMSnapshots)
						r.Post("/", h.createVMSnapshot)
						r.Post("/{snapId}/restore", h.restoreVMSnapshot)
						r.Delete("/{snapId}", h.deleteVMSnapshot)
					})
					r.Route("/firewall", func(r chi.Router) {
						r.Get("/rules", h.listFirewallRulesHandler)
						r.Post("/rules", h.createFirewallRuleHandler)
						r.Delete("/rules/{ruleId}", h.deleteFirewallRuleHandler)
					})
					r.Get("/backups", h.listVMBackups)
				})
			})
			r.Get("/operations", h.listOperations)

			// Node-scoped resource endpoints
			r.Route("/nodes", func(r chi.Router) {
				r.Get("/", h.listNodes)
				r.Post("/", h.createNode)
				r.Get("/health", h.getAllNodesHealth)
				r.Route("/{id}", func(r chi.Router) {
					r.Get("/", h.getNode)
					r.Patch("/", h.updateNode)
					r.Delete("/", h.deleteNode)
					r.Post("/maintenance", h.setNodeMaintenance)
					r.Get("/health", h.getNodeHealth)
					r.Get("/metrics", h.getNodeMetrics)
					r.Get("/vms", h.listNodeVMs)
					r.Get("/images", h.listNodeImages)
					r.Get("/storage", h.listNodeStoragePools)
					r.Get("/networks", h.listNodeNetworks)
				})
			})

			// RBAC endpoints
			r.Route("/users", func(r chi.Router) {
				r.Get("/", h.listUsers)
				r.Post("/", h.createUser)
				r.Route("/{id}", func(r chi.Router) {
					r.Get("/", h.getUser)
					r.Patch("/", h.updateUser)
					r.Delete("/", h.deleteUser)
					r.Post("/reset-password", h.resetPassword)
				})
			})
			r.Get("/roles", h.listRoles)
			r.Get("/audit-logs", h.listAuditLogs)

			// VM Templates endpoints
			r.Route("/vm-templates", func(r chi.Router) {
				r.Get("/", h.listVMTemplates)
				r.Post("/", h.createVMTemplate)
				r.Route("/{id}", func(r chi.Router) {
					r.Get("/", h.getVMTemplate)
					r.Delete("/", h.deleteVMTemplate)
					r.Post("/clone", h.cloneFromTemplate)
					r.Get("/preview", h.previewVMTemplate)
				})
			})

			// Cloud-init Templates endpoints
			r.Route("/cloud-init-templates", func(r chi.Router) {
				r.Get("/", h.listCloudInitTemplates)
				r.Post("/", h.createCloudInitTemplate)
				r.Route("/{id}", func(r chi.Router) {
					r.Get("/", h.getCloudInitTemplate)
					r.Delete("/", h.deleteCloudInitTemplate)
					r.Post("/render", h.renderCloudInitTemplate)
				})
			})

			// Backup Jobs endpoints
			r.Route("/backup-jobs", func(r chi.Router) {
				r.Get("/", h.listBackupJobs)
				r.Post("/", h.createBackupJob)
				r.Route("/{id}", func(r chi.Router) {
					r.Get("/", h.getBackupJob)
					r.Delete("/", h.deleteBackupJob)
					r.Post("/run", h.runBackupJob)
					r.Post("/toggle", h.toggleBackupJob)
				})
			})

			// VM Export/Import endpoints
			r.Post("/vms/{id}/export", h.exportVM)
			r.Post("/vms/import", h.importVM)
			r.Get("/exports/{id}/download", h.downloadExport)

			// Quota endpoints
			r.Route("/quotas", func(r chi.Router) {
				r.Get("/", h.listQuotas)
				r.Post("/", h.createQuota)
				r.Get("/me", h.getMyQuota)
				r.Post("/check", h.checkQuota)
				r.Route("/{userId}", func(r chi.Router) {
					r.Get("/", h.getQuota)
					r.Patch("/", h.updateQuota)
					r.Get("/usage", h.getUserUsage)
				})
			})

			// Usage endpoint (current user's usage)
			r.Get("/usage", h.getUsage)
		})
		// Prometheus metrics endpoint
		r.Get("/metrics", promhttp.Handler().ServeHTTP)
		// WebSocket console endpoint - outside auth middleware (token passed in query param)
		r.Get("/vms/console/ws", h.vmConsoleWebSocket)
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

// spaFileServer serves static files and falls back to index.html for SPA routes
func spaFileServer(r chi.Router, path string, root http.FileSystem) {
	if strings.ContainsAny(path, "{}*") {
		logger.L().Error("FileServer does not permit URL parameters", logger.F("path", path))
		return
	}

	if path != "/" && path[len(path)-1] != '/' {
		r.Get(path, http.RedirectHandler(path+"/", http.StatusMovedPermanently).ServeHTTP)
		path += "/"
	}
	path += "*"

	r.Get(path, func(w http.ResponseWriter, r *http.Request) {
		rctx := chi.RouteContext(r.Context())
		pathPrefix := strings.TrimSuffix(rctx.RoutePattern(), "/*")
		requestPath := strings.TrimPrefix(r.URL.Path, pathPrefix)
		
		// Try to open the requested file
		f, err := root.Open(requestPath)
		if err != nil {
			// File doesn't exist - serve index.html for SPA routing
			f, err = root.Open("index.html")
			if err != nil {
				http.NotFound(w, r)
				return
			}
			defer f.Close()
			
			// Check if it's a directory
			stat, err := f.Stat()
			if err != nil || stat.IsDir() {
				// Serve index.html for directory requests
				f, _ = root.Open("index.html")
				if f != nil {
					defer f.Close()
				}
			}
			
			w.Header().Set("Content-Type", "text/html")
			http.ServeContent(w, r, "index.html", stat.ModTime(), f.(io.ReadSeeker))
			return
		}
		defer f.Close()
		
		// Check if it's a directory
		stat, err := f.Stat()
		if err != nil {
			http.NotFound(w, r)
			return
		}
		
		if stat.IsDir() {
			// Try to serve index.html from the directory
			indexPath := filepath.Join(requestPath, "index.html")
			indexFile, err := root.Open(indexPath)
			if err != nil {
				// No index.html - serve root index.html for SPA
				f, _ = root.Open("index.html")
				if f != nil {
					defer f.Close()
					stat, _ = f.Stat()
					w.Header().Set("Content-Type", "text/html")
					http.ServeContent(w, r, "index.html", stat.ModTime(), f.(io.ReadSeeker))
					return
				}
				http.NotFound(w, r)
				return
			}
			defer indexFile.Close()
			indexStat, _ := indexFile.Stat()
			w.Header().Set("Content-Type", "text/html")
			http.ServeContent(w, r, "index.html", indexStat.ModTime(), indexFile.(io.ReadSeeker))
			return
		}
		
		// Serve the file
		http.ServeContent(w, r, stat.Name(), stat.ModTime(), f.(io.ReadSeeker))
	})
}

func (h *Handler) listVMSnapshots(w http.ResponseWriter, r *http.Request) {
	vmID := chi.URLParam(r, "id")
	snaps, err := h.vmService.ListSnapshots(r.Context(), vmID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{Code: "list_failed", Message: err.Error()})
		return
	}
	h.writeJSON(w, http.StatusOK, snaps)
}

func (h *Handler) createVMSnapshot(w http.ResponseWriter, r *http.Request) {
	vmID := chi.URLParam(r, "id")
	snap, err := h.vmService.CreateSnapshot(r.Context(), vmID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{Code: "create_failed", Message: err.Error()})
		return
	}
	h.writeJSON(w, http.StatusCreated, snap)
}

func (h *Handler) restoreVMSnapshot(w http.ResponseWriter, r *http.Request) {
	vmID := chi.URLParam(r, "id")
	snapID := chi.URLParam(r, "snapId")
	err := h.vmService.RestoreSnapshot(r.Context(), vmID, snapID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{Code: "restore_failed", Message: err.Error()})
		return
	}
	h.writeJSON(w, http.StatusOK, map[string]bool{"success": true})
}

func (h *Handler) deleteVMSnapshot(w http.ResponseWriter, r *http.Request) {
	vmID := chi.URLParam(r, "id")
	snapID := chi.URLParam(r, "snapId")
	err := h.vmService.DeleteSnapshot(r.Context(), vmID, snapID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{Code: "delete_failed", Message: err.Error()})
		return
	}
	h.writeJSON(w, http.StatusOK, map[string]bool{"success": true})
}
