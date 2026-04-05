package store

import (
	"testing"

	"github.com/chv/chv/internal/models"
)

func TestOperationIsTerminal(t *testing.T) {
	tests := []struct {
		status   models.OperationStatus
		expected bool
	}{
		{models.OpStatusPending, false},
		{models.OpStatusRunning, false},
		{models.OpStatusCompleted, true},
		{models.OpStatusFailed, true},
		{models.OpStatusCancelled, true},
	}

	for _, tt := range tests {
		t.Run(string(tt.status), func(t *testing.T) {
			op := &models.Operation{Status: tt.status}
			if got := op.IsTerminal(); got != tt.expected {
				t.Errorf("IsTerminal() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestOperationIsAsync(t *testing.T) {
	tests := []struct {
		category models.OperationCategory
		expected bool
	}{
		{models.OpCategorySync, false},
		{models.OpCategoryAsync, true},
	}

	for _, tt := range tests {
		t.Run(string(tt.category), func(t *testing.T) {
			op := &models.Operation{Category: tt.category}
			if got := op.IsAsync(); got != tt.expected {
				t.Errorf("IsAsync() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestOperationCanCancel(t *testing.T) {
	tests := []struct {
		status   models.OperationStatus
		expected bool
	}{
		{models.OpStatusPending, true},
		{models.OpStatusRunning, true},
		{models.OpStatusCompleted, false},
		{models.OpStatusFailed, false},
		{models.OpStatusCancelled, false},
	}

	for _, tt := range tests {
		t.Run(string(tt.status), func(t *testing.T) {
			op := &models.Operation{Status: tt.status}
			if got := op.CanCancel(); got != tt.expected {
				t.Errorf("CanCancel() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestOperationTypeConstants(t *testing.T) {
	tests := []struct {
		constant models.OperationType
		expected string
	}{
		{models.OpVMCreate, "vm_create"},
		{models.OpVMStart, "vm_start"},
		{models.OpVMStop, "vm_stop"},
		{models.OpVMReboot, "vm_reboot"},
		{models.OpVMDelete, "vm_delete"},
		{models.OpImageImport, "image_import"},
		{models.OpNodeRegister, "node_register"},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			if string(tt.constant) != tt.expected {
				t.Errorf("expected %s, got %s", tt.expected, tt.constant)
			}
		})
	}
}

func TestOperationStatusConstants(t *testing.T) {
	tests := []struct {
		constant models.OperationStatus
		expected string
	}{
		{models.OpStatusPending, "pending"},
		{models.OpStatusRunning, "running"},
		{models.OpStatusCompleted, "completed"},
		{models.OpStatusFailed, "failed"},
		{models.OpStatusCancelled, "cancelled"},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			if string(tt.constant) != tt.expected {
				t.Errorf("expected %s, got %s", tt.expected, tt.constant)
			}
		})
	}
}

func TestOperationCategoryConstants(t *testing.T) {
	tests := []struct {
		constant models.OperationCategory
		expected string
	}{
		{models.OpCategorySync, "sync"},
		{models.OpCategoryAsync, "async"},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			if string(tt.constant) != tt.expected {
				t.Errorf("expected %s, got %s", tt.expected, tt.constant)
			}
		})
	}
}

func TestActorTypeConstants(t *testing.T) {
	tests := []struct {
		constant models.ActorType
		expected string
	}{
		{models.ActorTypeUser, "user"},
		{models.ActorTypeSystem, "system"},
		{models.ActorTypeScheduler, "scheduler"},
		{models.ActorTypeReconciler, "reconciler"},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			if string(tt.constant) != tt.expected {
				t.Errorf("expected %s, got %s", tt.expected, tt.constant)
			}
		})
	}
}
