// chv-validator is a CLI tool to validate running VMs against the expected state
package main

import (
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"net/http"
	"os"
	"strings"
	"text/tabwriter"
	"time"
)

const (
	defaultControllerURL = "http://localhost:8080"
	defaultAgentURL      = "http://localhost:9090"
)

type ValidationResponse struct {
	Validation struct {
		RunningVMs []RunningVMInfo `json:"running_vms"`
		OrphanVMs  []RunningVMInfo `json:"orphan_vms"`
		MissingVMs []string        `json:"missing_vm_ids"`
		ValidVMs   []RunningVMInfo `json:"valid_vms"`
		Summary    struct {
			TotalRunning int `json:"total_running"`
			Valid        int `json:"valid"`
			Orphans      int `json:"orphans"`
			Missing      int `json:"missing"`
		} `json:"summary"`
	} `json:"validation"`
	Expected []string `json:"expected"`
}

type RunningVMInfo struct {
	PID           int    `json:"pid"`
	VMID          string `json:"vm_id"`
	SocketPath    string `json:"socket_path"`
	DiskPath      string `json:"disk_path"`
	SeedISOPath   string `json:"seed_iso_path"`
	VCPU          int    `json:"vcpu"`
	MemoryMB      int    `json:"memory_mb"`
	TAPDevice     string `json:"tap_device"`
	MACAddress    string `json:"mac_address"`
	IPAddress     string `json:"ip_address"`
	KernelPath    string `json:"kernel_path"`
	CommandLine   string `json:"command_line"`
	IsManaged     bool   `json:"is_managed"`
	WorkspacePath string `json:"workspace_path"`
}

type AgentValidationResponse struct {
	RunningVMs []RunningVMInfo `json:"running_vms"`
	OrphanVMs  []RunningVMInfo `json:"orphan_vms"`
	MissingVMs []string        `json:"missing_vm_ids"`
	ValidVMs   []RunningVMInfo `json:"valid_vms"`
	Summary    struct {
		TotalRunning int `json:"total_running"`
		Valid        int `json:"valid"`
		Orphans      int `json:"orphans"`
		Missing      int `json:"missing"`
	} `json:"summary"`
}

func main() {
	var (
		controllerURL = flag.String("controller", defaultControllerURL, "Controller API URL")
		agentURL      = flag.String("agent", "", "Agent API URL (direct mode)")
		authToken     = flag.String("token", os.Getenv("CHV_TOKEN"), "Auth token (or CHV_TOKEN env var)")
		verbose       = flag.Bool("v", false, "Verbose output")
		format        = flag.String("format", "table", "Output format: table, json")
	)
	flag.Parse()

	// Determine mode: controller or direct agent
	if *agentURL != "" {
		// Direct agent mode
		validateWithAgent(*agentURL, *authToken, *verbose, *format)
	} else {
		// Controller mode
		validateWithController(*controllerURL, *authToken, *verbose, *format)
	}
}

func validateWithController(url, token string, verbose bool, format string) {
	req, err := http.NewRequest("POST", url+"/api/v1/vms/validate", nil)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error creating request: %v\n", err)
		os.Exit(1)
	}

	req.Header.Set("Content-Type", "application/json")
	if token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	}

	client := &http.Client{Timeout: 30 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error connecting to controller: %v\n", err)
		os.Exit(1)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		fmt.Fprintf(os.Stderr, "Error: controller returned %s\n", resp.Status)
		os.Exit(1)
	}

	var result ValidationResponse
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		fmt.Fprintf(os.Stderr, "Error decoding response: %v\n", err)
		os.Exit(1)
	}

	outputResults(result.Validation.RunningVMs, result.Validation.OrphanVMs, 
		result.Validation.ValidVMs, result.Validation.MissingVMs, 
		result.Validation.Summary, verbose, format)
}

func validateWithAgent(url, token string, verbose bool, format string) {
	// Build request body
	reqBody := map[string]any{
		"expected_vm_ids": []string{}, // Empty means find all
	}
	body, _ := json.Marshal(reqBody)

	req, err := http.NewRequest("POST", url+"/v1/vms/validate", bytes.NewReader(body))
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error creating request: %v\n", err)
		os.Exit(1)
	}

	req.Header.Set("Content-Type", "application/json")
	if token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	}

	client := &http.Client{Timeout: 30 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error connecting to agent: %v\n", err)
		os.Exit(1)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		fmt.Fprintf(os.Stderr, "Error: agent returned %s\n", resp.Status)
		os.Exit(1)
	}

	var result AgentValidationResponse
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		fmt.Fprintf(os.Stderr, "Error decoding response: %v\n", err)
		os.Exit(1)
	}

	outputResults(result.RunningVMs, result.OrphanVMs, result.ValidVMs, 
		result.MissingVMs, result.Summary, verbose, format)
}

func outputResults(running, orphans, valid []RunningVMInfo, missing []string, 
	summary struct {
		TotalRunning int `json:"total_running"`
		Valid        int `json:"valid"`
		Orphans      int `json:"orphans"`
		Missing      int `json:"missing"`
	}, verbose bool, format string) {

	if format == "json" {
		output := map[string]any{
			"running_vms": running,
			"orphan_vms":  orphans,
			"missing_vms": missing,
			"valid_vms":   valid,
			"summary":     summary,
		}
		enc := json.NewEncoder(os.Stdout)
		enc.SetIndent("", "  ")
		enc.Encode(output)
		return
	}

	// Table format
	separator := strings.Repeat("=", 70)
	fmt.Println(separator)
	fmt.Println("VM VALIDATION REPORT")
	fmt.Println(separator)
	fmt.Println()

	// Summary
	fmt.Println("SUMMARY")
	fmt.Println("-------")
	fmt.Printf("Total Running VMs: %d\n", summary.TotalRunning)
	fmt.Printf("Valid (managed):   %d\n", summary.Valid)
	fmt.Printf("Orphans:           %d\n", summary.Orphans)
	fmt.Printf("Missing:           %d\n", summary.Missing)
	fmt.Println()

	// All running VMs
	if len(running) > 0 {
		fmt.Println("RUNNING VMs")
		fmt.Println("-----------")
		w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)
		fmt.Fprintln(w, "PID\tVM ID\tVCPU\tMEM\tIP\tMANAGED")
		for _, vm := range running {
			managed := "no"
			if vm.IsManaged {
				managed = "yes"
			}
			fmt.Fprintf(w, "%d\t%s\t%d\t%dM\t%s\t%s\n", 
				vm.PID, vm.VMID, vm.VCPU, vm.MemoryMB, vm.IPAddress, managed)
		}
		w.Flush()
		fmt.Println()
	}

	// Valid VMs
	if len(valid) > 0 {
		fmt.Println("VALID VMs (MANAGED)")
		fmt.Println("-------------------")
		w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)
		fmt.Fprintln(w, "PID\tVM ID\tVCPU\tMEM\tIP")
		for _, vm := range valid {
			fmt.Fprintf(w, "%d\t%s\t%d\t%dM\t%s\n", 
				vm.PID, vm.VMID, vm.VCPU, vm.MemoryMB, vm.IPAddress)
		}
		w.Flush()
		fmt.Println()
	}

	// Orphan VMs
	if len(orphans) > 0 {
		fmt.Println("⚠️  ORPHAN VMs (NOT MANAGED)")
		fmt.Println("------------------------------")
		for _, vm := range orphans {
			fmt.Printf("PID:      %d\n", vm.PID)
			fmt.Printf("VM ID:    %s\n", vm.VMID)
			fmt.Printf("VCPU:     %d\n", vm.VCPU)
			fmt.Printf("Memory:   %d MB\n", vm.MemoryMB)
			fmt.Printf("Disk:     %s\n", vm.DiskPath)
			fmt.Printf("Socket:   %s\n", vm.SocketPath)
			if vm.TAPDevice != "" {
				fmt.Printf("TAP:      %s\n", vm.TAPDevice)
			}
			if vm.IPAddress != "" {
				fmt.Printf("IP:       %s\n", vm.IPAddress)
			}
			if vm.MACAddress != "" {
				fmt.Printf("MAC:      %s\n", vm.MACAddress)
			}
			if verbose {
				fmt.Printf("Command:  %s\n", vm.CommandLine)
			}
			fmt.Println()
		}
	}

	// Missing VMs
	if len(missing) > 0 {
		fmt.Println("❌ MISSING VMs (EXPECTED BUT NOT RUNNING)")
		fmt.Println("------------------------------------------")
		for _, vmID := range missing {
			fmt.Printf("  - %s\n", vmID)
		}
		fmt.Println()
	}

	// Exit with error if issues found
	if summary.Orphans > 0 || summary.Missing > 0 {
		fmt.Println("⚠️  Validation found issues!")
		os.Exit(1)
	}

	fmt.Println("✅ All VMs are valid!")
}
