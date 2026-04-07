# CHV Platform Next Steps Implementation Plan

**Version:** v0.1.0-mvp1 → v0.2.0  
**Planning Date:** 2026-04-06  
**Target Completion:** 6 weeks

---

## Overview

This plan addresses the critical gaps identified in the state report. The work is organized into 4 phases:

1. **Phase 1: Critical Fixes** - Security and stability blockers
2. **Phase 2: MVP-1 Completion** - Complete core features
3. **Phase 3: Production Readiness** - Harden for production use
4. **Phase 4: Future Enhancements** - Post-MVP features

---

## Phase 1: Critical Fixes (Week 1)

### 1.1 Fix JWT Authentication in Agent

**Priority:** P0 (Security Blocker)  
**Effort:** 1 day  
**Location:** `internal/agent/server/http.go`

**Current State:**
```go
// TODO: Implement proper JWT validation
// For now, just validate token format (non-empty, reasonable length)
```

**Implementation:**
```go
// Validate JWT token using controller's public key
func (s *Server) validateJWT(tokenString string) (*Claims, error) {
    token, err := jwt.ParseWithClaims(tokenString, &Claims{}, func(token *jwt.Token) {
        return s.controllerPublicKey, nil
    })
    if err != nil {
        return nil, fmt.Errorf("invalid token: %w", err)
    }
    if claims, ok := token.Claims.(*Claims); ok && token.Valid {
        return claims, nil
    }
    return nil, fmt.Errorf("invalid token claims")
}
```

**Acceptance Criteria:**
- [ ] JWT signature is validated against controller's public key
- [ ] Expired tokens are rejected
- [ ] Invalid tokens return 401 Unauthorized
- [ ] Valid tokens allow access

---

### 1.2 Add Proper Logging

**Priority:** P0 (Debuggability)  
**Effort:** 1 day  
**Locations:**
- `internal/agent/manager/vm.go:276, 365, 373, 380, 431, 507`
- `internal/hypervisor/launcher.go:325`

**Implementation:**
Replace all `// TODO: Log warning` comments with actual logging:

```go
// In vm.go
s.logger.Warn("VM state update failed", 
    zap.String("vm_id", vm.VMID),
    zap.Error(err))

// In launcher.go  
l.logger.Warn("Failed to save VM state",
    zap.String("vm_id", state.VMID),
    zap.Error(err))
```

**Acceptance Criteria:**
- [ ] All TODO comments replaced with structured logging
- [ ] Logs include context (VM ID, operation, error details)
- [ ] Log levels appropriate (Warn for recoverable, Error for failures)

---

### 1.3 Fix Database Write Permissions

**Priority:** P0 (Stability)  
**Effort:** 0.5 day  
**Symptom:** `attempt to write a readonly database (1032)`

**Root Cause:** SQLite file permissions or concurrent access issues.

**Implementation:**
```go
// In store initialization, ensure proper permissions
func (s *SQLiteStore) initialize() error {
    // Set busy timeout to handle concurrent access
    _, err := s.db.Exec("PRAGMA busy_timeout = 5000;")
    if err != nil {
        return err
    }
    
    // Set journal mode for better concurrency
    _, err = s.db.Exec("PRAGMA journal_mode = WAL;")
    if err != nil {
        return err
    }
    
    return nil
}
```

**Acceptance Criteria:**
- [ ] No more "readonly database" errors
- [ ] Concurrent read/write works correctly
- [ ] Database file has correct ownership (chv:chv)

---

### 1.4 Fix Volume Lock Contention

**Priority:** P1 (Stability)  
**Effort:** 1 day  
**Symptom:** `Failed to lock byte 101` during qcow2 conversion

**Implementation:**
Add file locking with retries:

```go
func (m *VMManager) createVolumeFromImage(volumePath, imagePath string) error {
    // Use flock for advisory locking
    lockFile := volumePath + ".lock"
    lock := flock.New(lockFile)
    
    ctx, cancel := context.WithTimeout(context.Background(), 5*time.Minute)
    defer cancel()
    
    locked, err := lock.TryLockContext(ctx, 100*time.Millisecond)
    if err != nil || !locked {
        return fmt.Errorf("failed to acquire lock: %w", err)
    }
    defer lock.Unlock()
    
    // Proceed with conversion
    return m.storageMgr.ConvertImage(imagePath, volumePath, "qcow2")
}
```

**Acceptance Criteria:**
- [ ] No more "Failed to lock byte" errors
- [ ] Concurrent volume operations are serialized safely
- [ ] Timeout after reasonable period (5 minutes)

---

### 1.5 Add Health Check Endpoints

**Priority:** P1 (Operations)  
**Effort:** 0.5 day  

**Implementation:**
```go
// Controller health endpoint
GET /api/v1/health
{
  "status": "healthy",
  "components": {
    "database": "connected",
    "agent_connection": "healthy"
  },
  "timestamp": "2026-04-06T20:00:00Z"
}

// Agent health endpoint
GET /api/v1/health
{
  "status": "healthy", 
  "vm_count": 1,
  "resources": {
    "cpu_percent": 15,
    "memory_percent": 30
  }
}
```

**Acceptance Criteria:**
- [ ] Health endpoint returns 200 when healthy
- [ ] Returns 503 when dependencies are down
- [ ] Includes component status details

---

## Phase 2: MVP-1 Completion (Week 2-3)

### 2.1 Fix Cloud-init ISO Boot Priority

**Priority:** P1 (Feature)  
**Effort:** 2 days  
**Issue:** Firmware boots from ISO instead of boot volume when both attached

**Options:**

#### Option A: Boot Order Configuration
Investigate cloud-hypervisor `--boot` parameter (not available in v51.1).

#### Option B: Separate Disk Controllers
Attach ISO to different PCI slot (may not work with firmware).

#### Option C: Metadata Service (Recommended)
Implement a metadata service that VMs can query instead of using ISO:

```go
// Metadata service on host
GET 169.254.169.254/latest/meta-data/
- instance-id
- hostname
- network-config
- user-data

// VM fetches config via HTTP instead of CD-ROM
```

**Implementation (Option C):**
```go
// internal/agent/metadata/server.go
func (s *MetadataServer) Start() error {
    // Listen on 169.254.169.254:80 (link-local)
    // Serve cloud-init data for VMs
}

// Configure VMs to use metadata service
// No ISO needed - network config fetched via HTTP
```

**Acceptance Criteria:**
- [ ] VMs boot from boot volume correctly
- [ ] Network configuration is applied via metadata service
- [ ] Static IPs work correctly

---

### 2.2 Complete Console Resize

**Priority:** P1 (UX)  
**Effort:** 1 day  
**Location:** `internal/agent/console/websocket.go:282`

**Implementation:**
```go
func (c *ConsoleClient) handleResize(cols, rows int) error {
    // Get the PTY master FD
    pty := c.pty
    
    // Use TIOCSWINSZ ioctl to resize
    ws := &unix.Winsize{
        Col: uint16(cols),
        Row: uint16(rows),
    }
    
    err := unix.IoctlSetWinsize(int(pty.Fd()), unix.TIOCSWINSZ, ws)
    if err != nil {
        return fmt.Errorf("failed to resize PTY: %w", err)
    }
    
    return nil
}
```

**Acceptance Criteria:**
- [ ] Terminal resize works in browser console
- [ ] Resize is propagated to VM serial console
- [ ] No "not implemented" messages

---

### 2.3 Add Resource Quotas

**Priority:** P2 (Operations)  
**Effort:** 2 days  

**Implementation:**
```go
// Quota enforcement in reconcile
func (s *Service) enforceQuotas(ctx context.Context, vm *models.VirtualMachine) error {
    userID := vm.CreatedBy
    
    // Get current usage
    usage, err := s.store.GetUserResourceUsage(ctx, userID)
    if err != nil {
        return err
    }
    
    // Check against limits
    limits := s.getUserLimits(userID)
    
    if usage.CPU + vm.Spec.CPU > limits.MaxCPU {
        return fmt.Errorf("CPU quota exceeded: %d + %d > %d", 
            usage.CPU, vm.Spec.CPU, limits.MaxCPU)
    }
    
    if usage.MemoryMB + vm.Spec.MemoryMB > limits.MaxMemoryMB {
        return fmt.Errorf("memory quota exceeded")
    }
    
    return nil
}
```

**Acceptance Criteria:**
- [ ] Per-user CPU limits enforced
- [ ] Per-user memory limits enforced
- [ ] Per-user VM count limits enforced
- [ ] Clear error messages when quota exceeded

---

### 2.4 Enable Clone/Snapshot Features

**Priority:** P2 (Feature)  
**Effort:** 2 days  
**Location:** `internal/api/storage.go:47-48`

**Implementation:**
```go
// Storage pool capabilities
func (s *StorageAPI) getCapabilities() StorageCapabilities {
    return StorageCapabilities{
        SupportsClone:    true,  // Enable
        SupportsSnapshot: true,  // Enable
        // ...
    }
}

// Clone implementation using qemu-img
func (m *Manager) CloneVolume(sourcePath, destPath string) error {
    cmd := exec.Command("qemu-img", "create", "-f", "raw", 
        "-b", sourcePath, "-F", "raw", destPath)
    return cmd.Run()
}

// Snapshot using qcow2 backing files
func (m *Manager) CreateSnapshot(vmID string) (string, error) {
    // Create external snapshot
    snapshotPath := fmt.Sprintf("/var/lib/chv/snapshots/%s-%d.qcow2",
        vmID, time.Now().Unix())
    
    cmd := exec.Command("qemu-img", "create", "-f", "qcow2",
        "-b", volumePath, "-F", "raw", snapshotPath)
    return snapshotPath, cmd.Run()
}
```

**Acceptance Criteria:**
- [ ] Clone API endpoint works
- [ ] Snapshot API endpoint works
- [ ] Clone creates independent copy
- [ ] Snapshot creates qcow2 backing file

---

## Phase 3: Production Readiness (Week 4-6)

### 3.1 Add TLS/mTLS for gRPC

**Priority:** P1 (Security)  
**Effort:** 3 days  

**Implementation:**
```go
// Controller gRPC server with TLS
func (s *GRPCServer) setupTLS() (*tls.Config, error) {
    cert, err := tls.LoadX509KeyPair(s.config.TLSCert, s.config.TLSKey)
    if err != nil {
        return nil, err
    }
    
    caCert, err := os.ReadFile(s.config.TLSCA)
    if err != nil {
        return nil, err
    }
    
    caCertPool := x509.NewCertPool()
    caCertPool.AppendCertsFromPEM(caCert)
    
    return &tls.Config{
        Certificates: []tls.Certificate{cert},
        ClientCAs:    caCertPool,
        ClientAuth:   tls.RequireAndVerifyClientCert,
    }, nil
}

// Agent connects with client cert
func (c *Client) connect() (*grpc.ClientConn, error) {
    creds := credentials.NewTLS(&tls.Config{
        Certificates: []tls.Certificate{c.clientCert},
        RootCAs:      c.caCertPool,
    })
    
    return grpc.Dial(c.address, grpc.WithTransportCredentials(creds))
}
```

**Acceptance Criteria:**
- [ ] gRPC connections use TLS
- [ ] Client certificates validated
- [ ] Plaintext connections rejected (when TLS enabled)
- [ ] Certificate rotation supported

---

### 3.2 Add API Rate Limiting

**Priority:** P2 (Security)  
**Effort:** 2 days  

**Implementation:**
```go
// Rate limiter middleware
func RateLimiter(requestsPerMinute int) Middleware {
    limiter := rate.NewLimiter(rate.Every(time.Minute/requestsPerMinute), requestsPerMinute)
    
    return func(next Handler) Handler {
        return func(w http.ResponseWriter, r *http.Request) {
            if !limiter.Allow() {
                w.WriteHeader(http.StatusTooManyRequests)
                json.NewEncoder(w).Encode(APIError{
                    Code:    "RATE_LIMIT_EXCEEDED",
                    Message: "Too many requests. Please try again later.",
                })
                return
            }
            next(w, r)
        }
    }
}

// Apply to routes
router.Use(RateLimiter(60)) // 60 requests per minute
```

**Acceptance Criteria:**
- [ ] Rate limiting enforced per IP
- [ ] Rate limiting enforced per user
- [ ] Returns 429 Too Many Requests
- [ ] Configurable limits

---

### 3.3 Add Prometheus Metrics

**Priority:** P2 (Observability)  
**Effort:** 2 days  

**Implementation:**
```go
// Metrics definitions
var (
    vmCount = prometheus.NewGaugeVec(prometheus.GaugeOpts{
        Name: "chv_vm_count",
        Help: "Number of VMs by state",
    }, []string{"state"})
    
    vmOperations = prometheus.NewCounterVec(prometheus.CounterOpts{
        Name: "chv_vm_operations_total",
        Help: "VM operations by type",
    }, []string{"operation", "status"})
    
    apiLatency = prometheus.NewHistogramVec(prometheus.HistogramOpts{
        Name:    "chv_api_latency_seconds",
        Help:    "API latency by endpoint",
        Buckets: prometheus.DefBuckets,
    }, []string{"endpoint"})
)

// Register and expose
func init() {
    prometheus.MustRegister(vmCount, vmOperations, apiLatency)
}

// Endpoint
GET /metrics → Prometheus format
```

**Acceptance Criteria:**
- [ ] VM count metrics by state
- [ ] Operation counters (create, start, stop, delete)
- [ ] API latency histograms
- [ ] Resource usage metrics (CPU, memory)
- [ ] /metrics endpoint exposed

---

### 3.4 Add Operations Retention Policy

**Priority:** P3 (Maintenance)  
**Effort:** 1 day  

**Implementation:**
```go
// Background cleanup job
func (s *Service) cleanupOldOperations(ctx context.Context) {
    ticker := time.NewTicker(24 * time.Hour)
    defer ticker.Stop()
    
    for {
        select {
        case <-ticker.C:
            cutoff := time.Now().AddDate(0, 0, -90) // 90 days
            err := s.store.DeleteOperationsBefore(ctx, cutoff)
            if err != nil {
                log.Printf("Failed to cleanup operations: %v", err)
            }
        case <-ctx.Done():
            return
        }
    }
}
```

**Acceptance Criteria:**
- [ ] Operations older than 90 days deleted
- [ ] Cleanup runs daily
- [ ] No performance impact on active operations

---

## Phase 4: Future Enhancements (Post-MVP)

### 4.1 VM Pause/Resume
**Effort:** 2 days  
**Benefit:** Save VM state without full shutdown

### 4.2 Live Migration
**Effort:** 1-2 weeks  
**Benefit:** Move VMs between nodes without downtime

### 4.3 VXLAN/Overlay Networking
**Effort:** 1-2 weeks  
**Benefit:** Multi-node networking without physical VLANs

### 4.4 GPU/VFIO Support
**Effort:** 2-3 weeks  
**Benefit:** GPU passthrough for ML workloads

### 4.5 Windows Guest Support
**Effort:** 2-3 weeks  
**Benefit:** Broader guest OS support

---

## Implementation Schedule

| Week | Focus | Key Deliverables |
|------|-------|------------------|
| 1 | Critical Fixes | JWT auth, logging, DB fixes, locks, health checks |
| 2 | MVP Completion | Cloud-init metadata service, console resize |
| 3 | MVP Completion | Resource quotas, clone/snapshot |
| 4 | Production | TLS/mTLS implementation |
| 5 | Production | Rate limiting, metrics |
| 6 | Production | Retention policy, documentation |

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Cloud-hypervisor version limitations | Medium | High | Use metadata service instead of ISO |
| SQLite concurrency issues | Medium | Medium | WAL mode, proper locking |
| TLS certificate management | Low | High | Document rotation procedures |
| Resource quota enforcement overhead | Low | Low | Caching, async updates |

---

## Success Criteria

### Phase 1 Complete When:
- [ ] All P0 items merged
- [ ] No critical security vulnerabilities
- [ ] No database errors in logs

### Phase 2 Complete When:
- [ ] Cloud-init network config works
- [ ] Console resize works
- [ ] Resource quotas enforced

### Phase 3 Complete When:
- [ ] TLS enabled by default
- [ ] Metrics in Prometheus
- [ ] Rate limiting active

### v0.2.0 Release Ready When:
- [ ] All acceptance criteria met
- [ ] Load testing passed
- [ ] Security audit passed
- [ ] Documentation complete
