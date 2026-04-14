package operations

import (
	"context"
	"encoding/json"
	"time"

	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
)

// State constants
const (
	StatePending   = "pending"
	StateRunning   = "running"
	StateCompleted = "completed"
	StateFailed    = "failed"
)

// Resource types
const (
	ResourceTypeImage = "image"
)

// Operation types
const (
	OperationTypeImport = "import"
)

type Service struct {
	repo *db.Repository
}

func NewService(repo *db.Repository) *Service {
	return &Service{repo: repo}
}

func (s *Service) LogImageImportStart(ctx context.Context, imageID string, request any) (*models.Operation, error) {
	requestJSON, _ := json.Marshal(request)

	op := &models.Operation{
		ID:             uuid.NewString(),
		ResourceType:   ResourceTypeImage,
		ResourceID:     imageID,
		OperationType:  OperationTypeImport,
		State:          StatePending,
		RequestPayload: string(requestJSON),
		CreatedAt:      time.Now().UTC().Format(time.RFC3339),
	}

	if err := s.repo.CreateOperation(ctx, op); err != nil {
		return nil, err
	}

	return op, nil
}

func (s *Service) LogImageImportRunning(ctx context.Context, operationID string) error {
	op := &models.Operation{
		ID:        operationID,
		State:     StateRunning,
		StartedAt: time.Now().UTC().Format(time.RFC3339),
	}
	return s.repo.UpdateOperation(ctx, op)
}

func (s *Service) LogImageImportComplete(ctx context.Context, operationID string, result any) error {
	resultJSON, _ := json.Marshal(result)

	op := &models.Operation{
		ID:            operationID,
		State:         StateCompleted,
		ResultPayload: string(resultJSON),
		FinishedAt:    time.Now().UTC().Format(time.RFC3339),
	}
	return s.repo.UpdateOperation(ctx, op)
}

func (s *Service) LogImageImportFailed(ctx context.Context, operationID string, err error) error {
	errorJSON, _ := json.Marshal(map[string]string{
		"error": err.Error(),
	})

	op := &models.Operation{
		ID:           operationID,
		State:        StateFailed,
		ErrorPayload: string(errorJSON),
		FinishedAt:   time.Now().UTC().Format(time.RFC3339),
	}
	return s.repo.UpdateOperation(ctx, op)
}

// ListOperations returns operations with optional filtering
func (s *Service) ListOperations(ctx context.Context, resourceType string) ([]models.Operation, error) {
	ops, err := s.repo.ListOperations(ctx)
	if err != nil {
		return nil, err
	}

	// Filter by resource type if specified
	if resourceType != "" {
		var filtered []models.Operation
		for _, op := range ops {
			if op.ResourceType == resourceType {
				filtered = append(filtered, op)
			}
		}
		return filtered, nil
	}

	return ops, nil
}
