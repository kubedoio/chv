package quota

import (
	"context"
	"fmt"

	"github.com/chv/chv/internal/db"
	"github.com/google/uuid"
)

// Service provides quota management functionality
type Service struct {
	repo *db.Repository
}

// NewService creates a new quota service
func NewService(repo *db.Repository) *Service {
	return &Service{repo: repo}
}

// GetQuota retrieves the quota for a user
// If no quota exists, returns default quota
func (s *Service) GetQuota(ctx context.Context, userID string) (*Quota, error) {
	quota, err := s.repo.GetQuota(ctx, userID)
	if err != nil {
		return nil, err
	}

	if quota == nil {
		// Return default quota if none exists
		quota = DefaultQuota(userID)
	}

	return quota, nil
}

// SetQuota creates or updates a quota for a user
func (s *Service) SetQuota(ctx context.Context, quota *Quota) error {
	if quota.ID == "" {
		quota.ID = uuid.NewString()
	}

	return s.repo.UpsertQuota(ctx, quota)
}

// GetUsage retrieves current resource usage for a user
func (s *Service) GetUsage(ctx context.Context, userID string) (*Usage, error) {
	return s.repo.GetUserUsage(ctx, userID)
}

// GetUsageWithQuota retrieves both usage and quota for a user
func (s *Service) GetUsageWithQuota(ctx context.Context, userID string) (*UsageWithQuota, error) {
	quota, err := s.GetQuota(ctx, userID)
	if err != nil {
		return nil, fmt.Errorf("failed to get quota: %w", err)
	}

	usage, err := s.GetUsage(ctx, userID)
	if err != nil {
		return nil, fmt.Errorf("failed to get usage: %w", err)
	}

	return &UsageWithQuota{
		Quota: *quota,
		Usage: *usage,
	}, nil
}

// CheckQuota verifies if a user can allocate more of a specific resource
func (s *Service) CheckQuota(ctx context.Context, userID string, resource string, amount int) error {
	if amount <= 0 {
		return nil // No allocation needed
	}

	quota, err := s.GetQuota(ctx, userID)
	if err != nil {
		return fmt.Errorf("failed to get quota: %w", err)
	}

	usage, err := s.GetUsage(ctx, userID)
	if err != nil {
		return fmt.Errorf("failed to get usage: %w", err)
	}

	var current, limit int

	switch resource {
	case "vms":
		current = usage.VMs
		limit = quota.MaxVMs
	case "cpu":
		current = usage.CPUs
		limit = quota.MaxCPUs
	case "memory":
		if int64(usage.MemoryGB)+int64(amount) > int64(quota.MaxMemoryGB) {
			return fmt.Errorf("quota exceeded: memory usage would be %d GB (limit: %d GB)", usage.MemoryGB+amount, quota.MaxMemoryGB)
		}
		return nil
	case "storage":
		if int64(usage.StorageGB)+int64(amount) > int64(quota.MaxStorageGB) {
			return fmt.Errorf("quota exceeded: storage usage would be %d GB (limit: %d GB)", usage.StorageGB+amount, quota.MaxStorageGB)
		}
		return nil
	case "networks":
		current = usage.Networks
		limit = quota.MaxNetworks
	default:
		return fmt.Errorf("unknown resource type: %s", resource)
	}

	// For countable resources (VMs, CPU, networks)
	if current+amount > limit {
		return fmt.Errorf("quota exceeded: %s usage would be %d (limit: %d)", resource, current+amount, limit)
	}

	return nil
}

// CheckQuotaDetailed checks quota and returns detailed result
func (s *Service) CheckQuotaDetailed(ctx context.Context, userID string, resource string, amount int) (*CheckResult, error) {
	result := &CheckResult{
		Allowed:   true,
		Resource:  resource,
		Requested: amount,
	}

	if amount <= 0 {
		return result, nil
	}

	quota, err := s.GetQuota(ctx, userID)
	if err != nil {
		return nil, err
	}

	usage, err := s.GetUsage(ctx, userID)
	if err != nil {
		return nil, err
	}

	switch resource {
	case "vms":
		result.Current = usage.VMs
		result.Limit = quota.MaxVMs
		if usage.VMs+amount > quota.MaxVMs {
			result.Allowed = false
			result.Message = fmt.Sprintf("VM quota exceeded: %d + %d > %d", usage.VMs, amount, quota.MaxVMs)
		}
	case "cpu":
		result.Current = usage.CPUs
		result.Limit = quota.MaxCPUs
		if usage.CPUs+amount > quota.MaxCPUs {
			result.Allowed = false
			result.Message = fmt.Sprintf("CPU quota exceeded: %d + %d > %d", usage.CPUs, amount, quota.MaxCPUs)
		}
	case "memory":
		result.Current = usage.MemoryGB
		result.Limit = quota.MaxMemoryGB
		if usage.MemoryGB+amount > quota.MaxMemoryGB {
			result.Allowed = false
			result.Message = fmt.Sprintf("Memory quota exceeded: %d + %d > %d", usage.MemoryGB, amount, quota.MaxMemoryGB)
		}
	case "storage":
		result.Current = usage.StorageGB
		result.Limit = quota.MaxStorageGB
		if usage.StorageGB+amount > quota.MaxStorageGB {
			result.Allowed = false
			result.Message = fmt.Sprintf("Storage quota exceeded: %d + %d > %d", usage.StorageGB, amount, quota.MaxStorageGB)
		}
	case "networks":
		result.Current = usage.Networks
		result.Limit = quota.MaxNetworks
		if usage.Networks+amount > quota.MaxNetworks {
			result.Allowed = false
			result.Message = fmt.Sprintf("Network quota exceeded: %d + %d > %d", usage.Networks, amount, quota.MaxNetworks)
		}
	default:
		result.Allowed = false
		result.Message = fmt.Sprintf("unknown resource type: %s", resource)
	}

	return result, nil
}

// ListQuotas returns all quotas
func (s *Service) ListQuotas(ctx context.Context) ([]Quota, error) {
	return s.repo.ListQuotas(ctx)
}

// UpdateQuota updates a quota
func (s *Service) UpdateQuota(ctx context.Context, quota *Quota) error {
	return s.repo.UpdateQuota(ctx, quota)
}

// RefreshUsageCache recalculates and updates the usage cache for a user
func (s *Service) RefreshUsageCache(ctx context.Context, userID string) error {
	return s.repo.RefreshUserUsageCache(ctx, userID)
}
