package main

import (
	"context"
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/chv/chv/internal/agentclient"
	"github.com/chv/chv/internal/api"
	"github.com/chv/chv/internal/auth"
	"github.com/chv/chv/internal/backup"
	"github.com/chv/chv/internal/bootstrap"
	"github.com/chv/chv/internal/config"
	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/health"
	"github.com/chv/chv/internal/images"
	"github.com/chv/chv/internal/logger"
	"github.com/chv/chv/internal/metrics"
	"github.com/chv/chv/internal/operations"
	"github.com/chv/chv/internal/vm"
)

func main() {
	cfg := config.LoadController()

	// Initialize structured logger
	logCfg := logger.Config{
		Level:      getenv("CHV_LOG_LEVEL", "info"),
		Component:  "controller",
		Structured: true,
		LogDir:     cfg.LogDir,
	}
	if err := logger.InitDefault(logCfg); err != nil {
		// Fallback to basic logging
		fmt.Printf("Failed to initialize logger: %v\n", err)
	}
	log := logger.L()

	log.Info("Starting CHV controller", logger.F("addr", cfg.HTTPAddr))

	repo, err := db.Open(cfg.DatabasePath)
	if err != nil {
		log.Fatal("Failed to open database", logger.ErrorField(err))
	}
	defer repo.Close()
	log.Info("Database opened", logger.F("path", cfg.DatabasePath))

	// Create agent client (optional, falls back to direct if not configured)
	var agentClient *agentclient.Client
	if agentURL := os.Getenv("CHV_AGENT_URL"); agentURL != "" {
		agentToken := os.Getenv("CHV_AGENT_TOKEN")
		if agentToken != "" {
			agentClient = agentclient.NewClientWithAuth(agentURL, agentToken)
			log.Info("Agent client configured with auth", logger.F("url", agentURL))
		} else {
			agentClient = agentclient.NewClient(agentURL)
			log.Warn("Agent client configured without auth token — configure CHV_AGENT_TOKEN for security")
		}
	} else {
		log.Warn("No agent URL configured, some features will be limited")
	}

	bootstrapService, err := bootstrap.NewService(bootstrap.Config{
		DataRoot:      cfg.DataRoot,
		DatabasePath:  cfg.DatabasePath,
		BridgeName:    cfg.BridgeName,
		BridgeCIDR:    cfg.BridgeCIDR,
		LocaldiskPath: cfg.LocaldiskPath,
		Repository:    repo,
		AgentClient:   agentClient,
	})
	if err != nil {
		log.Fatal("Failed to create bootstrap service", logger.ErrorField(err))
	}

	// Create and start image import worker
	var imageWorker *images.Worker
	if agentURL := os.Getenv("CHV_AGENT_URL"); agentURL != "" {
		opService := operations.NewService(repo)
		agentToken := os.Getenv("CHV_AGENT_TOKEN")
		if agentToken != "" {
			imageWorker = images.NewWorkerWithAuth(repo, opService, agentURL, agentToken)
			log.Info("Image import worker started with auth")
		} else {
			imageWorker = images.NewWorker(repo, opService, agentURL)
			log.Info("Image import worker started (no auth)")
		}
		imageWorker.Start(context.Background())
		defer imageWorker.Stop()
	}

	// Create VM service (singleton)
	vmService := vm.NewService(repo, cfg.DataRoot)
	if agentURL := os.Getenv("CHV_AGENT_URL"); agentURL != "" {
		agentToken := os.Getenv("CHV_AGENT_TOKEN")
		if agentToken != "" {
			vmService.SetAgentClientWithAuth(agentURL, agentToken)
			log.Info("VM service configured with agent and auth")
		} else {
			vmService.SetAgentClient(agentURL)
			log.Info("VM service configured with agent (no auth)")
		}
	}

	// Create auth service and ensure admin user exists
	authService := auth.NewService(repo)
	if err := authService.EnsureAdminUser(context.Background()); err != nil {
		log.Error("Failed to ensure admin user", logger.ErrorField(err))
	} else {
		log.Info("Admin user ensured")
	}

	// Create backup service
	backupService := backup.NewService(repo, vmService, cfg.DataRoot)
	if err := backupService.Initialize(context.Background()); err != nil {
		log.Error("Failed to initialize backup service", logger.ErrorField(err))
	} else {
		log.Info("Backup service initialized")
	}
	defer backupService.Stop()

	handler := api.NewHandler(repo, authService, bootstrapService, cfg, imageWorker, vmService, backupService)

	// Start VM state reconciliation loop
	handler.StartReconciliationLoop(context.Background())
	log.Info("VM state reconciliation loop started")

	// Start heartbeat service for node health monitoring
	healthService := health.NewService(repo)
	healthService.StartHeartbeatService(30 * time.Second)
	log.Info("Heartbeat service started")
	defer healthService.Stop()

	// Initialize metrics
	metrics.Init()
	log.Info("Metrics initialized")

	// Start metrics collector
	collector := metrics.NewCollector(repo)
	if agentClient != nil {
		collector.SetAgentClient(agentClient)
		log.Info("Metrics collector started with agent")
	} else {
		log.Info("Metrics collector started (no agent)")
	}
	go collector.Start()
	defer collector.Stop()

	// Handle graceful shutdown
	stop := make(chan os.Signal, 1)
	signal.Notify(stop, os.Interrupt, syscall.SIGTERM)

	go func() {
		log.Info("CHV controller listening", logger.F("addr", cfg.HTTPAddr))
		if err := http.ListenAndServe(cfg.HTTPAddr, handler.Router()); err != nil {
			log.Fatal("Server failed", logger.ErrorField(err))
		}
	}()

	<-stop
	log.Info("Shutting down gracefully...")

	// Stop the reconciliation loop
	handler.StopReconciliationLoop()
}

func getenv(key, fallback string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return fallback
}
