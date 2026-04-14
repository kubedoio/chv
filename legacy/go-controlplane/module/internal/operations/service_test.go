package operations

import (
	"context"
	"errors"
	"testing"

	"github.com/chv/chv/internal/db"
)

func TestServiceLogImageImportStart(t *testing.T) {
	repo, cleanup := setupTestDB(t)
	defer cleanup()

	svc := NewService(repo)
	ctx := context.Background()

	request := map[string]string{
		"name":       "test-image",
		"source_url": "https://example.com/image.qcow2",
	}

	op, err := svc.LogImageImportStart(ctx, "image-123", request)
	if err != nil {
		t.Fatalf("LogImageImportStart failed: %v", err)
	}

	if op.ID == "" {
		t.Error("expected operation ID to be set")
	}
	if op.ResourceType != ResourceTypeImage {
		t.Errorf("expected resource type %q, got %q", ResourceTypeImage, op.ResourceType)
	}
	if op.ResourceID != "image-123" {
		t.Errorf("expected resource ID %q, got %q", "image-123", op.ResourceID)
	}
	if op.OperationType != OperationTypeImport {
		t.Errorf("expected operation type %q, got %q", OperationTypeImport, op.OperationType)
	}
	if op.State != StatePending {
		t.Errorf("expected state %q, got %q", StatePending, op.State)
	}
	if op.RequestPayload == "" {
		t.Error("expected request payload to be set")
	}
	if op.CreatedAt == "" {
		t.Error("expected created_at to be set")
	}
}

func TestServiceLogImageImportRunning(t *testing.T) {
	repo, cleanup := setupTestDB(t)
	defer cleanup()

	svc := NewService(repo)
	ctx := context.Background()

	// Create initial operation
	request := map[string]string{"name": "test-image"}
	op, _ := svc.LogImageImportStart(ctx, "image-123", request)

	// Update to running
	err := svc.LogImageImportRunning(ctx, op.ID)
	if err != nil {
		t.Fatalf("LogImageImportRunning failed: %v", err)
	}

	// Verify in DB
	ops, err := repo.ListOperations(ctx)
	if err != nil {
		t.Fatalf("ListOperations failed: %v", err)
	}
	if len(ops) != 1 {
		t.Fatalf("expected 1 operation, got %d", len(ops))
	}
	if ops[0].State != StateRunning {
		t.Errorf("expected state %q, got %q", StateRunning, ops[0].State)
	}
	if ops[0].StartedAt == "" {
		t.Error("expected started_at to be set")
	}
}

func TestServiceLogImageImportComplete(t *testing.T) {
	repo, cleanup := setupTestDB(t)
	defer cleanup()

	svc := NewService(repo)
	ctx := context.Background()

	// Create initial operation
	request := map[string]string{"name": "test-image"}
	op, _ := svc.LogImageImportStart(ctx, "image-123", request)

	// Complete the operation
	result := map[string]string{"status": "downloaded", "path": "/data/images/test.qcow2"}
	err := svc.LogImageImportComplete(ctx, op.ID, result)
	if err != nil {
		t.Fatalf("LogImageImportComplete failed: %v", err)
	}

	// Verify in DB
	ops, err := repo.ListOperations(ctx)
	if err != nil {
		t.Fatalf("ListOperations failed: %v", err)
	}
	if len(ops) != 1 {
		t.Fatalf("expected 1 operation, got %d", len(ops))
	}
	if ops[0].State != StateCompleted {
		t.Errorf("expected state %q, got %q", StateCompleted, ops[0].State)
	}
	if ops[0].ResultPayload == "" {
		t.Error("expected result payload to be set")
	}
	if ops[0].FinishedAt == "" {
		t.Error("expected finished_at to be set")
	}
}

func TestServiceLogImageImportFailed(t *testing.T) {
	repo, cleanup := setupTestDB(t)
	defer cleanup()

	svc := NewService(repo)
	ctx := context.Background()

	// Create initial operation
	request := map[string]string{"name": "test-image"}
	op, _ := svc.LogImageImportStart(ctx, "image-123", request)

	// Mark as failed
	testErr := errors.New("download failed: connection timeout")
	err := svc.LogImageImportFailed(ctx, op.ID, testErr)
	if err != nil {
		t.Fatalf("LogImageImportFailed failed: %v", err)
	}

	// Verify in DB
	ops, err := repo.ListOperations(ctx)
	if err != nil {
		t.Fatalf("ListOperations failed: %v", err)
	}
	if len(ops) != 1 {
		t.Fatalf("expected 1 operation, got %d", len(ops))
	}
	if ops[0].State != StateFailed {
		t.Errorf("expected state %q, got %q", StateFailed, ops[0].State)
	}
	if ops[0].ErrorPayload == "" {
		t.Error("expected error payload to be set")
	}
	if ops[0].FinishedAt == "" {
		t.Error("expected finished_at to be set")
	}
}

func TestServiceFullLifecycle(t *testing.T) {
	repo, cleanup := setupTestDB(t)
	defer cleanup()

	svc := NewService(repo)
	ctx := context.Background()

	// Step 1: Start
	request := map[string]string{
		"name":       "ubuntu-22.04",
		"source_url": "https://example.com/ubuntu.qcow2",
	}
	op, err := svc.LogImageImportStart(ctx, "img-456", request)
	if err != nil {
		t.Fatalf("LogImageImportStart failed: %v", err)
	}

	// Step 2: Running
	err = svc.LogImageImportRunning(ctx, op.ID)
	if err != nil {
		t.Fatalf("LogImageImportRunning failed: %v", err)
	}

	// Step 3: Complete
	result := map[string]any{
		"downloaded_bytes": 2147483648,
		"checksum_valid":   true,
	}
	err = svc.LogImageImportComplete(ctx, op.ID, result)
	if err != nil {
		t.Fatalf("LogImageImportComplete failed: %v", err)
	}

	// Verify final state
	ops, _ := repo.ListOperations(ctx)
	if len(ops) != 1 {
		t.Fatalf("expected 1 operation, got %d", len(ops))
	}

	finalOp := ops[0]
	if finalOp.State != StateCompleted {
		t.Errorf("expected state %q, got %q", StateCompleted, finalOp.State)
	}
	if finalOp.ResourceType != ResourceTypeImage {
		t.Errorf("expected resource type %q, got %q", ResourceTypeImage, finalOp.ResourceType)
	}
	if finalOp.ResourceID != "img-456" {
		t.Errorf("expected resource ID %q, got %q", "img-456", finalOp.ResourceID)
	}
}

func setupTestDB(t *testing.T) (*db.Repository, func()) {
	t.Helper()
	repo, err := db.Open(":memory:")
	if err != nil {
		t.Fatalf("failed to open test db: %v", err)
	}
	return repo, func() { repo.Close() }
}
