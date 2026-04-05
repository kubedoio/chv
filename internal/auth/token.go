// Package auth provides token-based authentication.
// This file extends the auth service with token management helpers.

package auth

import (
	"context"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
)

// TokenService provides additional token management functionality.
type TokenService struct {
	service *Service
}

// NewTokenService creates a new token service wrapper.
func NewTokenService(service *Service) *TokenService {
	return &TokenService{service: service}
}

// GenerateTokenResult contains the generated token information.
type GenerateTokenResult struct {
	Token     string
	TokenHash string
	ID        string
	ExpiresAt *time.Time
}

// CreateTokenWithExpiry creates a new API token with expiration.
func (s *TokenService) CreateTokenWithExpiry(ctx context.Context, name string, roleID *uuid.UUID, expiresIn time.Duration) (*GenerateTokenResult, error) {
	// Call the existing CreateToken method
	var expiresInPtr *time.Duration
	if expiresIn > 0 {
		expiresInPtr = &expiresIn
	}
	
	roleIDStr := ""
	if roleID != nil {
		roleIDStr = roleID.String()
	}
	
	result, err := s.service.CreateToken(ctx, name, roleIDStr, expiresInPtr)
	if err != nil {
		return nil, err
	}
	
	var expiresAt *time.Time
	if expiresIn > 0 {
		t := time.Now().Add(expiresIn)
		expiresAt = &t
	}
	
	return &GenerateTokenResult{
		Token:     result.Token,
		TokenHash: result.TokenHash,
		ID:        result.ID,
		ExpiresAt: expiresAt,
	}, nil
}

// ValidateAndGetToken validates a token and returns the full token model.
func (s *TokenService) ValidateAndGetToken(ctx context.Context, token string) (*models.APIToken, error) {
	return s.service.ValidateToken(ctx, token)
}
