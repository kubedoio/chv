package main

import (
	"context"
	"database/sql"
	"fmt"
	"log"
	"os"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/store"
	"github.com/google/uuid"
	_ "modernc.org/sqlite"
)

func main() {
	if len(os.Args) < 3 {
		fmt.Println("Usage: update-image <db-path> <image-id>")
		os.Exit(1)
	}

	dbPath := os.Args[1]
	imageID := os.Args[2]

	// Open database
	sqlDB, err := sql.Open("sqlite", dbPath)
	if err != nil {
		log.Fatalf("Failed to open database: %v", err)
	}
	defer sqlDB.Close()

	// Initialize store
	db, err := store.NewSQLiteStore(sqlDB)
	if err != nil {
		log.Fatalf("Failed to initialize store: %v", err)
	}

	// Parse image ID
	id, err := uuid.Parse(imageID)
	if err != nil {
		log.Fatalf("Invalid image ID: %v", err)
	}

	// Get image
	ctx := context.Background()
	image, err := db.GetImage(ctx, id)
	if err != nil {
		log.Fatalf("Failed to get image: %v", err)
	}
	if image == nil {
		log.Fatalf("Image not found: %s", imageID)
	}

	// Update image to ready
	image.Status = models.ImageStatusReady
	image.SizeBytes = 692407808 // Size of ubuntu-22.04.qcow2
	now := time.Now()
	image.ImportedAt = &now

	if err := db.UpdateImage(ctx, image); err != nil {
		log.Fatalf("Failed to update image: %v", err)
	}

	log.Printf("Image %s updated to status: %s, size: %d bytes", imageID, image.Status, image.SizeBytes)
}
