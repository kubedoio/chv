-- Add missing indexes identified during comprehensive review.

CREATE INDEX IF NOT EXISTS idx_volume_desired_state_attached_vm_id
    ON volume_desired_state(attached_vm_id);

CREATE INDEX IF NOT EXISTS idx_operations_requested_by
    ON operations(requested_by);

CREATE INDEX IF NOT EXISTS idx_vm_desired_state_generation
    ON vm_desired_state(desired_generation);

CREATE INDEX IF NOT EXISTS idx_vm_observed_state_generation
    ON vm_observed_state(observed_generation);

CREATE INDEX IF NOT EXISTS idx_operations_status_requested_at
    ON operations(status, requested_at);
