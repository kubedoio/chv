package api

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/go-chi/chi/v5"
)

func TestCORS_Middleware_Enabled(t *testing.T) {
	// Create a test handler with CORS enabled
	handler := &Handler{
		corsConfig: CORSConfig{
			Enabled:        true,
			AllowedOrigins: []string{"http://localhost:3000"},
		},
	}

	router := chi.NewRouter()
	handler.RegisterRoutes(router)

	// Test preflight OPTIONS request (doesn't require store)
	req := httptest.NewRequest("OPTIONS", "/api/v1/vms", nil)
	req.Header.Set("Origin", "http://localhost:3000")
	req.Header.Set("Access-Control-Request-Method", "POST")
	rec := httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("Expected status 200 for preflight, got %d", rec.Code)
	}

	// Check CORS headers
	allowOrigin := rec.Header().Get("Access-Control-Allow-Origin")
	if allowOrigin != "http://localhost:3000" {
		t.Errorf("Expected Access-Control-Allow-Origin: http://localhost:3000, got %s", allowOrigin)
	}

	allowCredentials := rec.Header().Get("Access-Control-Allow-Credentials")
	if allowCredentials != "true" {
		t.Errorf("Expected Access-Control-Allow-Credentials: true, got %s", allowCredentials)
	}
}

func TestCORS_Middleware_Disabled(t *testing.T) {
	// Create a test handler with CORS disabled
	handler := &Handler{
		corsConfig: CORSConfig{
			Enabled:        false,
			AllowedOrigins: []string{},
		},
	}

	router := chi.NewRouter()
	handler.RegisterRoutes(router)

	// Test preflight request - when CORS is disabled, OPTIONS returns 405 Method Not Allowed
	req := httptest.NewRequest("OPTIONS", "/api/v1/vms", nil)
	req.Header.Set("Origin", "http://localhost:3000")
	req.Header.Set("Access-Control-Request-Method", "POST")
	rec := httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	// CORS headers should not be present when disabled
	allowOrigin := rec.Header().Get("Access-Control-Allow-Origin")
	if allowOrigin != "" {
		t.Errorf("Expected no Access-Control-Allow-Origin header when CORS is disabled, got %s", allowOrigin)
	}
}

func TestCORS_PreflightRequest_AllowedMethods(t *testing.T) {
	// Create a test handler with CORS enabled
	handler := &Handler{
		corsConfig: CORSConfig{
			Enabled:        true,
			AllowedOrigins: []string{"http://localhost:3000"},
		},
	}

	router := chi.NewRouter()
	handler.RegisterRoutes(router)

	// Test preflight OPTIONS request
	req := httptest.NewRequest("OPTIONS", "/api/v1/vms", nil)
	req.Header.Set("Origin", "http://localhost:3000")
	req.Header.Set("Access-Control-Request-Method", "POST")
	req.Header.Set("Access-Control-Request-Headers", "Authorization, Content-Type")
	rec := httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("Expected status 200 for preflight, got %d", rec.Code)
	}

	// Check CORS headers
	allowOrigin := rec.Header().Get("Access-Control-Allow-Origin")
	if allowOrigin != "http://localhost:3000" {
		t.Errorf("Expected Access-Control-Allow-Origin: http://localhost:3000, got %s", allowOrigin)
	}

	allowMethods := rec.Header().Get("Access-Control-Allow-Methods")
	if allowMethods == "" {
		t.Error("Expected Access-Control-Allow-Methods header to be set")
	}

	allowHeaders := rec.Header().Get("Access-Control-Allow-Headers")
	if allowHeaders == "" {
		t.Error("Expected Access-Control-Allow-Headers header to be set")
	}

	maxAge := rec.Header().Get("Access-Control-Max-Age")
	if maxAge != "300" {
		t.Errorf("Expected Access-Control-Max-Age: 300, got %s", maxAge)
	}
}

func TestCORS_DefaultOrigins(t *testing.T) {
	handler := &Handler{
		corsConfig: CORSConfig{
			Enabled:        true,
			AllowedOrigins: []string{}, // Empty to test defaults
		},
	}

	origins := handler.getAllowedOrigins()
	if len(origins) != 2 {
		t.Errorf("Expected 2 default origins, got %d", len(origins))
	}

	expected := []string{"http://localhost:3000", "http://localhost:5173"}
	for i, origin := range expected {
		if origins[i] != origin {
			t.Errorf("Expected origin %s, got %s", origin, origins[i])
		}
	}
}

func TestCORS_CustomOrigins(t *testing.T) {
	customOrigins := []string{"https://example.com", "https://app.example.com"}
	handler := &Handler{
		corsConfig: CORSConfig{
			Enabled:        true,
			AllowedOrigins: customOrigins,
		},
	}

	origins := handler.getAllowedOrigins()
	if len(origins) != 2 {
		t.Errorf("Expected 2 custom origins, got %d", len(origins))
	}

	for i, origin := range customOrigins {
		if origins[i] != origin {
			t.Errorf("Expected origin %s, got %s", origin, origins[i])
		}
	}
}

func TestCORS_EnabledWithDefaults(t *testing.T) {
	// Test that when CORS is enabled but no origins specified, defaults are used
	handler := &Handler{
		corsConfig: CORSConfig{
			Enabled:        true,
			AllowedOrigins: nil, // nil should use defaults
		},
	}

	router := chi.NewRouter()
	handler.RegisterRoutes(router)

	// Test preflight with one of the default origins
	req := httptest.NewRequest("OPTIONS", "/api/v1/tokens", nil)
	req.Header.Set("Origin", "http://localhost:5173")
	req.Header.Set("Access-Control-Request-Method", "POST")
	rec := httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("Expected status 200 for preflight, got %d", rec.Code)
	}

	allowOrigin := rec.Header().Get("Access-Control-Allow-Origin")
	if allowOrigin != "http://localhost:5173" {
		t.Errorf("Expected Access-Control-Allow-Origin: http://localhost:5173, got %s", allowOrigin)
	}
}

func TestCORS_DisallowedOrigin(t *testing.T) {
	// Test that disallowed origins don't get CORS headers
	handler := &Handler{
		corsConfig: CORSConfig{
			Enabled:        true,
			AllowedOrigins: []string{"http://localhost:3000"},
		},
	}

	router := chi.NewRouter()
	handler.RegisterRoutes(router)

	// Test preflight with a non-allowed origin
	req := httptest.NewRequest("OPTIONS", "/api/v1/tokens", nil)
	req.Header.Set("Origin", "http://malicious-site.com")
	req.Header.Set("Access-Control-Request-Method", "POST")
	rec := httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	// When origin is not allowed, CORS middleware returns 200 but no Access-Control-Allow-Origin
	allowOrigin := rec.Header().Get("Access-Control-Allow-Origin")
	if allowOrigin != "" {
		t.Errorf("Expected no Access-Control-Allow-Origin for disallowed origin, got %s", allowOrigin)
	}
}
