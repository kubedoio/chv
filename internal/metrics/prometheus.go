package metrics

import "github.com/prometheus/client_golang/prometheus"

var (
	VMCount = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Name: "chv_vms_total",
		Help: "Total number of VMs",
	}, []string{"node_id", "state"})

	VMCPUUsage = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Name: "chv_vm_cpu_usage_percent",
		Help: "VM CPU usage percentage",
	}, []string{"vm_id", "node_id"})

	VMMemoryUsage = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Name: "chv_vm_memory_usage_bytes",
		Help: "VM memory usage",
	}, []string{"vm_id", "node_id"})

	NodeHealth = prometheus.NewGaugeVec(prometheus.GaugeOpts{
		Name: "chv_node_health",
		Help: "Node health (1=online, 0=offline)",
	}, []string{"node_id"})

	APIRequests = prometheus.NewCounterVec(prometheus.CounterOpts{
		Name: "chv_api_requests_total",
		Help: "Total API requests",
	}, []string{"method", "endpoint", "status"})

	APILatency = prometheus.NewHistogramVec(prometheus.HistogramOpts{
		Name:    "chv_api_request_duration_seconds",
		Help:    "API request latency",
		Buckets: []float64{0.001, 0.01, 0.1, 0.5, 1, 2, 5},
	}, []string{"method", "endpoint"})
)

func Init() {
	prometheus.MustRegister(VMCount, VMCPUUsage, VMMemoryUsage,
		NodeHealth, APIRequests, APILatency)
}
