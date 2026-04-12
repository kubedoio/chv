package quota

import "github.com/chv/chv/internal/models"

// Quota is an alias for models.Quota
type Quota = models.Quota

// Usage is an alias for models.ResourceUsage
type Usage = models.ResourceUsage

// UsageWithQuota combines usage and quota for a complete view
type UsageWithQuota struct {
	Quota Quota `json:"quota"`
	Usage Usage `json:"usage"`
}

// DefaultQuota returns a quota with default values
func DefaultQuota(userID string) *Quota {
	return &Quota{
		UserID:      userID,
		MaxVMs:      10,
		MaxCPUs:     20,
		MaxMemoryGB: 64,
		MaxStorageGB: 500,
		MaxNetworks: 5,
	}
}

// CheckResult represents the result of a quota check
type CheckResult struct {
	Allowed   bool   `json:"allowed"`
	Resource  string `json:"resource"`
	Requested int    `json:"requested"`
	Current   int    `json:"current"`
	Limit     int    `json:"limit"`
	Message   string `json:"message,omitempty"`
}


