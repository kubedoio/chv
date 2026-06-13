-- Architecture Designer: immutable version history.
-- Each successful save of a topology creates a new architecture_versions row
-- containing the original YAML, normalized model, and graph JSON for audit
-- and future migration. Cascades from topology so archive/hard-delete cleans
-- up history when the parent goes away.

CREATE TABLE IF NOT EXISTS architecture_versions (
    id text PRIMARY KEY,
    architecture_id text NOT NULL REFERENCES architecture_topologies (id) ON DELETE CASCADE,
    version_number integer NOT NULL,
    yaml_content text NOT NULL,
    design_graph_json text,
    normalized_model_json text,
    change_summary text,
    created_by text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS architecture_versions_architecture_id_idx
    ON architecture_versions (architecture_id);

CREATE INDEX IF NOT EXISTS architecture_versions_architecture_id_created_at_idx
    ON architecture_versions (architecture_id, created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS architecture_versions_architecture_version_number_uniq
    ON architecture_versions (architecture_id, version_number);
