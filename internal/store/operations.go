package store

import (
	"context"
	"fmt"
	"strings"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
)

// CreateOperation creates a new operation
func (s *PostgresStore) CreateOperation(ctx context.Context, op *models.Operation) error {
	return createOperation(ctx, s.pool, op)
}

// GetOperation gets an operation by ID
func (s *PostgresStore) GetOperation(ctx context.Context, id uuid.UUID) (*models.Operation, error) {
	return getOperation(ctx, s.pool, id)
}

// UpdateOperation updates an operation
func (s *PostgresStore) UpdateOperation(ctx context.Context, op *models.Operation) error {
	return updateOperation(ctx, s.pool, op)
}

// ListOperations lists operations with optional filters
func (s *PostgresStore) ListOperations(ctx context.Context, filters map[string]interface{}) ([]*models.Operation, error) {
	return listOperations(ctx, s.pool, filters)
}

// CreateOperationLog creates a new operation log entry
func (s *PostgresStore) CreateOperationLog(ctx context.Context, log *models.OperationLog) error {
	return createOperationLog(ctx, s.pool, log)
}

// GetOperationLogs gets logs for an operation
func (s *PostgresStore) GetOperationLogs(ctx context.Context, operationID uuid.UUID) ([]*models.OperationLog, error) {
	return getOperationLogs(ctx, s.pool, operationID)
}

func createOperation(ctx context.Context, q querier, op *models.Operation) error {
	sql := `
		INSERT INTO operations (
			id, type, category, status, status_message,
			resource_type, resource_id, actor_type, actor_id, node_id,
			request_payload, result_payload, error_details, progress_percent, progress_message,
			started_at, completed_at, created_at, updated_at
		) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
	`

	_, err := q.Exec(ctx, sql,
		op.ID, op.Type, op.Category, op.Status, op.StatusMessage,
		op.ResourceType, op.ResourceID, op.ActorType, op.ActorID, op.NodeID,
		op.RequestPayload, op.ResultPayload, op.ErrorDetails, op.ProgressPercent, op.ProgressMessage,
		op.StartedAt, op.CompletedAt, op.CreatedAt, op.UpdatedAt,
	)
	return err
}

func getOperation(ctx context.Context, q querier, id uuid.UUID) (*models.Operation, error) {
	sql := `
		SELECT 
			id, type, category, status, status_message,
			resource_type, resource_id, actor_type, actor_id, node_id,
			request_payload, result_payload, error_details, progress_percent, progress_message,
			started_at, completed_at, created_at, updated_at
		FROM operations WHERE id = $1
	`

	op := &models.Operation{}
	err := q.QueryRow(ctx, sql, id).Scan(
		&op.ID, &op.Type, &op.Category, &op.Status, &op.StatusMessage,
		&op.ResourceType, &op.ResourceID, &op.ActorType, &op.ActorID, &op.NodeID,
		&op.RequestPayload, &op.ResultPayload, &op.ErrorDetails, &op.ProgressPercent, &op.ProgressMessage,
		&op.StartedAt, &op.CompletedAt, &op.CreatedAt, &op.UpdatedAt,
	)
	if err == pgx.ErrNoRows {
		return nil, nil
	}
	return op, err
}

func updateOperation(ctx context.Context, q querier, op *models.Operation) error {
	sql := `
		UPDATE operations SET
			type = $2, category = $3, status = $4, status_message = $5,
			resource_type = $6, resource_id = $7, actor_type = $8, actor_id = $9, node_id = $10,
			request_payload = $11, result_payload = $12, error_details = $13, progress_percent = $14, progress_message = $15,
			started_at = $16, completed_at = $17, updated_at = $18
		WHERE id = $1
	`

	op.UpdatedAt = time.Now()
	_, err := q.Exec(ctx, sql,
		op.ID, op.Type, op.Category, op.Status, op.StatusMessage,
		op.ResourceType, op.ResourceID, op.ActorType, op.ActorID, op.NodeID,
		op.RequestPayload, op.ResultPayload, op.ErrorDetails, op.ProgressPercent, op.ProgressMessage,
		op.StartedAt, op.CompletedAt, op.UpdatedAt,
	)
	return err
}

func listOperations(ctx context.Context, q querier, filters map[string]interface{}) ([]*models.Operation, error) {
	whereClause := ""
	var args []interface{}
	argIdx := 1

	if filters != nil {
		var conditions []string

		if resourceType, ok := filters["resource_type"].(string); ok && resourceType != "" {
			conditions = append(conditions, fmt.Sprintf("resource_type = $%d", argIdx))
			args = append(args, resourceType)
			argIdx++
		}

		if resourceID, ok := filters["resource_id"].(uuid.UUID); ok {
			conditions = append(conditions, fmt.Sprintf("resource_id = $%d", argIdx))
			args = append(args, resourceID)
			argIdx++
		}

		if status, ok := filters["status"].(string); ok && status != "" {
			conditions = append(conditions, fmt.Sprintf("status = $%d", argIdx))
			args = append(args, status)
			argIdx++
		}

		if opType, ok := filters["type"].(string); ok && opType != "" {
			conditions = append(conditions, fmt.Sprintf("type = $%d", argIdx))
			args = append(args, opType)
			argIdx++
		}

		if nodeID, ok := filters["node_id"].(uuid.UUID); ok {
			conditions = append(conditions, fmt.Sprintf("node_id = $%d", argIdx))
			args = append(args, nodeID)
			argIdx++
		}

		if len(conditions) > 0 {
			whereClause = "WHERE " + strings.Join(conditions, " AND ")
		}
	}

	sql := fmt.Sprintf(`
		SELECT 
			id, type, category, status, status_message,
			resource_type, resource_id, actor_type, actor_id, node_id,
			request_payload, result_payload, error_details, progress_percent, progress_message,
			started_at, completed_at, created_at, updated_at
		FROM operations
		%s
		ORDER BY created_at DESC
	`, whereClause)

	rows, err := q.Query(ctx, sql, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var ops []*models.Operation
	for rows.Next() {
		op := &models.Operation{}
		err := rows.Scan(
			&op.ID, &op.Type, &op.Category, &op.Status, &op.StatusMessage,
			&op.ResourceType, &op.ResourceID, &op.ActorType, &op.ActorID, &op.NodeID,
			&op.RequestPayload, &op.ResultPayload, &op.ErrorDetails, &op.ProgressPercent, &op.ProgressMessage,
			&op.StartedAt, &op.CompletedAt, &op.CreatedAt, &op.UpdatedAt,
		)
		if err != nil {
			return nil, err
		}
		ops = append(ops, op)
	}

	return ops, rows.Err()
}

func createOperationLog(ctx context.Context, q querier, log *models.OperationLog) error {
	sql := `
		INSERT INTO operation_logs (
			id, operation_id, level, message, details, created_at
		) VALUES ($1, $2, $3, $4, $5, $6)
	`

	_, err := q.Exec(ctx, sql,
		log.ID, log.OperationID, log.Level, log.Message, log.Details, log.CreatedAt,
	)
	return err
}

func getOperationLogs(ctx context.Context, q querier, operationID uuid.UUID) ([]*models.OperationLog, error) {
	sql := `
		SELECT 
			id, operation_id, level, message, details, created_at
		FROM operation_logs
		WHERE operation_id = $1
		ORDER BY created_at ASC
	`

	rows, err := q.Query(ctx, sql, operationID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var logs []*models.OperationLog
	for rows.Next() {
		log := &models.OperationLog{}
		err := rows.Scan(
			&log.ID, &log.OperationID, &log.Level, &log.Message, &log.Details, &log.CreatedAt,
		)
		if err != nil {
			return nil, err
		}
		logs = append(logs, log)
	}

	return logs, rows.Err()
}
