# CHV Next Steps Plan

**Date:** 2026-04-09  
**Current Status:** MVP-1 Core Complete, Console Fixed, Ready for Enhancement

---

## Executive Summary

The CHV (Cloud Hypervisor Virtualization Platform) has reached a functional MVP state with:
- ✅ Complete VM lifecycle management
- ✅ Image import/upload with progress tracking
- ✅ Working VM console via WebSocket
- ✅ Basic networking and storage
- ✅ Web UI with real-time updates

This plan outlines the next phases of development to move from MVP to production-ready.

---

## Phase 1: Stability & Hardening (Immediate - 1-2 weeks)

### 1.1 Critical Fixes

| Priority | Item | Description | Effort |
|----------|------|-------------|--------|
| P0 | Remove fetcher stub | `internal/images/fetcher.go` is a stub - either implement or remove | 2h |
| P0 | VM orphan detection | Improve orphan VM detection when agent restarts | 4h |
| P1 | Error handling standardization | Standardize error responses across all agent handlers | 4h |
| P1 | Request ID propagation | Add request ID for end-to-end tracing | 4h |

### 1.2 Testing Gap Closure

| Priority | Item | Description | Effort |
|----------|------|-------------|--------|
| P1 | VM console tests | Unit tests for VM console service | 8h |
| P1 | Integration tests | Image import worker integration tests | 8h |
| P2 | E2E tests | VM lifecycle E2E tests with Playwright | 16h |

### 1.3 Observability Improvements

| Priority | Item | Description | Effort |
|----------|------|-------------|--------|
| P1 | Structured logging | Add correlation IDs and structured fields | 4h |
| P1 | Health checks | Add /health endpoint with dependency checks | 4h |
| P2 | Metrics export | Prometheus metrics endpoint | 8h |

---

## Phase 2: User Experience Enhancements (2-4 weeks)

### 2.1 Console Improvements

| Priority | Item | Description | Effort |
|----------|------|-------------|--------|
| P1 | Console resize | Support terminal resize (SIGWINCH) | 4h |
| P1 | Console history | Show last N lines of output on connect | 8h |
| P2 | Multiple consoles | Support multiple concurrent console sessions | 8h |
| P2 | Copy/paste | Clipboard integration for console | 4h |

### 2.2 VM Management UX

| Priority | Item | Description | Effort |
|----------|------|-------------|--------|
| P1 | VM power actions | Graceful shutdown with timeout, force stop | 8h |
| P1 | Boot logs | Capture and display VM boot output | 8h |
| P2 | VM timeline | Visual timeline of VM lifecycle events | 16h |
| P2 | VNC console | Optional VNC console for graphical VMs | 24h |

### 2.3 Image Management

| Priority | Item | Description | Effort |
|----------|------|-------------|--------|
| P1 | Image validation | Verify image integrity before use | 8h |
| P2 | Image caching | Cache imported images for faster cloning | 16h |
| P2 | Image versioning | Support multiple versions of same image | 24h |

---

## Phase 3: Production Readiness (4-6 weeks)

### 3.1 High Availability

| Priority | Item | Description | Effort |
|----------|------|-------------|--------|
| P1 | Controller HA | Support multiple controller instances | 40h |
| P2 | Database migration | PostgreSQL backend option | 40h |
| P2 | Session sharing | Redis for session/token storage | 24h |

### 3.2 Security Hardening

| Priority | Item | Description | Effort |
|----------|------|-------------|--------|
| P0 | RBAC | Role-based access control (admin, operator, viewer) | 40h |
| P1 | JWT authentication | Replace opaque tokens with JWT | 24h |
| P1 | Audit logging | Log all API actions with user context | 16h |
| P2 | mTLS | Mutual TLS for controller-agent communication | 24h |

### 3.3 Networking Enhancements

| Priority | Item | Description | Effort |
|----------|------|-------------|--------|
| P2 | VLAN support | 802.1Q VLAN tagging | 16h |
| P2 | DHCP server | Built-in DHCP for VM networks | 24h |
| P3 | SDN integration | Open vSwitch support | 40h |

---

## Phase 4: Advanced Features (6+ weeks)

### 4.1 Storage

| Priority | Item | Description | Effort |
|----------|------|-------------|--------|
| P2 | NFS storage | NFS-backed storage pools | 24h |
| P3 | Ceph/RBD | Ceph RBD storage backend | 40h |
| P3 | Live migration | VM live migration between hosts | 80h |

### 4.2 Multi-Host

| Priority | Item | Description | Effort |
|----------|------|-------------|--------|
| P2 | Agent registration | Dynamic agent registration | 24h |
| P3 | Host groups | Organize agents into groups/zones | 24h |
| P3 | VM scheduling | Basic scheduling across hosts | 40h |

### 4.3 Automation

| Priority | Item | Description | Effort |
|----------|------|-------------|--------|
| P2 | Cloud-init templates | Predefined cloud-init configurations | 16h |
| P2 | VM templates | Template-based VM creation | 24h |
| P3 | Auto-scaling | Auto-scale VMs based on metrics | 40h |

---

## Technical Debt Items

### Must Fix (Blocking Production)

1. **Image Fetcher Stub** (`internal/images/fetcher.go`)
   - Currently a stub that does nothing
   - Either implement proper URL fetching or remove
   - **Action:** Remove if not needed, or implement HTTP fetching

2. **VM State Consistency**
   - VM state can get out of sync if agent restarts
   - Need periodic reconciliation between controller DB and actual VM state
   - **Action:** Add background reconciliation loop

3. **Error Handling Consistency**
   - Some endpoints return plain text errors, others JSON
   - **Action:** Standardize on structured JSON errors

### Should Fix (Improves Reliability)

1. **Database Migrations**
   - Currently using inline schema creation
   - **Action:** Implement proper migration system (golang-migrate)

2. **Configuration Management**
   - Config is scattered across files
   - **Action:** Centralize config with validation

3. **Resource Limits**
   - No enforcement of VM resource quotas
   - **Action:** Add per-user/per-project resource limits

---

## Immediate Action Items (This Week)

### Day 1-2: Cleanup
- [ ] Remove `internal/images/fetcher.go` stub
- [ ] Standardize error handling in agent handlers
- [ ] Add request ID middleware

### Day 3-4: Testing
- [ ] Write VM console service unit tests
- [ ] Add integration test for image import
- [ ] Verify console WebSocket end-to-end

### Day 5: Documentation
- [ ] Document API endpoints
- [ ] Create deployment guide
- [ ] Update README with current features

---

## Success Criteria

### Phase 1 Complete When:
- All stubs removed or implemented
- Test coverage > 60% for critical paths
- No known critical bugs
- Health check endpoint responding

### Phase 2 Complete When:
- Console supports resize and history
- VM power actions work reliably
- Boot logs accessible from UI
- User feedback incorporated

### Phase 3 Complete When:
- HA setup documented and tested
- Security audit passed
- RBAC controls in place
- Production deployment guide complete

---

## Risk Assessment

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| VM state inconsistency | High | Medium | Add reconciliation loop, improve orphan detection |
| Console disconnects | Medium | Medium | Implement reconnection logic, session persistence |
| Image import failures | Medium | Low | Add retry logic, better error messages |
| Database corruption | High | Low | Regular backups, PostgreSQL migration |
| Security vulnerabilities | High | Low | Security audit, penetration testing |

---

## Resource Requirements

### Development
- 1x Backend Engineer (Go) - Full time
- 1x Frontend Engineer (Svelte/TS) - Part time
- 1x DevOps Engineer - Part time (Phases 3+)

### Infrastructure
- Test environment: 2x hypervisor nodes
- CI/CD: GitHub Actions or similar
- Monitoring: Prometheus + Grafana (Phase 3)

---

## Appendix: Current Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                         User Browser                         │
└──────────────────────────────────┬──────────────────────────┘
                                   │ HTTPS/WSS
                                   ▼
┌─────────────────────────────────────────────────────────────┐
│  Controller (Go)                                            │
│  ├── REST API (Chi)                                         │
│  ├── WebSocket Proxy (Gorilla)                              │
│  ├── SQLite Repository                                      │
│  └── Auth (Bearer Tokens)                                   │
└──────────────────────────────────┬──────────────────────────┘
                                   │ HTTP / Unix Socket
                                   ▼
┌─────────────────────────────────────────────────────────────┐
│  Agent (Go) - Runs on each hypervisor host                  │
│  ├── VM Management (Cloud Hypervisor)                       │
│  ├── Console Service (PTY/WebSocket)                        │
│  ├── TAP/Firewall Management                                │
│  └── Image Download Service                                 │
└──────────────────────────────────┬──────────────────────────┘
                                   │
                    ┌──────────────┼──────────────┐
                    ▼              ▼              ▼
              ┌─────────┐    ┌─────────┐    ┌─────────┐
              │   VMs   │    │  Images │    │ Networks│
              │ (CH)    │    │ (qcow2) │    │ (bridge)│
              └─────────┘    └─────────┘    └─────────┘
```

---

## Appendix: Feature Completeness Matrix

| Feature | Backend | Frontend | Integration | Status |
|---------|---------|----------|-------------|--------|
| VM CRUD | ✅ | ✅ | ✅ | Complete |
| VM Lifecycle | ✅ | ✅ | ✅ | Complete |
| Image Import | ✅ | ✅ | ✅ | Complete |
| Image Upload | ✅ | ✅ | ✅ | Complete |
| Console | ✅ | ✅ | ✅ | Complete |
| Snapshots | ✅ | ✅ | ✅ | Complete |
| Metrics | ✅ | ⚠️ | ⚠️ | Backend ready |
| Bulk Operations | ✅ | ✅ | ✅ | Complete |
| Networks | ✅ | ✅ | ✅ | Complete |
| Storage Pools | ✅ | ✅ | ✅ | Complete |
| Events | ✅ | ✅ | ✅ | Complete |
| Operations | ✅ | ✅ | ✅ | Complete |
| Authentication | ✅ | ✅ | ✅ | Complete |

**Legend:**
- ✅ Complete
- ⚠️ Partial/Needs work
- ❌ Not started

---

*Document Version: 1.0*  
*Last Updated: 2026-04-09*
