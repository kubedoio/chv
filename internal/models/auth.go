package models

import (
	"time"

	"github.com/google/uuid"
)

// Role represents an API role.
type Role struct {
	ID        uuid.UUID `json:"id" db:"id"`
	Name      string    `json:"name" db:"name"`
	CreatedAt time.Time `json:"created_at" db:"created_at"`
}

// APIToken represents an opaque machine token.
type APIToken struct {
	ID         uuid.UUID  `json:"id" db:"id"`
	Name       string     `json:"name" db:"name"`
	TokenHash  string     `json:"-" db:"token_hash"`
	RoleID     *uuid.UUID `json:"role_id" db:"role_id"`
	ExpiresAt  *time.Time `json:"expires_at" db:"expires_at"`
	RevokedAt  *time.Time `json:"revoked_at" db:"revoked_at"`
	CreatedAt  time.Time  `json:"created_at" db:"created_at"`
}

// IsValid returns true if the token is valid (not expired or revoked).
func (t *APIToken) IsValid() bool {
	now := time.Now()
	
	if t.RevokedAt != nil && t.RevokedAt.Before(now) {
		return false
	}
	
	if t.ExpiresAt != nil && t.ExpiresAt.Before(now) {
		return false
	}
	
	return true
}
