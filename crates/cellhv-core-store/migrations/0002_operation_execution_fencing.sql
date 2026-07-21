ALTER TABLE operations
ADD COLUMN active_attempt_token TEXT
CHECK (
    active_attempt_token IS NULL OR (
        length(active_attempt_token) BETWEEN 1 AND 128
        AND active_attempt_token NOT GLOB '*[^!-~]*'
    )
);

ALTER TABLE operations
ADD COLUMN completed_attempt_token TEXT
CHECK (
    completed_attempt_token IS NULL OR (
        length(completed_attempt_token) BETWEEN 1 AND 128
        AND completed_attempt_token NOT GLOB '*[^!-~]*'
    )
);

-- Version-1 running operations may already have crossed the side-effect
-- boundary. Preserve them as inspect-required rather than authorizing a retry.
UPDATE operations
SET active_attempt_token = 'legacy-ambiguous-v1'
WHERE status = 'running';

-- Preserve terminal version-1 rows as completed without inventing a claim
-- that could authorize execution.
UPDATE operations
SET completed_attempt_token = 'legacy-completed-v1'
WHERE status IN ('succeeded', 'failed', 'unsupported');
