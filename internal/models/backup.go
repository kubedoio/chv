package models

import "time"

// BackupJob represents a scheduled backup configuration
type BackupJob struct {
	ID          string     `json:"id"`
	VMID        string     `json:"vm_id"`
	Name        string     `json:"name"`
	Schedule    string     `json:"schedule"` // cron expression: "0 2 * * *" = daily at 2am
	Retention   int        `json:"retention"` // keep last N backups
	Destination string     `json:"destination"`
	Enabled     bool       `json:"enabled"`
	LastRun     *time.Time `json:"last_run,omitempty"`
	NextRun     *time.Time `json:"next_run,omitempty"`
	CreatedAt   time.Time  `json:"created_at"`
	VMName      string     `json:"vm_name,omitempty"` // populated on retrieval
}

// BackupHistory represents a single backup operation record
type BackupHistory struct {
	ID          string     `json:"id"`
	JobID       *string    `json:"job_id,omitempty"` // nil for manual backups
	VMID        string     `json:"vm_id"`
	SnapshotID  string     `json:"snapshot_id"`
	Status      string     `json:"status"` // running, completed, failed
	SizeBytes   int64      `json:"size_bytes"`
	StartedAt   time.Time  `json:"started_at"`
	CompletedAt *time.Time `json:"completed_at,omitempty"`
	Error       string     `json:"error,omitempty"`
	VMName      string     `json:"vm_name,omitempty"` // populated on retrieval
}
