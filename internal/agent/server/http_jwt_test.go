package server

import (
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"encoding/base64"
	"encoding/pem"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/chv/chv/internal/agent/console"
	"github.com/golang-jwt/jwt/v5"
)

// generateTestHMACKey generates a random HMAC key for testing.
func generateTestHMACKey() string {
	b := make([]byte, 32)
	rand.Read(b)
	return base64.StdEncoding.EncodeToString(b)
}

// generateTestRSAKeyPair generates an RSA key pair for testing.
func generateTestRSAKeyPair() (privateKey *rsa.PrivateKey, publicKeyPEM string, err error) {
	privateKey, err = rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		return nil, "", err
	}

	publicKeyBytes, err := x509.MarshalPKIXPublicKey(&privateKey.PublicKey)
	if err != nil {
		return nil, "", err
	}

	publicKeyPEM = string(pem.EncodeToMemory(&pem.Block{
		Type:  "PUBLIC KEY",
		Bytes: publicKeyBytes,
	}))

	return privateKey, publicKeyPEM, nil
}

// generateTestECDSAKeyPair generates an ECDSA key pair for testing.
func generateTestECDSAKeyPair() (privateKey *ecdsa.PrivateKey, publicKeyPEM string, err error) {
	privateKey, err = ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return nil, "", err
	}

	publicKeyBytes, err := x509.MarshalPKIXPublicKey(&privateKey.PublicKey)
	if err != nil {
		return nil, "", err
	}

	publicKeyPEM = string(pem.EncodeToMemory(&pem.Block{
		Type:  "PUBLIC KEY",
		Bytes: publicKeyBytes,
	}))

	return privateKey, publicKeyPEM, nil
}

// createTestToken creates a JWT token with specified claims and signing method.
func createTestToken(claims jwt.MapClaims, key interface{}, method jwt.SigningMethod) (string, error) {
	token := jwt.NewWithClaims(method, claims)
	return token.SignedString(key)
}

func TestNewHTTPServerWithJWT_HMAC(t *testing.T) {
	secret := generateTestHMACKey()

	jwtOption := &JWTOption{
		Secret:   secret,
		Issuer:   "test-issuer",
		Audience: "test-audience",
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, err := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)
	if err != nil {
		t.Fatalf("Failed to create HTTP server: %v", err)
	}

	if server.jwtSecret != secret {
		t.Errorf("Expected JWT secret to be set")
	}

	server.Stop(nil)
}

func TestNewHTTPServerWithJWT_RSA(t *testing.T) {
	_, publicKeyPEM, err := generateTestRSAKeyPair()
	if err != nil {
		t.Fatalf("Failed to generate RSA key pair: %v", err)
	}

	jwtOption := &JWTOption{
		PublicKeyPEM: publicKeyPEM,
		Issuer:       "test-issuer",
		Audience:     "test-audience",
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, err := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)
	if err != nil {
		t.Fatalf("Failed to create HTTP server: %v", err)
	}

	if server.jwtPublicKey == nil {
		t.Errorf("Expected JWT public key to be set")
	}

	server.Stop(nil)
}

func TestJWTAuthFunc_HMAC_ValidToken(t *testing.T) {
	secret := generateTestHMACKey()

	jwtOption := &JWTOption{
		Secret:   secret,
		Issuer:   "test-issuer",
		Audience: "test-audience",
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	// Create a valid token
	claims := jwt.MapClaims{
		"sub": "user123",
		"iss": "test-issuer",
		"aud": "test-audience",
		"exp": time.Now().Add(time.Hour).Unix(),
		"iat": time.Now().Unix(),
	}
	tokenString, err := createTestToken(claims, []byte(secret), jwt.SigningMethodHS256)
	if err != nil {
		t.Fatalf("Failed to create token: %v", err)
	}

	authFunc := server.createJWTAuthFunc(jwtOption)
	userID, allowed, err := authFunc(tokenString, "vm-123")

	if err != nil {
		t.Errorf("Expected no error, got: %v", err)
	}
	if !allowed {
		t.Error("Expected token to be allowed")
	}
	if userID != "user123" {
		t.Errorf("Expected userID to be 'user123', got: %s", userID)
	}

	server.Stop(nil)
}

func TestJWTAuthFunc_HMAC_InvalidSignature(t *testing.T) {
	secret := generateTestHMACKey()
	wrongSecret := generateTestHMACKey()

	jwtOption := &JWTOption{
		Secret: secret,
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	// Create token with wrong secret
	claims := jwt.MapClaims{
		"sub": "user123",
		"exp": time.Now().Add(time.Hour).Unix(),
	}
	tokenString, err := createTestToken(claims, []byte(wrongSecret), jwt.SigningMethodHS256)
	if err != nil {
		t.Fatalf("Failed to create token: %v", err)
	}

	authFunc := server.createJWTAuthFunc(jwtOption)
	_, allowed, err := authFunc(tokenString, "vm-123")

	if err == nil {
		t.Error("Expected error for invalid signature")
	}
	if allowed {
		t.Error("Expected token to be denied")
	}

	server.Stop(nil)
}

func TestJWTAuthFunc_HMAC_ExpiredToken(t *testing.T) {
	secret := generateTestHMACKey()

	jwtOption := &JWTOption{
		Secret: secret,
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	// Create expired token
	claims := jwt.MapClaims{
		"sub": "user123",
		"exp": time.Now().Add(-time.Hour).Unix(),
		"iat": time.Now().Add(-2 * time.Hour).Unix(),
	}
	tokenString, err := createTestToken(claims, []byte(secret), jwt.SigningMethodHS256)
	if err != nil {
		t.Fatalf("Failed to create token: %v", err)
	}

	authFunc := server.createJWTAuthFunc(jwtOption)
	_, allowed, err := authFunc(tokenString, "vm-123")

	if err == nil {
		t.Error("Expected error for expired token")
	}
	if allowed {
		t.Error("Expected token to be denied")
	}

	server.Stop(nil)
}

func TestJWTAuthFunc_HMAC_EmptyToken(t *testing.T) {
	secret := generateTestHMACKey()

	jwtOption := &JWTOption{
		Secret: secret,
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	authFunc := server.createJWTAuthFunc(jwtOption)
	_, allowed, err := authFunc("", "vm-123")

	if err == nil {
		t.Error("Expected error for empty token")
	}
	if allowed {
		t.Error("Expected token to be denied")
	}

	server.Stop(nil)
}

func TestJWTAuthFunc_HMAC_InvalidIssuer(t *testing.T) {
	secret := generateTestHMACKey()

	jwtOption := &JWTOption{
		Secret: secret,
		Issuer: "expected-issuer",
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	// Create token with wrong issuer
	claims := jwt.MapClaims{
		"sub": "user123",
		"iss": "wrong-issuer",
		"exp": time.Now().Add(time.Hour).Unix(),
	}
	tokenString, err := createTestToken(claims, []byte(secret), jwt.SigningMethodHS256)
	if err != nil {
		t.Fatalf("Failed to create token: %v", err)
	}

	authFunc := server.createJWTAuthFunc(jwtOption)
	_, allowed, err := authFunc(tokenString, "vm-123")

	if err == nil {
		t.Error("Expected error for invalid issuer")
	}
	if allowed {
		t.Error("Expected token to be denied")
	}

	server.Stop(nil)
}

func TestJWTAuthFunc_HMAC_InvalidAudience(t *testing.T) {
	secret := generateTestHMACKey()

	jwtOption := &JWTOption{
		Secret:   secret,
		Audience: "expected-audience",
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	// Create token with wrong audience
	claims := jwt.MapClaims{
		"sub": "user123",
		"aud": "wrong-audience",
		"exp": time.Now().Add(time.Hour).Unix(),
	}
	tokenString, err := createTestToken(claims, []byte(secret), jwt.SigningMethodHS256)
	if err != nil {
		t.Fatalf("Failed to create token: %v", err)
	}

	authFunc := server.createJWTAuthFunc(jwtOption)
	_, allowed, err := authFunc(tokenString, "vm-123")

	if err == nil {
		t.Error("Expected error for invalid audience")
	}
	if allowed {
		t.Error("Expected token to be denied")
	}

	server.Stop(nil)
}

func TestJWTAuthFunc_HMAC_MissingSubject(t *testing.T) {
	secret := generateTestHMACKey()

	jwtOption := &JWTOption{
		Secret: secret,
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	// Create token without subject
	claims := jwt.MapClaims{
		"exp": time.Now().Add(time.Hour).Unix(),
	}
	tokenString, err := createTestToken(claims, []byte(secret), jwt.SigningMethodHS256)
	if err != nil {
		t.Fatalf("Failed to create token: %v", err)
	}

	authFunc := server.createJWTAuthFunc(jwtOption)
	_, allowed, err := authFunc(tokenString, "vm-123")

	if err == nil {
		t.Error("Expected error for missing subject")
	}
	if allowed {
		t.Error("Expected token to be denied")
	}

	server.Stop(nil)
}

func TestJWTAuthFunc_RSA_ValidToken(t *testing.T) {
	privateKey, publicKeyPEM, err := generateTestRSAKeyPair()
	if err != nil {
		t.Fatalf("Failed to generate RSA key pair: %v", err)
	}

	jwtOption := &JWTOption{
		PublicKeyPEM: publicKeyPEM,
		Issuer:       "test-issuer",
		Audience:     "test-audience",
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	// Create a valid RSA-signed token
	claims := jwt.MapClaims{
		"sub": "user123",
		"iss": "test-issuer",
		"aud": "test-audience",
		"exp": time.Now().Add(time.Hour).Unix(),
		"iat": time.Now().Unix(),
	}
	tokenString, err := createTestToken(claims, privateKey, jwt.SigningMethodRS256)
	if err != nil {
		t.Fatalf("Failed to create token: %v", err)
	}

	authFunc := server.createJWTAuthFunc(jwtOption)
	userID, allowed, err := authFunc(tokenString, "vm-123")

	if err != nil {
		t.Errorf("Expected no error, got: %v", err)
	}
	if !allowed {
		t.Error("Expected token to be allowed")
	}
	if userID != "user123" {
		t.Errorf("Expected userID to be 'user123', got: %s", userID)
	}

	server.Stop(nil)
}

func TestJWTAuthFunc_RSA_InvalidSignature(t *testing.T) {
	_, publicKeyPEM, err := generateTestRSAKeyPair()
	if err != nil {
		t.Fatalf("Failed to generate RSA key pair: %v", err)
	}

	// Generate different key for signing
	wrongKey, _, err := generateTestRSAKeyPair()
	if err != nil {
		t.Fatalf("Failed to generate wrong RSA key pair: %v", err)
	}

	jwtOption := &JWTOption{
		PublicKeyPEM: publicKeyPEM,
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	// Create token signed with wrong key
	claims := jwt.MapClaims{
		"sub": "user123",
		"exp": time.Now().Add(time.Hour).Unix(),
	}
	tokenString, err := createTestToken(claims, wrongKey, jwt.SigningMethodRS256)
	if err != nil {
		t.Fatalf("Failed to create token: %v", err)
	}

	authFunc := server.createJWTAuthFunc(jwtOption)
	_, allowed, err := authFunc(tokenString, "vm-123")

	if err == nil {
		t.Error("Expected error for invalid signature")
	}
	if allowed {
		t.Error("Expected token to be denied")
	}

	server.Stop(nil)
}

func TestJWTAuthFunc_ECDSA_ValidToken(t *testing.T) {
	privateKey, publicKeyPEM, err := generateTestECDSAKeyPair()
	if err != nil {
		t.Fatalf("Failed to generate ECDSA key pair: %v", err)
	}

	jwtOption := &JWTOption{
		PublicKeyPEM: publicKeyPEM,
		Issuer:       "test-issuer",
		Audience:     "test-audience",
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	// Create a valid ECDSA-signed token
	claims := jwt.MapClaims{
		"sub": "user123",
		"iss": "test-issuer",
		"aud": "test-audience",
		"exp": time.Now().Add(time.Hour).Unix(),
		"iat": time.Now().Unix(),
	}
	tokenString, err := createTestToken(claims, privateKey, jwt.SigningMethodES256)
	if err != nil {
		t.Fatalf("Failed to create token: %v", err)
	}

	authFunc := server.createJWTAuthFunc(jwtOption)
	userID, allowed, err := authFunc(tokenString, "vm-123")

	if err != nil {
		t.Errorf("Expected no error, got: %v", err)
	}
	if !allowed {
		t.Error("Expected token to be allowed")
	}
	if userID != "user123" {
		t.Errorf("Expected userID to be 'user123', got: %s", userID)
	}

	server.Stop(nil)
}

func TestRequireAuth_MissingHeader(t *testing.T) {
	secret := generateTestHMACKey()

	jwtOption := &JWTOption{
		Secret: secret,
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	// Create a test handler
	handler := server.requireAuth(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("success"))
	}, jwtOption)

	// Make request without Authorization header
	req := httptest.NewRequest(http.MethodGet, "/test", nil)
	rr := httptest.NewRecorder()

	handler(rr, req)

	if rr.Code != http.StatusUnauthorized {
		t.Errorf("Expected status 401, got: %d", rr.Code)
	}

	wwwAuth := rr.Header().Get("WWW-Authenticate")
	if !strings.Contains(wwwAuth, "Bearer") {
		t.Errorf("Expected WWW-Authenticate header to contain 'Bearer', got: %s", wwwAuth)
	}

	server.Stop(nil)
}

func TestRequireAuth_InvalidHeaderFormat(t *testing.T) {
	secret := generateTestHMACKey()

	jwtOption := &JWTOption{
		Secret: secret,
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	// Create a test handler
	handler := server.requireAuth(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("success"))
	}, jwtOption)

	// Make request with invalid Authorization header format
	req := httptest.NewRequest(http.MethodGet, "/test", nil)
	req.Header.Set("Authorization", "Basic dXNlcjpwYXNz") // Basic auth instead of Bearer
	rr := httptest.NewRecorder()

	handler(rr, req)

	if rr.Code != http.StatusUnauthorized {
		t.Errorf("Expected status 401, got: %d", rr.Code)
	}

	server.Stop(nil)
}

func TestRequireAuth_ValidToken(t *testing.T) {
	secret := generateTestHMACKey()

	jwtOption := &JWTOption{
		Secret: secret,
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	// Create a test handler that checks user context
	handler := server.requireAuth(func(w http.ResponseWriter, r *http.Request) {
		userID := GetUserID(r.Context())
		if userID == "" {
			http.Error(w, "Missing user context", http.StatusInternalServerError)
			return
		}
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(userID))
	}, jwtOption)

	// Create a valid token
	claims := jwt.MapClaims{
		"sub": "user123",
		"exp": time.Now().Add(time.Hour).Unix(),
	}
	tokenString, _ := createTestToken(claims, []byte(secret), jwt.SigningMethodHS256)

	// Make request with valid token
	req := httptest.NewRequest(http.MethodGet, "/test", nil)
	req.Header.Set("Authorization", "Bearer "+tokenString)
	rr := httptest.NewRecorder()

	handler(rr, req)

	if rr.Code != http.StatusOK {
		t.Errorf("Expected status 200, got: %d - body: %s", rr.Code, rr.Body.String())
	}

	if rr.Body.String() != "user123" {
		t.Errorf("Expected body 'user123', got: %s", rr.Body.String())
	}

	server.Stop(nil)
}

func TestParsePublicKey_InvalidPEM(t *testing.T) {
	_, err := parsePublicKey("not valid pem")
	if err == nil {
		t.Error("Expected error for invalid PEM")
	}
}

func TestParsePublicKey_RSA_PKCS1(t *testing.T) {
	privateKey, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		t.Fatalf("Failed to generate RSA key: %v", err)
	}

	// Create PKCS1 format public key
	publicKeyBytes := x509.MarshalPKCS1PublicKey(&privateKey.PublicKey)
	publicKeyPEM := string(pem.EncodeToMemory(&pem.Block{
		Type:  "RSA PUBLIC KEY",
		Bytes: publicKeyBytes,
	}))

	key, err := parsePublicKey(publicKeyPEM)
	if err != nil {
		t.Errorf("Failed to parse RSA PKCS1 public key: %v", err)
	}

	if _, ok := key.(*rsa.PublicKey); !ok {
		t.Error("Expected RSA public key")
	}
}

func TestParsePublicKey_UnknownType(t *testing.T) {
	// Create a PEM block with unknown type
	pemBlock := pem.EncodeToMemory(&pem.Block{
		Type:  "UNKNOWN KEY",
		Bytes: []byte("dummy data"),
	})

	_, err := parsePublicKey(string(pemBlock))
	if err == nil {
		t.Error("Expected error for unknown key type")
	}
}

func TestNewHTTPServerWithJWT_NoJWTConfig(t *testing.T) {
	// Test that server can be created without JWT config (falls back to allowing all)
	consoleManager := console.NewManager("/tmp/test-logs")
	server, err := NewHTTPServerWithJWT(":0", consoleManager, nil, nil)
	if err != nil {
		t.Fatalf("Failed to create HTTP server without JWT config: %v", err)
	}

	// The auth function should still work but deny all tokens since no validation is configured
	authFunc := server.createJWTAuthFunc(nil)
	_, allowed, err := authFunc("any-token", "vm-123")

	// Without JWT config, any token validation attempt should fail
	// because the signing method won't be configured
	if allowed {
		t.Error("Expected token to be denied when no JWT config is provided")
	}
	if err == nil {
		t.Error("Expected error when validating token without JWT config")
	}

	server.Stop(nil)
}

func TestGetUserID(t *testing.T) {
	// Test extracting user ID from context
	ctx := (&http.Request{}).Context()
	ctx = context.WithValue(ctx, "userID", "test-user")

	userID := GetUserID(ctx)
	if userID != "test-user" {
		t.Errorf("Expected userID 'test-user', got: %s", userID)
	}

	// Test with empty context
	emptyCtx := (&http.Request{}).Context()
	userID = GetUserID(emptyCtx)
	if userID != "" {
		t.Errorf("Expected empty userID, got: %s", userID)
	}
}

// Test for backward compatibility - server without JWT should still work
func TestNewHTTPServer_BackwardCompatibility(t *testing.T) {
	consoleManager := console.NewManager("/tmp/test-logs")
	server := NewHTTPServer(":0", consoleManager)

	if server == nil {
		t.Fatal("Expected server to be created")
	}

	if server.wsServer == nil {
		t.Error("Expected WebSocket server to be initialized")
	}

	server.Stop(nil)
}

// Test token with multiple audiences
func TestJWTAuthFunc_HMAC_MultipleAudiences(t *testing.T) {
	secret := generateTestHMACKey()

	jwtOption := &JWTOption{
		Secret:   secret,
		Audience: "expected-audience",
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	// Create token with multiple audiences including the expected one
	claims := jwt.MapClaims{
		"sub": "user123",
		"aud": []string{"other-audience", "expected-audience"},
		"exp": time.Now().Add(time.Hour).Unix(),
	}
	tokenString, err := createTestToken(claims, []byte(secret), jwt.SigningMethodHS256)
	if err != nil {
		t.Fatalf("Failed to create token: %v", err)
	}

	authFunc := server.createJWTAuthFunc(jwtOption)
	userID, allowed, err := authFunc(tokenString, "vm-123")

	if err != nil {
		t.Errorf("Expected no error, got: %v", err)
	}
	if !allowed {
		t.Error("Expected token to be allowed")
	}
	if userID != "user123" {
		t.Errorf("Expected userID to be 'user123', got: %s", userID)
	}

	server.Stop(nil)
}

// Test unsupported signing method
func TestJWTAuthFunc_UnsupportedSigningMethod(t *testing.T) {
	secret := generateTestHMACKey()

	jwtOption := &JWTOption{
		Secret: secret,
	}

	consoleManager := console.NewManager("/tmp/test-logs")
	server, _ := NewHTTPServerWithJWT(":0", consoleManager, jwtOption, nil)

	// Create a token with an unsupported signing method
	// We'll use None signing method which should be rejected
	token := jwt.New(jwt.SigningMethodNone)
	token.Claims = jwt.MapClaims{
		"sub": "user123",
		"exp": time.Now().Add(time.Hour).Unix(),
	}
	tokenString, err := token.SigningString()
	if err != nil {
		t.Fatalf("Failed to create token string: %v", err)
	}
	// Add empty signature for None method
	tokenString = tokenString + "."

	authFunc := server.createJWTAuthFunc(jwtOption)
	_, allowed, err := authFunc(tokenString, "vm-123")

	if err == nil {
		t.Error("Expected error for unsupported signing method")
	}
	if allowed {
		t.Error("Expected token to be denied")
	}

	server.Stop(nil)
}
