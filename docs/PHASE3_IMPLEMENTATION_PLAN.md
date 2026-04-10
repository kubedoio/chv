# CHV Phase 3 Implementation Plan

**Phase:** Production Readiness (Excluding HA & PostgreSQL)  
**Duration:** 3-4 weeks  
**Goal:** Add advanced networking, VM templates, monitoring, and automation features

---

## Overview

Phase 3 focuses on production-ready features that don't require HA or PostgreSQL. These features enhance the platform's capabilities for real-world deployments.

**Excluded from this phase:**
- Controller HA (multiple instances) - Future Phase 4
- PostgreSQL backend - Future Phase 4
- Session sharing with Redis - Future Phase 4

**Included in this phase:**
1. Advanced Networking (VLANs, DHCP, firewall)
2. VM Templates and Cloud-init
3. Monitoring & Metrics (Prometheus)
4. Backup & Disaster Recovery
5. Resource Quotas and Limits

---

## Phase 3A: Advanced Networking (Week 1)

### Goals
- VLAN support for network segmentation
- Built-in DHCP server for VM networks
- Firewall rules for VM security
- Network isolation and micro-segmentation

### Implementation Tasks

#### Backend (Go)

**1. VLAN Support** (`internal/networking/vlan.go`)
```go
type VLANNetwork struct {
    ID          string
    NodeID      string
    Name        string
    VLANID      int      // 1-4094
    ParentBridge string  // chvbr0
    CIDR        string
    GatewayIP   string
}

func (s *Service) CreateVLANNetwork(ctx context.Context, vlan *VLANNetwork) error
func (s *Service) DeleteVLANNetwork(ctx context.Context, id string) error
```

**2. DHCP Server** (`internal/networking/dhcp.go`)
```go
type DHCPServer struct {
    NetworkID   string
    RangeStart  string   // 10.0.0.100
    RangeEnd    string   // 10.0.0.200
    LeaseTime   time.Duration
}

func (s *Service) StartDHCPServer(networkID string) error
func (s *Service) StopDHCPServer(networkID string) error
func (s *Service) GetDHCPLeases(networkID string) ([]DHCPLease, error)
```

**3. Firewall Rules** (`internal/networking/firewall.go`)
```go
type FirewallRule struct {
    ID          string
    VMID        string
    Direction   string   // ingress/egress
    Protocol    string   // tcp/udp/icmp
    PortRange   string   // 80, 443, 22-80
    SourceCIDR  string   // 0.0.0.0/0, 10.0.0.0/24
    Action      string   // allow/deny
    Priority    int      // 100-999
}

func (s *Service) ApplyFirewallRules(vmID string, rules []FirewallRule) error
```

**4. API Endpoints** (`internal/api/networks.go`)
```
POST   /api/v1/networks/{id}/vlans
GET    /api/v1/networks/{id}/vlans
DELETE /api/v1/networks/{id}/vlans/{vlanId}

POST   /api/v1/networks/{id}/dhcp/start
POST   /api/v1/networks/{id}/dhcp/stop
GET    /api/v1/networks/{id}/dhcp/leases

POST   /api/v1/vms/{id}/firewall/rules
GET    /api/v1/vms/{id}/firewall/rules
DELETE /api/v1/vms/{id}/firewall/rules/{ruleId}
```

#### Frontend (Svelte)

**1. Network Detail Page** (`ui/src/routes/networks/[id]/+page.svelte`)
- VLAN configuration tab
- DHCP server settings
- Lease table

**2. Firewall Rule Editor** (`ui/src/lib/components/FirewallRuleEditor.svelte`)
- Rule builder with form
- Visual rule ordering (drag-drop)
- Port/protocol selector
- Source CIDR validation

#### Database Schema
```sql
-- VLAN networks
CREATE TABLE vlan_networks (
    id TEXT PRIMARY KEY,
    network_id TEXT NOT NULL,
    vlan_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    cidr TEXT NOT NULL,
    gateway_ip TEXT NOT NULL,
    FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE CASCADE,
    UNIQUE(network_id, vlan_id)
);

-- DHCP leases
CREATE TABLE dhcp_leases (
    id TEXT PRIMARY KEY,
    network_id TEXT NOT NULL,
    mac_address TEXT NOT NULL,
    ip_address TEXT NOT NULL,
    hostname TEXT,
    lease_start TEXT NOT NULL,
    lease_end TEXT NOT NULL,
    FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE CASCADE,
    UNIQUE(network_id, mac_address),
    UNIQUE(network_id, ip_address)
);

-- Firewall rules
CREATE TABLE firewall_rules (
    id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    direction TEXT NOT NULL,
    protocol TEXT NOT NULL,
    port_range TEXT,
    source_cidr TEXT NOT NULL,
    action TEXT NOT NULL,
    priority INTEGER NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE
);
CREATE INDEX idx_firewall_vm ON firewall_rules(vm_id, priority);
```

---

## Phase 3B: VM Templates & Cloud-init (Week 1-2)

### Goals
- VM templates for rapid provisioning
- Cloud-init template library
- Clone from template
- Template versioning

### Implementation Tasks

#### Backend (Go)

**1. VM Templates** (`internal/vm/templates.go`)
```go
type VMTemplate struct {
    ID          string
    Name        string
    Description string
    VCPU        int
    MemoryMB    int
    ImageID     string
    NetworkID   string
    StoragePoolID string
    CloudInitConfig string  // YAML
    Tags        []string
    CreatedAt   time.Time
}

func (s *Service) CreateTemplate(ctx context.Context, vmID string, template *VMTemplate) error
func (s *Service) CloneFromTemplate(ctx context.Context, templateID string, name string) (*VirtualMachine, error)
```

**2. Cloud-init Templates** (`internal/cloudinit/templates.go`)
```go
// Predefined templates
const (
    CloudInitBasic = `#cloud-config
users:
  - name: {{.Username}}
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - {{.SSHKey}}
`
    CloudInitDocker = `#cloud-config
package_update: true
packages:
  - docker.io
  - docker-compose
users:
  - name: {{.Username}}
    groups: docker
`
    CloudInitKubernetes = `#cloud-config
package_update: true
packages:
  - apt-transport-https
  - ca-certificates
  - curl
runcmd:
  - curl -fsSL https://pkgs.k8s.io/core:/stable:/v1.29/deb/Release.key | sudo gpg --dearmor -o /etc/apt/keyrings/kubernetes-apt-keyring.gpg
  - echo 'deb [signed-by=/etc/apt/keyrings/kubernetes-apt-keyring.gpg] https://pkgs.k8s.io/core:/stable:/v1.29/deb/ /' | sudo tee /etc/apt/sources.list.d/kubernetes.list
  - sudo apt-get update
  - sudo apt-get install -y kubelet kubeadm kubectl
`
)

type CloudInitTemplate struct {
    ID          string
    Name        string
    Description string
    Content     string  // Go template
    Variables   []string // Required variables
}

func RenderCloudInit(templateID string, variables map[string]string) (string, error)
```

**3. API Endpoints** (`internal/api/templates.go`)
```
# VM Templates
GET    /api/v1/vm-templates
POST   /api/v1/vm-templates
GET    /api/v1/vm-templates/{id}
DELETE /api/v1/vm-templates/{id}
POST   /api/v1/vm-templates/{id}/clone

# Cloud-init Templates
GET    /api/v1/cloud-init-templates
POST   /api/v1/vms/{id}/cloud-init/apply
```

#### Frontend (Svelte)

**1. Template Library** (`ui/src/routes/templates/+page.svelte`)
- Grid of template cards
- Quick clone button
- Template details modal

**2. Create from Template** (`ui/src/lib/components/CreateFromTemplate.svelte`)
- Template selector
- Variable input form (dynamic based on template)
- Preview rendered cloud-init

**3. Cloud-init Editor** (`ui/src/lib/components/CloudInitEditor.svelte`)
- YAML editor with syntax highlighting
- Variable placeholder helper
- Preview mode

#### Database Schema
```sql
-- VM templates
CREATE TABLE vm_templates (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    vcpu INTEGER NOT NULL,
    memory_mb INTEGER NOT NULL,
    image_id TEXT NOT NULL,
    network_id TEXT NOT NULL,
    storage_pool_id TEXT NOT NULL,
    cloud_init_config TEXT,
    tags TEXT, -- JSON array
    created_at TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE
);

-- Cloud-init templates
CREATE TABLE cloud_init_templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    content TEXT NOT NULL,
    variables TEXT, -- JSON array
    created_at TEXT NOT NULL
);

-- Insert default templates
INSERT INTO cloud_init_templates (id, name, description, content, variables, created_at) VALUES
('cit-basic', 'Basic User Setup', 'Creates a user with sudo access', '#cloud-config\nusers:\n  - name: {{.Username}}\n    sudo: ALL=(ALL) NOPASSWD:ALL\n    ssh_authorized_keys:\n      - {{.SSHKey}}', '["Username", "SSHKey"]', datetime('now')),
('cit-docker', 'Docker Ready', 'Ubuntu with Docker pre-installed', '#cloud-config\npackage_update: true\npackages:\n  - docker.io\nusers:\n  - name: {{.Username}}\n    groups: docker', '["Username"]', datetime('now'));
```

---

## Phase 3C: Monitoring & Metrics (Week 2-3)

### Goals
- Prometheus metrics export
- VM resource utilization graphs
- Node health dashboards
- Alerting framework

### Implementation Tasks

#### Backend (Go)

**1. Prometheus Metrics** (`internal/metrics/prometheus.go`)
```go
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
        Help: "VM memory usage in bytes",
    }, []string{"vm_id", "node_id"})
    
    NodeHealth = prometheus.NewGaugeVec(prometheus.GaugeOpts{
        Name: "chv_node_health",
        Help: "Node health status (1=online, 0=offline)",
    }, []string{"node_id"})
    
    APIRequests = prometheus.NewCounterVec(prometheus.CounterOpts{
        Name: "chv_api_requests_total",
        Help: "Total API requests",
    }, []string{"method", "endpoint", "status"})
    
    APILatency = prometheus.NewHistogramVec(prometheus.HistogramOpts{
        Name: "chv_api_request_duration_seconds",
        Help: "API request latency",
    }, []string{"method", "endpoint"})
)
```

**2. Metrics Collection** (`internal/metrics/collector.go`)
```go
type Collector struct {
    repo *db.Repository
    interval time.Duration
}

func (c *Collector) Start() {
    // Collect VM metrics every 15 seconds
    // Collect node metrics every 30 seconds
}

func (c *Collector) collectVMMetrics() {
    // Query agent for VM CPU/memory usage
    // Update Prometheus gauges
}
```

**3. API Endpoint** (`internal/api/metrics.go`)
```
GET /api/v1/metrics  # Prometheus scrape endpoint
```

#### Frontend (Svelte)

**1. Metrics Dashboard** (`ui/src/routes/metrics/+page.svelte`)
- Time-series graphs (using Chart.js or lightweight alternative)
- VM resource usage over time
- Node health history
- Top resource consumers

**2. VM Metrics Widget** (`ui/src/lib/components/VMMetricsWidget.svelte`)
- CPU usage sparkline
- Memory usage bar
- Network I/O counters
- Disk I/O counters

**3. Node Metrics** (`ui/src/lib/components/NodeMetricsPanel.svelte`)
- Cluster-wide resource utilization
- Node comparison charts
- Capacity planning indicators

#### Database Schema
```sql
-- Metrics history (optional - for UI graphs)
CREATE TABLE vm_metrics_history (
    id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    cpu_percent REAL,
    memory_used_mb INTEGER,
    memory_total_mb INTEGER,
    disk_read_bytes INTEGER,
    disk_write_bytes INTEGER,
    net_rx_bytes INTEGER,
    net_tx_bytes INTEGER,
    timestamp TEXT NOT NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE
);
CREATE INDEX idx_vm_metrics_time ON vm_metrics_history(vm_id, timestamp);
```

---

## Phase 3D: Backup & Disaster Recovery (Week 3)

### Goals
- VM snapshot scheduling
- Automated backups
- Export/import VMs
- Disaster recovery procedures

### Implementation Tasks

#### Backend (Go)

**1. Backup Service** (`internal/backup/service.go`)
```go
type BackupJob struct {
    ID          string
    VMID        string
    Schedule    string   // cron expression
    Retention   int      // number of backups to keep
    Destination string   // local path or S3 URI
    LastRun     time.Time
    NextRun     time.Time
}

func (s *Service) CreateBackup(vmID string, name string) (*VMSnapshot, error)
func (s *Service) RestoreBackup(vmID string, snapshotID string) error
func (s *Service) ExportVM(vmID string, format string) (string, error)  // Returns download URL
func (s *Service) ImportVM(name string, imagePath string, metadataPath string) (*VirtualMachine, error)
```

**2. Scheduled Backups** (`internal/backup/scheduler.go`)
```go
type Scheduler struct {
    cron *cron.Cron
}

func (s *Scheduler) AddJob(job *BackupJob) error
func (s *Scheduler) RemoveJob(jobID string) error
```

**3. API Endpoints** (`internal/api/backups.go`)
```
# Snapshots (existing - enhanced)
GET    /api/v1/vms/{id}/snapshots
POST   /api/v1/vms/{id}/snapshots
POST   /api/v1/vms/{id}/snapshots/{snapId}/restore
DELETE /api/v1/vms/{id}/snapshots/{snapId}

# Backup Jobs
GET    /api/v1/backup-jobs
POST   /api/v1/backup-jobs
GET    /api/v1/backup-jobs/{id}
DELETE /api/v1/backup-jobs/{id}
POST   /api/v1/backup-jobs/{id}/run

# Export/Import
POST   /api/v1/vms/{id}/export
POST   /api/v1/vms/import
GET    /api/v1/exports/{id}/download
```

#### Frontend (Svelte)

**1. Backup Jobs Page** (`ui/src/routes/backup-jobs/+page.svelte`)
- List of scheduled backups
- Create backup job wizard
- Backup history

**2. VM Snapshots Panel** (`ui/src/lib/components/VMSnapshotsPanel.svelte`)
- Snapshot timeline
- One-click restore
- Create snapshot button

**3. Export/Import** (`ui/src/lib/components/VMExportImport.svelte`)
- Export format selection (QCOW2, OVA, raw)
- Import from URL or upload
- Progress tracking

#### Database Schema
```sql
-- Backup jobs
CREATE TABLE backup_jobs (
    id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    name TEXT NOT NULL,
    schedule TEXT NOT NULL,  -- cron expression
    retention INTEGER DEFAULT 7,
    destination TEXT NOT NULL,
    last_run TEXT,
    next_run TEXT,
    enabled INTEGER DEFAULT 1,
    created_at TEXT NOT NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE
);

-- Backup history
CREATE TABLE backup_history (
    id TEXT PRIMARY KEY,
    job_id TEXT,
    vm_id TEXT NOT NULL,
    snapshot_id TEXT,
    status TEXT NOT NULL,  -- running, completed, failed
    started_at TEXT NOT NULL,
    completed_at TEXT,
    size_bytes INTEGER,
    error TEXT,
    FOREIGN KEY(job_id) REFERENCES backup_jobs(id) ON DELETE SET NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE
);
```

---

## Phase 3E: Resource Quotas (Week 3-4)

### Goals
- Per-user resource limits
- Per-project quotas
- Usage tracking and enforcement
- Quota alerts

### Implementation Tasks

#### Backend (Go)

**1. Quota Service** (`internal/quota/service.go`)
```go
type Quota struct {
    UserID       string
    MaxVMs       int
    MaxCPU       int      // total vCPUs
    MaxMemory    int64    // total MB
    MaxStorage   int64    // total GB
    MaxNetworks  int
}

type Usage struct {
    VMs      int
    CPUs     int
    Memory   int64
    Storage  int64
    Networks int
}

func (s *Service) CheckQuota(userID string, resource string, amount int) error
func (s *Service) GetUsage(userID string) (*Usage, error)
func (s *Service) EnforceQuota(userID string) error
```

**2. Quota Middleware** (`internal/api/quota.go`)
```go
func QuotaMiddleware(resource string) func(http.Handler) http.Handler {
    return func(next http.Handler) http.Handler {
        return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
            // Check quota before allowing creation
        })
    }
}
```

**3. API Endpoints**
```
GET    /api/v1/quotas
POST   /api/v1/quotas
GET    /api/v1/usage
GET    /api/v1/users/{id}/usage
```

#### Frontend (Svelte)

**1. Quota Dashboard** (`ui/src/routes/quotas/+page.svelte`)
- Usage bars vs limits
- Alerts for approaching limits
- Project-level aggregation

**2. Quota Settings** (Admin only)
- Set per-user limits
- Default quotas for new users

#### Database Schema
```sql
-- Quotas
CREATE TABLE quotas (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL UNIQUE,
    max_vms INTEGER DEFAULT 10,
    max_cpu INTEGER DEFAULT 20,
    max_memory_mb INTEGER DEFAULT 65536,  -- 64GB
    max_storage_gb INTEGER DEFAULT 500,
    max_networks INTEGER DEFAULT 5,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Usage tracking (denormalized for performance)
CREATE TABLE usage_cache (
    user_id TEXT PRIMARY KEY,
    vms INTEGER DEFAULT 0,
    cpus INTEGER DEFAULT 0,
    memory_mb INTEGER DEFAULT 0,
    storage_gb INTEGER DEFAULT 0,
    networks INTEGER DEFAULT 0,
    last_updated TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);
```

---

## Implementation Order

### Week 1
- **Days 1-2:** Phase 3A - VLAN and DHCP
- **Days 3-4:** Phase 3A - Firewall rules
- **Day 5:** Testing and bug fixes

### Week 2
- **Days 1-2:** Phase 3B - VM Templates
- **Days 3-4:** Phase 3B - Cloud-init templates
- **Day 5:** Phase 3C - Prometheus metrics

### Week 3
- **Days 1-2:** Phase 3C - Metrics dashboard
- **Days 3-4:** Phase 3D - Backup service
- **Day 5:** Phase 3D - Export/import

### Week 4
- **Days 1-2:** Phase 3E - Resource quotas
- **Days 3-4:** Integration testing
- **Day 5:** Documentation and deployment

---

## API Summary

### New Endpoints

```
# VLANs
POST   /api/v1/networks/{id}/vlans
GET    /api/v1/networks/{id}/vlans
DELETE /api/v1/networks/{id}/vlans/{vlanId}

# DHCP
POST   /api/v1/networks/{id}/dhcp/start
POST   /api/v1/networks/{id}/dhcp/stop
GET    /api/v1/networks/{id}/dhcp/leases

# Firewall
POST   /api/v1/vms/{id}/firewall/rules
GET    /api/v1/vms/{id}/firewall/rules
DELETE /api/v1/vms/{id}/firewall/rules/{ruleId}

# Templates
GET    /api/v1/vm-templates
POST   /api/v1/vm-templates
POST   /api/v1/vm-templates/{id}/clone
GET    /api/v1/cloud-init-templates

# Metrics
GET    /api/v1/metrics

# Backups
GET    /api/v1/backup-jobs
POST   /api/v1/backup-jobs
POST   /api/v1/vms/{id}/export
POST   /api/v1/vms/import

# Quotas
GET    /api/v1/quotas
POST   /api/v1/quotas
GET    /api/v1/usage
```

---

## Success Criteria

### Phase 3A (Networking)
- [ ] VLAN networks can be created and assigned to VMs
- [ ] DHCP server assigns IPs to VMs automatically
- [ ] Firewall rules block/allow traffic as configured
- [ ] Network isolation works between VLANs

### Phase 3B (Templates)
- [ ] VM templates can be created from existing VMs
- [ ] New VMs can be cloned from templates
- [ ] Cloud-init templates render correctly
- [ ] Template library has 3+ predefined templates

### Phase 3C (Metrics)
- [ ] Prometheus endpoint exports metrics
- [ ] VM resource graphs show historical data
- [ ] Node health dashboard displays all nodes
- [ ] API latency histograms available

### Phase 3D (Backup)
- [ ] Scheduled backups run automatically
- [ ] VMs can be exported and imported
- [ ] Snapshots can be restored
- [ ] Backup retention policies enforced

### Phase 3E (Quotas)
- [ ] Users cannot exceed their resource limits
- [ ] Usage dashboard shows current consumption
- [ ] Alerts shown when approaching limits
- [ ] Quotas can be adjusted by admins

---

## Dependencies

### New Go Dependencies
```bash
go get github.com/prometheus/client_golang/prometheus
go get github.com/prometheus/client_golang/prometheus/promhttp
go get github.com/robfig/cron/v3
```

### New Frontend Dependencies
```bash
npm install chart.js  # For metrics graphs
npm install yaml      # For cloud-init validation
```

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| DHCP conflicts | Use dedicated bridge, conflict detection |
| Firewall lockout | Always allow SSH (port 22) by default |
| Backup storage | Monitor disk space, retention policies |
| Metrics overhead | Sample every 15s, 7-day retention |
| Quota enforcement | Check before creation, not after |
