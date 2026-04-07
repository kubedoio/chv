package middleware

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"sync"
	"time"

	"github.com/google/uuid"
	"golang.org/x/time/rate"
)

// contextKey is the type for context keys
type contextKey string

// UserIDContextKey is the key for user ID in context
const UserIDContextKey contextKey = "user_id"

// UserRateLimiter provides per-user rate limiting for authenticated requests
type UserRateLimiter struct {
	users map[uuid.UUID]*visitor
	mu    sync.RWMutex

	rps               rate.Limit
	burst             int
	cleanupInterval   time.Duration
	fallbackToIPLimit bool
	ipLimiter         *RateLimiter
}

// NewUserRateLimiter creates a new per-user rate limiter
func NewUserRateLimiter(rpm float64, burst int) *UserRateLimiter {
	url := &UserRateLimiter{
		users:             make(map[uuid.UUID]*visitor),
		rps:               rate.Limit(rpm / 60),
		burst:             burst,
		cleanupInterval:   DefaultCleanupInterval,
		fallbackToIPLimit: true,
	}

	// Start cleanup goroutine
	go url.cleanupOldUsers()

	return url
}

// SetFallbackIPLimiter sets an IP-based limiter for unauthenticated requests
func (url *UserRateLimiter) SetFallbackIPLimiter(limiter *RateLimiter) {
	url.ipLimiter = limiter
}

// getLimiter returns the rate limiter for the given user ID
func (url *UserRateLimiter) getLimiter(userID uuid.UUID) *rate.Limiter {
	url.mu.Lock()
	defer url.mu.Unlock()

	v, exists := url.users[userID]
	if !exists {
		limiter := rate.NewLimiter(url.rps, url.burst)
		url.users[userID] = &visitor{
			limiter:  limiter,
			lastSeen: time.Now(),
		}
		return limiter
	}

	v.lastSeen = time.Now()
	return v.limiter
}

// Middleware returns an HTTP middleware that applies per-user rate limiting
func (url *UserRateLimiter) Middleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Get user from context (set by auth middleware)
		userID, ok := GetUserIDFromContext(r.Context())
		if !ok {
			// Fall back to IP-based limiting for unauthenticated
			if url.fallbackToIPLimit && url.ipLimiter != nil {
				url.ipLimiter.Middleware(next).ServeHTTP(w, r)
				return
			}

			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusUnauthorized)
			json.NewEncoder(w).Encode(map[string]interface{}{
				"error": map[string]string{
					"code":    "UNAUTHORIZED",
					"message": "Authentication required",
				},
			})
			return
		}

		limiter := url.getLimiter(userID)

		// Add rate limit headers
		w.Header().Set("X-RateLimit-Limit", fmt.Sprintf("%d", url.burst))
		w.Header().Set("X-RateLimit-Remaining", fmt.Sprintf("%d", int(limiter.Tokens())))

		if !limiter.Allow() {
			w.Header().Set("Content-Type", "application/json")
			w.Header().Set("Retry-After", "60")
			w.Header().Set("X-RateLimit-Reset", fmt.Sprintf("%d", time.Now().Add(time.Minute).Unix()))
			w.WriteHeader(http.StatusTooManyRequests)

			json.NewEncoder(w).Encode(map[string]interface{}{
				"error": map[string]string{
					"code":    "RATE_LIMIT_EXCEEDED",
					"message": "API quota exceeded. Please slow down.",
				},
			})
			return
		}

		next.ServeHTTP(w, r)
	})
}

// cleanupOldUsers periodically removes inactive user limiters
func (url *UserRateLimiter) cleanupOldUsers() {
	ticker := time.NewTicker(url.cleanupInterval)
	defer ticker.Stop()

	for range ticker.C {
		url.mu.Lock()
		for userID, v := range url.users {
			if time.Since(v.lastSeen) > url.cleanupInterval {
				delete(url.users, userID)
			}
		}
		url.mu.Unlock()
	}
}

// WithUserID adds user ID to context
func WithUserID(ctx context.Context, userID uuid.UUID) context.Context {
	return context.WithValue(ctx, UserIDContextKey, userID)
}

// GetUserIDFromContext retrieves user ID from context
func GetUserIDFromContext(ctx context.Context) (uuid.UUID, bool) {
	userID, ok := ctx.Value(UserIDContextKey).(uuid.UUID)
	return userID, ok
}
