package backup

import (
	"archive/tar"
	"compress/gzip"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/logger"
	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/vm"
	"github.com/google/uuid"
	"github.com/robfig/cron/v3"
)

// Re-export types from models for backwards compatibility
type BackupJob = models.BackupJob
type BackupHistory = models.BackupHistory

// Service handles backup operations and scheduling
type Service struct {
	repo      *db.Repository
	vmService *vm.Service
	dataRoot  string
	cron      *cron.Cron
	jobs      map[string]cron.EntryID
	jobsMu    sync.RWMutex
}

// NewService creates a new backup service
func NewService(repo *db.Repository, vmService *vm.Service, dataRoot string) *Service {
	s := &Service{
		repo:      repo,
		vmService: vmService,
		dataRoot:  dataRoot,
		cron:      cron.New(cron.WithSeconds()),
		jobs:      make(map[string]cron.EntryID),
	}
	s.cron.Start()
	return s
}

// Stop stops the backup service and its cron scheduler
func (s *Service) Stop() {
	if s.cron != nil {
		s.cron.Stop()
	}
}

// LoadJobs loads existing backup jobs from the database and schedules them
func (s *Service) LoadJobs(ctx context.Context) error {
	jobs, err := s.repo.ListBackupJobs(ctx)
	if err != nil {
		return fmt.Errorf("failed to list backup jobs: %w", err)
	}

	for _, job := range jobs {
		if !job.Enabled {
			continue
		}
		// Schedule the job
		jobCopy := job // capture loop variable
		entryID, err := s.cron.AddFunc(job.Schedule, func() {
			if _, err := s.RunManualBackup(ctx, jobCopy.VMID); err != nil {
				logger.Error("Scheduled backup job failed", logger.ErrorField(err), logger.StringField("job_id", jobCopy.ID))
			}
		})
		if err != nil {
			logger.Error("Failed to schedule backup job", logger.ErrorField(err), logger.StringField("job_id", job.ID))
			continue
		}
		s.jobsMu.Lock()
		s.jobs[job.ID] = entryID
		s.jobsMu.Unlock()
	}

	return nil
}

// CreateJob creates a new scheduled backup job
func (s *Service) CreateJob(ctx context.Context, job *BackupJob) error {
	if job.VMID == "" {
		return fmt.Errorf("VM ID is required")
	}
	if job.Name == "" {
		return fmt.Errorf("job name is required")
	}
	if job.Schedule == "" {
		return fmt.Errorf("schedule (cron expression) is required")
	}
	if job.Retention <= 0 {
		job.Retention = 7 // default retention
	}
	if job.Destination == "" {
		job.Destination = filepath.Join(s.dataRoot, "backups")
	}

	job.ID = uuid.NewString()
	job.CreatedAt = time.Now().UTC()
	job.Enabled = true

	// Validate cron expression
	if _, err := cron.ParseStandard(job.Schedule); err != nil {
		return fmt.Errorf("invalid cron expression: %w", err)
	}

	if err := s.repo.CreateBackupJob(ctx, job); err != nil {
		return fmt.Errorf("failed to create backup job: %w", err)
	}

	// Schedule the job if enabled
	if err := s.scheduleJob(job); err != nil {
		return fmt.Errorf("failed to schedule job: %w", err)
	}

	return nil
}

// GetJob retrieves a backup job by ID
func (s *Service) GetJob(ctx context.Context, id string) (*BackupJob, error) {
	return s.repo.GetBackupJob(ctx, id)
}

// ListJobs lists all backup jobs
func (s *Service) ListJobs(ctx context.Context) ([]BackupJob, error) {
	return s.repo.ListBackupJobs(ctx)
}

// ListJobsByVM lists backup jobs for a specific VM
func (s *Service) ListJobsByVM(ctx context.Context, vmID string) ([]BackupJob, error) {
	return s.repo.ListBackupJobsByVM(ctx, vmID)
}

// UpdateJob updates a backup job
func (s *Service) UpdateJob(ctx context.Context, job *BackupJob) error {
	// Unschedule the old job
	s.unscheduleJob(job.ID)

	// Update in database
	if err := s.repo.UpdateBackupJob(ctx, job); err != nil {
		return fmt.Errorf("failed to update job: %w", err)
	}

	// Reschedule if enabled
	if job.Enabled {
		return s.scheduleJob(job)
	}

	return nil
}

// DeleteJob deletes a backup job
func (s *Service) DeleteJob(ctx context.Context, id string) error {
	s.unscheduleJob(id)
	return s.repo.DeleteBackupJob(ctx, id)
}

// EnableJob enables a backup job
func (s *Service) EnableJob(ctx context.Context, id string) error {
	job, err := s.repo.GetBackupJob(ctx, id)
	if err != nil {
		return err
	}
	if job == nil {
		return fmt.Errorf("job not found")
	}

	job.Enabled = true
	if err := s.repo.UpdateBackupJob(ctx, job); err != nil {
		return err
	}

	return s.scheduleJob(job)
}

// DisableJob disables a backup job
func (s *Service) DisableJob(ctx context.Context, id string) error {
	job, err := s.repo.GetBackupJob(ctx, id)
	if err != nil {
		return err
	}
	if job == nil {
		return fmt.Errorf("job not found")
	}

	job.Enabled = false
	if err := s.repo.UpdateBackupJob(ctx, job); err != nil {
		return err
	}

	s.unscheduleJob(id)
	return nil
}

// RunJob executes a backup job immediately (manual run)
func (s *Service) RunJob(ctx context.Context, jobID string) (*BackupHistory, error) {
	job, err := s.repo.GetBackupJob(ctx, jobID)
	if err != nil {
		return nil, err
	}
	if job == nil {
		return nil, fmt.Errorf("job not found")
	}

	return s.runBackup(ctx, job.VMID, &job.ID)
}

// RunManualBackup runs a manual backup for a VM
func (s *Service) RunManualBackup(ctx context.Context, vmID string) (*BackupHistory, error) {
	return s.runBackup(ctx, vmID, nil)
}

// runBackup performs the actual backup operation
func (s *Service) runBackup(ctx context.Context, vmID string, jobID *string) (*BackupHistory, error) {
	history := &BackupHistory{
		ID:        uuid.NewString(),
		JobID:     jobID,
		VMID:      vmID,
		Status:    "running",
		StartedAt: time.Now(),
	}

	if err := s.repo.CreateBackupHistory(ctx, history); err != nil {
		return nil, fmt.Errorf("failed to create backup history: %w", err)
	}

	// Execute backup in background
	go s.executeBackup(history)

	return history, nil
}

// executeBackup performs the actual backup work
func (s *Service) executeBackup(history *BackupHistory) {
	ctx := context.Background()

	// Get VM details
	vm, err := s.vmService.GetVM(ctx, history.VMID)
	if err != nil {
		s.completeBackup(history.ID, "", fmt.Errorf("failed to get VM: %w", err))
		return
	}

	// Create backup directory
	backupDir := filepath.Join(s.dataRoot, "backups", history.VMID)
	if err := os.MkdirAll(backupDir, 0755); err != nil {
		s.completeBackup(history.ID, "", fmt.Errorf("failed to create backup directory: %w", err))
		return
	}

	// Generate backup filename
	timestamp := time.Now().UTC().Format("20060102-150405")
	backupFile := filepath.Join(backupDir, fmt.Sprintf("%s-%s.tar.gz", vm.Name, timestamp))

	// Create snapshot first
	snapshot, err := s.vmService.CreateSnapshot(ctx, history.VMID)
	if err != nil {
		s.completeBackup(history.ID, "", fmt.Errorf("failed to create snapshot: %w", err))
		return
	}

	history.SnapshotID = snapshot.ID

	// Create tar.gz archive
	if err := s.createBackupArchive(vm, backupFile); err != nil {
		s.completeBackup(history.ID, snapshot.ID, fmt.Errorf("failed to create backup archive: %w", err))
		return
	}

	// Get file size
	info, err := os.Stat(backupFile)
	if err != nil {
		s.completeBackup(history.ID, snapshot.ID, fmt.Errorf("failed to stat backup file: %w", err))
		return
	}

	history.SizeBytes = info.Size()
	s.completeBackup(history.ID, snapshot.ID, nil)

	// Apply retention policy if this was a scheduled job
	if history.JobID != nil {
		s.applyRetention(ctx, *history.JobID, history.VMID)
	}
}

// createBackupArchive creates a compressed archive of VM files
func (s *Service) createBackupArchive(vm *models.VirtualMachine, backupFile string) error {
	file, err := os.Create(backupFile)
	if err != nil {
		return err
	}
	defer file.Close()

	gzWriter := gzip.NewWriter(file)
	defer gzWriter.Close()

	tarWriter := tar.NewWriter(gzWriter)
	defer tarWriter.Close()

	// Add disk image to archive
	if vm.DiskPath != "" {
		if err := s.addFileToArchive(tarWriter, vm.DiskPath, "disk.qcow2"); err != nil {
			return err
		}
	}

	// Add metadata
	metaData := map[string]interface{}{
		"vm_id":      vm.ID,
		"name":       vm.Name,
		"vcpu":       vm.VCPU,
		"memory_mb":  vm.MemoryMB,
		"created_at": time.Now().UTC(),
	}
	metaJSON, _ := json.Marshal(metaData)

	metaHeader := &tar.Header{
		Name: "metadata.json",
		Mode: 0644,
		Size: int64(len(metaJSON)),
	}
	if err := tarWriter.WriteHeader(metaHeader); err != nil {
		return err
	}
	if _, err := tarWriter.Write(metaJSON); err != nil {
		return err
	}

	return nil
}

// addFileToArchive adds a file to the tar archive
func (s *Service) addFileToArchive(tw *tar.Writer, filePath, arcName string) error {
	file, err := os.Open(filePath)
	if err != nil {
		return err
	}
	defer file.Close()

	info, err := file.Stat()
	if err != nil {
		return err
	}

	header, err := tar.FileInfoHeader(info, "")
	if err != nil {
		return err
	}
	header.Name = arcName

	if err := tw.WriteHeader(header); err != nil {
		return err
	}

	_, err = io.Copy(tw, file)
	return err
}

// completeBackup finalizes a backup record
func (s *Service) completeBackup(historyID, snapshotID string, err error) {
	ctx := context.Background()

	history, _ := s.repo.GetBackupHistory(ctx, historyID)
	if history == nil {
		return
	}

	now := time.Now()
	history.CompletedAt = &now
	history.SnapshotID = snapshotID

	if err != nil {
		history.Status = "failed"
		history.Error = err.Error()
		logger.L().Error("Backup failed",
			logger.F("history_id", historyID),
			logger.ErrorField(err))
	} else {
		history.Status = "completed"
		logger.L().Info("Backup completed",
			logger.F("history_id", historyID),
			logger.F("size_bytes", history.SizeBytes))
	}

	if updateErr := s.repo.UpdateBackupHistory(ctx, history); updateErr != nil {
		logger.L().Error("Failed to update backup history",
			logger.F("history_id", historyID),
			logger.ErrorField(updateErr))
	}

	// Update job last run time
	if history.JobID != nil {
		s.repo.UpdateBackupJobLastRun(ctx, *history.JobID, now)
	}
}

// applyRetention removes old backups beyond retention limit
func (s *Service) applyRetention(ctx context.Context, jobID, vmID string) {
	histories, err := s.repo.ListBackupHistoryByJob(ctx, jobID)
	if err != nil {
		logger.L().Error("Failed to list backup history for retention",
			logger.F("job_id", jobID),
			logger.ErrorField(err))
		return
	}

	job, err := s.repo.GetBackupJob(ctx, jobID)
	if err != nil || job == nil {
		return
	}

	// Sort by start time (newest first)
	sort.Slice(histories, func(i, j int) bool {
		return histories[i].StartedAt.After(histories[j].StartedAt)
	})

	// Remove excess backups
	if len(histories) > job.Retention {
		for _, h := range histories[job.Retention:] {
			if h.Status == "completed" {
				s.deleteBackup(ctx, &h)
			}
		}
	}
}

// deleteBackup removes a backup file and its record
func (s *Service) deleteBackup(ctx context.Context, history *BackupHistory) {
	// Delete backup file
	backupDir := filepath.Join(s.dataRoot, "backups", history.VMID)
	files, _ := filepath.Glob(filepath.Join(backupDir, "*.tar.gz"))
	for _, f := range files {
		if strings.Contains(f, history.StartedAt.Format("20060102-150405")) {
			os.Remove(f)
			break
		}
	}

	// Delete history record
	s.repo.DeleteBackupHistory(ctx, history.ID)
}

// scheduleJob adds a job to the cron scheduler
func (s *Service) scheduleJob(job *BackupJob) error {
	s.jobsMu.Lock()
	defer s.jobsMu.Unlock()

	// Remove existing schedule if any
	if entryID, exists := s.jobs[job.ID]; exists {
		s.cron.Remove(entryID)
	}

	entryID, err := s.cron.AddFunc(job.Schedule, func() {
		s.runBackup(context.Background(), job.VMID, &job.ID)
	})
	if err != nil {
		return err
	}

	s.jobs[job.ID] = entryID
	return nil
}

// unscheduleJob removes a job from the cron scheduler
func (s *Service) unscheduleJob(jobID string) {
	s.jobsMu.Lock()
	defer s.jobsMu.Unlock()

	if entryID, exists := s.jobs[jobID]; exists {
		s.cron.Remove(entryID)
		delete(s.jobs, jobID)
	}
}

// ListHistory lists backup history for a VM
func (s *Service) ListHistory(ctx context.Context, vmID string) ([]BackupHistory, error) {
	return s.repo.ListBackupHistory(ctx, vmID)
}

// GetHistory retrieves a backup history record
func (s *Service) GetHistory(ctx context.Context, id string) (*BackupHistory, error) {
	return s.repo.GetBackupHistory(ctx, id)
}

// DeleteHistory deletes a backup history record
func (s *Service) DeleteHistory(ctx context.Context, id string) error {
	history, err := s.repo.GetBackupHistory(ctx, id)
	if err != nil {
		return err
	}
	if history == nil {
		return fmt.Errorf("backup history not found")
	}

	s.deleteBackup(ctx, history)
	return nil
}

// GetStats returns backup statistics
func (s *Service) GetStats(ctx context.Context) (map[string]int64, error) {
	return s.repo.GetBackupStats(ctx)
}

// Restore restores a VM from backup
func (s *Service) Restore(ctx context.Context, historyID, newName string) (*models.VirtualMachine, error) {
	history, err := s.repo.GetBackupHistory(ctx, historyID)
	if err != nil {
		return nil, err
	}
	if history == nil {
		return nil, fmt.Errorf("backup not found")
	}

	if history.Status != "completed" {
		return nil, fmt.Errorf("cannot restore from incomplete backup")
	}

	// Get backup file path
	backupDir := filepath.Join(s.dataRoot, "backups", history.VMID)
	var backupFile string
	files, _ := filepath.Glob(filepath.Join(backupDir, "*.tar.gz"))
	for _, f := range files {
		if strings.Contains(f, history.StartedAt.Format("20060102-150405")) {
			backupFile = f
			break
		}
	}

	if backupFile == "" {
		return nil, fmt.Errorf("backup file not found")
	}

	// Restore from snapshot if available
	if history.SnapshotID != "" {
		// Get original VM to find workspace
		origVM, err := s.vmService.GetVM(ctx, history.VMID)
		if err != nil {
			return nil, fmt.Errorf("failed to get original VM: %w", err)
		}

		// Restore the snapshot
		if err := s.vmService.RestoreSnapshot(ctx, history.VMID, history.SnapshotID); err != nil {
			return nil, fmt.Errorf("failed to restore snapshot: %w", err)
		}

		// If new name is different, clone it
		if newName != "" && newName != origVM.Name {
			cloneInput := vm.CloneVMInput{
				Name:       newName,
				SourceVMID: origVM.ID,
			}
			clonedVM, err := s.vmService.CloneVM(ctx, cloneInput)
			if err != nil {
				return nil, fmt.Errorf("failed to clone VM: %w", err)
			}
			return clonedVM, nil
		}

		return origVM, nil
	}

	return nil, fmt.Errorf("backup does not have a valid snapshot")
}

// Initialize loads and schedules all enabled backup jobs
func (s *Service) Initialize(ctx context.Context) error {
	jobs, err := s.repo.ListBackupJobs(ctx)
	if err != nil {
		return fmt.Errorf("failed to list backup jobs: %w", err)
	}

	for _, job := range jobs {
		if job.Enabled {
			if err := s.scheduleJob(&job); err != nil {
				logger.L().Error("Failed to schedule backup job",
					logger.F("job_id", job.ID),
					logger.ErrorField(err))
			}
		}
	}

	logger.L().Info("Backup service initialized", logger.F("jobs_scheduled", len(s.jobs)))
	return nil
}

// RunBackup runs a backup for a VM (manual or scheduled)
func (s *Service) RunBackup(ctx context.Context, vmID string, jobID *string) (*BackupHistory, error) {
	if jobID != nil {
		return s.RunJob(ctx, *jobID)
	}
	return s.RunManualBackup(ctx, vmID)
}

// ListBackupHistory lists backup history for a VM (alias for ListHistory)
func (s *Service) ListBackupHistory(ctx context.Context, vmID string) ([]BackupHistory, error) {
	return s.ListHistory(ctx, vmID)
}

// ListAllBackupHistory lists all backup history across all VMs
func (s *Service) ListAllBackupHistory(ctx context.Context) ([]BackupHistory, error) {
	return s.repo.ListAllBackupHistory(ctx)
}

// ExportVM exports a VM to a file and returns the path
func (s *Service) ExportVM(ctx context.Context, vmID string) (string, error) {
	vm, err := s.vmService.GetVM(ctx, vmID)
	if err != nil {
		return "", fmt.Errorf("failed to get VM: %w", err)
	}
	if vm == nil {
		return "", fmt.Errorf("VM not found: %s", vmID)
	}

	exportDir := filepath.Join(s.dataRoot, "exports")
	if err := os.MkdirAll(exportDir, 0755); err != nil {
		return "", fmt.Errorf("failed to create export directory: %w", err)
	}

	timestamp := time.Now().UTC().Format("20060102-150405")
	exportPath := filepath.Join(exportDir, fmt.Sprintf("%s-export-%s.tar.gz", vm.Name, timestamp))

	if err := s.createBackupArchive(vm, exportPath); err != nil {
		return "", fmt.Errorf("failed to create export: %w", err)
	}

	return exportPath, nil
}

// GetExportFilePath returns the file path for an export by ID (filename pattern)
func (s *Service) GetExportFilePath(exportID string) (string, error) {
	exportDir := filepath.Join(s.dataRoot, "exports")
	files, err := filepath.Glob(filepath.Join(exportDir, "*"+exportID+"*.tar.gz"))
	if err != nil {
		return "", err
	}
	if len(files) == 0 {
		return "", fmt.Errorf("export not found: %s", exportID)
	}
	return files[0], nil
}

// ToggleJobEnabled enables or disables a backup job
func (s *Service) ToggleJobEnabled(ctx context.Context, id string, enabled bool) error {
	job, err := s.GetJob(ctx, id)
	if err != nil {
		return err
	}
	if job == nil {
		return fmt.Errorf("job not found")
	}

	job.Enabled = enabled
	if err := s.repo.UpdateBackupJob(ctx, job); err != nil {
		return err
	}

	if enabled {
		return s.scheduleJob(job)
	}
	s.unscheduleJob(id)
	return nil
}

// ImportVM imports a VM from an export file
func (s *Service) ImportVM(ctx context.Context, name string, exportPath string) (*models.VirtualMachine, error) {
	file, err := os.Open(exportPath)
	if err != nil {
		return nil, fmt.Errorf("failed to open export file: %w", err)
	}
	defer file.Close()

	gzipReader, err := gzip.NewReader(file)
	if err != nil {
		return nil, fmt.Errorf("failed to create gzip reader: %w", err)
	}
	defer gzipReader.Close()

	tarReader := tar.NewReader(gzipReader)

	var metadata map[string]interface{}
	var diskData []byte

	for {
		header, err := tarReader.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return nil, fmt.Errorf("failed to read tar: %w", err)
		}

		switch header.Name {
		case "metadata.json":
			metadataBytes := make([]byte, header.Size)
			if _, err := io.ReadFull(tarReader, metadataBytes); err != nil {
				return nil, fmt.Errorf("failed to read metadata: %w", err)
			}
			if err := json.Unmarshal(metadataBytes, &metadata); err != nil {
				return nil, fmt.Errorf("failed to parse metadata: %w", err)
			}
		case "disk.qcow2":
			diskData = make([]byte, header.Size)
			if _, err := io.ReadFull(tarReader, diskData); err != nil {
				return nil, fmt.Errorf("failed to read disk: %w", err)
			}
		}
	}

	if metadata == nil {
		return nil, fmt.Errorf("metadata.json not found in export")
	}

	// Create new VM with imported settings
	vcpu := 2
	memoryMB := 2048
	imageID := ""
	networkID := ""
	storagePoolID := ""

	if v, ok := metadata["vcpu"].(float64); ok {
		vcpu = int(v)
	}
	if v, ok := metadata["memory_mb"].(float64); ok {
		memoryMB = int(v)
	}
	if v, ok := metadata["image_id"].(string); ok {
		imageID = v
	}
	if v, ok := metadata["network_id"].(string); ok {
		networkID = v
	}
	if v, ok := metadata["storage_pool_id"].(string); ok {
		storagePoolID = v
	}

	// Use defaults if not found in metadata
	if imageID == "" {
		images, _ := s.repo.ListImages(ctx)
		if len(images) > 0 {
			imageID = images[0].ID
		}
	}
	if networkID == "" {
		networks, _ := s.repo.ListNetworks(ctx)
		if len(networks) > 0 {
			networkID = networks[0].ID
		}
	}
	if storagePoolID == "" {
		pools, _ := s.repo.ListStoragePools(ctx)
		if len(pools) > 0 {
			storagePoolID = pools[0].ID
		}
	}

	input := vm.CreateVMInput{
		Name:          name,
		ImageID:       imageID,
		StoragePoolID: storagePoolID,
		NetworkID:     networkID,
		VCPU:          vcpu,
		MemoryMB:      memoryMB,
	}

	newVM, err := s.vmService.CreateVM(ctx, input)
	if err != nil {
		return nil, fmt.Errorf("failed to create VM: %w", err)
	}

	// Replace disk with imported data
	if diskData != nil {
		if err := os.WriteFile(newVM.DiskPath, diskData, 0644); err != nil {
			// Cleanup on failure
			s.vmService.DeleteVM(ctx, newVM.ID)
			return nil, fmt.Errorf("failed to write imported disk: %w", err)
		}
	}

	return newVM, nil
}
