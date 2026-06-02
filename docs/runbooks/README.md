# CHV Disaster Recovery Runbooks

This directory contains operational runbooks for backup, restore, and disaster recovery scenarios. Each runbook is designed to be followed under pressure — steps are numbered, commands are copy-pasteable, and prerequisites are listed upfront.

## Runbook Index

| Runbook | Scenario | Automation Level | Time to Execute |
|---------|----------|------------------|-----------------|
| [VM Snapshot Restore](vm-snapshot-restore.md) | Restore a VM from a local snapshot | Fully automated via API/UI | 2–5 min |
| [Volume Snapshot Restore](volume-snapshot-restore.md) | Restore a volume from a local snapshot | Automated (local/Ceph); manual steps for LVM/iSCSI | 2–10 min |
| [Backup Artifact Restore](backup-artifact-restore.md) | Restore a VM from a shipped S3/NFS backup artifact | **Manual** — no restore worker yet | 10–30 min |
| [Control Plane DR](control-plane-dr.md) | Recover control plane after host failure | Partially automated + manual steps | 15–30 min |
| [Full Site Recovery](full-site-recovery.md) | Rebuild entire site from backups | Manual | 1–4 hrs |

## Severity Levels

Runbooks use the following severity taxonomy:

- **SEV-1** — Complete service outage, data loss risk, or security incident. Page the on-call immediately.
- **SEV-2** — Degraded service, partial data unavailability, or single-node failure. Respond within 30 minutes.
- **SEV-3** — Non-urgent recovery, planned migration, or drill. Can be scheduled.

## Before You Begin

All runbooks assume:
- You have `ssh` access to the control plane host.
- `chvctl` is installed and configured (`/etc/chv/chvctl.toml`).
- You have `sudo` privileges on CHV hosts.
- For S3 restores: you have `aws` CLI or `s3cmd` installed with credentials for the backup bucket.

## Quick Reference: What's Automated vs Manual

| Capability | Status | How to Invoke |
|------------|--------|---------------|
| Scheduled VM backups with S3/NFS shipping | ✅ Automated | `POST /v1/backups/schedules` or UI |
| On-demand VM backup | ✅ Automated | `chvctl backup run <VM_ID>` or UI |
| Retention enforcement (count + days) | ✅ Automated | Configured per schedule |
| VM snapshot restore (local) | ✅ Automated | UI → VM → Snapshots → Restore |
| Volume snapshot restore (local/Ceph) | ✅ Automated | API `POST /v1/volumes/restore-snapshot` |
| Volume snapshot restore (LVM/iSCSI) | ❌ Not implemented | Manual disk replacement required |
| Restore from shipped S3/NFS artifact | ❌ No worker | Follow [Backup Artifact Restore](backup-artifact-restore.md) |
| Control plane DB restore | ⚠️ Manual | Follow [Control Plane DR](control-plane-dr.md) |

## See Also

- [`docs/OPERATIONS.md`](../OPERATIONS.md) — Day-2 operations, monitoring, troubleshooting
- [`docs/specs/component/backup-system.md`](../specs/component/backup-system.md) — Backup system architecture
