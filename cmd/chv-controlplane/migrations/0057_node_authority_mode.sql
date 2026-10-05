-- #378: node authority mode as reported by the agent (a config fact,
-- static per agent process) at enrollment and on every periodic
-- inventory cycle (~30 s). Typed carrier for accept-time policy checks
-- (volume snapshot/clone/restore/delete-snapshot rejection on
-- core-managed nodes).
--
-- Nullable and backfill-free on purpose: existing rows — and nodes whose
-- agent has not reported a mode — stay NULL, and every consumer fails
-- OPEN on NULL (the agent's fail-closed dispatch remains the enforcement
-- boundary; the accept-time check is UX hardening only).

ALTER TABLE node_inventory ADD COLUMN authority_mode text;
