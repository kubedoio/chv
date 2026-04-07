package auth

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"strings"
	"time"

	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
)

type TokenResult struct {
	ID        string `json:"id"`
	Token     string `json:"token"`
	TokenHash string `json:"-"`
}

type Service struct {
	repo *db.Repository
}

func NewService(repo *db.Repository) *Service {
	return &Service{repo: repo}
}

func (s *Service) CreateToken(ctx context.Context, name string) (*TokenResult, error) {
	secret := make([]byte, 24)
	if _, err := rand.Read(secret); err != nil {
		return nil, err
	}

	token := "chv_live_" + hex.EncodeToString(secret)
	tokenModel := &models.APIToken{
		ID:        uuid.NewString(),
		Name:      name,
		TokenHash: hashToken(token),
		CreatedAt: time.Now().UTC().Format(time.RFC3339),
	}
	if err := s.repo.CreateToken(ctx, tokenModel); err != nil {
		return nil, err
	}

	return &TokenResult{
		ID:        tokenModel.ID,
		Token:     token,
		TokenHash: tokenModel.TokenHash,
	}, nil
}

func (s *Service) ValidateToken(ctx context.Context, raw string) (*models.APIToken, error) {
	raw = strings.TrimSpace(strings.TrimPrefix(raw, "Bearer "))
	if raw == "" {
		return nil, errors.New("missing bearer token")
	}

	token, err := s.repo.GetAPITokenByHash(ctx, hashToken(raw))
	if err != nil {
		return nil, err
	}
	if token == nil {
		return nil, errors.New("invalid token")
	}
	if token.RevokedAt != nil {
		return nil, errors.New("revoked token")
	}
	if token.ExpiresAt != nil {
		expiresAt, err := time.Parse(time.RFC3339, *token.ExpiresAt)
		if err == nil && time.Now().UTC().After(expiresAt) {
			return nil, errors.New("expired token")
		}
	}
	return token, nil
}

func hashToken(raw string) string {
	sum := sha256.Sum256([]byte(raw))
	return hex.EncodeToString(sum[:])
}
