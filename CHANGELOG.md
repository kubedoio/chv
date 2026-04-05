# Changelog

All notable changes to CHV will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial MVP-1 implementation

## [0.1.0] - 2026-04-05

### Added

#### Core Platform
- Controller-Agent architecture with gRPC communication
- PostgreSQL-backed control plane with state reconciliation
- RESTful API for VM lifecycle management
- Authentication system with opaque bearer tokens

#### VM Management
- VM lifecycle operations (create, start, stop, reboot, delete)
- Cloud-image-first provisioning (no ISO workflows)
- Cloud-init integration for VM initialization
- Basic online disk resize support
- Raw runtime disk format for simplicity

#### Scheduling
- Multi-strategy scheduler (BestFit, FirstFit, LeastLoaded, RoundRobin)
- Node inventory and resource tracking
- VM placement with constraint checking
- Maintenance/drain mode support

#### Networking
- Linux bridge networking
- TAP device management per VM
- MAC address generation (deterministic from VM ID)

#### Storage
- Local storage pool support
- NFS storage pool support
- Volume creation and management
- Image import and backing storage

#### Security
- **Path traversal prevention**: VM ID validation to prevent directory escape attacks
- Input validation at all API boundaries
- Structured error handling without information leakage

#### Reliability
- **Race condition fixes**: Launcher cleanup synchronized with `sync.RWMutex` and `sync.Once`
- Persistent VM state for crash recovery
- Idempotent operations with operation ID tracking
- Graceful shutdown cascade (API → SIGTERM → SIGKILL)
- Desired vs actual state reconciliation

#### Testing
- Comprehensive unit test suite (100+ tests)
- Race detector clean (`go test -race`)
- Agent client tests with mock gRPC server (70.9% coverage)
- Reconciler service tests (85%+ coverage)
- Hypervisor, network, and cloud-init tests

#### Documentation
- Architecture Decision Records (7 ADRs)
- Product specifications and system contracts
- Design system for VMware/Proxmox-style UI
- API documentation with curl examples
- Contributing guidelines

#### Build & Deploy
- Docker Compose setup for local development
- Multi-stage Dockerfiles for all components
- Makefile with common tasks
- Systemd unit files for production deployment

### Known Limitations

#### MVP-1 Scope
- Linux guests only
- Cloud Hypervisor VMM exclusively
- Insecure gRPC (no TLS) - documented for production hardening
- No live migration
- No Windows guest support
- No GPU/VFIO support

### Security Notes

#### Fixed in this Release
- **CVE-2026-XXXX**: Path traversal vulnerability in VM ID handling
  - All VM IDs are now validated before use in file paths
  - Validation rejects path separators (`/`, `\`) and traversal sequences (`..`)

### Technical Debt

#### Scheduled for v0.2.0
- Add TLS/mTLS for gRPC connections
- Implement VM console access (serial/VNC)
- Add metrics collection and monitoring
- Image download and conversion worker

### Contributors

- CHV Development Team

---

## Release Notes Template

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- New features

### Changed
- Changes to existing functionality

### Deprecated
- Soon-to-be removed features

### Removed
- Removed features

### Fixed
- Bug fixes

### Security
- Security fixes
```
