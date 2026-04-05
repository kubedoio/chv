package models

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

// OperationCategory represents the category of operation
type OperationCategory string

const (
	OpCategorySync  OperationCategory = "sync"
	OpCategoryAsync OperationCategory = "async"
)

// OperationType represents the type of operation
type OperationType string

const (
	OpVMCreate     OperationType = "vm_create"
	OpVMStart      OperationType = "vm_start"
	OpVMStop       OperationType = "vm_stop"
	OpVMReboot     OperationType = "vm_reboot"
	OpVMDelete     OperationType = "vm_delete"
	OpVMConsole    OperationType = "vm_console"
	OpImageImport  OperationType = "image_import"
	OpNodeRegister OperationType = "node_register"
)

// OperationStatus represents the status of an operation
type OperationStatus string

const (
	OpStatusPending   OperationStatus = "pending"
	OpStatusRunning   OperationStatus = "running"
	OpStatusCompleted OperationStatus = "completed"
	OpStatusFailed    OperationStatus = "failed"
	OpStatusCancelled OperationStatus = "cancelled"
)

// ActorType represents the type of actor that initiated the operation
type ActorType string

const (
	ActorTypeUser       ActorType = "user"
	ActorTypeSystem     ActorType = "system"
	ActorTypeScheduler  ActorType = "scheduler"
	ActorTypeReconciler ActorType = "reconciler"
)

// Operation represents an operation in the system
type Operation struct {
	ID             uuid.UUID         `json:"id" db:"id"`
	OperationType  OperationType     `json:"operation_type" db:"operation_type"`
	Category       OperationCategory `json:"category" db:"category"`
	Status         OperationStatus   `json:"status" db:"status"`
	StatusMessage  string            `json:"status_message" db:"status_message"`
	ResourceType   string            `json:"resource_type" db:"resource_type"`
	ResourceID     *uuid.UUID        `json:"resource_id" db:"resource_id"`
	ActorType      ActorType         `json:"actor_type" db:"actor_type"`
	ActorID        string            `json:"actor_id" db:"actor_id"`
	NodeID         *uuid.UUID        `json:"node_id" db:"node_id"`
	RequestPayload json.RawMessage   `json:"request_payload" db:"request_payload"`
	ResultPayload  json.RawMessage   `json:"result_payload" db:"result_payload"`
	ErrorPayload   json.RawMessage   `json:"error_payload" db:"error_payload"`
	StartedAt      *time.Time        `json:"started_at" db:"started_at"`
	FinishedAt     *time.Time        `json:"finished_at" db:"finished_at"`
	CreatedAt      time.Time         `json:"created_at" db:"created_at"`
	UpdatedAt      time.Time         `json:"updated_at" db:"updated_at"`
}

// IsTerminal returns true if the operation is in a terminal state
func (o *Operation) IsTerminal() bool {
	return o.Status == OpStatusCompleted || o.Status == OpStatusFailed || o.Status == OpStatusCancelled
}

// IsAsync returns true if the operation is asynchronous
func (o *Operation) IsAsync() bool {
	return o.Category == OpCategoryAsync
}

// CanCancel returns true if the operation can be cancelled
func (o *Operation) CanCancel() bool {
	return o.Status == OpStatusPending || o.Status == OpStatusRunning
}

// OperationLog represents a log entry for an operation
type OperationLog struct {
	ID          uuid.UUID       `json:"id" db:"id"`
	OperationID uuid.UUID       `json:"operation_id" db:"operation_id"`
	Level       string          `json:"level" db:"level"`
	Message     string          `json:"message" db:"message"`
	Details     json.RawMessage `json:"details" db:"details"`
	CreatedAt   time.Time       `json:"created_at" db:"created_at"`
}
