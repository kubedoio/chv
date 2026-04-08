package api

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"

	"github.com/chv/chv/internal/auth"
	"github.com/chv/chv/internal/bootstrap"
	"github.com/chv/chv/internal/config"
	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/images"
	"github.com/chv/chv/internal/vm"
	"github.com/go-chi/chi/v5"
)

type Handler struct {
	repo        *db.Repository
	auth        *auth.Service
	bootstrap   *bootstrap.Service
	config      config.ControllerConfig
	router      chi.Router
	imageWorker *images.Worker
	vmService   *vm.Service
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

func NewHandler(repo *db.Repository, authService *auth.Service, bootstrapService *bootstrap.Service, cfg config.ControllerConfig, imageWorker *images.Worker, vmService *vm.Service) *Handler {
	handler := &Handler{
		repo:        repo,
		auth:        authService,
		bootstrap:   bootstrapService,
		config:      cfg,
		router:      chi.NewRouter(),
		imageWorker: imageWorker,
		vmService:   vmService,
	}
	handler.registerRoutes()
	return handler
}

func (h *Handler) Router() http.Handler {
	return h.router
}

// corsMiddleware handles CORS headers and preflight requests
func corsMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Set CORS headers
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")
		w.Header().Set("Access-Control-Max-Age", "86400")

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

	h.router.Get("/health", func(w http.ResponseWriter, _ *http.Request) {
		h.writeJSON(w, http.StatusOK, map[string]any{"ok": true})
	})

	// Serve static files for UI (SPA support)
	workDir, _ := os.Getwd()
	filesDir := http.Dir(filepath.Join(workDir, "ui", "build"))
	spaFileServer(h.router, "/", filesDir)

	h.router.Route("/api/v1", func(r chi.Router) {
		r.Post("/tokens", h.createToken)
		r.Post("/auth/login", h.login)
		r.Post("/auth/logout", h.logout)
		r.Get("/auth/me", h.getCurrentUser)
		r.Get("/install/status", h.installStatus)
		r.Post("/install/bootstrap", h.installBootstrap)
		r.Post("/install/repair", h.installRepair)

		r.Group(func(r chi.Router) {
			r.Use(h.authMiddleware)
			r.Post("/login/validate", h.loginValidate)
			r.Get("/networks", h.listNetworks)
			r.Post("/networks", h.createNetwork)
			r.Get("/storage-pools", h.listStoragePools)
			r.Post("/storage-pools", h.createStoragePool)
			r.Get("/images", h.listImages)
			r.Post("/images/import", h.createImage)
			r.Get("/images/{id}/progress", h.getImageProgress)
			r.Get("/events", h.listEvents)
			r.Route("/vms", func(r chi.Router) {
				r.Get("/", h.listVMs)
				r.Post("/", h.createVM)
				r.Route("/{id}", func(r chi.Router) {
					r.Get("/", h.getVM)
					r.Get("/status", h.getVMStatus)
					r.Get("/metrics", h.getVMMetrics)
					r.Post("/start", h.startVM)
					r.Post("/stop", h.stopVM)
					r.Post("/restart", h.restartVM)
					r.Delete("/", h.deleteVM)
					r.Get("/console", h.getVMConsole)
				})
			})
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

// spaFileServer serves static files and falls back to index.html for SPA routes
func spaFileServer(r chi.Router, path string, root http.FileSystem) {
	if strings.ContainsAny(path, "{}*") {
		panic("FileServer does not permit any URL parameters.")
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
