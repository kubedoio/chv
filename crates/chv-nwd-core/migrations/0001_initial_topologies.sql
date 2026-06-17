-- chv-nwd-core migration 0001: initial topologies table.
-- Mirrors the original CREATE TABLE shape that pre-dated versioned migrations,
-- so a fresh DB and an in-place upgrade converge on the same final schema
-- once 0002 and 0003 are applied.
CREATE TABLE IF NOT EXISTS topologies (
    network_id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    bridge_name TEXT NOT NULL,
    namespace_name TEXT NOT NULL,
    subnet_cidr TEXT NOT NULL,
    gateway_ip TEXT NOT NULL,
    runtime_status TEXT NOT NULL
);
