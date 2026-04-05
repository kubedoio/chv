# ADR-0006: MVP-1 API auth uses opaque tokens

## Status
Accepted

## Context
JWT-based auth is not required for MVP-1 machine access and would add unnecessary complexity.

## Decision
Use opaque bearer tokens.
Store only SHA-256 hashes in PostgreSQL.

## Consequences
### Positive
- simple and understandable auth model
- easy revocation
- no JWT validation/distribution complexity

### Negative
- no human SSO model in MVP-1
- auth model will need expansion later
