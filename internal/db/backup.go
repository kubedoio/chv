package db

import (
	"context"
	"database/sql"
	"fmt"
	"time"

	"github.com/chv/chv/internal/models"
)

// CreateBackupJob creates a new backup job record
func (r *Repository) CreateBackupJob(ctx context.Context, job *models.BackupJob) error {
	var lastRun, nextRun interface{}
	if job.LastRun != nil {
		lastRun = job.LastRun.UTC().Format(time.RFC3339)
	}
	if job.NextRun != nil {
		nextRun = job.NextRun.UTC().Format(time.RFC3339)
	}

	_, err := r.db.ExecContext(ctx,
		`INSERT INTO backup_jobs (id, vm_id, name, schedule, retention, destination, enabled, last_run, next_run, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		job.ID,
		job.VMID,
		job.Name,
		job.Schedule,
		job.Retention,
		job.Destination,
		boolInt(job.Enabled),
		lastRun,
		nextRun,
		job.CreatedAt.UTC().Format(time.RFC3339),
	)
	return err
}

// GetBackupJob retrieves a backup job by ID
func (r *Repository) GetBackupJob(ctx context.Context, id string) (*models.BackupJob, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT j.id, j.vm_id, j.name, j.schedule, j.retention, j.destination, j.enabled, j.last_run, j.next_run, j.created_at, v.name as vm_name
		 FROM backup_jobs j
		 LEFT JOIN virtual_machines v ON j.vm_id = v.id
		 WHERE j.id = ?`, id)

	return scanBackupJob(row)
}

// ListBackupJobs retrieves all backup jobs
func (r *Repository) ListBackupJobs(ctx context.Context) ([]models.BackupJob, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT j.id, j.vm_id, j.name, j.schedule, j.retention, j.destination, j.enabled, j.last_run, j.next_run, j.created_at, v.name as vm_name
		 FROM backup_jobs j
		 LEFT JOIN virtual_machines v ON j.vm_id = v.id
		 ORDER BY j.created_at DESC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var jobs []models.BackupJob
	for rows.Next() {
		job, err := scanBackupJob(rows)
		if err != nil {
			return nil, err
		}
		jobs = append(jobs, *job)
	}
	return jobs, rows.Err()
}

// ListBackupJobsByVM retrieves backup jobs for a specific VM
func (r *Repository) ListBackupJobsByVM(ctx context.Context, vmID string) ([]models.BackupJob, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT j.id, j.vm_id, j.name, j.schedule, j.retention, j.destination, j.enabled, j.last_run, j.next_run, j.created_at, v.name as vm_name
		 FROM backup_jobs j
		 LEFT JOIN virtual_machines v ON j.vm_id = v.id
		 WHERE j.vm_id = ?
		 ORDER BY j.created_at DESC`, vmID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var jobs []models.BackupJob
	for rows.Next() {
		job, err := scanBackupJob(rows)
		if err != nil {
			return nil, err
		}
		jobs = append(jobs, *job)
	}
	return jobs, rows.Err()
}

// UpdateBackupJob updates a backup job
func (r *Repository) UpdateBackupJob(ctx context.Context, job *models.BackupJob) error {
	var lastRun, nextRun interface{}
	if job.LastRun != nil {
		lastRun = job.LastRun.UTC().Format(time.RFC3339)
	}
	if job.NextRun != nil {
		nextRun = job.NextRun.UTC().Format(time.RFC3339)
	}

	_, err := r.db.ExecContext(ctx,
		`UPDATE backup_jobs SET name = ?, schedule = ?, retention = ?, destination = ?, enabled = ?, last_run = ?, next_run = ?
		 WHERE id = ?`,
		job.Name,
		job.Schedule,
		job.Retention,
		job.Destination,
		boolInt(job.Enabled),
		lastRun,
		nextRun,
		job.ID,
	)
	return err
}

// UpdateBackupJobLastRun updates the last run time of a backup job
func (r *Repository) UpdateBackupJobLastRun(ctx context.Context, id string, lastRun time.Time) error {
	_, err := r.db.ExecContext(ctx,
		`UPDATE backup_jobs SET last_run = ? WHERE id = ?`,
		lastRun.UTC().Format(time.RFC3339),
		id,
	)
	return err
}

// DeleteBackupJob deletes a backup job
func (r *Repository) DeleteBackupJob(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM backup_jobs WHERE id = ?`, id)
	return err
}

// CreateBackupHistory creates a backup history record
func (r *Repository) CreateBackupHistory(ctx context.Context, history *models.BackupHistory) error {
	var jobID interface{}
	if history.JobID != nil {
		jobID = *history.JobID
	}

	_, err := r.db.ExecContext(ctx,
		`INSERT INTO backup_history (id, job_id, vm_id, snapshot_id, status, size_bytes, started_at, completed_at, error)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		history.ID,
		jobID,
		history.VMID,
		history.SnapshotID,
		history.Status,
		history.SizeBytes,
		history.StartedAt.UTC().Format(time.RFC3339),
		nil,
		nil,
	)
	return err
}

// UpdateBackupHistory updates a backup history record
func (r *Repository) UpdateBackupHistory(ctx context.Context, history *models.BackupHistory) error {
	var completedAt interface{}
	if history.CompletedAt != nil {
		completedAt = history.CompletedAt.UTC().Format(time.RFC3339)
	}

	_, err := r.db.ExecContext(ctx,
		`UPDATE backup_history SET status = ?, size_bytes = ?, completed_at = ?, error = ?, snapshot_id = ?
		 WHERE id = ?`,
		history.Status,
		history.SizeBytes,
		completedAt,
		history.Error,
		history.SnapshotID,
		history.ID,
	)
	return err
}

// GetBackupHistory retrieves a backup history record by ID
func (r *Repository) GetBackupHistory(ctx context.Context, id string) (*models.BackupHistory, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT h.id, h.job_id, h.vm_id, h.snapshot_id, h.status, h.size_bytes, h.started_at, h.completed_at, h.error, v.name as vm_name
		 FROM backup_history h
		 LEFT JOIN virtual_machines v ON h.vm_id = v.id
		 WHERE h.id = ?`, id)

	return scanBackupHistory(row)
}

// ListBackupHistory retrieves backup history for a VM
func (r *Repository) ListBackupHistory(ctx context.Context, vmID string) ([]models.BackupHistory, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT h.id, h.job_id, h.vm_id, h.snapshot_id, h.status, h.size_bytes, h.started_at, h.completed_at, h.error, v.name as vm_name
		 FROM backup_history h
		 LEFT JOIN virtual_machines v ON h.vm_id = v.id
		 WHERE h.vm_id = ?
		 ORDER BY h.started_at DESC`, vmID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var histories []models.BackupHistory
	for rows.Next() {
		history, err := scanBackupHistory(rows)
		if err != nil {
			return nil, err
		}
		histories = append(histories, *history)
	}
	return histories, rows.Err()
}

// ListBackupHistoryByJob retrieves backup history for a job
func (r *Repository) ListBackupHistoryByJob(ctx context.Context, jobID string) ([]models.BackupHistory, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT h.id, h.job_id, h.vm_id, h.snapshot_id, h.status, h.size_bytes, h.started_at, h.completed_at, h.error, v.name as vm_name
		 FROM backup_history h
		 LEFT JOIN virtual_machines v ON h.vm_id = v.id
		 WHERE h.job_id = ?
		 ORDER BY h.started_at DESC`, jobID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var histories []models.BackupHistory
	for rows.Next() {
		history, err := scanBackupHistory(rows)
		if err != nil {
			return nil, err
		}
		histories = append(histories, *history)
	}
	return histories, rows.Err()
}

// ListAllBackupHistory retrieves all backup history
func (r *Repository) ListAllBackupHistory(ctx context.Context) ([]models.BackupHistory, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT h.id, h.job_id, h.vm_id, h.snapshot_id, h.status, h.size_bytes, h.started_at, h.completed_at, h.error, v.name as vm_name
		 FROM backup_history h
		 LEFT JOIN virtual_machines v ON h.vm_id = v.id
		 ORDER BY h.started_at DESC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var histories []models.BackupHistory
	for rows.Next() {
		history, err := scanBackupHistory(rows)
		if err != nil {
			return nil, err
		}
		histories = append(histories, *history)
	}
	return histories, rows.Err()
}

// DeleteBackupHistory deletes a backup history record
func (r *Repository) DeleteBackupHistory(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM backup_history WHERE id = ?`, id)
	return err
}

// scanner interface for scanning rows
type scanner interface {
	Scan(dest ...any) error
}

// scanBackupJob scans a backup job from a row
func scanBackupJob(row scanner) (*models.BackupJob, error) {
	var job models.BackupJob
	var enabled int
	var lastRun, nextRun sql.NullString
	var vmName sql.NullString
	var createdAt string

	if err := row.Scan(
		&job.ID,
		&job.VMID,
		&job.Name,
		&job.Schedule,
		&job.Retention,
		&job.Destination,
		&enabled,
		&lastRun,
		&nextRun,
		&createdAt,
		&vmName,
	); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}

	job.Enabled = enabled == 1
	if lastRun.Valid {
		t, _ := time.Parse(time.RFC3339, lastRun.String)
		job.LastRun = &t
	}
	if nextRun.Valid {
		t, _ := time.Parse(time.RFC3339, nextRun.String)
		job.NextRun = &t
	}
	if t, err := time.Parse(time.RFC3339, createdAt); err == nil {
		job.CreatedAt = t
	}
	if vmName.Valid {
		job.VMName = vmName.String
	}

	return &job, nil
}

// scanBackupHistory scans a backup history from a row
func scanBackupHistory(row scanner) (*models.BackupHistory, error) {
	var history models.BackupHistory
	var jobID sql.NullString
	var sizeBytes sql.NullInt64
	var completedAt sql.NullString
	var errorStr sql.NullString
	var vmName sql.NullString
	var startedAt string

	if err := row.Scan(
		&history.ID,
		&jobID,
		&history.VMID,
		&history.SnapshotID,
		&history.Status,
		&sizeBytes,
		&startedAt,
		&completedAt,
		&errorStr,
		&vmName,
	); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}

	if jobID.Valid {
		history.JobID = &jobID.String
	}
	if sizeBytes.Valid {
		history.SizeBytes = sizeBytes.Int64
	}
	if t, err := time.Parse(time.RFC3339, startedAt); err == nil {
		history.StartedAt = t
	}
	if completedAt.Valid {
		t, _ := time.Parse(time.RFC3339, completedAt.String)
		history.CompletedAt = &t
	}
	if errorStr.Valid {
		history.Error = errorStr.String
	}
	if vmName.Valid {
		history.VMName = vmName.String
	}

	return &history, nil
}

// GetBackupStats returns statistics about backups
func (r *Repository) GetBackupStats(ctx context.Context) (map[string]int64, error) {
	stats := make(map[string]int64)

	// Total jobs
	var totalJobs int64
	err := r.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM backup_jobs`).Scan(&totalJobs)
	if err != nil {
		return nil, fmt.Errorf("failed to count jobs: %w", err)
	}
	stats["total_jobs"] = totalJobs

	// Enabled jobs
	var enabledJobs int64
	err = r.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM backup_jobs WHERE enabled = 1`).Scan(&enabledJobs)
	if err != nil {
		return nil, fmt.Errorf("failed to count enabled jobs: %w", err)
	}
	stats["enabled_jobs"] = enabledJobs

	// Total backups
	var totalBackups int64
	err = r.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM backup_history`).Scan(&totalBackups)
	if err != nil {
		return nil, fmt.Errorf("failed to count backups: %w", err)
	}
	stats["total_backups"] = totalBackups

	// Successful backups
	var successfulBackups int64
	err = r.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM backup_history WHERE status = 'completed'`).Scan(&successfulBackups)
	if err != nil {
		return nil, fmt.Errorf("failed to count successful backups: %w", err)
	}
	stats["successful_backups"] = successfulBackups

	// Failed backups
	var failedBackups int64
	err = r.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM backup_history WHERE status = 'failed'`).Scan(&failedBackups)
	if err != nil {
		return nil, fmt.Errorf("failed to count failed backups: %w", err)
	}
	stats["failed_backups"] = failedBackups

	return stats, nil
}
