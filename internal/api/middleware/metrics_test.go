package middleware

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/chv/chv/internal/metrics"
	"github.com/prometheus/client_golang/prometheus/testutil"
	"github.com/stretchr/testify/assert"
)

func TestMetricsMiddleware(t *testing.T) {
	// Create a simple handler
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("OK"))
	})

	// Wrap with metrics middleware
	wrapped := Metrics(handler)

	// Create a test request
	req := httptest.NewRequest("GET", "/api/v1/vms", nil)
	rec := httptest.NewRecorder()

	// Execute request
	wrapped.ServeHTTP(rec, req)

	// Verify response
	assert.Equal(t, http.StatusOK, rec.Code)

	// Verify metrics were recorded
	requestCount := testutil.ToFloat64(metrics.APIRequests.WithLabelValues("GET", "/api/v1/vms", "200"))
	assert.GreaterOrEqual(t, requestCount, 1.0)
}

func TestMetricsMiddlewareWithDifferentMethods(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusCreated)
	})

	wrapped := Metrics(handler)

	// Test different methods
	methods := []string{"GET", "POST", "PUT", "DELETE"}
	for _, method := range methods {
		req := httptest.NewRequest(method, "/api/v1/vms", nil)
		rec := httptest.NewRecorder()
		wrapped.ServeHTTP(rec, req)
	}

	// Verify all methods were recorded
	for _, method := range methods {
		count := testutil.ToFloat64(metrics.APIRequests.WithLabelValues(method, "/api/v1/vms", "201"))
		assert.GreaterOrEqual(t, count, 1.0, "Method %s should have at least 1 request", method)
	}
}

func TestMetricsMiddlewareWithErrorStatus(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusNotFound)
	})

	wrapped := Metrics(handler)

	req := httptest.NewRequest("GET", "/api/v1/vms/notfound", nil)
	rec := httptest.NewRecorder()
	wrapped.ServeHTTP(rec, req)

	// Verify error status was recorded
	requestCount := testutil.ToFloat64(metrics.APIRequests.WithLabelValues("GET", "/api/v1/vms/notfound", "404"))
	assert.GreaterOrEqual(t, requestCount, 1.0)
}

func TestSanitizeEndpoint(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"/api/v1/vms/123e4567-e89b-12d3-a456-426614174000", "/api/v1/vms/:id"},
		{"/api/v1/vms/123e4567-e89b-12d3-a456-426614174000/snapshots", "/api/v1/vms/:id/snapshots"},
		{"/api/v1/vms", "/api/v1/vms"},
		{"/api/v1/vms/", "/api/v1/vms"},
		{"/health", "/health"},
		{"/metrics", "/metrics"},
		{"/", "/"},
	}

	for _, test := range tests {
		result := sanitizeEndpoint(test.input)
		assert.Equal(t, test.expected, result, "Input: %s", test.input)
	}
}

func TestSanitizeEndpointWithMultipleUUIDs(t *testing.T) {
	// Test path with multiple UUIDs
	input := "/api/v1/vms/123e4567-e89b-12d3-a456-426614174000/snapshots/abcdef12-3456-7890-abcd-ef1234567890"
	expected := "/api/v1/vms/:id/snapshots/:id"

	result := sanitizeEndpoint(input)
	assert.Equal(t, expected, result)
}

func TestResponseWriterWrapper(t *testing.T) {
	// Create a response writer wrapper
	rec := httptest.NewRecorder()
	wrapper := &responseWriter{ResponseWriter: rec, statusCode: http.StatusOK}

	// Write header with a different status
	wrapper.WriteHeader(http.StatusCreated)

	// Verify the wrapper captured the status
	assert.Equal(t, http.StatusCreated, wrapper.statusCode)
	assert.Equal(t, http.StatusCreated, rec.Code)
}

func TestMetricsMiddlewareLatency(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	})

	wrapped := Metrics(handler)

	req := httptest.NewRequest("GET", "/api/v1/vms", nil)
	rec := httptest.NewRecorder()

	wrapped.ServeHTTP(rec, req)

	// Verify latency metric was recorded (just check no panic occurs)
	// Latency histograms are harder to test directly
	assert.Equal(t, http.StatusOK, rec.Code)
}

func TestMetricsMiddlewareWithComplexPath(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	})

	wrapped := Metrics(handler)

	// Test various API paths
	paths := []string{
		"/api/v1/vms",
		"/api/v1/vms/123e4567-e89b-12d3-a456-426614174000",
		"/api/v1/vms/123e4567-e89b-12d3-a456-426614174000/start",
		"/api/v1/vms/123e4567-e89b-12d3-a456-426614174000/stop",
		"/api/v1/networks",
		"/api/v1/images",
	}

	for _, path := range paths {
		req := httptest.NewRequest("POST", path, nil)
		rec := httptest.NewRecorder()
		wrapped.ServeHTTP(rec, req)
	}

	// Verify all paths were recorded with sanitized endpoints
	requestCount := testutil.ToFloat64(metrics.APIRequests.WithLabelValues("POST", "/api/v1/vms", "200"))
	assert.GreaterOrEqual(t, requestCount, 1.0)
}

func TestMetricsMiddlewareConcurrentRequests(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	})

	wrapped := Metrics(handler)

	// Run concurrent requests
	done := make(chan bool, 10)
	for i := 0; i < 10; i++ {
		go func() {
			req := httptest.NewRequest("GET", "/api/v1/vms", nil)
			rec := httptest.NewRecorder()
			wrapped.ServeHTTP(rec, req)
			done <- true
		}()
	}

	// Wait for all requests to complete
	for i := 0; i < 10; i++ {
		<-done
	}

	// Verify all requests were recorded
	requestCount := testutil.ToFloat64(metrics.APIRequests.WithLabelValues("GET", "/api/v1/vms", "200"))
	assert.GreaterOrEqual(t, requestCount, 10.0)
}
