-- Add owner_id to resources for fine-grained RBAC

ALTER TABLE vms ADD COLUMN owner_id TEXT;
ALTER TABLE volumes ADD COLUMN owner_id TEXT;
ALTER TABLE networks ADD COLUMN owner_id TEXT;

CREATE INDEX IF NOT EXISTS idx_vms_owner_id ON vms(owner_id);
CREATE INDEX IF NOT EXISTS idx_volumes_owner_id ON volumes(owner_id);
CREATE INDEX IF NOT EXISTS idx_networks_owner_id ON networks(owner_id);
