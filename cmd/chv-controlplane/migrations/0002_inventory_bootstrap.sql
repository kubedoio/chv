-- Add JSONB columns to node_inventory for extended metadata
ALTER TABLE node_inventory ADD COLUMN IF NOT EXISTS storage_classes jsonb;
ALTER TABLE node_inventory ADD COLUMN IF NOT EXISTS network_capabilities jsonb;
ALTER TABLE node_inventory ADD COLUMN IF NOT EXISTS labels jsonb;

-- Add node_bootstrap_results table for detailed bootstrap logging
CREATE TABLE IF NOT EXISTS node_bootstrap_results (
    node_id text PRIMARY KEY REFERENCES nodes (node_id) ON DELETE CASCADE,
    operation_id text,
    success boolean NOT NULL,
    error_message text,
    details jsonb,
    started_at timestamptz,
    completed_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- Index for bootstrap status
CREATE INDEX IF NOT EXISTS node_bootstrap_results_success_idx ON node_bootstrap_results (success);
