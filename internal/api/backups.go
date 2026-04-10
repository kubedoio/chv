package api

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/chv/chv/internal/logger"
	"github.com/chv/chv/internal/models"
	"github.com/go-chi/chi/v5"
)

// BackupJobRequest represents a request to create/update a backup job
type BackupJobRequest struct {
	VMID        string `json:"vm_id"`
	Name        string `json:"name"`
	Schedule    string `json:"schedule"`
	Retention   int    `json:"retention"`
	Destination string `json:"destination"`
}

// BackupJobResponse wraps a backup job with additional metadata
type BackupJobResponse struct {
	*models.BackupJob
	NextRunFormatted string `json:"next_run_formatted,omitempty"`
	LastRunFormatted string `json:"last_run_formatted,omitempty"`
}

// ImportVMRequest represents a request to import a VM
type ImportVMRequest struct {
	Name string `json:"name"`
}

// listBackupJobs handles GET /api/v1/backup-jobs
func (h *Handler) listBackupJobs(w http.ResponseWriter, r *http.Request) {
	if h.backupService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:    "service_unavailable",
			Message: "Backup service is not available",
		})
		return
	}

	jobs, err := h.backupService.ListJobs(r.Context())
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "list_failed",
			Message: fmt.Sprintf("Failed to list backup jobs: %v", err),
		})
		return
	}

	// Format response
	responses := make([]BackupJobResponse, len(jobs))
	for i, job := range jobs {
		resp := BackupJobResponse{BackupJob: &job}
		if job.NextRun != nil {
			resp.NextRunFormatted = formatDuration(time.Until(*job.NextRun))
		}
		if job.LastRun != nil {
			resp.LastRunFormatted = formatTimeAgo(*job.LastRun)
		}
		responses[i] = resp
	}

	h.writeJSON(w, http.StatusOK, responses)
}

// createBackupJob handles POST /api/v1/backup-jobs
func (h *Handler) createBackupJob(w http.ResponseWriter, r *http.Request) {
	if h.backupService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:    "service_unavailable",
			Message: "Backup service is not available",
		})
		return
	}

	var req BackupJobRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:    "invalid_request",
			Message: fmt.Sprintf("Failed to decode request: %v", err),
		})
		return
	}

	// Validate required fields
	if req.VMID == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:    "missing_field",
			Message: "vm_id is required",
		})
		return
	}
	if req.Name == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:    "missing_field",
			Message: "name is required",
		})
		return
	}
	if req.Schedule == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:    "missing_field",
			Message: "schedule is required (cron expression)",
		})
		return
	}

	job := &models.BackupJob{
		VMID:        req.VMID,
		Name:        req.Name,
		Schedule:    req.Schedule,
		Retention:   req.Retention,
		Destination: req.Destination,
	}

	if err := h.backupService.CreateJob(r.Context(), job); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "create_failed",
			Message: fmt.Sprintf("Failed to create backup job: %v", err),
		})
		return
	}

	h.writeJSON(w, http.StatusCreated, job)
}

// getBackupJob handles GET /api/v1/backup-jobs/{id}
func (h *Handler) getBackupJob(w http.ResponseWriter, r *http.Request) {
	if h.backupService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:    "service_unavailable",
			Message: "Backup service is not available",
		})
		return
	}

	id := chi.URLParam(r, "id")
	job, err := h.backupService.GetJob(r.Context(), id)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "get_failed",
			Message: fmt.Sprintf("Failed to get backup job: %v", err),
		})
		return
	}
	if job == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:    "not_found",
			Message: "Backup job not found",
		})
		return
	}

	resp := BackupJobResponse{BackupJob: job}
	if job.NextRun != nil {
		resp.NextRunFormatted = formatDuration(time.Until(*job.NextRun))
	}
	if job.LastRun != nil {
		resp.LastRunFormatted = formatTimeAgo(*job.LastRun)
	}

	h.writeJSON(w, http.StatusOK, resp)
}

// deleteBackupJob handles DELETE /api/v1/backup-jobs/{id}
func (h *Handler) deleteBackupJob(w http.ResponseWriter, r *http.Request) {
	if h.backupService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:    "service_unavailable",
			Message: "Backup service is not available",
		})
		return
	}

	id := chi.URLParam(r, "id")
	if err := h.backupService.DeleteJob(r.Context(), id); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "delete_failed",
			Message: fmt.Sprintf("Failed to delete backup job: %v", err),
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]bool{"success": true})
}

// runBackupJob handles POST /api/v1/backup-jobs/{id}/run
func (h *Handler) runBackupJob(w http.ResponseWriter, r *http.Request) {
	if h.backupService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:    "service_unavailable",
			Message: "Backup service is not available",
		})
		return
	}

	id := chi.URLParam(r, "id")
	job, err := h.backupService.GetJob(r.Context(), id)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "get_failed",
			Message: fmt.Sprintf("Failed to get backup job: %v", err),
		})
		return
	}
	if job == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:    "not_found",
			Message: "Backup job not found",
		})
		return
	}

	history, err := h.backupService.RunBackup(r.Context(), job.VMID, &id)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "run_failed",
			Message: fmt.Sprintf("Failed to run backup: %v", err),
		})
		return
	}

	h.writeJSON(w, http.StatusAccepted, history)
}

// listVMBackups handles GET /api/v1/vms/{id}/backups
func (h *Handler) listVMBackups(w http.ResponseWriter, r *http.Request) {
	if h.backupService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:    "service_unavailable",
			Message: "Backup service is not available",
		})
		return
	}

	vmID := chi.URLParam(r, "id")
	histories, err := h.backupService.ListBackupHistory(r.Context(), vmID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "list_failed",
			Message: fmt.Sprintf("Failed to list backups: %v", err),
		})
		return
	}

	h.writeJSON(w, http.StatusOK, histories)
}

// exportVM handles POST /api/v1/vms/{id}/export
func (h *Handler) exportVM(w http.ResponseWriter, r *http.Request) {
	if h.backupService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:    "service_unavailable",
			Message: "Backup service is not available",
		})
		return
	}

	vmID := chi.URLParam(r, "id")
	exportPath, err := h.backupService.ExportVM(r.Context(), vmID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "export_failed",
			Message: fmt.Sprintf("Failed to export VM: %v", err),
		})
		return
	}

	// Generate export ID from filename
	filename := filepath.Base(exportPath)
	exportID := strings.TrimSuffix(filename, filepath.Ext(filename))

	h.writeJSON(w, http.StatusAccepted, map[string]string{
		"export_id":   exportID,
		"filename":    filename,
		"status":      "processing",
		"download_url": fmt.Sprintf("/api/v1/exports/%s/download", exportID),
	})
}

// downloadExport handles GET /api/v1/exports/{id}/download
func (h *Handler) downloadExport(w http.ResponseWriter, r *http.Request) {
	if h.backupService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:    "service_unavailable",
			Message: "Backup service is not available",
		})
		return
	}

	exportID := chi.URLParam(r, "id")
	exportPath, err := h.backupService.GetExportFilePath(exportID)
	if err != nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:    "not_found",
			Message: "Export not found",
		})
		return
	}

	// Open file
	file, err := os.Open(exportPath)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "open_failed",
			Message: fmt.Sprintf("Failed to open export file: %v", err),
		})
		return
	}
	defer file.Close()

	// Get file info
	info, err := file.Stat()
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "stat_failed",
			Message: fmt.Sprintf("Failed to stat export file: %v", err),
		})
		return
	}

	// Set headers
	w.Header().Set("Content-Type", "application/gzip")
	w.Header().Set("Content-Disposition", fmt.Sprintf("attachment; filename=\"%s\"", filepath.Base(exportPath)))
	w.Header().Set("Content-Length", fmt.Sprintf("%d", info.Size()))

	// Stream file
	if _, err := io.Copy(w, file); err != nil {
		logger.L().Error("Failed to stream export file", logger.ErrorField(err))
	}
}

// importVM handles POST /api/v1/vms/import
func (h *Handler) importVM(w http.ResponseWriter, r *http.Request) {
	if h.backupService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:    "service_unavailable",
			Message: "Backup service is not available",
		})
		return
	}

	// Parse multipart form
	if err := r.ParseMultipartForm(10 << 30); err != nil { // 10GB max
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:    "parse_failed",
			Message: fmt.Sprintf("Failed to parse multipart form: %v", err),
		})
		return
	}

	// Get file
	file, header, err := r.FormFile("file")
	if err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:    "file_required",
			Message: "Export file is required",
		})
		return
	}
	defer file.Close()

	// Get VM name
	vmName := r.FormValue("name")
	if vmName == "" {
		// Use filename as default
		vmName = strings.TrimSuffix(header.Filename, filepath.Ext(header.Filename))
		vmName = strings.TrimSuffix(vmName, filepath.Ext(vmName)) // Remove .tar if .tar.gz
	}

	// Save to temp file
	tempDir := os.TempDir()
	tempPath := filepath.Join(tempDir, fmt.Sprintf("import-%d.tar.gz", time.Now().UnixNano()))
	
	out, err := os.Create(tempPath)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "temp_file_failed",
			Message: fmt.Sprintf("Failed to create temp file: %v", err),
		})
		return
	}
	defer os.Remove(tempPath) // Cleanup

	if _, err := io.Copy(out, file); err != nil {
		out.Close()
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "save_failed",
			Message: fmt.Sprintf("Failed to save uploaded file: %v", err),
		})
		return
	}
	out.Close()

	// Import VM
	vm, err := h.backupService.ImportVM(r.Context(), vmName, tempPath)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "import_failed",
			Message: fmt.Sprintf("Failed to import VM: %v", err),
		})
		return
	}

	h.writeJSON(w, http.StatusCreated, vm)
}

// toggleBackupJob handles POST /api/v1/backup-jobs/{id}/toggle
func (h *Handler) toggleBackupJob(w http.ResponseWriter, r *http.Request) {
	if h.backupService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:    "service_unavailable",
			Message: "Backup service is not available",
		})
		return
	}

	id := chi.URLParam(r, "id")
	job, err := h.backupService.GetJob(r.Context(), id)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "get_failed",
			Message: fmt.Sprintf("Failed to get backup job: %v", err),
		})
		return
	}
	if job == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:    "not_found",
			Message: "Backup job not found",
		})
		return
	}

	// Toggle enabled state
	newState := !job.Enabled
	if err := h.backupService.ToggleJobEnabled(r.Context(), id, newState); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:    "toggle_failed",
			Message: fmt.Sprintf("Failed to toggle backup job: %v", err),
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]interface{}{
		"success":  true,
		"enabled":  newState,
		"job_id":   id,
	})
}

// Helper functions for formatting
func formatDuration(d time.Duration) string {
	if d < 0 {
		return "overdue"
	}
	if d < time.Minute {
		return "in a few seconds"
	}
	if d < time.Hour {
		return fmt.Sprintf("in %d minutes", int(d.Minutes()))
	}
	if d < 24*time.Hour {
		return fmt.Sprintf("in %d hours", int(d.Hours()))
	}
	return fmt.Sprintf("in %d days", int(d.Hours()/24))
}

func formatTimeAgo(t time.Time) string {
	d := time.Since(t)
	if d < time.Minute {
		return "just now"
	}
	if d < time.Hour {
		return fmt.Sprintf("%d minutes ago", int(d.Minutes()))
	}
	if d < 24*time.Hour {
		return fmt.Sprintf("%d hours ago", int(d.Hours()))
	}
	return fmt.Sprintf("%d days ago", int(d.Hours()/24))
}
