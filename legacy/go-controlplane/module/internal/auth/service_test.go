package auth

import (
	"context"
	"path/filepath"
	"testing"

	"github.com/chv/chv/internal/db"
)

func TestServiceCreatesAndValidatesOpaqueToken(t *testing.T) {
	repo, err := db.Open(filepath.Join(t.TempDir(), "chv.db"))
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}
	defer repo.Close()

	service := NewService(repo)
	result, err := service.CreateToken(context.Background(), "admin")
	if err != nil {
		t.Fatalf("CreateToken() error = %v", err)
	}

	if result.Token == "" || result.TokenHash == "" {
		t.Fatalf("expected token and hash to be set")
	}

	token, err := service.ValidateToken(context.Background(), result.Token)
	if err != nil {
		t.Fatalf("ValidateToken() error = %v", err)
	}

	if token.Name != "admin" {
		t.Fatalf("expected token name admin, got %q", token.Name)
	}
}
