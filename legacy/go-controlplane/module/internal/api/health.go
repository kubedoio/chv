package api

import (
	"context"
	"net/http"
	"time"
)

// HealthResponse represents the health check response
type HealthResponse struct {
	Status    string            `json:"status"`
	Checks    map[string]string `json:"checks"`
	Timestamp string            `json:"timestamp"`
}

// healthHandler performs comprehensive health checks
func (h *Handler) healthHandler(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	checks := make(map[string]string)
	overallHealthy := true

	// Check database connectivity
	if err := h.checkDatabase(ctx); err != nil {
		checks["database"] = "error"
		overallHealthy = false
	} else {
		checks["database"] = "ok"
	}

	// Check agent connectivity
	if err := h.checkAgent(ctx); err != nil {
		checks["agent"] = "error"
		overallHealthy = false
	} else {
		checks["agent"] = "ok"
	}

	// Determine status and HTTP code
	status := "healthy"
	httpStatus := http.StatusOK
	if !overallHealthy {
		status = "unhealthy"
		httpStatus = http.StatusServiceUnavailable
	}

	response := HealthResponse{
		Status:    status,
		Checks:    checks,
		Timestamp: time.Now().UTC().Format(time.RFC3339),
	}

	h.writeJSON(w, httpStatus, response)
}

// checkDatabase verifies database connectivity by pinging the SQLite database
func (h *Handler) checkDatabase(ctx context.Context) error {
	// The repo holds the *sql.DB which has a PingContext method
	// We access it through the repository - it has a db field
	return h.repo.PingContext(ctx)
}

// checkAgent verifies agent connectivity by making an HTTP GET to agent /health
func (h *Handler) checkAgent(ctx context.Context) error {
	agentURL := h.config.AgentURL
	if agentURL == "" {
		// Agent URL not configured, skip check
		return nil
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, agentURL+"/health", nil)
	if err != nil {
		return err
	}

	client := &http.Client{Timeout: 3 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return err
	}

	return nil
}
