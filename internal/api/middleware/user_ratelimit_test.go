package middleware

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/google/uuid"
)

func TestNewUserRateLimiter(t *testing.T) {
	url := NewUserRateLimiter(120, 20)
	if url == nil {
		t.Fatal("Expected user rate limiter to be created")
	}
	if url.rps != 2.0 { // 120 rpm = 2 rps
		t.Errorf("Expected rps to be 2.0, got %v", url.rps)
	}
	if url.burst != 20 {
		t.Errorf("Expected burst to be 20, got %d", url.burst)
	}
	if !url.fallbackToIPLimit {
		t.Error("Expected fallbackToIPLimit to be true by default")
	}
}

func TestUserRateLimiter_WithUserID(t *testing.T) {
	url := NewUserRateLimiter(120, 20)

	handler := url.Middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("OK"))
	}))

	// Test without user ID (should return 401 without fallback)
	url.fallbackToIPLimit = false
	req1 := httptest.NewRequest(http.MethodGet, "/test", nil)
	rr1 := httptest.NewRecorder()
	handler.ServeHTTP(rr1, req1)

	if rr1.Code != http.StatusUnauthorized {
		t.Errorf("Expected status 401 without user ID, got %d", rr1.Code)
	}

	// Test with user ID in context
	userID := uuid.MustParse("550e8400-e29b-41d4-a716-446655440000")
	ctx := WithUserID(context.Background(), userID)

	req2 := httptest.NewRequest(http.MethodGet, "/test", nil).WithContext(ctx)
	rr2 := httptest.NewRecorder()
	handler.ServeHTTP(rr2, req2)

	if rr2.Code != http.StatusOK {
		t.Errorf("Expected status 200 with user ID, got %d", rr2.Code)
	}

	// Check rate limit headers
	limit := rr2.Header().Get("X-RateLimit-Limit")
	if limit != "20" {
		t.Errorf("Expected X-RateLimit-Limit to be 20, got %s", limit)
	}
}

func TestUserRateLimiter_RateLimitExceeded(t *testing.T) {
	url := NewUserRateLimiter(1, 1) // 1 rpm, burst 1

	handler := url.Middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	userID := uuid.MustParse("550e8400-e29b-41d4-a716-446655440000")

	// First request should succeed
	ctx1 := WithUserID(context.Background(), userID)
	req1 := httptest.NewRequest(http.MethodGet, "/test", nil).WithContext(ctx1)
	rr1 := httptest.NewRecorder()
	handler.ServeHTTP(rr1, req1)

	if rr1.Code != http.StatusOK {
		t.Errorf("Expected first request to succeed, got %d", rr1.Code)
	}

	// Second request should be rate limited
	ctx2 := WithUserID(context.Background(), userID)
	req2 := httptest.NewRequest(http.MethodGet, "/test", nil).WithContext(ctx2)
	rr2 := httptest.NewRecorder()
	handler.ServeHTTP(rr2, req2)

	if rr2.Code != http.StatusTooManyRequests {
		t.Errorf("Expected status 429, got %d", rr2.Code)
	}

	// Check error response
	var resp map[string]interface{}
	if err := json.Unmarshal(rr2.Body.Bytes(), &resp); err != nil {
		t.Fatalf("Failed to parse error response: %v", err)
	}

	errorObj, ok := resp["error"].(map[string]interface{})
	if !ok {
		t.Fatal("Expected error object in response")
	}

	if errorObj["code"] != "RATE_LIMIT_EXCEEDED" {
		t.Errorf("Expected error code RATE_LIMIT_EXCEEDED, got %s", errorObj["code"])
	}
}

func TestUserRateLimiter_DifferentUsers(t *testing.T) {
	url := NewUserRateLimiter(1, 1) // 1 rpm, burst 1

	handler := url.Middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	userID1 := uuid.MustParse("550e8400-e29b-41d4-a716-446655440001")
	userID2 := uuid.MustParse("550e8400-e29b-41d4-a716-446655440002")

	// User 1 makes a request
	ctx1 := WithUserID(context.Background(), userID1)
	req1 := httptest.NewRequest(http.MethodGet, "/test", nil).WithContext(ctx1)
	rr1 := httptest.NewRecorder()
	handler.ServeHTTP(rr1, req1)

	if rr1.Code != http.StatusOK {
		t.Errorf("User 1 first request: Expected 200, got %d", rr1.Code)
	}

	// User 2 makes a request (should succeed - different user)
	ctx2 := WithUserID(context.Background(), userID2)
	req2 := httptest.NewRequest(http.MethodGet, "/test", nil).WithContext(ctx2)
	rr2 := httptest.NewRecorder()
	handler.ServeHTTP(rr2, req2)

	if rr2.Code != http.StatusOK {
		t.Errorf("User 2 first request: Expected 200, got %d", rr2.Code)
	}

	// User 1 second request (should be rate limited)
	ctx3 := WithUserID(context.Background(), userID1)
	req3 := httptest.NewRequest(http.MethodGet, "/test", nil).WithContext(ctx3)
	rr3 := httptest.NewRecorder()
	handler.ServeHTTP(rr3, req3)

	if rr3.Code != http.StatusTooManyRequests {
		t.Errorf("User 1 second request: Expected 429, got %d", rr3.Code)
	}
}

func TestUserRateLimiter_FallbackToIP(t *testing.T) {
	ipLimiter := NewRateLimiter(1, 1)
	url := NewUserRateLimiter(120, 20)
	url.SetFallbackIPLimiter(ipLimiter)

	handler := url.Middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	// Request without user ID should use IP limiter
	req := httptest.NewRequest(http.MethodGet, "/test", nil)
	req.RemoteAddr = "192.168.1.1:12345"
	rr := httptest.NewRecorder()
	handler.ServeHTTP(rr, req)

	if rr.Code != http.StatusOK {
		t.Errorf("Expected status 200 with IP fallback, got %d", rr.Code)
	}
}

func TestWithUserID(t *testing.T) {
	userID := uuid.MustParse("550e8400-e29b-41d4-a716-446655440000")
	ctx := WithUserID(context.Background(), userID)

	retrievedID, ok := GetUserIDFromContext(ctx)
	if !ok {
		t.Error("Expected to retrieve user ID from context")
	}
	if retrievedID != userID {
		t.Errorf("Expected user ID %s, got %s", userID, retrievedID)
	}
}

func TestGetUserIDFromContext_NotSet(t *testing.T) {
	_, ok := GetUserIDFromContext(context.Background())
	if ok {
		t.Error("Expected GetUserIDFromContext to return false when not set")
	}
}

func TestGetUserIDFromContext_WrongType(t *testing.T) {
	// Set a non-uuid value
	ctx := context.WithValue(context.Background(), UserIDContextKey, "not-a-uuid")
	_, ok := GetUserIDFromContext(ctx)
	if ok {
		t.Error("Expected GetUserIDFromContext to return false for wrong type")
	}
}

func TestUserRateLimiter_Headers(t *testing.T) {
	url := NewUserRateLimiter(120, 20)

	handler := url.Middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	userID := uuid.MustParse("550e8400-e29b-41d4-a716-446655440000")
	ctx := WithUserID(context.Background(), userID)

	req := httptest.NewRequest(http.MethodGet, "/test", nil).WithContext(ctx)
	rr := httptest.NewRecorder()
	handler.ServeHTTP(rr, req)

	// Check headers
	if rr.Header().Get("X-RateLimit-Limit") != "20" {
		t.Errorf("Expected X-RateLimit-Limit to be 20, got %s",
			rr.Header().Get("X-RateLimit-Limit"))
	}

	if rr.Header().Get("X-RateLimit-Remaining") == "" {
		t.Error("Expected X-RateLimit-Remaining to be set")
	}
}

func TestUserRateLimiter_RateLimitHeadersOnReject(t *testing.T) {
	url := NewUserRateLimiter(1, 1) // 1 rpm

	handler := url.Middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	userID := uuid.MustParse("550e8400-e29b-41d4-a716-446655440000")
	ctx := WithUserID(context.Background(), userID)

	// First request
	req1 := httptest.NewRequest(http.MethodGet, "/test", nil).WithContext(ctx)
	rr1 := httptest.NewRecorder()
	handler.ServeHTTP(rr1, req1)

	// Second request (rate limited)
	req2 := httptest.NewRequest(http.MethodGet, "/test", nil).WithContext(ctx)
	rr2 := httptest.NewRecorder()
	handler.ServeHTTP(rr2, req2)

	if rr2.Code != http.StatusTooManyRequests {
		t.Fatalf("Expected 429, got %d", rr2.Code)
	}

	// Check rate limit headers on rejected request
	if rr2.Header().Get("X-RateLimit-Limit") != "1" {
		t.Errorf("Expected X-RateLimit-Limit to be 1, got %s",
			rr2.Header().Get("X-RateLimit-Limit"))
	}

	if rr2.Header().Get("Retry-After") != "60" {
		t.Errorf("Expected Retry-After to be 60, got %s",
			rr2.Header().Get("Retry-After"))
	}

	if rr2.Header().Get("X-RateLimit-Reset") == "" {
		t.Error("Expected X-RateLimit-Reset to be set")
	}
}
