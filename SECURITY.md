# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1.0 | :x:                |

## Reporting a Vulnerability

We take security seriously. If you discover a security vulnerability, please report it responsibly.

### How to Report

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please report them via email to:

- **security@chv.local** (replace with actual security contact)

Please include:
- A description of the vulnerability
- Steps to reproduce the issue
- Possible impact
- Any suggested fixes (if available)

### Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial assessment**: Within 5 business days
- **Fix timeline**: Based on severity (see below)
- **Disclosure**: Coordinated with reporter

### Severity Levels

| Level | Description | Response Time | Fix Timeline |
|-------|-------------|---------------|--------------|
| Critical | Remote code execution, data breach | 24 hours | 7 days |
| High | Privilege escalation, VM escape | 48 hours | 14 days |
| Medium | Denial of service, information disclosure | 5 days | 30 days |
| Low | Minor issues, defense in depth | 14 days | 90 days |

## Security Best Practices

### For Operators

#### Network Security
- Run CHV components on isolated management networks
- Use firewall rules to restrict access to:
  - Controller API (port 8080) - Admin access only
  - Controller gRPC (port 9090) - Agent nodes only
  - Agent gRPC (port 9091) - Controller only

#### Authentication
- Use strong, random API tokens
- Rotate tokens regularly
- Use short token expiration for automation
- Store tokens securely (use secrets management)

#### VM Isolation
- Use separate bridges for different tenant networks
- Enable CPU/memory limits per VM
- Monitor for resource exhaustion attacks

### For Developers

#### Secure Coding
- Validate all user inputs at API boundaries
- Use parameterized queries for database operations
- Avoid shell command construction (use exec with arrays)
- Never log sensitive data (tokens, passwords)

#### Dependencies
- Run `go mod tidy` regularly
- Check for vulnerable dependencies: `govulncheck ./...`
- Keep base Docker images updated

## Security Features

### Current Implementation (v0.1.0)

#### Authentication & Authorization
- ✅ Opaque bearer token authentication
- ✅ Token expiration and revocation
- ⚠️ No RBAC (planned for v0.3.0)

#### Input Validation
- ✅ VM ID path traversal prevention
- ✅ UUID format validation
- ✅ Resource limit validation

#### Network Security
- ⚠️ gRPC without TLS (MVP-1 limitation)
- ✅ Unix socket for local CHV API
- ⚠️ No API rate limiting (planned for v0.2.0)

#### Data Protection
- ✅ No secrets in code or logs
- ✅ Structured error messages (no internal details)
- ⚠️ No encryption at rest (planned for v0.3.0)

### Planned Security Improvements

#### v0.2.0
- [ ] mTLS for Controller-Agent communication
- [ ] API rate limiting
- [ ] VM resource quotas and limits
- [ ] Audit logging

#### v0.3.0
- [ ] Role-based access control (RBAC)
- [ ] Encryption at rest for sensitive data
- [ ] Secret management integration
- [ ] Security scanning in CI/CD

## Security Hardening Checklist

### Production Deployment

- [ ] Enable mTLS for all gRPC connections
- [ ] Configure firewall rules
- [ ] Use strong PostgreSQL passwords
- [ ] Enable PostgreSQL SSL
- [ ] Run agents as non-root (use capabilities for networking)
- [ ] Enable audit logging
- [ ] Set up monitoring and alerting
- [ ] Regular security updates

### VM Security

- [ ] Use cloud-init for secure VM initialization
- [ ] Disable password authentication (use SSH keys)
- [ ] Keep VM images updated
- [ ] Use network policies to restrict VM traffic

## Known Security Limitations

### MVP-1 (v0.1.0)

1. **No TLS for gRPC**: Controller-Agent communication is unencrypted
   - Mitigation: Run on isolated private network
   - Fix planned: v0.2.0

2. **No API Rate Limiting**: Potential for DoS via API abuse
   - Mitigation: Use reverse proxy with rate limiting
   - Fix planned: v0.2.0

3. **Agent runs as root**: Required for TAP device creation
   - Mitigation: Use minimal container capabilities
   - Fix planned: Evaluate unprivileged options

4. **No Encryption at Rest**: VM data stored unencrypted
   - Mitigation: Use encrypted filesystems
   - Fix planned: v0.3.0

## Security Acknowledgments

We thank the following individuals for responsibly disclosing security issues:

- [Your name here] - Thank you for helping keep CHV secure!

## Related Resources

- [OWASP Cloud Security](https://owasp.org/www-project-cloud-security/)
- [CIS Benchmarks](https://www.cisecurity.org/cis-benchmarks)
- [Cloud Hypervisor Security](https://github.com/cloud-hypervisor/cloud-hypervisor/blob/main/docs/security.md)

---

Last updated: 2026-04-05
