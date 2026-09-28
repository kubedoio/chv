-- ADR-021 stretched-L2 fabric: node fabric identity in the VTEP registry.
-- public_key / underlay_endpoint / fabric_ip / underlay_mtu describe the
-- node's WireGuard fabric identity (public key only; the private key never
-- leaves the node). binding_generation on vni_allocations fences VNI
-- re-binding: it is stamped on every (re-)allocation and carried in compiled
-- fabric plans so nwd can detect a changed VNI binding.

ALTER TABLE vtep_registry ADD COLUMN public_key TEXT;
ALTER TABLE vtep_registry ADD COLUMN underlay_endpoint TEXT;
ALTER TABLE vtep_registry ADD COLUMN fabric_ip TEXT;
ALTER TABLE vtep_registry ADD COLUMN underlay_mtu INTEGER;

ALTER TABLE vni_allocations ADD COLUMN binding_generation INTEGER NOT NULL DEFAULT 1;
