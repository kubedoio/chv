# Compatibility Matrix

## Purpose
Track supported version combinations for a tested node bundle.

## Required dimensions
- control plane version
- `chv-agent` version
- `chv-stord` version
- `chv-nwd` version
- Cloud Hypervisor version
- host helper bundle version

## Rules
- each supported node bundle must be testable as a versioned unit
- independent component upgrade is allowed only when this matrix explicitly permits it
- rollback targets must also exist in this matrix

## Suggested table shape

| Control Plane | chv-agent | chv-stord | chv-nwd | Cloud Hypervisor | Host Bundle | Status | Notes |
|---|---|---|---|---|---|---|---|
| 0.1.x | 0.1.x | 0.1.x | 0.1.x | pinned | 0.1.x | supported | MVP-1 baseline |

## Versioning guidance
- use explicit compatibility ranges where possible
- do not allow silent semantic drift across RPC contracts
- bump API version namespaces when backward compatibility breaks
