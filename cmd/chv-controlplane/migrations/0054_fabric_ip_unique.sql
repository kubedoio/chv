-- ADR-021 fabric transport IP uniqueness (review finding M2).
--
-- fabric_ip is allocated from the shared 100.100.0.0/16 pool by picking
-- the lowest free address inside a transaction. Two concurrent
-- registrations can observe the same free address before either commits,
-- so the allocation alone does not guarantee uniqueness. This unique
-- index is the guarantee: a losing registration fails with a constraint
-- violation and the store layer retries the allocation (bounded) before
-- returning a structured conflict error.
--
-- SQLite unique indexes ignore NULLs, so nodes without a fabric transport
-- IP (not yet registered for fabric) are unaffected.

CREATE UNIQUE INDEX IF NOT EXISTS idx_vtep_registry_fabric_ip ON vtep_registry(fabric_ip);
