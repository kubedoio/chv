package middleware

import (
	"net/http"
	"strings"
)

// TieredRateLimiter provides different rate limits based on endpoint type
type TieredRateLimiter struct {
	// Strict limits for expensive operations
	strict *RateLimiter // 10 req/min - VM create, delete, image import

	// Standard limits for normal operations
	standard *RateLimiter // 60 req/min - VM list, get, update

	// Relaxed limits for health checks
	relaxed *RateLimiter // 300 req/min - Health, ping
}

// NewTieredRateLimiter creates a tiered rate limiter with default settings
func NewTieredRateLimiter() *TieredRateLimiter {
	return &TieredRateLimiter{
		strict:   NewRateLimiter(StrictRequestsPerMinute, StrictBurstSize),
		standard: NewRateLimiter(DefaultRequestsPerMinute, DefaultBurstSize),
		relaxed:  NewRateLimiter(RelaxedRequestsPerMinute, RelaxedBurstSize),
	}
}

// NewCustomTieredRateLimiter creates a tiered rate limiter with custom settings
func NewCustomTieredRateLimiter(strictRPM, strictBurst, standardRPM, standardBurst, relaxedRPM, relaxedBurst float64) *TieredRateLimiter {
	return &TieredRateLimiter{
		strict:   NewRateLimiter(strictRPM, int(strictBurst)),
		standard: NewRateLimiter(standardRPM, int(standardBurst)),
		relaxed:  NewRateLimiter(relaxedRPM, int(relaxedBurst)),
	}
}

// Middleware returns an HTTP middleware that applies tiered rate limiting
func (tr *TieredRateLimiter) Middleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Select limiter based on endpoint and method
		var limiter *RateLimiter

		switch {
		case tr.isStrictEndpoint(r.URL.Path, r.Method):
			limiter = tr.strict
		case tr.isRelaxedEndpoint(r.URL.Path):
			limiter = tr.relaxed
		default:
			limiter = tr.standard
		}

		limiter.Middleware(next).ServeHTTP(w, r)
	})
}

// isStrictEndpoint returns true for expensive write operations
func (tr *TieredRateLimiter) isStrictEndpoint(path, method string) bool {
	// Strict endpoints are POST/PUT/DELETE operations that are resource-intensive
	if method != http.MethodPost && method != http.MethodPut && method != http.MethodDelete {
		return false
	}

	// VM operations
	if strings.HasPrefix(path, "/api/v1/vms") {
		// Create VM, start, stop, reboot, resize, delete
		return true
	}

	// Image import and delete
	if strings.HasPrefix(path, "/api/v1/images") {
		return true
	}

	// Network create/delete
	if strings.HasPrefix(path, "/api/v1/networks") {
		return true
	}

	// Storage pool create/delete
	if strings.HasPrefix(path, "/api/v1/storage-pools") {
		return true
	}

	// Snapshot operations
	if strings.Contains(path, "/snapshots") {
		return true
	}

	// Volume clone
	if strings.HasPrefix(path, "/api/v1/volumes") && strings.HasSuffix(path, "/clone") {
		return true
	}

	// Node registration and maintenance
	if strings.HasPrefix(path, "/api/v1/nodes") {
		return true
	}

	return false
}

// isRelaxedEndpoint returns true for health checks and light endpoints
func (tr *TieredRateLimiter) isRelaxedEndpoint(path string) bool {
	// Health endpoints
	if strings.HasPrefix(path, "/health") {
		return true
	}

	// API v1 health
	if strings.HasPrefix(path, "/api/v1/health") {
		return true
	}

	// Metrics (if not requiring strict auth)
	if strings.HasPrefix(path, "/metrics") {
		return true
	}

	return false
}
