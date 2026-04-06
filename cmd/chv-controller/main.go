package main

import (
	"context"
	"flag"
	"log"
	"net"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/chv/chv/internal/agent"
	"github.com/chv/chv/internal/api"
	"github.com/chv/chv/internal/auth"
	"github.com/chv/chv/internal/config"
	"github.com/chv/chv/internal/reconcile"
	"github.com/chv/chv/internal/scheduler"
	"github.com/chv/chv/internal/store"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/cors"
	"github.com/jackc/pgx/v5/pgxpool"
	"google.golang.org/grpc"
)

func main() {
	configPath := flag.String("config", "", "Path to configuration file (YAML)")
	flag.Parse()

	// Load configuration from file and environment
	cfg, err := config.LoadControllerConfig(*configPath)
	if err != nil {
		log.Fatalf("Failed to load configuration: %v", err)
	}

	log.Printf("Starting CHV Controller")
	log.Printf("HTTP address: %s", cfg.HTTPAddr)
	log.Printf("gRPC address: %s", cfg.GRPCAddr)
	log.Printf("Log level: %s", cfg.LogLevel)

	ctx := context.Background()

	// Connect to database
	pool, err := pgxpool.New(ctx, cfg.DatabaseURL)
	if err != nil {
		log.Fatalf("Failed to connect to database: %v", err)
	}
	defer pool.Close()

	// Test connection
	if err := pool.Ping(ctx); err != nil {
		log.Fatalf("Failed to ping database: %v", err)
	}
	log.Println("Connected to database")

	// Initialize store
	db := store.NewPostgresStore(pool)

	// Initialize components
	authService := auth.NewService(db)
	schedulerService := scheduler.NewService(db)
	agentClient := agent.NewClient()
	reconciler := reconcile.NewService(db, schedulerService, agentClient)

	// Create HTTP router with CORS
	router := chi.NewRouter()
	
	// Apply CORS middleware from configuration
	corsMiddleware := cors.New(cors.Options{
		AllowedOrigins:   cfg.CORS.AllowedOrigins,
		AllowedMethods:   cfg.CORS.AllowedMethods,
		AllowedHeaders:   cfg.CORS.AllowedHeaders,
		ExposedHeaders:   cfg.CORS.ExposedHeaders,
		AllowCredentials: cfg.CORS.AllowCredentials,
		MaxAge:           cfg.CORS.MaxAge,
	})
	router.Use(corsMiddleware.Handler)
	
	apiHandler := api.NewHandler(db, authService, schedulerService, reconciler)
	apiHandler.RegisterRoutes(router)

	// Create gRPC server
	grpcServer := grpc.NewServer()
	// Register agent service (to be implemented)

	// Start HTTP server
	httpServer := &http.Server{
		Addr:    cfg.HTTPAddr,
		Handler: router,
	}

	go func() {
		log.Printf("Starting HTTP server on %s", cfg.HTTPAddr)
		if err := httpServer.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatalf("HTTP server error: %v", err)
		}
	}()

	// Start gRPC server
	go func() {
		lis, err := net.Listen("tcp", cfg.GRPCAddr)
		if err != nil {
			log.Fatalf("Failed to listen on gRPC addr: %v", err)
		}
		log.Printf("Starting gRPC server on %s", cfg.GRPCAddr)
		if err := grpcServer.Serve(lis); err != nil {
			log.Fatalf("gRPC server error: %v", err)
		}
	}()

	// Start reconciler
	reconciler.Start(ctx)
	defer reconciler.Stop()

	// Wait for shutdown signal
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)
	<-sigCh

	log.Println("Shutting down...")

	// Graceful shutdown
	shutdownCtx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	if err := httpServer.Shutdown(shutdownCtx); err != nil {
		log.Printf("HTTP server shutdown error: %v", err)
	}

	grpcServer.GracefulStop()

	log.Println("Shutdown complete")
}
