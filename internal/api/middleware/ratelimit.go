// Package middleware provides HTTP middleware for the API.
package middleware

import (
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"strings"
	"sync"
	"time"

	"golang.org/x/time/rate"
)

// Default rate limiting constants
const (
	// DefaultRequestsPerMinute is the default rate limit for standard endpoints
	DefaultRequestsPerMinute = 60
	// DefaultBurstSize is the default burst size for standard endpoints
	DefaultBurstSize = 10

	// StrictRequestsPerMinute is the rate limit for expensive operations (VM create, delete)
	StrictRequestsPerMinute = 10
	// StrictBurstSize is the burst size for expensive operations
	StrictBurstSize = 5

	// RelaxedRequestsPerMinute is the rate limit for health checks and light endpoints
	RelaxedRequestsPerMinute = 300
	// RelaxedBurstSize is the burst size for health checks
	RelaxedBurstSize = 50

	// DefaultCleanupInterval is how often to clean up old limiters
	DefaultCleanupInterval = 10 * time.Minute
)

// RateLimiter provides per-IP rate limiting using token bucket algorithm
type RateLimiter struct {
	// Per-IP rate limiters
	visitors map[string]*visitor
	mu       sync.RWMutex

	// Configuration
	requestsPerSecond rate.Limit
	burstSize         int

	// Cleanup
	cleanupInterval time.Duration
}

// visitor tracks a single IP's rate limiter and last seen time
type visitor struct {
	limiter  *rate.Limiter
	lastSeen time.Time
}

// NewRateLimiter creates a new rate limiter with the specified RPS and burst
func NewRateLimiter(rpm float64, burst int) *RateLimiter {
	rl := &RateLimiter{
		visitors:          make(map[string]*visitor),
		requestsPerSecond: rate.Limit(rpm / 60), // Convert RPM to RPS
		burstSize:         burst,
		cleanupInterval:   DefaultCleanupInterval,
	}

	// Start cleanup goroutine
	go rl.cleanupOldVisitors()

	return rl
}

// getLimiter returns the rate limiter for the given IP, creating one if needed
func (rl *RateLimiter) getLimiter(ip string) *rate.Limiter {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	v, exists := rl.visitors[ip]
	if !exists {
		limiter := rate.NewLimiter(rl.requestsPerSecond, rl.burstSize)
		rl.visitors[ip] = &visitor{
			limiter:  limiter,
			lastSeen: time.Now(),
		}
		return limiter
	}

	v.lastSeen = time.Now()
	return v.limiter
}

// Middleware returns an HTTP middleware that applies rate limiting
func (rl *RateLimiter) Middleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ip := GetClientIP(r)
		limiter := rl.getLimiter(ip)

		// Add rate limit headers
		w.Header().Set("X-RateLimit-Limit", fmt.Sprintf("%d", rl.burstSize))
		w.Header().Set("X-RateLimit-Remaining", fmt.Sprintf("%d", int(limiter.Tokens())))

		if !limiter.Allow() {
			w.Header().Set("Content-Type", "application/json")
			w.Header().Set("Retry-After", "60")
			w.Header().Set("X-RateLimit-Reset", fmt.Sprintf("%d", time.Now().Add(time.Minute).Unix()))
			w.WriteHeader(http.StatusTooManyRequests)

			json.NewEncoder(w).Encode(map[string]interface{}{
				"error": map[string]string{
					"code":    "RATE_LIMIT_EXCEEDED",
					"message": "Too many requests. Please slow down.",
				},
			})
			return
		}

		next.ServeHTTP(w, r)
	})
}

// cleanupOldVisitors periodically removes inactive limiters to prevent memory leaks
func (rl *RateLimiter) cleanupOldVisitors() {
	ticker := time.NewTicker(rl.cleanupInterval)
	defer ticker.Stop()

	for range ticker.C {
		rl.mu.Lock()
		for ip, v := range rl.visitors {
			// Remove if visitor hasn't been seen in the last cleanup interval
			if time.Since(v.lastSeen) > rl.cleanupInterval {
				delete(rl.visitors, ip)
			}
		}
		rl.mu.Unlock()
	}
}

// GetClientIP extracts the client IP from the request, checking proxy headers
func GetClientIP(r *http.Request) string {
	// Check X-Forwarded-For header (if behind proxy/load balancer)
	xff := r.Header.Get("X-Forwarded-For")
	if xff != "" {
		ips := strings.Split(xff, ",")
		return strings.TrimSpace(ips[0])
	}

	// Check X-Real-Ip header
	xri := r.Header.Get("X-Real-Ip")
	if xri != "" {
		return xri
	}

	// Fall back to RemoteAddr
	ip, _, err := net.SplitHostPort(r.RemoteAddr)
	if err != nil {
		return r.RemoteAddr
	}
	return ip
}
