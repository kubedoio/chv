package models

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

// OperationStatus represents the operation status.
type OperationStatus string

const (
	OperationStatusPending    OperationStatus = "pending"
	OperationStatusInProgress OperationStatus = "in_progress"
	OperationStatusSucceeded  OperationStatus = "succeeded"
	OperationStatusFailed     OperationStatus = "failed"
	OperationStatusTimedOut   OperationStatus = "timed_out"
	OperationStatusAborted    OperationStatus = "aborted"
)

// Operation represents a tracked operation.
type Operation struct {
	ID             uuid.UUID       `json:"id" db:"id"`
	ResourceType   string          `json:"resource_type" db:"resource_type"`
	ResourceID     uuid.UUID       `json:"resource_id" db:"resource_id"`
	OperationType  string          `json:"operation_type" db:"operation_type"`
	Status         OperationStatus `json:"status" db:"status"`
	RequestPayload json.RawMessage `json:"request_payload" db:"request_payload"`
	ResultPayload  json.RawMessage `json:"result_payload,omitempty" db:"result_payload"`
	ErrorPayload   json.RawMessage `json:"error_payload,omitempty" db:"error_payload"`
	StartedAt      *time.Time      `json:"started_at" db:"started_at"`
	FinishedAt     *time.Time      `json:"finished_at" db:"finished_at"`
	CreatedAt      time.Time       `json:"created_at" db:"created_at"`
}

// IsComplete returns true if the operation is complete.
func (o *Operation) IsComplete() bool {
	return o.Status == OperationStatusSucceeded ||
		o.Status == OperationStatusFailed ||
		o.Status == OperationStatusTimedOut ||
		o.Status == OperationStatusAborted
}

// IsSuccessful returns true if the operation succeeded.
func (o *Operation) IsSuccessful() bool {
	return o.Status == OperationStatusSucceeded
}

// SetRequest sets the request payload.
func (o *Operation) SetRequest(req interface{}) error {
	data, err := json.Marshal(req)
	if err != nil {
		return err
	}
	o.RequestPayload = data
	return nil
}

// SetResult sets the result payload.
func (o *Operation) SetResult(result interface{}) error {
	data, err := json.Marshal(result)
	if err != nil {
		return err
	}
	o.ResultPayload = data
	return nil
}

// SetError sets the error payload.
func (o *Operation) SetError(err interface{}) error {
	data, e := json.Marshal(err)
	if e != nil {
		return e
	}
	o.ErrorPayload = data
	return nil
}
