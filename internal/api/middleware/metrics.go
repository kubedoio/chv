// Package middleware provides HTTP middleware for the API.
package middleware

import (
	"net/http"
	"regexp"
	"strconv"
	"time"

	"github.com/chv/chv/internal/metrics"
)

// responseWriter wraps http.ResponseWriter to capture the status code.
type responseWriter struct {
	http.ResponseWriter
	statusCode int
}

// WriteHeader captures the status code before writing it.
func (rw *responseWriter) WriteHeader(code int) {
	rw.statusCode = code
	rw.ResponseWriter.WriteHeader(code)
}

// Metrics returns an HTTP middleware that records Prometheus metrics for each request.
// It tracks request count by method/endpoint/status and latency by method/endpoint.
func Metrics(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()

		// Wrap response writer to capture status code
		wrapped := &responseWriter{ResponseWriter: w, statusCode: http.StatusOK}

		next.ServeHTTP(wrapped, r)

		duration := time.Since(start).Seconds()

		// Sanitize endpoint for cleaner metrics (remove UUIDs)
		endpoint := sanitizeEndpoint(r.URL.Path)

		// Record metrics
		metrics.APIRequests.WithLabelValues(
			r.Method,
			endpoint,
			strconv.Itoa(wrapped.statusCode),
		).Inc()

		metrics.APILatency.WithLabelValues(
			r.Method,
			endpoint,
		).Observe(duration)
	})
}

// uuidRegex matches UUID patterns in URLs.
var uuidRegex = regexp.MustCompile(`[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}`)

// sanitizeEndpoint removes UUIDs from paths to group similar requests.
// For example: /api/v1/vms/123e4567-e89b-12d3-a456-426614174000 -> /api/v1/vms/:id
func sanitizeEndpoint(path string) string {
	// Replace UUIDs with :id
	path = uuidRegex.ReplaceAllString(path, ":id")

	// Remove trailing slashes
	if len(path) > 1 && path[len(path)-1] == '/' {
		path = path[:len(path)-1]
	}

	return path
}
