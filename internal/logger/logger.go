package logger

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"os"
	"path/filepath"
	"runtime"
	"sync"
	"time"
)

// Level represents logging severity
type Level int

const (
	DebugLevel Level = iota
	InfoLevel
	WarnLevel
	ErrorLevel
	FatalLevel
)

func (l Level) String() string {
	switch l {
	case DebugLevel:
		return "debug"
	case InfoLevel:
		return "info"
	case WarnLevel:
		return "warn"
	case ErrorLevel:
		return "error"
	case FatalLevel:
		return "fatal"
	default:
		return "unknown"
	}
}

// Logger provides structured logging
type Logger struct {
	level      Level
	output     io.Writer
	component  string
	requestID  string
	structured bool
	mu         sync.RWMutex
}

// Config holds logger configuration
type Config struct {
	Level      string
	Output     io.Writer
	Component  string
	Structured bool
	LogDir     string
}

var (
	defaultLogger *Logger
	once          sync.Once
)

// InitDefault initializes the default logger
func InitDefault(cfg Config) error {
	var err error
	once.Do(func() {
		defaultLogger, err = New(cfg)
	})
	return err
}

// L returns the default logger
func L() *Logger {
	if defaultLogger == nil {
		defaultLogger = &Logger{
			level:      InfoLevel,
			output:     os.Stdout,
			structured: true,
		}
	}
	return defaultLogger
}

// New creates a new logger
func New(cfg Config) (*Logger, error) {
	level := parseLevel(cfg.Level)

	output := cfg.Output
	if output == nil {
		output = os.Stdout
	}

	// Setup file output if log directory specified
	if cfg.LogDir != "" {
		if err := os.MkdirAll(cfg.LogDir, 0755); err != nil {
			return nil, fmt.Errorf("failed to create log directory: %w", err)
		}

		logFile := filepath.Join(cfg.LogDir, fmt.Sprintf("%s.log", cfg.Component))
		f, err := os.OpenFile(logFile, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0644)
		if err != nil {
			return nil, fmt.Errorf("failed to open log file: %w", err)
		}

		// Write to both stdout and file
		output = io.MultiWriter(os.Stdout, f)
	}

	return &Logger{
		level:      level,
		output:     output,
		component:  cfg.Component,
		structured: cfg.Structured,
	}, nil
}

// WithComponent returns a logger with a component name
func (l *Logger) WithComponent(component string) *Logger {
	return &Logger{
		level:      l.level,
		output:     l.output,
		component:  component,
		requestID:  l.requestID,
		structured: l.structured,
	}
}

// WithRequestID returns a logger with a request ID
func (l *Logger) WithRequestID(requestID string) *Logger {
	return &Logger{
		level:      l.level,
		output:     l.output,
		component:  l.component,
		requestID:  requestID,
		structured: l.structured,
	}
}

// WithContext extracts request ID from context and returns logger
func (l *Logger) WithContext(ctx context.Context) *Logger {
	if ctx == nil {
		return l
	}
	if reqID, ok := ctx.Value("request_id").(string); ok {
		return l.WithRequestID(reqID)
	}
	return l
}

// Debug logs a debug message
func (l *Logger) Debug(msg string, fields ...Field) {
	l.log(DebugLevel, msg, fields...)
}

// Info logs an info message
func (l *Logger) Info(msg string, fields ...Field) {
	l.log(InfoLevel, msg, fields...)
}

// Warn logs a warning message
func (l *Logger) Warn(msg string, fields ...Field) {
	l.log(WarnLevel, msg, fields...)
}

// Error logs an error message
func (l *Logger) Error(msg string, fields ...Field) {
	l.log(ErrorLevel, msg, fields...)
}

// Fatal logs a fatal message and exits
func (l *Logger) Fatal(msg string, fields ...Field) {
	l.log(FatalLevel, msg, fields...)
	os.Exit(1)
}

// log writes a log entry
func (l *Logger) log(level Level, msg string, fields ...Field) {
	if level < l.level {
		return
	}

	l.mu.RLock()
	defer l.mu.RUnlock()

	entry := LogEntry{
		Timestamp: time.Now().UTC().Format(time.RFC3339),
		Level:     level.String(),
		Component: l.component,
		Message:   msg,
		Fields:    make(map[string]interface{}),
	}

	if l.requestID != "" {
		entry.RequestID = l.requestID
	}

	// Add caller info for debug/error levels
	if level <= DebugLevel || level >= ErrorLevel {
		_, file, line, ok := runtime.Caller(3)
		if ok {
			entry.Caller = fmt.Sprintf("%s:%d", filepath.Base(file), line)
		}
	}

	// Add fields
	for _, f := range fields {
		entry.Fields[f.Key] = f.Value
	}

	if l.structured {
		// JSON output
		encoder := json.NewEncoder(l.output)
		encoder.SetEscapeHTML(false)
		if err := encoder.Encode(entry); err != nil {
			log.Printf("failed to encode log entry: %v", err)
		}
	} else {
		// Human-readable output
		fmt.Fprintf(l.output, "[%s] %s %s", entry.Timestamp, entry.Level, msg)
		if entry.RequestID != "" {
			fmt.Fprintf(l.output, " [req:%s]", entry.RequestID)
		}
		for k, v := range entry.Fields {
			fmt.Fprintf(l.output, " %s=%v", k, v)
		}
		fmt.Fprintln(l.output)
	}
}

// LogEntry represents a structured log entry
type LogEntry struct {
	Timestamp string                 `json:"timestamp"`
	Level     string                 `json:"level"`
	Component string                 `json:"component,omitempty"`
	RequestID string                 `json:"request_id,omitempty"`
	Message   string                 `json:"message"`
	Caller    string                 `json:"caller,omitempty"`
	Fields    map[string]interface{} `json:"fields,omitempty"`
}

// Field represents a log field
type Field struct {
	Key   string
	Value interface{}
}

// F creates a field
func F(key string, value interface{}) Field {
	return Field{Key: key, Value: value}
}

// ErrorField creates an error field
func ErrorField(err error) Field {
	if err == nil {
		return F("error", nil)
	}
	return F("error", err.Error())
}

// StringField creates a string field
func StringField(key, value string) Field {
	return F(key, value)
}

// IntField creates an int field
func IntField(key string, value int) Field {
	return F(key, value)
}

// DurationField creates a duration field
func DurationField(key string, value time.Duration) Field {
	return F(key, value.String())
}

func parseLevel(level string) Level {
	switch level {
	case "debug":
		return DebugLevel
	case "info":
		return InfoLevel
	case "warn", "warning":
		return WarnLevel
	case "error":
		return ErrorLevel
	case "fatal":
		return FatalLevel
	default:
		return InfoLevel
	}
}

// Package-level convenience functions

func Debug(msg string, fields ...Field) { L().Debug(msg, fields...) }
func Info(msg string, fields ...Field)  { L().Info(msg, fields...) }
func Warn(msg string, fields ...Field)  { L().Warn(msg, fields...) }
func Error(msg string, fields ...Field) { L().Error(msg, fields...) }
func Fatal(msg string, fields ...Field) { L().Fatal(msg, fields...) }
