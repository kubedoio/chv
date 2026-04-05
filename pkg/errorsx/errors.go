// Package errorsx provides structured error types for CHV.
package errorsx

import (
	"encoding/json"
	"fmt"
)

// ErrorCode represents a machine-readable error code.
type ErrorCode string

const (
	// Generic errors
	ErrInternal          ErrorCode = "INTERNAL_ERROR"
	ErrInvalidRequest    ErrorCode = "INVALID_REQUEST"
	ErrNotFound          ErrorCode = "NOT_FOUND"
	ErrAlreadyExists     ErrorCode = "ALREADY_EXISTS"
	ErrUnauthorized      ErrorCode = "UNAUTHORIZED"
	ErrForbidden         ErrorCode = "FORBIDDEN"
	ErrTimeout           ErrorCode = "TIMEOUT"
	ErrNotImplemented    ErrorCode = "NOT_IMPLEMENTED"
	ErrServiceUnavailable ErrorCode = "SERVICE_UNAVAILABLE"
	
	// Domain-specific errors
	ErrNodeNotAvailable    ErrorCode = "NODE_NOT_AVAILABLE"
	ErrNodeInMaintenance   ErrorCode = "NODE_IN_MAINTENANCE"
	ErrInsufficientResources ErrorCode = "INSUFFICIENT_RESOURCES"
	ErrImageNotFound       ErrorCode = "IMAGE_NOT_FOUND"
	ErrImageImportFailed   ErrorCode = "IMAGE_IMPORT_FAILED"
	ErrNetworkNotFound     ErrorCode = "NETWORK_NOT_FOUND"
	ErrNetworkConfigFailed ErrorCode = "NETWORK_CONFIG_FAILED"
	ErrStoragePoolNotFound ErrorCode = "STORAGE_POOL_NOT_FOUND"
	ErrStoragePoolFull     ErrorCode = "STORAGE_POOL_FULL"
	ErrVolumeNotFound      ErrorCode = "VOLUME_NOT_FOUND"
	ErrVolumeResizeFailed  ErrorCode = "VOLUME_RESIZE_FAILED"
	ErrVolumeResizeUnsupported ErrorCode = "VOLUME_RESIZE_UNSUPPORTED"
	ErrVMNotFound          ErrorCode = "VM_NOT_FOUND"
	ErrVMInvalidState      ErrorCode = "VM_INVALID_STATE"
	ErrVMLaunchFailed      ErrorCode = "VM_LAUNCH_FAILED"
	ErrPlacementFailed     ErrorCode = "PLACEMENT_FAILED"
	ErrMaintenanceBlocked  ErrorCode = "MAINTENANCE_BLOCKED"
)

// Error represents a structured error with context.
type Error struct {
	Code         ErrorCode `json:"code"`
	Message      string    `json:"message"`
	ResourceType string    `json:"resource_type,omitempty"`
	ResourceID   string    `json:"resource_id,omitempty"`
	Retryable    bool      `json:"retryable"`
	Hint         string    `json:"hint,omitempty"`
	Cause        error     `json:"-"`
}

// Error implements the error interface.
func (e *Error) Error() string {
	if e.Cause != nil {
		return fmt.Sprintf("%s: %s (cause: %v)", e.Code, e.Message, e.Cause)
	}
	return fmt.Sprintf("%s: %s", e.Code, e.Message)
}

// Unwrap returns the cause for errors.Is/errors.As support.
func (e *Error) Unwrap() error {
	return e.Cause
}

// MarshalJSON implements custom JSON serialization.
func (e *Error) MarshalJSON() ([]byte, error) {
	type Alias Error
	return json.Marshal(&struct {
		*Alias
		Error string `json:"error,omitempty"`
	}{
		Alias: (*Alias)(e),
		Error: e.Message,
	})
}

// New creates a new structured error.
func New(code ErrorCode, message string) *Error {
	return &Error{
		Code:      code,
		Message:   message,
		Retryable: isRetryable(code),
	}
}

// Wrap wraps an existing error with a structured error.
func Wrap(code ErrorCode, message string, cause error) *Error {
	return &Error{
		Code:      code,
		Message:   message,
		Cause:     cause,
		Retryable: isRetryable(code),
	}
}

// WithResource adds resource context to an error.
func (e *Error) WithResource(resourceType, resourceID string) *Error {
	e.ResourceType = resourceType
	e.ResourceID = resourceID
	return e
}

// WithHint adds a remediation hint to an error.
func (e *Error) WithHint(hint string) *Error {
	e.Hint = hint
	return e
}

// WithRetryable sets the retryable flag explicitly.
func (e *Error) WithRetryable(retryable bool) *Error {
	e.Retryable = retryable
	return e
}

// isRetryable determines if an error code is retryable by default.
func isRetryable(code ErrorCode) bool {
	switch code {
	case ErrTimeout,
		ErrServiceUnavailable,
		ErrNodeNotAvailable,
		ErrInternal:
		return true
	case ErrInvalidRequest,
		ErrNotFound,
		ErrAlreadyExists,
		ErrUnauthorized,
		ErrForbidden,
		ErrNotImplemented,
		ErrVMInvalidState:
		return false
	default:
		return false
	}
}

// IsErrorCode checks if an error has a specific code.
func IsErrorCode(err error, code ErrorCode) bool {
	if e, ok := err.(*Error); ok {
		return e.Code == code
	}
	return false
}
