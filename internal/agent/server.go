package agent

import (
	"context"
	"crypto/subtle"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"time"

	"github.com/chv/chv/internal/agent/handlers"
	"github.com/chv/chv/internal/agent/services"
	"github.com/chv/chv/internal/cloudinit"
	"github.com/chv/chv/internal/config"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
)

type Server struct {
	addr   string
	cfg    config.AgentConfig
	router chi.Router
	server *http.Server
}

func NewServer(addr string, cfg config.AgentConfig) *Server {
	if addr == "" {
		addr = ":9090"
	}

	r := chi.NewRouter()

	// Middleware
	r.Use(middleware.Logger)
	r.Use(middleware.Recoverer)
	r.Use(middleware.RequestID)
	r.Use(middleware.RealIP)
	r.Use(jsonContentType)

	s := &Server{
		addr:   addr,
		cfg:    cfg,
		router: r,
	}

	// Auth middleware needs s to be initialized first
	r.Use(s.authMiddleware)

	s.routes()

	s.server = &http.Server{
		Addr:         addr,
		Handler:      r,
		ReadTimeout:  30 * time.Second,
		WriteTimeout: 30 * time.Second,
		IdleTimeout:  120 * time.Second,
	}

	return s
}

func (s *Server) routes() {
	s.router.Get("/health", s.handleHealth)

	// Install service
	installService := services.NewInstallService(s.cfg)
	installHandler := handlers.NewInstallHandler(installService)

	// Bootstrap service
	bootstrapService := services.NewBootstrapService()
	bootstrapHandler := handlers.NewBootstrapHandler(bootstrapService)

	// Firewall service (Stage 2 Security)
	fwService := services.NewFirewallService(s.cfg.DataRoot, s.cfg.BridgeName)
	if err := fwService.Init(); err != nil {
		fmt.Fprintf(os.Stderr, "Warning: failed to initialize firewall: %v\n", err)
	}

	// Image download service
	imageDownloadService := services.NewImageDownloadService()
	imageHandler := handlers.NewImageHandler(imageDownloadService)

	// Cloud-init/Seed ISO service
	seedISOService := services.NewSeedISOService()
	cloudInitHandler := handlers.NewCloudInitHandler(seedISOService)

	// VM management service
	cloudInitRenderer := cloudinit.NewRenderer(s.cfg.DataRoot)
	vmService := services.NewVMManagementService(s.cfg.DataRoot, s.cfg.BridgeName, fwService, seedISOService, cloudInitRenderer)

	// Recover orphan VMs on startup
	if recovered, err := vmService.ScanAndRecoverOrphans(s.cfg.DataRoot); err != nil {
		fmt.Fprintf(os.Stderr, "Warning: failed to scan for orphan VMs: %v\n", err)
	} else if len(recovered) > 0 {
		fmt.Fprintf(os.Stderr, "Recovered %d orphan VM(s) on startup: %v\n", len(recovered), recovered)
	}

	vmHealthService := services.NewVMHealthService()
	vmConsoleService := services.NewVMConsoleService()
	vmHandler := handlers.NewVMHandler(vmService, vmHealthService, vmConsoleService)

	s.router.Route("/v1", func(r chi.Router) {
		r.Get("/install/check", installHandler.Check)
		r.Post("/install/bootstrap", bootstrapHandler.Bootstrap)
		r.Post("/images/download", imageHandler.Download)
		r.Get("/cloud-init/support", cloudInitHandler.CheckISOSupport)
		r.Post("/cloud-init/seed-iso", cloudInitHandler.GenerateSeedISO)
		r.Post("/vms/start", vmHandler.StartVM)
		r.Post("/vms/stop", vmHandler.StopVM)
		r.Post("/vms/destroy", vmHandler.DestroyVM)
		r.Post("/vms/provision", vmHandler.ProvisionVM)
		r.Post("/vms/status", vmHandler.GetVMStatus)
		r.Get("/vms/running", vmHandler.ListRunningVMs)
		r.Post("/vms/metrics", vmHandler.GetVMMetrics)
		r.Post("/vms/health", vmHandler.HealthCheck)
		r.Get("/vms/console", vmHandler.Console)
		r.Get("/vms/vnc", vmHandler.VNCConsole)
		r.Post("/vms/snapshots", vmHandler.CreateSnapshot)
		r.Post("/vms/snapshots/list", vmHandler.ListSnapshots)
		r.Post("/vms/snapshots/restore", vmHandler.RestoreSnapshot)
		r.Post("/vms/snapshots/delete", vmHandler.DeleteSnapshot)
	})
}

func (s *Server) handleHealth(w http.ResponseWriter, r *http.Request) {
	respondJSON(w, http.StatusOK, map[string]any{
		"ok":      true,
		"service": "chv-agent",
	})
}

func (s *Server) Start() error {
	return s.server.ListenAndServe()
}

func (s *Server) Shutdown(ctx context.Context) error {
	return s.server.Shutdown(ctx)
}

// authMiddleware validates the Authorization header for controller requests
func (s *Server) authMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Health check is always public
		if r.URL.Path == "/health" {
			next.ServeHTTP(w, r)
			return
		}

		// If no auth token is configured, allow all requests (development mode)
		if s.cfg.AuthToken == "" {
			next.ServeHTTP(w, r)
			return
		}

		// Validate Bearer token
		authHeader := r.Header.Get("Authorization")
		const prefix = "Bearer "
		if len(authHeader) < len(prefix) || authHeader[:len(prefix)] != prefix {
			http.Error(w, `{"error":"unauthorized"}`, http.StatusUnauthorized)
			return
		}

		token := authHeader[len(prefix):]
		// Constant-time comparison to prevent timing attacks
		if !constantTimeEqual(token, s.cfg.AuthToken) {
			http.Error(w, `{"error":"unauthorized"}`, http.StatusUnauthorized)
			return
		}

		next.ServeHTTP(w, r)
	})
}

// constantTimeEqual compares two strings in constant time to prevent timing attacks
func constantTimeEqual(a, b string) bool {
	return subtle.ConstantTimeCompare([]byte(a), []byte(b)) == 1
}

// Middleware
func jsonContentType(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Skip for WebSocket upgrade requests
		if r.Header.Get("Upgrade") == "websocket" {
			next.ServeHTTP(w, r)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		next.ServeHTTP(w, r)
	})
}

// Helper
func respondJSON(w http.ResponseWriter, status int, payload any) {
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(payload)
}
