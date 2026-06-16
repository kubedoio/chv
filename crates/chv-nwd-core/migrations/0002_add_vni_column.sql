-- chv-nwd-core migration 0002: VXLAN VNI column for overlay topologies.
-- The runtime guards this with a PRAGMA table_info check (see migrations/mod.rs)
-- so an in-place upgrade from a database where the previous unversioned
-- `let _ = ALTER TABLE` already added the column does not fail.
ALTER TABLE topologies ADD COLUMN vni INTEGER;
