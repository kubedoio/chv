package main

import (
	"context"
	"log"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/chv/chv/internal/agent"
	"github.com/chv/chv/internal/config"
)

func main() {
	cfg := config.LoadAgent()
	
	// Default listen address
	addr := os.Getenv("CHV_AGENT_ADDR")
	if addr == "" {
		addr = ":9090"
	}

	server := agent.NewServer(addr, cfg)

	// Channel for OS signals
	stop := make(chan os.Signal, 1)
	signal.Notify(stop, os.Interrupt, syscall.SIGTERM)

	// Run server in goroutine
	go func() {
		log.Printf("CHV agent version v0.1.0 starting on %s...", addr)
		log.Printf("Data Root: %s", cfg.DataRoot)
		log.Printf("Bridge: %s", cfg.BridgeName)
		
		if err := server.Start(); err != nil {
			log.Fatalf("Agent server failed: %v", err)
		}
	}()

	// Wait for stop signal
	<-stop

	log.Println("Shutting down agent...")
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	if err := server.Shutdown(ctx); err != nil {
		log.Printf("Graceful shutdown failed: %v", err)
	}

	log.Println("Agent stopped.")
}
