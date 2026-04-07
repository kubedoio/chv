// Package quota provides resource quota enforcement for users.
package quota

import (
	"context"
	"fmt"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/store"
	"github.com/google/uuid"
)

// Service handles resource quota enforcement.
type Service struct {
	store store.Store
}

// NewService creates a new quota service.
func NewService(store store.Store) *Service {
	return &Service{store: store}
}

// QuotaExceededError represents a quota violation.
type QuotaExceededError struct {
	Resource string
	Used     int64
	Limit    int64
	Delta    int64
}

func (e *QuotaExceededError) Error() string {
	return fmt.Sprintf("%s quota exceeded: using %d of %d (requested %d)",
		e.Resource, e.Used, e.Limit, e.Delta)
}

// CheckQuota verifies if a user can allocate the requested resources.
func (s *Service) CheckQuota(ctx context.Context, userID string, cpu int, memoryMB int64, vmCount int, diskGB int64) error {
	if userID == "" || userID == "anonymous" {
		// Skip quota check for anonymous users or system operations
		return nil
	}

	// Ensure quota exists (creates default if not set)
	if err := s.store.EnsureQuota(ctx, userID); err != nil {
		return fmt.Errorf("failed to ensure quota: %w", err)
	}

	// Get quota
	quota, err := s.store.GetQuota(ctx, userID)
	if err != nil {
		return fmt.Errorf("failed to get quota: %w", err)
	}
	if quota == nil {
		// Should not happen after EnsureQuota, but handle gracefully
		uid, _ := uuid.Parse(userID)
		quota = models.DefaultQuota(uid)
	}

	// Get current usage
	usage, err := s.store.GetUsage(ctx, userID)
	if err != nil {
		return fmt.Errorf("failed to get resource usage: %w", err)
	}

	// Check CPU quota
	if cpu > 0 && usage.CPUsUsed+cpu > quota.MaxCPUs {
		return &QuotaExceededError{
			Resource: "CPU",
			Used:     int64(usage.CPUsUsed),
			Limit:    int64(quota.MaxCPUs),
			Delta:    int64(cpu),
		}
	}

	// Check memory quota
	if memoryMB > 0 && usage.MemoryMBUsed+memoryMB > quota.MaxMemoryMB {
		return &QuotaExceededError{
			Resource: "memory",
			Used:     usage.MemoryMBUsed,
			Limit:    quota.MaxMemoryMB,
			Delta:    memoryMB,
		}
	}

	// Check VM count quota
	if vmCount > 0 && usage.VMCount+vmCount > quota.MaxVMCount {
		return &QuotaExceededError{
			Resource: "VM count",
			Used:     int64(usage.VMCount),
			Limit:    int64(quota.MaxVMCount),
			Delta:    int64(vmCount),
		}
	}

	// Check disk quota
	if diskGB > 0 && usage.DiskGBUsed+diskGB > quota.MaxDiskGB {
		return &QuotaExceededError{
			Resource: "disk",
			Used:     usage.DiskGBUsed,
			Limit:    quota.MaxDiskGB,
			Delta:    diskGB,
		}
	}

	return nil
}

// CheckVMCreation checks if a user can create a VM with the given spec.
func (s *Service) CheckVMCreation(ctx context.Context, userID string, spec *models.VMSpec, diskSizeGB int64) error {
	return s.CheckQuota(ctx, userID, int(spec.CPU), spec.MemoryMB, 1, diskSizeGB)
}

// UpdateUsageForVMCreation updates resource usage when a VM is created.
func (s *Service) UpdateUsageForVMCreation(ctx context.Context, userID string, spec *models.VMSpec, diskSizeGB int64) error {
	if userID == "" || userID == "anonymous" {
		return nil
	}

	delta := models.ResourceUsage{
		CPUsUsed:     int(spec.CPU),
		MemoryMBUsed: spec.MemoryMB,
		VMCount:      1,
		DiskGBUsed:   diskSizeGB,
	}
	return s.store.UpdateUsage(ctx, userID, delta)
}

// UpdateUsageForVMDeletion updates resource usage when a VM is deleted.
func (s *Service) UpdateUsageForVMDeletion(ctx context.Context, userID string, spec *models.VMSpec, diskSizeGB int64) error {
	if userID == "" || userID == "anonymous" {
		return nil
	}

	delta := models.ResourceUsage{
		CPUsUsed:     -int(spec.CPU),
		MemoryMBUsed: -spec.MemoryMB,
		VMCount:      -1,
		DiskGBUsed:   -diskSizeGB,
	}
	return s.store.UpdateUsage(ctx, userID, delta)
}

// GetQuotaAndUsage returns the quota and current usage for a user.
func (s *Service) GetQuotaAndUsage(ctx context.Context, userID string) (*models.ResourceQuota, *models.ResourceUsage, error) {
	if userID == "" || userID == "anonymous" {
		return nil, nil, fmt.Errorf("invalid user ID")
	}

	// Ensure quota exists
	if err := s.store.EnsureQuota(ctx, userID); err != nil {
		return nil, nil, fmt.Errorf("failed to ensure quota: %w", err)
	}

	quota, err := s.store.GetQuota(ctx, userID)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to get quota: %w", err)
	}

	usage, err := s.store.GetUsage(ctx, userID)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to get usage: %w", err)
	}

	return quota, usage, nil
}

// SetQuota sets a custom quota for a user (admin only).
func (s *Service) SetQuota(ctx context.Context, quota *models.ResourceQuota) error {
	return s.store.SetQuota(ctx, quota)
}
