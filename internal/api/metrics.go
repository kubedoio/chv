package api

import (
	"context"
	"encoding/json"
	"net/http"
	"runtime"
	"time"

	"github.com/prometheus/client_golang/prometheus/promhttp"
)

// Metrics represents system metrics.
type Metrics struct {
	Timestamp time.Time `json:"timestamp"`
	
	// VM Stats
	VMStats struct {
		Total     int `json:"total"`
		Running   int `json:"running"`
		Stopped   int `json:"stopped"`
		Error     int `json:"error"`
		Provisioning int `json:"provisioning"`
	} `json:"vms"`
	
	// Node Stats
	NodeStats struct {
		Total     int `json:"total"`
		Online    int `json:"online"`
		Offline   int `json:"offline"`
	} `json:"nodes"`
	
	// Image Stats
	ImageStats struct {
		Total     int `json:"total"`
		Ready     int `json:"ready"`
		Importing int `json:"importing"`
		Failed    int `json:"failed"`
	} `json:"images"`
	
	// System Stats
	System struct {
		GoVersion    string `json:"go_version"`
		Goroutines   int    `json:"goroutines"`
		MemoryMB     uint64 `json:"memory_mb"`
		Uptime       string `json:"uptime"`
	} `json:"system"`
}

// metricsStartTime records when the server started.
var metricsStartTime = time.Now()

func (h *Handler) metrics(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	
	metrics := &Metrics{
		Timestamp: time.Now(),
	}
	
	// Collect VM stats
	vms, err := h.store.ListVMs(ctx)
	if err == nil {
		metrics.VMStats.Total = len(vms)
		for _, vm := range vms {
			switch vm.ActualState {
			case "running":
				metrics.VMStats.Running++
			case "stopped":
				metrics.VMStats.Stopped++
			case "error":
				metrics.VMStats.Error++
			case "provisioning":
				metrics.VMStats.Provisioning++
			}
		}
	}
	
	// Collect node stats
	nodes, err := h.store.ListNodes(ctx)
	if err == nil {
		metrics.NodeStats.Total = len(nodes)
		for _, node := range nodes {
			switch node.Status {
			case "online":
				metrics.NodeStats.Online++
			default:
				metrics.NodeStats.Offline++
			}
		}
	}
	
	// Collect image stats
	images, err := h.store.ListImages(ctx)
	if err == nil {
		metrics.ImageStats.Total = len(images)
		for _, image := range images {
			switch image.Status {
			case "ready":
				metrics.ImageStats.Ready++
			case "importing":
				metrics.ImageStats.Importing++
			case "failed", "error":
				metrics.ImageStats.Failed++
			}
		}
	}
	
	// System stats
	var m runtime.MemStats
	runtime.ReadMemStats(&m)
	
	metrics.System.GoVersion = runtime.Version()
	metrics.System.Goroutines = runtime.NumGoroutine()
	metrics.System.MemoryMB = m.Alloc / 1024 / 1024
	metrics.System.Uptime = time.Since(metricsStartTime).Round(time.Second).String()
	
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(metrics)
}

// healthCheck returns a detailed health status.
func (h *Handler) healthCheck(w http.ResponseWriter, r *http.Request) {
	status := map[string]interface{}{
		"status":    "healthy",
		"timestamp": time.Now().UTC(),
		"checks": map[string]interface{}{
			"database": h.checkDatabase(r.Context()),
			"storage":  "ok",
		},
	}
	
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(status)
}

// checkDatabase checks database connectivity.
func (h *Handler) checkDatabase(ctx context.Context) string {
	// Try to list nodes as a health check
	_, err := h.store.ListNodes(ctx)
	if err != nil {
		return "unhealthy: " + err.Error()
	}
	return "ok"
}

// prometheusMetrics returns Prometheus-formatted metrics.
func (h *Handler) prometheusMetrics(w http.ResponseWriter, r *http.Request) {
	promhttp.Handler().ServeHTTP(w, r)
}
