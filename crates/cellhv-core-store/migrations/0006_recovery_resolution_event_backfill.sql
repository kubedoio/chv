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
-- stores upgrade as a no-op. The sequence base is computed once (CTE) so
-- every backfilled row gets a distinct sequence even if the statement
-- inserts many rows.
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
)
ORDER BY a.operation_id, a.revision;
