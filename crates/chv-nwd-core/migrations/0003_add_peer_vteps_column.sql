-- chv-nwd-core migration 0003: peer VTEP list for VXLAN overlay topologies.
-- Stored as a JSON-encoded TEXT array; default '[]' keeps the column
-- non-NULL for rows inserted before per-topology VTEPs were tracked.
-- As with 0002, the runtime detects the column via PRAGMA table_info to
-- support in-place upgrades from the prior unversioned ALTER TABLE.
ALTER TABLE topologies ADD COLUMN peer_vteps TEXT NOT NULL DEFAULT '[]';
