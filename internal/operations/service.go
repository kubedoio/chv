package operations

import (
	"context"
	"encoding/json"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/store"
	"github.com/chv/chv/pkg/uuidx"
	"github.com/google/uuid"
)

// Service handles operation business logic
type Service struct {
	store store.Store
}

// NewService creates a new operations service
func NewService(store store.Store) *Service {
	return &Service{store: store}
}

// Start creates and starts tracking a new operation
func (s *Service) Start(ctx context.Context, opType models.OperationType, category models.OperationCategory,
	resourceType string, resourceID *uuid.UUID, actorType models.ActorType, actorID string,
	requestPayload interface{}) (*models.Operation, error) {

	now := time.Now()

	// Serialize request payload
	var requestJSON json.RawMessage
	if requestPayload != nil {
		data, err := json.Marshal(requestPayload)
		if err != nil {
			return nil, err
		}
		requestJSON = data
	}

	op := &models.Operation{
		ID:             uuidx.New(),
		Type:           opType,
		Category:       category,
		Status:         models.OpStatusRunning,
		StatusMessage:  "Operation started",
		ResourceType:   resourceType,
		ResourceID:     resourceID,
		ActorType:      actorType,
		ActorID:        actorID,
		RequestPayload: requestJSON,
		StartedAt:      &now,
		CreatedAt:      now,
		UpdatedAt:      now,
	}

	if err := s.store.CreateOperation(ctx, op); err != nil {
		return nil, err
	}

	return op, nil
}

// Complete marks an operation as completed
func (s *Service) Complete(ctx context.Context, opID uuid.UUID, result interface{}) error {
	op, err := s.store.GetOperation(ctx, opID)
	if err != nil {
		return err
	}
	if op == nil {
		return nil
	}

	now := time.Now()

	// Serialize result payload
	if result != nil {
		data, err := json.Marshal(result)
		if err != nil {
			return err
		}
		op.ResultPayload = data
	}

	op.Status = models.OpStatusCompleted
	op.StatusMessage = "Operation completed successfully"
	op.CompletedAt = &now
	op.UpdatedAt = now

	return s.store.UpdateOperation(ctx, op)
}

// Fail marks an operation as failed
func (s *Service) Fail(ctx context.Context, opID uuid.UUID, err error) error {
	if err == nil {
		return nil
	}

	op, storeErr := s.store.GetOperation(ctx, opID)
	if storeErr != nil {
		return storeErr
	}
	if op == nil {
		return nil
	}

	now := time.Now()

	// Serialize error payload
	errorData := map[string]string{
		"error": err.Error(),
	}
	errorJSON, _ := json.Marshal(errorData)
	op.ErrorDetails = errorJSON

	op.Status = models.OpStatusFailed
	op.StatusMessage = err.Error()
	op.CompletedAt = &now
	op.UpdatedAt = now

	return s.store.UpdateOperation(ctx, op)
}

// UpdateProgress updates operation progress
func (s *Service) UpdateProgress(ctx context.Context, opID uuid.UUID, percent int, message string) error {
	op, err := s.store.GetOperation(ctx, opID)
	if err != nil {
		return err
	}
	if op == nil {
		return nil
	}

	op.StatusMessage = message
	op.UpdatedAt = time.Now()

	return s.store.UpdateOperation(ctx, op)
}

// Log adds a log entry to an operation
func (s *Service) Log(ctx context.Context, opID uuid.UUID, level, message string, details interface{}) error {
	now := time.Now()

	// Serialize details
	var detailsJSON json.RawMessage
	if details != nil {
		data, err := json.Marshal(details)
		if err != nil {
			return err
		}
		detailsJSON = data
	}

	log := &models.OperationLog{
		ID:          uuidx.New(),
		OperationID: opID,
		Level:       level,
		Message:     message,
		Details:     detailsJSON,
		CreatedAt:   now,
	}

	return s.store.CreateOperationLog(ctx, log)
}
