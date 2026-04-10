package audit

import (
	"context"
	"sync"
	"time"

	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
)

// Logger handles audit logging with buffering and background flushing
type Logger struct {
	repo   *db.Repository
	buffer []models.AuditLog
	mu     sync.Mutex
	ticker *time.Ticker
	stop   chan struct{}
}

// NewLogger creates a new audit logger with background flush
func NewLogger(repo *db.Repository) *Logger {
	l := &Logger{
		repo:   repo,
		buffer: make([]models.AuditLog, 0, 100),
		ticker: time.NewTicker(5 * time.Second),
		stop:   make(chan struct{}),
	}
	go l.flushLoop()
	return l
}

// Stop stops the background flush goroutine
func (l *Logger) Stop() {
	close(l.stop)
	l.ticker.Stop()
	l.Flush()
}

// Log creates an audit log entry
func (l *Logger) Log(userID, userName, action, resourceType, resourceID, details, ipAddress string, success bool, err error) {
	log := models.AuditLog{
		ID:           uuid.NewString(),
		UserID:       userID,
		UserName:     userName,
		Action:       action,
		ResourceType: resourceType,
		ResourceID:   resourceID,
		Details:      details,
		IPAddress:    ipAddress,
		Success:      success,
		CreatedAt:    time.Now().UTC().Format(time.RFC3339),
	}

	if err != nil {
		log.Error = err.Error()
	}

	l.mu.Lock()
	l.buffer = append(l.buffer, log)
	shouldFlush := len(l.buffer) >= 50
	l.mu.Unlock()

	if shouldFlush {
		l.Flush()
	}
}

// Flush writes all buffered logs to the database
func (l *Logger) Flush() error {
	l.mu.Lock()
	if len(l.buffer) == 0 {
		l.mu.Unlock()
		return nil
	}
	logs := make([]models.AuditLog, len(l.buffer))
	copy(logs, l.buffer)
	l.buffer = l.buffer[:0]
	l.mu.Unlock()

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	for _, log := range logs {
		if err := l.repo.CreateAuditLog(ctx, &log); err != nil {
			// Re-add to buffer on error
			l.mu.Lock()
			l.buffer = append(l.buffer, log)
			l.mu.Unlock()
		}
	}

	return nil
}

// flushLoop runs in the background to periodically flush logs
func (l *Logger) flushLoop() {
	for {
		select {
		case <-l.ticker.C:
			l.Flush()
		case <-l.stop:
			return
		}
	}
}

// LogAction is a helper for common action logging
func (l *Logger) LogAction(userID, userName, action, resourceType, resourceID, ipAddress string, success bool) {
	l.Log(userID, userName, action, resourceType, resourceID, "", ipAddress, success, nil)
}

// LogActionWithError logs an action that resulted in an error
func (l *Logger) LogActionWithError(userID, userName, action, resourceType, resourceID, ipAddress string, err error) {
	l.Log(userID, userName, action, resourceType, resourceID, "", ipAddress, false, err)
}
