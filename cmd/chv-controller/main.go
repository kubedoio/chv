package main

import (
	"context"
	"flag"
	"log"
	"net"
	"net/http"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"github.com/chv/chv/internal/agent"
	"github.com/chv/chv/internal/api"
	"github.com/chv/chv/internal/auth"
	"github.com/chv/chv/internal/reconcile"
	"github.com/chv/chv/internal/scheduler"
	"github.com/chv/chv/internal/store"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/cors"
	"github.com/jackc/pgx/v5/pgxpool"
	"google.golang.org/grpc"
)

// Config holds controller configuration.
type Config struct {
	DatabaseURL string `yaml:"database_url" env:"CHV_DATABASE_URL"`
	HTTPAddr    string `yaml:"http_addr" env:"CHV_HTTP_ADDR" default:":8080"`
	GRPCAddr    string `yaml:"grpc_addr" env:"CHV_GRPC_ADDR" default:":9090"`
	LogLevel    string `yaml:"log_level" env:"CHV_LOG_LEVEL" default:"info"`
}

func main() {
	configPath := flag.String("config", "", "Path to config file")
	flag.Parse()

	// Load config (simplified - in production use proper config loading)
	cfg := &Config{
		DatabaseURL: getEnv("CHV_DATABASE_URL", "postgres://chv:chv@localhost:5432/chv?sslmode=disable"),
		HTTPAddr:    getEnv("CHV_HTTP_ADDR", ":8080"),
		GRPCAddr:    getEnv("CHV_GRPC_ADDR", ":9090"),
		LogLevel:    getEnv("CHV_LOG_LEVEL", "info"),
	}

	if *configPath != "" {
		// Load from file if provided
		log.Printf("Loading config from %s", *configPath)
	}

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

	// Create HTTP router
	router := chi.NewRouter()
	
	// CORS middleware - allow WebUI access from 10.5.199.83 and localhost
	corsMiddleware := cors.New(cors.Options{
		AllowedOrigins:   getAllowedOrigins(),
		AllowedMethods:   []string{"GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"},
		AllowedHeaders:   []string{"Accept", "Authorization", "Content-Type", "X-Requested-With"},
		ExposedHeaders:   []string{"Link"},
		AllowCredentials: true,
		MaxAge:           300, // Maximum value not ignored by any of major browsers
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

func getEnv(key, defaultValue string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return defaultValue
}

// getAllowedOrigins returns the list of allowed CORS origins.
// Defaults to allowing 10.5.199.83 (WebUI) and localhost for development.
func getAllowedOrigins() []string {
	if env := os.Getenv("CHV_CORS_ORIGINS"); env != "" {
		return strings.Split(env, ",")
	}
	// Default origins - WebUI at 10.5.199.83 and localhost dev
	return []string{
		"http://10.5.199.83",
		"http://10.5.199.83:3000",
		"http://localhost",
		"http://localhost:3000",
		"http://127.0.0.1",
		"http://127.0.0.1:3000",
	}
}
