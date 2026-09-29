-- Migration 0006: backfill the `operation.recovery_assessed` events that
-- `resolve_interrupted_operation` failed to write before the fix.
--
-- Every assessment row must carry a matching event (event_id
-- `<operation_id>:recovery-assessed:<revision>`): the load-time
-- `validate_recovery_assessment_rows` check enforces a one-to-one
-- correspondence. An operator resolution (resolve_inspect_required) wrote
-- its assessment revision WITHOUT the event, leaving the store permanently
-- unopenable — the agent crashed on every restart with
-- `Integrity("recovery assessments and recovery events are not one-to-one")`
-- (M2.5 run-10 qualification, section 9b).
--
-- The event content is fully determined by the assessment row (the same
-- canonical payload every assessment writer records), so the backfill
-- restores exactly what the missing write would have persisted — it does
-- not guess. Idempotent: `event_id` is the primary key and the NOT EXISTS
-- guard skips assessments that already carry their event, so healthy
-- stores upgrade as a no-op. The sequence base is computed from the
-- pre-statement max (a statement's own inserts are not visible to its
-- scans) so every backfilled row gets a distinct, contiguous sequence
-- even across many rows.
--
-- Known, accepted caveats:
-- - Order inversion: backfilled events append at the journal's tail, so
--   on a repaired store a resolution's recovery event sequences AFTER
--   that operation's terminal event (the fixed writer orders them
--   recovery-then-terminal). No validator or consumer depends on that
--   ordering today; inserting mid-stream would require renumbering every
--   later sequence, which is strictly worse. Audit-log readers of
--   repaired stores must not assume recovery-assessed precedes terminal.
-- - Scope: this migration repairs ONLY the missing-event hole. Any other
--   integrity defect still fails validation inside the upgrade
--   transaction (fail closed) and requires manual repair.
WITH sequence_base(max_sequence) AS (
    SELECT coalesce(max(sequence), 0) FROM events
)
INSERT INTO events (event_id, sequence, operation_id, vm_id, kind, payload_json)
SELECT
    a.operation_id || ':recovery-assessed:' || a.revision,
    sequence_base.max_sequence
        + row_number() OVER (ORDER BY a.operation_id, a.revision),
    a.operation_id,
    o.vm_id,
    'operation.recovery_assessed',
    json_object(
        'classification', a.classification,
        'disposition', a.disposition,
        'evidence_fingerprint', a.evidence_fingerprint,
        'revision', a.revision
    )
FROM operation_recovery_assessments a
JOIN operations o ON o.operation_id = a.operation_id
CROSS JOIN sequence_base
WHERE NOT EXISTS (
    SELECT 1 FROM events e
    WHERE e.event_id = a.operation_id || ':recovery-assessed:' || a.revision
);
