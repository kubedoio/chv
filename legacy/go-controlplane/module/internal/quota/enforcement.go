package quota

import (
	"context"
	"fmt"
)

// Enforcement provides high-level quota enforcement helpers
type Enforcement struct {
	service *Service
}

// NewEnforcement creates a new quota enforcement helper
func NewEnforcement(service *Service) *Enforcement {
	return &Enforcement{service: service}
}

// CanCreateVM checks if a user can create a VM with the specified resources
// Returns nil if allowed, error with details if quota would be exceeded
func (e *Enforcement) CanCreateVM(ctx context.Context, userID string, vcpu int, memoryMB int64, storageGB int64) error {
	return e.service.CanCreateVM(ctx, userID, vcpu, memoryMB, storageGB)
}

// CanCreateNetwork checks if a user can create a network
func (e *Enforcement) CanCreateNetwork(ctx context.Context, userID string) error {
	return e.service.CheckQuota(ctx, userID, "networks", 1)
}

// CanCreateStoragePool checks if a user can create a storage pool
func (e *Enforcement) CanCreateStoragePool(ctx context.Context, userID string, sizeGB int64) error {
	return e.service.CheckQuota(ctx, userID, "storage", int(sizeGB))
}

// VMResources holds the resources required by a VM
type VMResources struct {
	VCPU      int
	MemoryMB  int64
	StorageGB int64
}

// CheckVMCreation checks quota for VM creation and returns detailed result
func (e *Enforcement) CheckVMCreation(ctx context.Context, userID string, resources VMResources) (*VMCreationCheckResult, error) {
	results := make(map[string]*ResourceCheck)

	// Check VMs count
	vmResult, err := e.service.CheckQuotaDetailed(ctx, userID, "vms", 1)
	if err != nil {
		return nil, err
	}
	results["vms"] = &ResourceCheck{
		Resource:  "vms",
		Requested: 1,
		Current:   vmResult.Current,
		Limit:     vmResult.Limit,
		Allowed:   vmResult.Allowed,
		Message:   vmResult.Message,
	}

	// Check CPU
	cpuResult, err := e.service.CheckQuotaDetailed(ctx, userID, "cpu", resources.VCPU)
	if err != nil {
		return nil, err
	}
	results["cpu"] = &ResourceCheck{
		Resource:  "cpu",
		Requested: resources.VCPU,
		Current:   cpuResult.Current,
		Limit:     cpuResult.Limit,
		Allowed:   cpuResult.Allowed,
		Message:   cpuResult.Message,
	}

	// Check Memory
	memoryGB := int(resources.MemoryMB / 1024)
	if resources.MemoryMB%1024 > 0 {
		memoryGB++ // Round up
	}
	memoryResult, err := e.service.CheckQuotaDetailed(ctx, userID, "memory", memoryGB)
	if err != nil {
		return nil, err
	}
	results["memory"] = &ResourceCheck{
		Resource:  "memory",
		Requested: memoryGB,
		Current:   memoryResult.Current,
		Limit:     memoryResult.Limit,
		Allowed:   memoryResult.Allowed,
		Message:   memoryResult.Message,
	}

	// Check Storage
	storageResult, err := e.service.CheckQuotaDetailed(ctx, userID, "storage", int(resources.StorageGB))
	if err != nil {
		return nil, err
	}
	results["storage"] = &ResourceCheck{
		Resource:  "storage",
		Requested: int(resources.StorageGB),
		Current:   storageResult.Current,
		Limit:     storageResult.Limit,
		Allowed:   storageResult.Allowed,
		Message:   storageResult.Message,
	}

	// Determine overall result
	overallAllowed := true
	var failedChecks []string
	for _, check := range results {
		if !check.Allowed {
			overallAllowed = false
			failedChecks = append(failedChecks, check.Message)
		}
	}

	return &VMCreationCheckResult{
		Allowed:   overallAllowed,
		Resources: results,
		Messages:  failedChecks,
	}, nil
}

// ResourceCheck represents the check result for a single resource
type ResourceCheck struct {
	Resource  string `json:"resource"`
	Requested int    `json:"requested"`
	Current   int    `json:"current"`
	Limit     int    `json:"limit"`
	Allowed   bool   `json:"allowed"`
	Message   string `json:"message,omitempty"`
}

// VMCreationCheckResult represents the overall check result for VM creation
type VMCreationCheckResult struct {
	Allowed   bool                     `json:"allowed"`
	Resources map[string]*ResourceCheck `json:"resources"`
	Messages  []string                 `json:"messages,omitempty"`
}

// UsageSummary provides a summary of usage vs quota
type UsageSummary struct {
	VMs       ResourceUsage
	CPUs      ResourceUsage
	Memory    ResourceUsage
	Storage   ResourceUsage
	Networks  ResourceUsage
}

// ResourceUsage represents usage for a single resource type
type ResourceUsage struct {
	Used      int    `json:"used"`
	Limit     int    `json:"limit"`
	Available int    `json:"available"`
	Percent   int    `json:"percent"`
}

// GetUsageSummary returns a summary of resource usage for a user
func (e *Enforcement) GetUsageSummary(ctx context.Context, userID string) (*UsageSummary, error) {
	usageWithQuota, err := e.service.GetUsageWithQuota(ctx, userID)
	if err != nil {
		return nil, err
	}

	makeUsage := func(used, limit int) ResourceUsage {
		available := limit - used
		if available < 0 {
			available = 0
		}
		percent := 0
		if limit > 0 {
			percent = (used * 100) / limit
		}
		return ResourceUsage{
			Used:      used,
			Limit:     limit,
			Available: available,
			Percent:   percent,
		}
	}

	return &UsageSummary{
		VMs:      makeUsage(usageWithQuota.Usage.VMs, usageWithQuota.Quota.MaxVMs),
		CPUs:     makeUsage(usageWithQuota.Usage.CPUs, usageWithQuota.Quota.MaxCPUs),
		Memory:   makeUsage(usageWithQuota.Usage.MemoryGB, usageWithQuota.Quota.MaxMemoryGB),
		Storage:  makeUsage(usageWithQuota.Usage.StorageGB, usageWithQuota.Quota.MaxStorageGB),
		Networks: makeUsage(usageWithQuota.Usage.Networks, usageWithQuota.Quota.MaxNetworks),
	}, nil
}

// QuotaExceededError represents a quota violation
type QuotaExceededError struct {
	Resource  string
	Requested int
	Current   int
	Limit     int
}

func (e *QuotaExceededError) Error() string {
	return fmt.Sprintf("quota exceeded for %s: requested %d, current %d, limit %d",
		e.Resource, e.Requested, e.Current, e.Limit)
}

// IsQuotaExceeded checks if an error is a quota exceeded error
func IsQuotaExceeded(err error) bool {
	if err == nil {
		return false
	}
	_, ok := err.(*QuotaExceededError)
	return ok
}

// GetQuotaStatus returns the status of a user's quota (for alerts)
type QuotaStatus struct {
	Resource  string `json:"resource"`
	Percent   int    `json:"percent"`
	IsWarning bool   `json:"is_warning"`
	IsCritical bool  `json:"is_critical"`
}

const (
	// WarningThreshold is the usage percentage at which to show warnings
	WarningThreshold = 80
	// CriticalThreshold is the usage percentage at which to show critical alerts
	CriticalThreshold = 95
)

// GetQuotaStatuses returns the status of all quotas for alerting
func (e *Enforcement) GetQuotaStatuses(ctx context.Context, userID string) ([]QuotaStatus, error) {
	summary, err := e.GetUsageSummary(ctx, userID)
	if err != nil {
		return nil, err
	}

	statuses := []QuotaStatus{
		makeStatus("vms", summary.VMs),
		makeStatus("cpu", summary.CPUs),
		makeStatus("memory", summary.Memory),
		makeStatus("storage", summary.Storage),
		makeStatus("networks", summary.Networks),
	}

	return statuses, nil
}

func makeStatus(resource string, usage ResourceUsage) QuotaStatus {
	return QuotaStatus{
		Resource:   resource,
		Percent:    usage.Percent,
		IsWarning:  usage.Percent >= WarningThreshold && usage.Percent < CriticalThreshold,
		IsCritical: usage.Percent >= CriticalThreshold,
	}
}
