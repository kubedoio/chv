-- Hot-path index for the incomplete-operation scan: every process start and
-- every executor scan filters status IN ('accepted','running') ordered by
-- accepted_at,operation_id (see list_incomplete_operations /
-- list_incomplete_execution_operations). Without this index a long-lived
-- node retaining terminal history full-scans and temp-b-tree-sorts the whole
-- operations table on every scan.
CREATE INDEX operations_recovery_idx ON operations(status, accepted_at, operation_id);

-- Serving index for the bounded newest-N listing (list_operations: ORDER BY
-- accepted_at DESC, operation_id DESC LIMIT n). SQLite scans this ASC index
-- backwards, so the listing stops after n index entries instead of
-- full-scanning and top-n-sorting the retained history.
CREATE INDEX operations_listing_idx ON operations(accepted_at, operation_id);
