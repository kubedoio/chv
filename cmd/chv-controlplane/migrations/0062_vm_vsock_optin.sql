-- G5 vsock monitoring transport (prompt-06 task 2): the per-VM opt-in
-- for the optional virtio-vsock device. Additive only — existing rows
-- keep the historical meaning (0 = no device, exactly as before the
-- column existed; the device is strictly opt-in and never silently
-- attached). The CID and host socket path are node-authority runtime
-- state and are deliberately NOT stored here.
ALTER TABLE vm_desired_state ADD COLUMN vsock_enabled INTEGER NOT NULL DEFAULT 0;
