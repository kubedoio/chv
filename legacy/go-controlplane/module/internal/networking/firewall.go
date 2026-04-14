package networking

import (
	"context"
	"database/sql"
	"fmt"
	"net"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/google/uuid"
)

// FirewallRule represents a firewall rule for VM traffic filtering
type FirewallRule struct {
	ID          string `json:"id"`
	VMID        string `json:"vm_id"`
	Direction   string `json:"direction"`   // "ingress" or "egress"
	Protocol    string `json:"protocol"`    // "tcp", "udp", "icmp", or "all"
	PortRange   string `json:"port_range,omitempty"`  // e.g., "80", "22-80", ""
	SourceCIDR  string `json:"source_cidr"` // e.g., "0.0.0.0/0", "10.0.0.0/24"
	Action      string `json:"action"`      // "allow" or "deny"
	Priority    int    `json:"priority"`    // 100-999, lower is evaluated first
	Description string `json:"description,omitempty"`
	CreatedAt   string `json:"created_at,omitempty"`
}

// Validate validates the firewall rule
func (f *FirewallRule) Validate() error {
	if f.VMID == "" {
		return fmt.Errorf("vm_id is required")
	}
	
	// Validate direction
	if f.Direction != "ingress" && f.Direction != "egress" {
		return fmt.Errorf("direction must be 'ingress' or 'egress'")
	}
	
	// Validate protocol
	validProtocols := map[string]bool{"tcp": true, "udp": true, "icmp": true, "all": true}
	if !validProtocols[f.Protocol] {
		return fmt.Errorf("protocol must be 'tcp', 'udp', 'icmp', or 'all'")
	}
	
	// Validate port range (only for tcp/udp)
	if f.PortRange != "" && (f.Protocol == "tcp" || f.Protocol == "udp") {
		if err := validatePortRange(f.PortRange); err != nil {
			return fmt.Errorf("invalid port_range: %w", err)
		}
	}
	
	// Validate source CIDR
	if f.SourceCIDR == "" {
		f.SourceCIDR = "0.0.0.0/0"
	}
	if _, _, err := net.ParseCIDR(f.SourceCIDR); err != nil {
		return fmt.Errorf("invalid source_cidr: %w", err)
	}
	
	// Validate action
	if f.Action != "allow" && f.Action != "deny" {
		return fmt.Errorf("action must be 'allow' or 'deny'")
	}
	
	// Validate priority
	if f.Priority < 100 || f.Priority > 999 {
		return fmt.Errorf("priority must be between 100 and 999")
	}
	
	return nil
}

// validatePortRange validates a port range string (e.g., "80", "22-80", "443,8443")
func validatePortRange(portRange string) error {
	// Check for single port or range
	if strings.Contains(portRange, "-") {
		parts := strings.Split(portRange, "-")
		if len(parts) != 2 {
			return fmt.Errorf("invalid port range format")
		}
		start, err := strconv.Atoi(strings.TrimSpace(parts[0]))
		if err != nil {
			return fmt.Errorf("invalid start port")
		}
		end, err := strconv.Atoi(strings.TrimSpace(parts[1]))
		if err != nil {
			return fmt.Errorf("invalid end port")
		}
		if start < 1 || start > 65535 || end < 1 || end > 65535 {
			return fmt.Errorf("ports must be between 1 and 65535")
		}
		if start >= end {
			return fmt.Errorf("start port must be less than end port")
		}
	} else if strings.Contains(portRange, ",") {
		// Multiple ports
		ports := strings.Split(portRange, ",")
		for _, p := range ports {
			port, err := strconv.Atoi(strings.TrimSpace(p))
			if err != nil {
				return fmt.Errorf("invalid port: %s", p)
			}
			if port < 1 || port > 65535 {
				return fmt.Errorf("port %d must be between 1 and 65535", port)
			}
		}
	} else {
		// Single port
		port, err := strconv.Atoi(portRange)
		if err != nil {
			return fmt.Errorf("invalid port")
		}
		if port < 1 || port > 65535 {
			return fmt.Errorf("port must be between 1 and 65535")
		}
	}
	return nil
}

// Matches checks if a packet matches this rule
func (f *FirewallRule) Matches(srcIP string, dstPort int, protocol string) bool {
	// Check protocol
	if f.Protocol != "all" && f.Protocol != protocol {
		return false
	}
	
	// Check source IP against CIDR
	if !f.matchesCIDR(srcIP) {
		return false
	}
	
	// Check port (for tcp/udp)
	if f.PortRange != "" && (protocol == "tcp" || protocol == "udp") {
		if !f.matchesPort(dstPort) {
			return false
		}
	}
	
	return true
}

// matchesCIDR checks if an IP matches the rule's CIDR
func (f *FirewallRule) matchesCIDR(ip string) bool {
	_, ipNet, err := net.ParseCIDR(f.SourceCIDR)
	if err != nil {
		return false
	}
	parsedIP := net.ParseIP(ip)
	if parsedIP == nil {
		return false
	}
	return ipNet.Contains(parsedIP)
}

// matchesPort checks if a port matches the rule's port range
func (f *FirewallRule) matchesPort(port int) bool {
	if f.PortRange == "" {
		return true
	}
	
	if strings.Contains(f.PortRange, "-") {
		parts := strings.Split(f.PortRange, "-")
		start, _ := strconv.Atoi(strings.TrimSpace(parts[0]))
		end, _ := strconv.Atoi(strings.TrimSpace(parts[1]))
		return port >= start && port <= end
	}
	
	if strings.Contains(f.PortRange, ",") {
		ports := strings.Split(f.PortRange, ",")
		for _, p := range ports {
			pInt, _ := strconv.Atoi(strings.TrimSpace(p))
			if port == pInt {
				return true
			}
		}
		return false
	}
	
	rulePort, _ := strconv.Atoi(f.PortRange)
	return port == rulePort
}

// FirewallRepository defines the interface for firewall database operations
type FirewallRepository interface {
	CreateFirewallRule(ctx context.Context, rule *FirewallRule) error
	GetFirewallRuleByID(ctx context.Context, id string) (*FirewallRule, error)
	ListFirewallRulesByVM(ctx context.Context, vmID string) ([]FirewallRule, error)
	UpdateFirewallRule(ctx context.Context, rule *FirewallRule) error
	DeleteFirewallRule(ctx context.Context, id string) error
	DeleteFirewallRulesByVM(ctx context.Context, vmID string) error
}

// FirewallManager manages firewall rules
type FirewallManager struct {
	repo FirewallRepository
}

// NewFirewallManager creates a new firewall manager
func NewFirewallManager(repo FirewallRepository) *FirewallManager {
	return &FirewallManager{repo: repo}
}

// ApplyFirewallRules applies a set of firewall rules to a VM
// This replaces all existing rules for the VM with the new set
func (m *FirewallManager) ApplyFirewallRules(ctx context.Context, vmID string, rules []FirewallRule) error {
	// Validate all rules first
	for i := range rules {
		rules[i].VMID = vmID
		if err := rules[i].Validate(); err != nil {
			return fmt.Errorf("rule %d validation failed: %w", i, err)
		}
	}
	
	// Delete existing rules
	if err := m.repo.DeleteFirewallRulesByVM(ctx, vmID); err != nil {
		return fmt.Errorf("failed to delete existing rules: %w", err)
	}
	
	// Create new rules
	now := time.Now().UTC().Format(time.RFC3339)
	for i := range rules {
		rules[i].ID = uuid.NewString()
		rules[i].CreatedAt = now
		if err := m.repo.CreateFirewallRule(ctx, &rules[i]); err != nil {
			return fmt.Errorf("failed to create rule %d: %w", i, err)
		}
	}
	
	return nil
}

// ListFirewallRules returns all firewall rules for a VM
func (m *FirewallManager) ListFirewallRules(ctx context.Context, vmID string) ([]FirewallRule, error) {
	rules, err := m.repo.ListFirewallRulesByVM(ctx, vmID)
	if err != nil {
		return nil, err
	}
	
	// Sort by priority (lower first)
	sort.Slice(rules, func(i, j int) bool {
		return rules[i].Priority < rules[j].Priority
	})
	
	return rules, nil
}

// AddFirewallRule adds a single firewall rule to a VM
func (m *FirewallManager) AddFirewallRule(ctx context.Context, rule *FirewallRule) error {
	if err := rule.Validate(); err != nil {
		return err
	}
	
	rule.ID = uuid.NewString()
	rule.CreatedAt = time.Now().UTC().Format(time.RFC3339)
	
	return m.repo.CreateFirewallRule(ctx, rule)
}

// DeleteFirewallRule removes a firewall rule
func (m *FirewallManager) DeleteFirewallRule(ctx context.Context, ruleID string) error {
	return m.repo.DeleteFirewallRule(ctx, ruleID)
}

// EvaluateRules evaluates all rules for a packet and returns the action
// Returns "allow" or "deny" based on the first matching rule
func (m *FirewallManager) EvaluateRules(ctx context.Context, vmID string, srcIP string, dstPort int, protocol string, direction string) (string, error) {
	rules, err := m.ListFirewallRules(ctx, vmID)
	if err != nil {
		return "deny", err
	}
	
	// Default deny if no rules match
	if len(rules) == 0 {
		return "allow", nil // Allow all if no rules defined
	}
	
	// Find first matching rule
	for _, rule := range rules {
		if rule.Direction != direction {
			continue
		}
		if rule.Matches(srcIP, dstPort, protocol) {
			return rule.Action, nil
		}
	}
	
	// No rule matched - default deny for ingress, allow for egress
	if direction == "ingress" {
		return "deny", nil
	}
	return "allow", nil
}

// EnsureDefaultRules creates default rules for a new VM
// This ensures SSH access is always available
func (m *FirewallManager) EnsureDefaultRules(ctx context.Context, vmID string) error {
	// Check if any rules exist
	rules, err := m.repo.ListFirewallRulesByVM(ctx, vmID)
	if err != nil {
		return err
	}
	
	if len(rules) > 0 {
		return nil // Rules already exist
	}
	
	// Create default SSH allow rule
	now := time.Now().UTC().Format(time.RFC3339)
	sshRule := &FirewallRule{
		ID:          uuid.NewString(),
		VMID:        vmID,
		Direction:   "ingress",
		Protocol:    "tcp",
		PortRange:   "22",
		SourceCIDR:  "0.0.0.0/0",
		Action:      "allow",
		Priority:    100,
		Description: "Default SSH access",
		CreatedAt:   now,
	}
	
	return m.repo.CreateFirewallRule(ctx, sshRule)
}

// DBFirewallRepository implements FirewallRepository using SQLite
type DBFirewallRepository struct {
	db *sql.DB
}

// NewDBFirewallRepository creates a new firewall database repository
func NewDBFirewallRepository(db *sql.DB) FirewallRepository {
	return &DBFirewallRepository{db: db}
}

// CreateFirewallRule creates a firewall rule
func (r *DBFirewallRepository) CreateFirewallRule(ctx context.Context, rule *FirewallRule) error {
	_, err := r.db.ExecContext(ctx,
		`INSERT INTO firewall_rules (id, vm_id, direction, protocol, port_range, source_cidr, action, priority, description, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		rule.ID, rule.VMID, rule.Direction, rule.Protocol, nullableString(rule.PortRange), 
		rule.SourceCIDR, rule.Action, rule.Priority, nullableString(rule.Description), rule.CreatedAt,
	)
	return err
}

// GetFirewallRuleByID retrieves a firewall rule by ID
func (r *DBFirewallRepository) GetFirewallRuleByID(ctx context.Context, id string) (*FirewallRule, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, vm_id, direction, protocol, port_range, source_cidr, action, priority, description, created_at
		 FROM firewall_rules WHERE id = ?`, id)
	
	return r.scanRule(row)
}

// ListFirewallRulesByVM retrieves all firewall rules for a VM
func (r *DBFirewallRepository) ListFirewallRulesByVM(ctx context.Context, vmID string) ([]FirewallRule, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, vm_id, direction, protocol, port_range, source_cidr, action, priority, description, created_at
		 FROM firewall_rules WHERE vm_id = ? ORDER BY priority ASC`, vmID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var rules []FirewallRule
	for rows.Next() {
		var rule FirewallRule
		var portRange, description sql.NullString
		err := rows.Scan(&rule.ID, &rule.VMID, &rule.Direction, &rule.Protocol, &portRange, 
			&rule.SourceCIDR, &rule.Action, &rule.Priority, &description, &rule.CreatedAt)
		if err != nil {
			return nil, err
		}
		if portRange.Valid {
			rule.PortRange = portRange.String
		}
		if description.Valid {
			rule.Description = description.String
		}
		rules = append(rules, rule)
	}
	return rules, rows.Err()
}

// UpdateFirewallRule updates a firewall rule
func (r *DBFirewallRepository) UpdateFirewallRule(ctx context.Context, rule *FirewallRule) error {
	_, err := r.db.ExecContext(ctx,
		`UPDATE firewall_rules SET direction = ?, protocol = ?, port_range = ?, source_cidr = ?, action = ?, priority = ?, description = ?
		 WHERE id = ?`,
		rule.Direction, rule.Protocol, nullableString(rule.PortRange), rule.SourceCIDR, 
		rule.Action, rule.Priority, nullableString(rule.Description), rule.ID,
	)
	return err
}

// DeleteFirewallRule removes a firewall rule
func (r *DBFirewallRepository) DeleteFirewallRule(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM firewall_rules WHERE id = ?`, id)
	return err
}

// DeleteFirewallRulesByVM removes all firewall rules for a VM
func (r *DBFirewallRepository) DeleteFirewallRulesByVM(ctx context.Context, vmID string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM firewall_rules WHERE vm_id = ?`, vmID)
	return err
}

func (r *DBFirewallRepository) scanRule(row *sql.Row) (*FirewallRule, error) {
	var rule FirewallRule
	var portRange, description sql.NullString
	err := row.Scan(&rule.ID, &rule.VMID, &rule.Direction, &rule.Protocol, &portRange, 
		&rule.SourceCIDR, &rule.Action, &rule.Priority, &description, &rule.CreatedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if portRange.Valid {
		rule.PortRange = portRange.String
	}
	if description.Valid {
		rule.Description = description.String
	}
	return &rule, nil
}

// RuleValidationError represents a validation error for firewall rules
type RuleValidationError struct {
	Field   string
	Message string
}

func (e *RuleValidationError) Error() string {
	return fmt.Sprintf("validation error for field '%s': %s", e.Field, e.Message)
}

// IsValidCIDR checks if a string is a valid CIDR notation
func IsValidCIDR(cidr string) bool {
	_, _, err := net.ParseCIDR(cidr)
	return err == nil
}

// NormalizeCIDR normalizes a CIDR string
func NormalizeCIDR(cidr string) (string, error) {
	_, ipNet, err := net.ParseCIDR(cidr)
	if err != nil {
		return "", err
	}
	return ipNet.String(), nil
}

// ExtractIP extracts the IP address from a CIDR
func ExtractIP(cidr string) string {
	ip, _, err := net.ParseCIDR(cidr)
	if err != nil {
		return ""
	}
	return ip.String()
}

// IsPrivateIP checks if an IP is in a private range
func IsPrivateIP(ip string) bool {
	parsedIP := net.ParseIP(ip)
	if parsedIP == nil {
		return false
	}
	
	privateRanges := []string{
		"10.0.0.0/8",
		"172.16.0.0/12",
		"192.168.0.0/16",
	}
	
	for _, cidr := range privateRanges {
		_, ipNet, _ := net.ParseCIDR(cidr)
		if ipNet.Contains(parsedIP) {
			return true
		}
	}
	return false
}

var validHostnameRegexp = regexp.MustCompile(`^[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?$`)

// IsValidHostname checks if a string is a valid hostname
func IsValidHostname(hostname string) bool {
	if len(hostname) > 253 {
		return false
	}
	return validHostnameRegexp.MatchString(hostname)
}
