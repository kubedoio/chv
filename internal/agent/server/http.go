// Package server provides the gRPC and HTTP server implementation for the CHV Agent.
package server

import (
	"context"
	"crypto/ecdsa"
	"crypto/rsa"
	"crypto/x509"
	"encoding/json"
	"encoding/pem"
	"fmt"
	"log"
	"net/http"
	"path"
	"strings"
	"time"

	"github.com/chv/chv/internal/agent/console"
	"github.com/chv/chv/internal/hypervisor"
	"github.com/chv/chv/internal/validation"
	"github.com/golang-jwt/jwt/v5"
	"github.com/prometheus/client_golang/prometheus/promhttp"
)

// HTTPServer provides HTTP endpoints including WebSocket console access.
type HTTPServer struct {
	server         *http.Server
	consoleManager *console.Manager
	wsServer       *console.WebSocketServer
	launcher       *hypervisor.Launcher
	listenAddr     string
	jwtSecret      string
	jwtPublicKey   interface{}
	tlsEnabled     bool
	tlsCert        string
	tlsKey         string
}

// JWTOption configures JWT validation for the HTTP server.
type JWTOption struct {
	// Secret for HMAC validation (HS256, HS384, HS512)
	Secret string

	// PublicKeyPEM for RSA/ECDSA validation (RS256, RS384, RS512, ES256, ES384, ES512)
	// If provided, this takes precedence over Secret
	PublicKeyPEM string

	// Issuer is the expected token issuer (e.g., "chv-controller")
	Issuer string

	// Audience is the expected audience (e.g., "chv-agent")
	Audience string
}

// CHVClaims defines the expected JWT claims structure.
type CHVClaims struct {
	jwt.RegisteredClaims
	// Additional custom claims can be added here
	Permissions []string `json:"permissions,omitempty"`
}

// NewHTTPServer creates a new HTTP server with default (no-op) authentication.
// Deprecated: Use NewHTTPServerWithJWT for proper JWT authentication.
func NewHTTPServer(listenAddr string, consoleManager *console.Manager) *HTTPServer {
	return NewHTTPServerWithAuth(listenAddr, consoleManager, nil, nil)
}

// NewHTTPServerWithAuth creates a new HTTP server with custom auth function.
// If authFunc is nil, a default implementation that validates non-empty tokens is used.
// Deprecated: Use NewHTTPServerWithJWT for proper JWT authentication.
func NewHTTPServerWithAuth(listenAddr string, consoleManager *console.Manager, authFunc console.AuthFunc, launcher *hypervisor.Launcher) *HTTPServer {
	// Use provided auth function or create default
	if authFunc == nil {
		// Default auth: reject empty tokens
		authFunc = func(token string, vmID string) (userID string, allowed bool, err error) {
			if token == "" {
				return "", false, fmt.Errorf("authentication required")
			}
			if len(token) < 10 || len(token) > 4096 {
				return "", false, fmt.Errorf("invalid token format")
			}
			return "user", true, nil
		}
	}

	wsServer := console.NewWebSocketServer(consoleManager, authFunc)

	mux := http.NewServeMux()

	// Health check endpoint (legacy - redirects to /api/v1/health)
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		http.Redirect(w, r, "/api/v1/health", http.StatusMovedPermanently)
	})

	// API v1 health endpoint
	mux.HandleFunc("/api/v1/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		
		// Get VM count from launcher if available
		vmCount := 0
		if launcher != nil {
			instances := launcher.ListInstances()
			vmCount = len(instances)
		}
		
		health := map[string]interface{}{
			"status":    "healthy",
			"version":   "v0.1.0",
			"vm_count":  vmCount,
			"timestamp": time.Now().UTC(),
		}
		
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(health)
	})

	// Console WebSocket endpoint - path: /vms/{vm-id}/console
	mux.HandleFunc("/vms/", func(w http.ResponseWriter, r *http.Request) {
		// Parse and validate VM ID from path
		vmID, err := extractVMIDFromPath(r.URL.Path)
		if err != nil {
			http.Error(w, fmt.Sprintf("Invalid VM ID: %v", err), http.StatusBadRequest)
			return
		}

		// Add vm_id to query
		q := r.URL.Query()
		q.Set("vm_id", vmID)
		r.URL.RawQuery = q.Encode()

		wsServer.ServeHTTP(w, r)
	})

	// Console info endpoint
	mux.HandleFunc("/consoles", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		info := wsServer.GetConsoleInfo()
		w.Header().Set("Content-Type", "application/json")
		console.JSONResponse(w, http.StatusOK, info)
	})

	server := &http.Server{
		Addr:         listenAddr,
		Handler:      mux,
		ReadTimeout:  30 * time.Second,
		WriteTimeout: 30 * time.Second,
		IdleTimeout:  120 * time.Second,
	}

	return &HTTPServer{
		server:         server,
		consoleManager: consoleManager,
		wsServer:       wsServer,
		launcher:       launcher,
		listenAddr:     listenAddr,
	}
}

// writeHealthResponse writes the health check response.
func writeHealthResponse(w http.ResponseWriter, launcher *hypervisor.Launcher) {
	w.Header().Set("Content-Type", "application/json")

	// Get VM count from launcher if available
	vmCount := 0
	if launcher != nil {
		instances := launcher.ListInstances()
		vmCount = len(instances)
	}

	health := map[string]interface{}{
		"status":    "healthy",
		"version":   "v0.1.0",
		"vm_count":  vmCount,
		"timestamp": time.Now().UTC(),
	}

	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(health)
}

// HTTPServerOption is a functional option for configuring the HTTP server.
type HTTPServerOption func(*HTTPServer)

// WithTLS enables TLS for the HTTP server.
func WithTLS(certPath, keyPath string) HTTPServerOption {
	return func(h *HTTPServer) {
		h.tlsEnabled = true
		h.tlsCert = certPath
		h.tlsKey = keyPath
	}
}

// NewHTTPServerWithJWT creates a new HTTP server with JWT authentication.
// The jwtOption parameter configures how JWT tokens are validated.
func NewHTTPServerWithJWT(listenAddr string, consoleManager *console.Manager, jwtOption *JWTOption, launcher *hypervisor.Launcher, opts ...HTTPServerOption) (*HTTPServer, error) {
	h := &HTTPServer{
		listenAddr:     listenAddr,
		consoleManager: consoleManager,
		launcher:       launcher,
	}

	// Apply functional options
	for _, opt := range opts {
		opt(h)
	}

	// Parse and store JWT validation key
	if jwtOption != nil {
		if jwtOption.PublicKeyPEM != "" {
			publicKey, err := parsePublicKey(jwtOption.PublicKeyPEM)
			if err != nil {
				return nil, fmt.Errorf("failed to parse JWT public key: %w", err)
			}
			h.jwtPublicKey = publicKey
		} else if jwtOption.Secret != "" {
			h.jwtSecret = jwtOption.Secret
		}
	}

	// Create auth function that validates JWT tokens
	authFunc := h.createJWTAuthFunc(jwtOption)

	wsServer := console.NewWebSocketServer(consoleManager, authFunc)
	h.wsServer = wsServer

	mux := http.NewServeMux()

	// Health check endpoint (legacy - redirects to /api/v1/health)
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		http.Redirect(w, r, "/api/v1/health", http.StatusMovedPermanently)
	})

	// API v1 health endpoint (no auth required)
	mux.HandleFunc("/api/v1/health", func(w http.ResponseWriter, r *http.Request) {
		writeHealthResponse(w, h.launcher)
	})

	// Prometheus metrics endpoint (no auth required)
	mux.HandleFunc("/metrics", func(w http.ResponseWriter, r *http.Request) {
		promhttp.Handler().ServeHTTP(w, r)
	})

	// Console WebSocket endpoint - path: /vms/{vm-id}/console
	mux.HandleFunc("/vms/", func(w http.ResponseWriter, r *http.Request) {
		// Parse and validate VM ID from path
		vmID, err := extractVMIDFromPath(r.URL.Path)
		if err != nil {
			http.Error(w, fmt.Sprintf("Invalid VM ID: %v", err), http.StatusBadRequest)
			return
		}

		// Add vm_id to query
		q := r.URL.Query()
		q.Set("vm_id", vmID)
		r.URL.RawQuery = q.Encode()

		wsServer.ServeHTTP(w, r)
	})

	// Console info endpoint (requires auth)
	mux.HandleFunc("/consoles", h.requireAuth(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		info := wsServer.GetConsoleInfo()
		w.Header().Set("Content-Type", "application/json")
		console.JSONResponse(w, http.StatusOK, info)
	}, jwtOption))

	h.server = &http.Server{
		Addr:         listenAddr,
		Handler:      mux,
		ReadTimeout:  30 * time.Second,
		WriteTimeout: 30 * time.Second,
		IdleTimeout:  120 * time.Second,
	}

	return h, nil
}

// createJWTAuthFunc creates an AuthFunc that validates JWT tokens.
func (h *HTTPServer) createJWTAuthFunc(jwtOption *JWTOption) console.AuthFunc {
	return func(tokenString string, vmID string) (userID string, allowed bool, err error) {
		if tokenString == "" {
			return "", false, fmt.Errorf("authentication required")
		}

		// Parse and validate the token
		claims := &CHVClaims{}
		token, err := jwt.ParseWithClaims(tokenString, claims, func(token *jwt.Token) (interface{}, error) {
			// Validate signing method
			switch token.Method.(type) {
			case *jwt.SigningMethodHMAC:
				if h.jwtSecret == "" {
					return nil, fmt.Errorf("HMAC signing method not configured")
				}
				return []byte(h.jwtSecret), nil
			case *jwt.SigningMethodRSA, *jwt.SigningMethodRSAPSS:
				if h.jwtPublicKey == nil {
					return nil, fmt.Errorf("RSA signing method not configured")
				}
				return h.jwtPublicKey, nil
			case *jwt.SigningMethodECDSA:
				if h.jwtPublicKey == nil {
					return nil, fmt.Errorf("ECDSA signing method not configured")
				}
				return h.jwtPublicKey, nil
			default:
				return nil, fmt.Errorf("unsupported signing method: %v", token.Header["alg"])
			}
		})

		if err != nil {
			return "", false, fmt.Errorf("invalid token: %w", err)
		}

		if !token.Valid {
			return "", false, fmt.Errorf("token is invalid")
		}

		// Validate issuer if configured
		if jwtOption != nil && jwtOption.Issuer != "" {
			if claims.Issuer != jwtOption.Issuer {
				return "", false, fmt.Errorf("invalid issuer: %s", claims.Issuer)
			}
		}

		// Validate audience if configured
		if jwtOption != nil && jwtOption.Audience != "" {
			matched := false
			for _, aud := range claims.Audience {
				if aud == jwtOption.Audience {
					matched = true
					break
				}
			}
			if !matched {
				return "", false, fmt.Errorf("invalid audience")
			}
		}

		// Get subject (user ID)
		userID = claims.Subject
		if userID == "" {
			return "", false, fmt.Errorf("token missing subject claim")
		}

		return userID, true, nil
	}
}

// requireAuth wraps an HTTP handler with JWT authentication.
func (h *HTTPServer) requireAuth(next http.HandlerFunc, jwtOption *JWTOption) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		// Extract token from Authorization header
		authHeader := r.Header.Get("Authorization")
		if authHeader == "" {
			w.Header().Set("WWW-Authenticate", `Bearer realm="chv-agent"`)
			http.Error(w, `{"error":"unauthorized","message":"Missing authorization header"}`, http.StatusUnauthorized)
			return
		}

		// Parse Bearer token
		parts := strings.SplitN(authHeader, " ", 2)
		if len(parts) != 2 || strings.ToLower(parts[0]) != "bearer" {
			w.Header().Set("WWW-Authenticate", `Bearer realm="chv-agent", error="invalid_token"`)
			http.Error(w, `{"error":"unauthorized","message":"Invalid authorization header format"}`, http.StatusUnauthorized)
			return
		}

		tokenString := parts[1]

		// Validate token
		authFunc := h.createJWTAuthFunc(jwtOption)
		userID, allowed, err := authFunc(tokenString, "")
		if err != nil || !allowed {
			w.Header().Set("WWW-Authenticate", `Bearer realm="chv-agent", error="invalid_token"`)
			if err != nil {
				http.Error(w, fmt.Sprintf(`{"error":"unauthorized","message":"%s"}`, err.Error()), http.StatusUnauthorized)
			} else {
				http.Error(w, `{"error":"forbidden","message":"Access denied"}`, http.StatusForbidden)
			}
			return
		}

		// Set user context
		ctx := context.WithValue(r.Context(), "userID", userID)
		next(w, r.WithContext(ctx))
	}
}

// parsePublicKey parses a PEM-encoded public key (RSA or ECDSA).
func parsePublicKey(pemString string) (interface{}, error) {
	block, _ := pem.Decode([]byte(pemString))
	if block == nil {
		return nil, fmt.Errorf("failed to parse PEM block")
	}

	switch block.Type {
	case "RSA PUBLIC KEY":
		// PKCS1 format
		key, err := x509.ParsePKCS1PublicKey(block.Bytes)
		if err != nil {
			return nil, fmt.Errorf("failed to parse RSA PKCS1 public key: %w", err)
		}
		return key, nil
	case "PUBLIC KEY":
		// PKIX format - try to parse and determine key type
		key, err := x509.ParsePKIXPublicKey(block.Bytes)
		if err != nil {
			return nil, fmt.Errorf("failed to parse PKIX public key: %w", err)
		}
		switch k := key.(type) {
		case *rsa.PublicKey:
			return k, nil
		case *ecdsa.PublicKey:
			return k, nil
		default:
			return nil, fmt.Errorf("unsupported key type in PKIX public key")
		}
	case "ECDSA PUBLIC KEY":
		key, err := x509.ParsePKIXPublicKey(block.Bytes)
		if err != nil {
			return nil, fmt.Errorf("failed to parse ECDSA public key: %w", err)
		}
		if ecKey, ok := key.(*ecdsa.PublicKey); ok {
			return ecKey, nil
		}
		return nil, fmt.Errorf("not an ECDSA public key")
	default:
		// Try generic parsing for unknown types
		key, err := x509.ParsePKIXPublicKey(block.Bytes)
		if err != nil {
			return nil, fmt.Errorf("unsupported key type: %s", block.Type)
		}
		return key, nil
	}
}

// Start starts the HTTP server in a goroutine.
func (s *HTTPServer) Start() error {
	go func() {
		log.Printf("Starting CHV Agent HTTP server on %s (TLS: %v)", s.listenAddr, s.tlsEnabled)
		var err error
		if s.tlsEnabled {
			err = s.server.ListenAndServeTLS(s.tlsCert, s.tlsKey)
		} else {
			err = s.server.ListenAndServe()
		}
		if err != nil && err != http.ErrServerClosed {
			log.Fatalf("HTTP server error: %v", err)
		}
	}()
	return nil
}

// Stop gracefully stops the HTTP server.
func (s *HTTPServer) Stop(ctx context.Context) error {
	if s.wsServer != nil {
		s.wsServer.Close()
	}
	return s.server.Shutdown(ctx)
}

// extractVMIDFromPath extracts and validates the VM ID from URL path.
// Expected format: /vms/{vm-id}/console or /vms/{vm-id}/
func extractVMIDFromPath(urlPath string) (string, error) {
	// Clean the path to remove any . or .. components
	cleanPath := path.Clean(urlPath)

	// Path must start with /vms/
	const prefix = "/vms/"
	if !strings.HasPrefix(cleanPath, prefix) {
		return "", fmt.Errorf("path must start with %s", prefix)
	}

	// Extract the part after /vms/
	afterPrefix := cleanPath[len(prefix):]

	// Remove trailing /console if present
	const consoleSuffix = "/console"
	if strings.HasSuffix(afterPrefix, consoleSuffix) {
		afterPrefix = afterPrefix[:len(afterPrefix)-len(consoleSuffix)]
	}

	// Remove any trailing slashes
	afterPrefix = strings.TrimSuffix(afterPrefix, "/")

	// Validate the VM ID
	if err := validation.ValidateID(afterPrefix); err != nil {
		return "", err
	}

	return afterPrefix, nil
}

// GetWebSocketServer returns the WebSocket server for testing.
func (s *HTTPServer) GetWebSocketServer() *console.WebSocketServer {
	return s.wsServer
}

// GetUserID extracts the user ID from the request context.
func GetUserID(ctx context.Context) string {
	if userID, ok := ctx.Value("userID").(string); ok {
		return userID
	}
	return ""
}
