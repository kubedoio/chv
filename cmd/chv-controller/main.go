package main

import (
	"context"
	"database/sql"
	"flag"
	"fmt"
	"log"
	"net"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"syscall"
	"time"

	"github.com/chv/chv/internal/agent"
	"github.com/chv/chv/internal/api"
	"github.com/chv/chv/internal/api/middleware"
	"github.com/chv/chv/internal/auth"
	"github.com/chv/chv/internal/cert"
	"github.com/chv/chv/internal/config"
	"github.com/chv/chv/internal/reconcile"
	"github.com/chv/chv/internal/scheduler"
	"github.com/chv/chv/internal/store"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/cors"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials"
	_ "modernc.org/sqlite"
)

// initSchema initializes the database schema if tables don't exist
func initSchema(db *sql.DB) error {
	// Check if schema is already initialized
	var count int
	err := db.QueryRow("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='roles'").Scan(&count)
	if err == nil && count > 0 {
		// Schema already exists, skip initialization
		return nil
	}

	// Read schema file - try multiple locations
	schemaPaths := []string{
		"configs/schema_sqlite.sql",
		"/app/configs/schema_sqlite.sql",
		"/srv/data02/projects/chv/configs/schema_sqlite.sql",
	}
	
	var schemaPath string
	for _, path := range schemaPaths {
		if _, err := os.Stat(path); err == nil {
			schemaPath = path
			break
		}
	}
	
	if schemaPath == "" {
		return fmt.Errorf("schema file not found in any of: %v", schemaPaths)
	}

	schema, err := os.ReadFile(schemaPath)
	if err != nil {
		return err
	}

	// Execute schema statements
	_, err = db.Exec(string(schema))
	return err
}

// migrateSchema handles schema migrations for existing databases
func migrateSchema(db *sql.DB) error {
	// Migration 1: Add size_bytes and imported_at to images table
	var hasSizeBytes int
	err := db.QueryRow("SELECT count(*) FROM pragma_table_info('images') WHERE name='size_bytes'").Scan(&hasSizeBytes)
	if err != nil {
		return fmt.Errorf("failed to check size_bytes column: %w", err)
	}
	if hasSizeBytes == 0 {
		log.Println("Migrating: Adding size_bytes column to images table")
		if _, err := db.Exec("ALTER TABLE images ADD COLUMN size_bytes BIGINT"); err != nil {
			return fmt.Errorf("failed to add size_bytes column: %w", err)
		}
	}

	var hasImportedAt int
	err = db.QueryRow("SELECT count(*) FROM pragma_table_info('images') WHERE name='imported_at'").Scan(&hasImportedAt)
	if err != nil {
		return fmt.Errorf("failed to check imported_at column: %w", err)
	}
	if hasImportedAt == 0 {
		log.Println("Migrating: Adding imported_at column to images table")
		if _, err := db.Exec("ALTER TABLE images ADD COLUMN imported_at TEXT"); err != nil {
			return fmt.Errorf("failed to add imported_at column: %w", err)
		}
	}

	// Migration 2: Create snapshots table
	var hasSnapshotsTable int
	err = db.QueryRow("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='snapshots'").Scan(&hasSnapshotsTable)
	if err != nil {
		return fmt.Errorf("failed to check snapshots table: %w", err)
	}
	if hasSnapshotsTable == 0 {
		log.Println("Migrating: Creating snapshots table")
		_, err := db.Exec(`
			CREATE TABLE snapshots (
				id TEXT PRIMARY KEY,
				vm_id TEXT REFERENCES virtual_machines(id) ON DELETE CASCADE,
				volume_id TEXT REFERENCES volumes(id) ON DELETE CASCADE,
				name TEXT NOT NULL,
				description TEXT,
				path TEXT NOT NULL,
				status TEXT NOT NULL DEFAULT 'creating',
				size_bytes BIGINT NOT NULL DEFAULT 0,
				created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
			)
		`)
		if err != nil {
			return fmt.Errorf("failed to create snapshots table: %w", err)
		}
		// Create index
		if _, err := db.Exec("CREATE INDEX idx_snapshots_vm_id ON snapshots(vm_id)"); err != nil {
			return fmt.Errorf("failed to create snapshots index: %w", err)
		}
	}

	// Migration 3: Add created_by column to virtual_machines
	var hasCreatedBy int
	err = db.QueryRow("SELECT count(*) FROM pragma_table_info('virtual_machines') WHERE name='created_by'").Scan(&hasCreatedBy)
	if err != nil {
		return fmt.Errorf("failed to check created_by column: %w", err)
	}
	if hasCreatedBy == 0 {
		log.Println("Migrating: Adding created_by column to virtual_machines table")
		if _, err := db.Exec("ALTER TABLE virtual_machines ADD COLUMN created_by TEXT NOT NULL DEFAULT 'anonymous'"); err != nil {
			return fmt.Errorf("failed to add created_by column: %w", err)
		}
		if _, err := db.Exec("CREATE INDEX idx_vms_created_by ON virtual_machines(created_by)"); err != nil {
			return fmt.Errorf("failed to create created_by index: %w", err)
		}
	}

	// Migration 4: Create resource_quotas table
	var hasResourceQuotasTable int
	err = db.QueryRow("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='resource_quotas'").Scan(&hasResourceQuotasTable)
	if err != nil {
		return fmt.Errorf("failed to check resource_quotas table: %w", err)
	}
	if hasResourceQuotasTable == 0 {
		log.Println("Migrating: Creating resource_quotas table")
		_, err := db.Exec(`
			CREATE TABLE resource_quotas (
				user_id TEXT PRIMARY KEY,
				max_cpus INTEGER NOT NULL DEFAULT 8,
				max_memory_mb INTEGER NOT NULL DEFAULT 16384,
				max_vm_count INTEGER NOT NULL DEFAULT 5,
				max_disk_gb INTEGER NOT NULL DEFAULT 100,
				created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
				updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
			)
		`)
		if err != nil {
			return fmt.Errorf("failed to create resource_quotas table: %w", err)
		}
	}

	// Migration 5: Create resource_usage table
	var hasResourceUsageTable int
	err = db.QueryRow("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='resource_usage'").Scan(&hasResourceUsageTable)
	if err != nil {
		return fmt.Errorf("failed to check resource_usage table: %w", err)
	}
	if hasResourceUsageTable == 0 {
		log.Println("Migrating: Creating resource_usage table")
		_, err := db.Exec(`
			CREATE TABLE resource_usage (
				user_id TEXT PRIMARY KEY,
				cpus_used INTEGER NOT NULL DEFAULT 0,
				memory_mb_used INTEGER NOT NULL DEFAULT 0,
				vm_count INTEGER NOT NULL DEFAULT 0,
				disk_gb_used INTEGER NOT NULL DEFAULT 0,
				updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
			)
		`)
		if err != nil {
			return fmt.Errorf("failed to create resource_usage table: %w", err)
		}
	}

	return nil
}

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

	// Ensure database directory exists
	if err := os.MkdirAll(filepath.Dir(cfg.DatabasePath), 0755); err != nil {
		log.Fatalf("Failed to create database directory: %v", err)
	}

	// Connect to SQLite database
	sqlDB, err := sql.Open("sqlite", cfg.DatabasePath)
	if err != nil {
		log.Fatalf("Failed to open database: %v", err)
	}
	defer sqlDB.Close()

	// Test connection
	if err := sqlDB.Ping(); err != nil {
		log.Fatalf("Failed to ping database: %v", err)
	}
	log.Println("Connected to SQLite database")

	// Configure SQLite connection pool for safe concurrent access
	// SQLite handles one writer at a time, so limit to 1 connection
	sqlDB.SetMaxOpenConns(1)
	sqlDB.SetMaxIdleConns(1)
	sqlDB.SetConnMaxLifetime(time.Hour)

	// Initialize store (this also sets up WAL mode and other pragmas)
	db, err := store.NewSQLiteStore(sqlDB)
	if err != nil {
		log.Fatalf("Failed to initialize store: %v", err)
	}

	// Initialize schema if needed
	if err := initSchema(sqlDB); err != nil {
		log.Fatalf("Failed to initialize database schema: %v", err)
	}

	// Run migrations for existing databases
	if err := migrateSchema(sqlDB); err != nil {
		log.Fatalf("Failed to migrate database schema: %v", err)
	}

	// Initialize components
	authService := auth.NewService(db)
	schedulerService := scheduler.NewService(db)

	// Configure agent client with TLS if enabled
	var agentClient agent.Client
	if cfg.TLS.Enabled {
		tlsConfig, err := cert.ClientTLSConfig(cfg.TLS)
		if err != nil {
			log.Printf("Warning: failed to configure TLS for agent client: %v", err)
			agentClient = agent.NewClient()
		} else {
			agentClient = agent.NewClient(agent.WithTLS(credentials.NewTLS(tlsConfig)))
			log.Println("Agent client TLS enabled")
		}
	} else {
		agentClient = agent.NewClient()
	}

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

	// Add Prometheus metrics middleware
	router.Use(middleware.Metrics)

	// Apply rate limiting middleware if enabled
	if cfg.RateLimit.Enabled {
		log.Println("Rate limiting enabled")
		tieredLimiter := middleware.NewCustomTieredRateLimiter(
			float64(cfg.RateLimit.Endpoints.StrictRPM),
			float64(cfg.RateLimit.Endpoints.StrictBurst),
			float64(cfg.RateLimit.Endpoints.StandardRPM),
			float64(cfg.RateLimit.Endpoints.StandardBurst),
			float64(cfg.RateLimit.Endpoints.RelaxedRPM),
			float64(cfg.RateLimit.Endpoints.RelaxedBurst),
		)
		router.Use(tieredLimiter.Middleware)
	}

	apiHandler := api.NewHandler(db, authService, schedulerService, reconciler, agentClient, cfg)
	apiHandler.RegisterRoutes(router)

	// Create gRPC server
	grpcServer := grpc.NewServer()
	// Register agent service (to be implemented)

	// Configure and start HTTP server
	var httpServer *http.Server
	if cfg.TLS.Enabled && cfg.TLS.Cert != "" && cfg.TLS.Key != "" {
		// HTTPS mode
		tlsConfig, err := cert.HTTPSTLSConfig(cfg.TLS)
		if err != nil {
			log.Fatalf("Failed to configure HTTPS: %v", err)
		}
		httpServer = &http.Server{
			Addr:      cfg.HTTPAddr,
			Handler:   router,
			TLSConfig: tlsConfig,
		}
		go func() {
			log.Printf("Starting HTTPS server on %s", cfg.HTTPAddr)
			if err := httpServer.ListenAndServeTLS(cfg.TLS.Cert, cfg.TLS.Key); err != nil && err != http.ErrServerClosed {
				log.Fatalf("HTTPS server error: %v", err)
			}
		}()
	} else {
		// HTTP mode
		httpServer = &http.Server{
			Addr:    cfg.HTTPAddr,
			Handler: router,
		}
		go func() {
			log.Printf("Starting HTTP server on %s", cfg.HTTPAddr)
			if err := httpServer.ListenAndServe(); err != nil && err != http.ErrServerClosed {
				log.Fatalf("HTTP server error: %v", err)
			}
		}()
	}

	// Configure and start gRPC server
	go func() {
		lis, err := net.Listen("tcp", cfg.GRPCAddr)
		if err != nil {
			log.Fatalf("Failed to listen on gRPC addr: %v", err)
		}

		// Configure gRPC server with TLS if enabled
		var opts []grpc.ServerOption
		if cfg.TLS.Enabled && cfg.TLS.Cert != "" && cfg.TLS.Key != "" {
			tlsConfig, err := cert.ServerTLSConfig(cfg.TLS)
			if err != nil {
				log.Fatalf("Failed to configure gRPC TLS: %v", err)
			}
			opts = append(opts, grpc.Creds(credentials.NewTLS(tlsConfig)))
			log.Printf("Starting gRPC server on %s (TLS enabled)", cfg.GRPCAddr)
		} else {
			log.Printf("Starting gRPC server on %s (insecure)", cfg.GRPCAddr)
		}

		grpcServer = grpc.NewServer(opts...)

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
