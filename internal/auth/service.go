package auth

import (
	"context"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/hex"
	"fmt"
	"log"
	"strings"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/store"
	"github.com/chv/chv/pkg/uuidx"
	"github.com/google/uuid"
)

// Service handles authentication.
type Service struct {
	store store.Store
}

// NewService creates a new auth service.
func NewService(store store.Store) *Service {
	return &Service{store: store}
}

// TokenResult contains the created token info.
type TokenResult struct {
	Token     string
	TokenHash string
	ID        string
}

// CreateToken creates a new API token.
func (s *Service) CreateToken(ctx context.Context, name string, roleID string, expiresIn *time.Duration) (*TokenResult, error) {
	// Generate random token (32 bytes = 64 hex chars)
	rawToken := uuidx.NewString() + uuidx.NewString()
	token := "chv_" + rawToken[:60]
	
	// Hash the token
	hash := hashToken(token)
	
	// Create token record
	var roleUUID *string
	if roleID != "" {
		roleUUID = &roleID
	}
	
	var expiresAt *time.Time
	if expiresIn != nil {
		t := time.Now().Add(*expiresIn)
		expiresAt = &t
	}
	
	var rolePtr interface{}
	if roleUUID != nil {
		id := uuidx.MustParse(*roleUUID)
		rolePtr = &id
	}
	
	tokenModel := &models.APIToken{
		ID:        uuidx.New(),
		Name:      name,
		TokenHash: hash,
		CreatedAt: time.Now(),
	}
	
	if expiresAt != nil {
		tokenModel.ExpiresAt = expiresAt
	}
	
	if rolePtr != nil {
		id := rolePtr.(uuid.UUID)
		tokenModel.RoleID = &id
	}
	
	if err := s.store.CreateAPIToken(ctx, tokenModel); err != nil {
		return nil, fmt.Errorf("failed to create token: %w", err)
	}
	
	return &TokenResult{
		Token:     token,
		TokenHash: hash,
		ID:        tokenModel.ID.String(),
	}, nil
}

// ValidateToken validates a bearer token.
func (s *Service) ValidateToken(ctx context.Context, token string) (*models.APIToken, error) {
	log.Printf("ValidateToken called")
	// Extract token from "Bearer " prefix if present
	token = strings.TrimPrefix(token, "Bearer ")
	token = strings.TrimSpace(token)
	
	if token == "" {
		return nil, fmt.Errorf("empty token")
	}
	
	// Hash the provided token
	hash := hashToken(token)
	
	// Look up token by hash
	log.Printf("Looking up token with hash: %s...", hash[:20])
	tokenModel, err := s.store.GetAPITokenByHash(ctx, hash)
	if err != nil {
		return nil, fmt.Errorf("failed to lookup token: %w", err)
	}
	
	log.Printf("Token lookup result: model=%v, err=%v", tokenModel, err)
	if tokenModel == nil {
		return nil, fmt.Errorf("invalid token")
	}
	
	if !tokenModel.IsValid() {
		return nil, fmt.Errorf("token expired or revoked")
	}
	
	return tokenModel, nil
}

// RevokeToken revokes an API token.
func (s *Service) RevokeToken(ctx context.Context, id string) error {
	uuid := uuidx.MustParse(id)
	return s.store.RevokeAPIToken(ctx, uuid)
}

// hashToken creates a SHA-256 hash of a token.
func hashToken(token string) string {
	hash := sha256.Sum256([]byte(token))
	return hex.EncodeToString(hash[:])
}

// ConstantTimeCompare compares two strings in constant time.
func ConstantTimeCompare(a, b string) bool {
	return subtle.ConstantTimeCompare([]byte(a), []byte(b)) == 1
}
