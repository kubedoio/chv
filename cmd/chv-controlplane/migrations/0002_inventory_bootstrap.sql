-- Add text columns to node_inventory for extended metadata
ALTER TABLE node_inventory ADD COLUMN storage_classes text;
ALTER TABLE node_inventory ADD COLUMN network_capabilities text;
ALTER TABLE node_inventory ADD COLUMN labels text;

-- Add node_bootstrap_results table for detailed bootstrap logging
CREATE TABLE IF NOT EXISTS node_bootstrap_results (
    node_id text PRIMARY KEY REFERENCES nodes (node_id) ON DELETE CASCADE,
    operation_id text,
    success integer NOT NULL,
    error_message text,
    details text,
    started_at text,
    completed_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

-- Index for bootstrap status
CREATE INDEX IF NOT EXISTS node_bootstrap_results_success_idx ON node_bootstrap_results (success);
