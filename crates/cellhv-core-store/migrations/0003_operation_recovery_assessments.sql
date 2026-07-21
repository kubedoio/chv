CREATE TABLE operation_recovery_assessments (
    operation_id TEXT NOT NULL REFERENCES operations(operation_id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    active_attempt_token TEXT NOT NULL CHECK (
        length(active_attempt_token) BETWEEN 1 AND 128
        AND active_attempt_token NOT GLOB '*[^!-~]*'
    ),
    classification TEXT NOT NULL CHECK (classification IN (
        'ownership_matched', 'owned_alive_socket_unavailable', 'exited_owned',
        'foreign_conflict', 'ambiguous_preserve', 'duplicate_conflict',
        'corrupt_ownership'
    )),
    disposition TEXT NOT NULL CHECK (disposition IN (
        'ownership_matched_pending_control', 'exited_pending_policy', 'quarantined'
    )),
    evidence_fingerprint TEXT NOT NULL CHECK (
        length(evidence_fingerprint) = 64
        AND evidence_fingerprint NOT GLOB '*[^0-9a-f]*'
    ),
    evidence_json TEXT NOT NULL CHECK (
        json_valid(evidence_json)
        AND json_type(evidence_json) = 'object'
        AND length(CAST(evidence_json AS BLOB)) <= 16384
    ),
    assessed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (operation_id, revision),
    UNIQUE (operation_id, active_attempt_token, evidence_fingerprint)
) STRICT;

CREATE INDEX operation_recovery_latest_idx
ON operation_recovery_assessments(operation_id, revision DESC);
