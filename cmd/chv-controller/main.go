package main

import (
	"log"
	"net/http"

	"github.com/chv/chv/internal/api"
	"github.com/chv/chv/internal/auth"
	"github.com/chv/chv/internal/bootstrap"
	"github.com/chv/chv/internal/config"
	"github.com/chv/chv/internal/db"
)

func main() {
	cfg := config.LoadController()

	repo, err := db.Open(cfg.DatabasePath)
	if err != nil {
		log.Fatalf("open sqlite repository: %v", err)
	}
	defer repo.Close()

	bootstrapService, err := bootstrap.NewService(bootstrap.Config{
		DataRoot:      cfg.DataRoot,
		DatabasePath:  cfg.DatabasePath,
		BridgeName:    cfg.BridgeName,
		BridgeCIDR:    cfg.BridgeCIDR,
		LocaldiskPath: cfg.LocaldiskPath,
		Repository:    repo,
	})
	if err != nil {
		log.Fatalf("bootstrap service: %v", err)
	}

	handler := api.NewHandler(repo, auth.NewService(repo), bootstrapService)
	log.Printf("CHV controller listening on %s", cfg.HTTPAddr)
	if err := http.ListenAndServe(cfg.HTTPAddr, handler.Router()); err != nil {
		log.Fatal(err)
	}
}
