// Package logger provides structured logging for CHV.
package logger

import (
	"fmt"
	"log"
	"os"
	"sync"
)

// Level represents a log level.
type Level int

const (
	// DebugLevel is the debug log level.
	DebugLevel Level = iota
	// InfoLevel is the info log level.
	InfoLevel
	// WarnLevel is the warn log level.
	WarnLevel
	// ErrorLevel is the error log level.
	ErrorLevel
)

// String returns the string representation of a log level.
func (l Level) String() string {
	switch l {
	case DebugLevel:
		return "DEBUG"
	case InfoLevel:
		return "INFO"
	case WarnLevel:
		return "WARN"
	case ErrorLevel:
		return "ERROR"
	default:
		return "UNKNOWN"
	}
}

// ParseLevel parses a log level from a string.
func ParseLevel(s string) Level {
	switch s {
	case "debug", "DEBUG":
		return DebugLevel
	case "info", "INFO":
		return InfoLevel
	case "warn", "WARN", "warning", "WARNING":
		return WarnLevel
	case "error", "ERROR":
		return ErrorLevel
	default:
		return InfoLevel
	}
}

// Logger is a structured logger.
type Logger struct {
	level  Level
	prefix string
	fields map[string]interface{}
	mu     sync.RWMutex
}

// New creates a new logger.
func New(level Level) *Logger {
	return &Logger{
		level:  level,
		prefix: "",
		fields: make(map[string]interface{}),
	}
}

// NewFromEnv creates a new logger from environment.
func NewFromEnv() *Logger {
	levelStr := os.Getenv("CHV_LOG_LEVEL")
	if levelStr == "" {
		levelStr = "info"
	}
	return New(ParseLevel(levelStr))
}

// WithField adds a field to the logger.
func (l *Logger) WithField(key string, value interface{}) *Logger {
	newLogger := &Logger{
		level:  l.level,
		prefix: l.prefix,
		fields: make(map[string]interface{}),
	}
	for k, v := range l.fields {
		newLogger.fields[k] = v
	}
	newLogger.fields[key] = value
	return newLogger
}

// WithFields adds multiple fields to the logger.
func (l *Logger) WithFields(fields map[string]interface{}) *Logger {
	newLogger := &Logger{
		level:  l.level,
		prefix: l.prefix,
		fields: make(map[string]interface{}),
	}
	for k, v := range l.fields {
		newLogger.fields[k] = v
	}
	for k, v := range fields {
		newLogger.fields[k] = v
	}
	return newLogger
}

// WithPrefix adds a prefix to the logger.
func (l *Logger) WithPrefix(prefix string) *Logger {
	return &Logger{
		level:  l.level,
		prefix: prefix,
		fields: l.fields,
	}
}

// Debug logs a debug message.
func (l *Logger) Debug(msg string) {
	if l.level <= DebugLevel {
		l.log(DebugLevel, msg)
	}
}

// Debugf logs a formatted debug message.
func (l *Logger) Debugf(format string, args ...interface{}) {
	if l.level <= DebugLevel {
		l.log(DebugLevel, fmt.Sprintf(format, args...))
	}
}

// Info logs an info message.
func (l *Logger) Info(msg string) {
	if l.level <= InfoLevel {
		l.log(InfoLevel, msg)
	}
}

// Infof logs a formatted info message.
func (l *Logger) Infof(format string, args ...interface{}) {
	if l.level <= InfoLevel {
		l.log(InfoLevel, fmt.Sprintf(format, args...))
	}
}

// Warn logs a warning message.
func (l *Logger) Warn(msg string) {
	if l.level <= WarnLevel {
		l.log(WarnLevel, msg)
	}
}

// Warnf logs a formatted warning message.
func (l *Logger) Warnf(format string, args ...interface{}) {
	if l.level <= WarnLevel {
		l.log(WarnLevel, fmt.Sprintf(format, args...))
	}
}

// Error logs an error message.
func (l *Logger) Error(msg string) {
	if l.level <= ErrorLevel {
		l.log(ErrorLevel, msg)
	}
}

// Errorf logs a formatted error message.
func (l *Logger) Errorf(format string, args ...interface{}) {
	if l.level <= ErrorLevel {
		l.log(ErrorLevel, fmt.Sprintf(format, args...))
	}
}

// log logs a message.
func (l *Logger) log(level Level, msg string) {
	l.mu.RLock()
	defer l.mu.RUnlock()

	prefix := ""
	if l.prefix != "" {
		prefix = "[" + l.prefix + "] "
	}

	// Format fields
	fieldsStr := ""
	if len(l.fields) > 0 {
		fieldsStr = " {"
		first := true
		for k, v := range l.fields {
			if !first {
				fieldsStr += " "
			}
			fieldsStr += fmt.Sprintf("%s=%v", k, v)
			first = false
		}
		fieldsStr += "}"
	}

	log.Printf("[%s] %s%s%s", level.String(), prefix, msg, fieldsStr)
}

// SetLevel sets the log level.
func (l *Logger) SetLevel(level Level) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.level = level
}

// Standard logger instance.
var std = NewFromEnv()

// Debug logs a debug message to the standard logger.
func Debug(msg string) { std.Debug(msg) }

// Debugf logs a formatted debug message to the standard logger.
func Debugf(format string, args ...interface{}) { std.Debugf(format, args...) }

// Info logs an info message to the standard logger.
func Info(msg string) { std.Info(msg) }

// Infof logs a formatted info message to the standard logger.
func Infof(format string, args ...interface{}) { std.Infof(format, args...) }

// Warn logs a warning message to the standard logger.
func Warn(msg string) { std.Warn(msg) }

// Warnf logs a formatted warning message to the standard logger.
func Warnf(format string, args ...interface{}) { std.Warnf(format, args...) }

// Error logs an error message to the standard logger.
func Error(msg string) { std.Error(msg) }

// Errorf logs a formatted error message to the standard logger.
func Errorf(format string, args ...interface{}) { std.Errorf(format, args...) }

// WithField adds a field to the standard logger.
func WithField(key string, value interface{}) *Logger { return std.WithField(key, value) }

// WithFields adds multiple fields to the standard logger.
func WithFields(fields map[string]interface{}) *Logger { return std.WithFields(fields) }
