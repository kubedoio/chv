package agent

import (
	"context"
	"encoding/json"
	"net/http"
	"time"

	"github.com/chv/chv/internal/agent/handlers"
	"github.com/chv/chv/internal/agent/services"
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

	// Image download service
	imageDownloadService := services.NewImageDownloadService()
	imageHandler := handlers.NewImageHandler(imageDownloadService)

	// Cloud-init/Seed ISO service
	seedISOService := services.NewSeedISOService()
	cloudInitHandler := handlers.NewCloudInitHandler(seedISOService)

	// VM management service
	vmService := services.NewVMManagementService()
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
		r.Post("/vms/status", vmHandler.GetVMStatus)
		r.Get("/vms/running", vmHandler.ListRunningVMs)
		r.Post("/vms/metrics", vmHandler.GetVMMetrics)
		r.Post("/vms/health", vmHandler.HealthCheck)
		r.Get("/vms/console", vmHandler.Console)
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

// Middleware
func jsonContentType(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		next.ServeHTTP(w, r)
	})
}

// Helper
func respondJSON(w http.ResponseWriter, status int, payload any) {
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(payload)
}
