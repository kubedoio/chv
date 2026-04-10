package vm

import (
	"context"
	"fmt"
	"time"

	"github.com/chv/chv/internal/agentapi"
	"github.com/chv/chv/internal/agentclient"
	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/models"
)

// BootLogger handles boot log capture and retrieval
type BootLogger struct {
	repo        *db.Repository
	agentClient *agentclient.Client
}

// NewBootLogger creates a new boot logger
func NewBootLogger(repo *db.Repository) *BootLogger {
	return &BootLogger{
		repo: repo,
	}
}

// SetAgentClient sets the agent client for fetching logs
func (l *BootLogger) SetAgentClient(client *agentclient.Client) {
	l.agentClient = client
}

// CaptureLogs fetches and stores boot logs from the agent
func (l *BootLogger) CaptureLogs(ctx context.Context, vmID string, pid int) error {
	if l.agentClient == nil {
		return fmt.Errorf("agent client not available")
	}

	req := &agentapi.VMBootLogRequest{
		VMID:  vmID,
		Lines: 0, // Get all lines
	}

	resp, err := l.agentClient.GetBootLogs(ctx, req)
	if err != nil {
		return fmt.Errorf("failed to fetch boot logs from agent: %w", err)
	}

	// Store logs in database
	for _, line := range resp.Lines {
		entry := &models.VMBootLogEntry{
			LineNumber: line.LineNumber,
			Content:    line.Content,
			Timestamp:  parseTimestamp(line.Timestamp),
		}
		if err := l.repo.CreateVMBootLog(ctx, vmID, entry); err != nil {
			// Log but don't fail - continue storing remaining logs
			fmt.Printf("Warning: failed to store boot log line %d: %v\n", line.LineNumber, err)
		}
	}

	return nil
}

// GetLogs retrieves boot logs for a VM
func (l *BootLogger) GetLogs(ctx context.Context, vmID string, lines int) ([]models.VMBootLogEntry, error) {
	return l.repo.GetVMBootLogs(ctx, vmID, lines)
}

// StartCapture begins polling for boot logs during VM startup
func (l *BootLogger) StartCapture(ctx context.Context, vmID string, pid int) {
	// Clear old logs for this VM
	_ = l.repo.ClearVMBootLogs(ctx, vmID)

	// Start polling in background
	go l.pollLogs(ctx, vmID, pid)
}

// pollLogs periodically fetches logs from the agent
func (l *BootLogger) pollLogs(ctx context.Context, vmID string, pid int) {
	ticker := time.NewTicker(5 * time.Second)
	defer ticker.Stop()

	// Poll for up to 10 minutes (VM should boot within this time)
	timeout := time.After(10 * time.Minute)

	for {
		select {
		case <-ticker.C:
			if err := l.CaptureLogs(ctx, vmID, pid); err != nil {
				// VM may have stopped or logs not available
				return
			}
		case <-timeout:
			return
		case <-ctx.Done():
			return
		}
	}
}

func parseTimestamp(ts string) time.Time {
	t, err := time.Parse(time.RFC3339, ts)
	if err != nil {
		return time.Now()
	}
	return t
}
