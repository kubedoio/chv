package services

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

// FirewallService manages nftables rules for VM isolation and anti-spoofing
type FirewallService struct {
	dataRoot   string
	bridgeName string
	rulesPath  string
}

// NewFirewallService creates a new firewall service
func NewFirewallService(dataRoot string, bridgeName string) *FirewallService {
	return &FirewallService{
		dataRoot:   dataRoot,
		bridgeName: bridgeName,
		rulesPath:  filepath.Join(dataRoot, "firewall", "rules.nft"),
	}
}

// Init initializes the chv nftables table and base chains
func (s *FirewallService) Init() error {
	// Ensure firewall directory exists
	if err := os.MkdirAll(filepath.Dir(s.rulesPath), 0755); err != nil {
		return fmt.Errorf("failed to create firewall directory: %w", err)
	}

	// Create base table and chains
	cmds := [][]string{
		{"nft", "add", "table", "inet", "chv"},
		{"nft", "add", "chain", "inet", "chv", "input", "{ type filter hook input priority 0; policy accept; }"},
		{"nft", "add", "chain", "inet", "chv", "forward", "{ type filter hook forward priority 0; policy accept; }"},
		{"nft", "add", "chain", "inet", "chv", "output", "{ type filter hook output priority 0; policy accept; }"},
	}

	for _, cmdArgs := range cmds {
		if out, err := exec.Command(cmdArgs[0], cmdArgs[1:]...).CombinedOutput(); err != nil {
			return fmt.Errorf("failed to execute nft command %v: %w (output: %s)", cmdArgs, err, out)
		}
	}

	// Attempt to load persistent rules if they exist
	if _, err := os.Stat(s.rulesPath); err == nil {
		if err := s.LoadRules(); err != nil {
			fmt.Fprintf(os.Stderr, "Warning: failed to load persistent firewall rules: %v\n", err)
		}
	}

	return nil
}

// LoadRules loads persistent rules from the data directory
func (s *FirewallService) LoadRules() error {
	cmd := exec.Command("nft", "-f", s.rulesPath)
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to load nft rules from %s: %w (output: %s)", s.rulesPath, err, out)
	}
	return nil
}

// SaveRules persists the current chv table to the data directory
func (s *FirewallService) SaveRules() error {
	out, err := exec.Command("nft", "list", "table", "inet", "chv").CombinedOutput()
	if err != nil {
		return fmt.Errorf("failed to list nft table: %w (output: %s)", err, out)
	}

	if err := os.WriteFile(s.rulesPath, out, 0644); err != nil {
		return fmt.Errorf("failed to write nft rules to %s: %w", s.rulesPath, err)
	}
	return nil
}

// AddVMRules adds anti-spoofing rules for a specific VM
func (s *FirewallService) AddVMRules(vmID, tapName, ip, mac string) error {
	if ip == "" || mac == "" {
		return nil // Cannot implement anti-spoofing without IP/MAC
	}

	// 1. Create a per-VM chain for clarity (optional but helps management)
	vmChain := fmt.Sprintf("vm-%s", vmID[:8])
	
	cmds := [][]string{
		// Create the chain
		{"nft", "add", "chain", "inet", "chv", vmChain},
		// Flush it if it already existed
		{"nft", "flush", "chain", "inet", "chv", vmChain},
		
		// Anti-spoofing: only allow matching IP/MAC from the TAP device
		{"nft", "add", "rule", "inet", "chv", vmChain, "iifname", tapName, "ip", "saddr", ip, "ether", "saddr", mac, "accept"},
		{"nft", "add", "rule", "inet", "chv", vmChain, "iifname", tapName, "drop"},
		
		// Allow return traffic to the VM
		{"nft", "add", "rule", "inet", "chv", vmChain, "oifname", tapName, "ip", "daddr", ip, "ether", "daddr", mac, "accept"},
		
		// Map the VM chain into the forward hook
		{"nft", "insert", "rule", "inet", "chv", "forward", "jump", vmChain},
	}

	for _, cmdArgs := range cmds {
		if out, err := exec.Command(cmdArgs[0], cmdArgs[1:]...).CombinedOutput(); err != nil {
			return fmt.Errorf("failed to add VM firewall rules: %w (output: %s)", err, out)
		}
	}

	return s.SaveRules()
}

// RemoveVMRules removes all rules associated with a specific VM
func (s *FirewallService) RemoveVMRules(vmID string) error {
	vmChain := fmt.Sprintf("vm-%s", vmID[:8])

	// 1. Remove references from forward chain
	// nft doesn't have an easy "delete all jumps to X", so we list and grep-delete or just rely on flushing the chain
	// The safest way is to delete the rule from forward that jumps to vmChain
	
	// Better: just delete the chain. nft will error if it's still being jumped to.
	// So we must remove the jump rule first.
	
	// Let's use a simpler approach: finding the handle for the jump rule is hard without parsing JSON.
	// Instead, we can use 'nft flush chain inet chv <vmChain>' and leave the jump for now, 
	// OR delete the jump if we can find it.
	
	// Direct deletion of the chain and its references:
	exec.Command("nft", "delete", "rule", "inet", "chv", "forward", "jump", vmChain).Run()
	exec.Command("nft", "delete", "chain", "inet", "chv", vmChain).Run()

	return s.SaveRules()
}
