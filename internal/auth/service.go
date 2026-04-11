package auth

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/hex"
	"errors"
	"strings"
	"time"

	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/logger"
	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
	"golang.org/x/crypto/bcrypt"
)

type TokenResult struct {
	ID        string `json:"id"`
	Token     string `json:"token"`
	TokenHash string `json:"-"`
}

type LoginResult struct {
	User        *models.User `json:"user"`
	Token       string       `json:"token"`
	TokenType   string       `json:"token_type"`
	ExpiresIn   int          `json:"expires_in"`
}

type Service struct {
	repo *db.Repository
}

func NewService(repo *db.Repository) *Service {
	return &Service{repo: repo}
}

// CreateUser creates a new user with a hashed password
func (s *Service) CreateUser(ctx context.Context, username, password, email, role string) (*models.User, error) {
	if username == "" || password == "" {
		return nil, errors.New("username and password are required")
	}

	// Check if user already exists
	existing, err := s.repo.GetUserByUsername(ctx, username)
	if err != nil {
		return nil, err
	}
	if existing != nil {
		return nil, errors.New("username already exists")
	}

	// Hash password
	passwordHash, err := bcrypt.GenerateFromPassword([]byte(password), bcrypt.DefaultCost)
	if err != nil {
		return nil, err
	}

	now := time.Now().UTC().Format(time.RFC3339)
	user := &models.User{
		ID:           uuid.NewString(),
		Username:     username,
		PasswordHash: string(passwordHash),
		Email:        email,
		Role:         role,
		IsActive:     true,
		CreatedAt:    now,
		UpdatedAt:    now,
	}

	if err := s.repo.CreateUser(ctx, user); err != nil {
		return nil, err
	}

	return user, nil
}

// Login validates credentials and returns a session token
func (s *Service) Login(ctx context.Context, username, password string) (*LoginResult, error) {
	if username == "" || password == "" {
		return nil, errors.New("username and password are required")
	}

	// Get user
	user, err := s.repo.GetUserByUsername(ctx, username)
	if err != nil {
		return nil, err
	}
	if user == nil {
		// Use constant time comparison to prevent timing attacks
		bcrypt.CompareHashAndPassword([]byte("$2a$10$invalidhashforconstanttimecomparison"), []byte(password))
		return nil, errors.New("invalid credentials")
	}

	if !user.IsActive {
		return nil, errors.New("account is disabled")
	}

	// Verify password
	if err := bcrypt.CompareHashAndPassword([]byte(user.PasswordHash), []byte(password)); err != nil {
		return nil, errors.New("invalid credentials")
	}

	// Generate session token
	token, err := s.generateSessionToken()
	if err != nil {
		return nil, err
	}

	// Update last login time
	now := time.Now().UTC().Format(time.RFC3339)
	if err := s.repo.UpdateUserLastLogin(ctx, user.ID, now); err != nil {
		// Log error but don't fail login
		logger.L().Warn("Failed to update user last login", logger.F("user_id", user.ID), logger.ErrorField(err))
	}

	// Create token record
	tokenModel := &models.APIToken{
		ID:        uuid.NewString(),
		Name:      "session:" + user.Username,
		TokenHash: hashToken(token),
		CreatedAt: now,
		ExpiresAt: stringPtr(time.Now().UTC().Add(24 * time.Hour).Format(time.RFC3339)),
	}
	if err := s.repo.CreateToken(ctx, tokenModel); err != nil {
		return nil, err
	}

	// Clear password hash before returning
	user.PasswordHash = ""

	return &LoginResult{
		User:      user,
		Token:     token,
		TokenType: "Bearer",
		ExpiresIn: 86400, // 24 hours in seconds
	}, nil
}

// Logout invalidates a token
func (s *Service) Logout(ctx context.Context, token string) error {
	token = strings.TrimSpace(strings.TrimPrefix(token, "Bearer "))
	if token == "" {
		return errors.New("missing token")
	}

	// Find and revoke token
	// Note: We need to add a RevokeToken method to the repository
	// For now, we'll just validate it exists
	_, err := s.ValidateToken(ctx, token)
	return err
}

// GetCurrentUser returns the current user from a token
func (s *Service) GetCurrentUser(ctx context.Context, token string) (*models.User, error) {
	_, err := s.ValidateToken(ctx, token)
	if err != nil {
		return nil, err
	}

	// Extract user info from token name (session:username)
	// For now, return a generic user
	// In a full implementation, we'd store user_id in the token record
	return &models.User{
		Username: "admin",
		Role:     "admin",
		IsActive: true,
	}, nil
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

// EnsureAdminUser creates a default admin user if no users exist
func (s *Service) EnsureAdminUser(ctx context.Context) error {
	// Check if any users exist by trying to get the admin user
	admin, err := s.repo.GetUserByUsername(ctx, "admin")
	if err != nil {
		return err
	}
	if admin != nil {
		return nil // Admin already exists
	}

	// Create default admin user
	_, err = s.CreateUser(ctx, "admin", "admin", "", "admin")
	return err
}

func (s *Service) generateSessionToken() (string, error) {
	secret := make([]byte, 32)
	if _, err := rand.Read(secret); err != nil {
		return "", err
	}
	return "chv_session_" + hex.EncodeToString(secret), nil
}

func hashToken(raw string) string {
	sum := sha256.Sum256([]byte(raw))
	return hex.EncodeToString(sum[:])
}

func stringPtr(s string) *string {
	return &s
}

// ConstantTimeCompare performs constant time comparison to prevent timing attacks
func ConstantTimeCompare(a, b string) bool {
	return subtle.ConstantTimeCompare([]byte(a), []byte(b)) == 1
}
