package cloudinit

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"text/template"
)

// Config holds cloud-init configuration
type Config struct {
	VMID              string
	VMName            string
	Hostname          string
	Username          string
	Password          string
	SSHAuthorizedKeys []string
	UserData          string // Raw user-data override
}

// Validate checks required fields
func (c *Config) Validate() error {
	if c.VMID == "" {
		return fmt.Errorf("VMID is required")
	}
	if c.VMName == "" {
		return fmt.Errorf("VMName is required")
	}
	return nil
}

// Renderer generates cloud-init files
type Renderer struct {
	workspaceBase string
}

func NewRenderer(workspaceBase string) *Renderer {
	return &Renderer{workspaceBase: workspaceBase}
}

// Render generates all cloud-init files for a VM
func (r *Renderer) Render(ctx context.Context, vmID string, cfg Config) (*RenderResult, error) {
	if err := cfg.Validate(); err != nil {
		return nil, fmt.Errorf("invalid config: %w", err)
	}

	// VM workspace: {workspaceBase}/vms/{vmID}/cloudinit/
	cloudinitDir := filepath.Join(r.workspaceBase, "vms", vmID, "cloudinit")
	if err := os.MkdirAll(cloudinitDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create cloudinit dir: %w", err)
	}

	// Render user-data
	userDataPath := filepath.Join(cloudinitDir, "user-data")
	if err := r.renderUserData(userDataPath, cfg); err != nil {
		return nil, fmt.Errorf("failed to render user-data: %w", err)
	}

	// Render meta-data
	metaDataPath := filepath.Join(cloudinitDir, "meta-data")
	if err := r.renderMetaData(metaDataPath, cfg); err != nil {
		return nil, fmt.Errorf("failed to render meta-data: %w", err)
	}

	// Render network-config (optional)
	networkConfigPath := filepath.Join(cloudinitDir, "network-config")
	if err := r.renderNetworkConfig(networkConfigPath, cfg); err != nil {
		return nil, fmt.Errorf("failed to render network-config: %w", err)
	}

	return &RenderResult{
		UserDataPath:      userDataPath,
		MetaDataPath:      metaDataPath,
		NetworkConfigPath: networkConfigPath,
		CloudinitDir:      cloudinitDir,
	}, nil
}

func (r *Renderer) renderUserData(path string, cfg Config) error {
	// If raw user-data provided, use it
	if cfg.UserData != "" {
		return os.WriteFile(path, []byte(cfg.UserData), 0644)
	}

	// Otherwise, generate from template
	tmpl, err := template.New("user-data").Parse(userDataTemplate)
	if err != nil {
		return err
	}

	file, err := os.Create(path)
	if err != nil {
		return err
	}
	defer file.Close()

	return tmpl.Execute(file, cfg)
}

func (r *Renderer) renderMetaData(path string, cfg Config) error {
	tmpl, err := template.New("meta-data").Parse(metaDataTemplate)
	if err != nil {
		return err
	}

	file, err := os.Create(path)
	if err != nil {
		return err
	}
	defer file.Close()

	return tmpl.Execute(file, cfg)
}

func (r *Renderer) renderNetworkConfig(path string, cfg Config) error {
	// For MVP, use a simple DHCP config
	return os.WriteFile(path, []byte(networkConfigTemplate), 0644)
}

// RenderResult contains paths to generated cloud-init files
type RenderResult struct {
	UserDataPath      string
	MetaDataPath      string
	NetworkConfigPath string
	CloudinitDir      string
}

// Templates
const userDataTemplate = `#cloud-config
hostname: {{.VMName}}
manage_etc_hosts: true
{{- if .Username }}
users:
  - name: {{.Username}}
    sudo: ALL=(ALL) NOPASSWD:ALL
    {{- if .SSHAuthorizedKeys }}
    ssh_authorized_keys:
      {{- range .SSHAuthorizedKeys }}
      - {{.}}
      {{- end }}
    {{- end }}
{{- end }}
chpasswd:
  list: |
    {{.Username}}:{{.Password}}
  expire: False
package_update: true
packages:
  - qemu-guest-agent
`

const metaDataTemplate = `instance-id: {{.VMID}}
local-hostname: {{.VMName}}
`

const networkConfigTemplate = `version: 2
ethernets:
  eth0:
    dhcp4: true
    match:
      name: eth*
`

// Template represents a reusable cloud-init template
type Template struct {
	ID          string   `json:"id"`
	Name        string   `json:"name"`
	Description string   `json:"description"`
	Content     string   `json:"content"`
	Variables   []string `json:"variables"`
}

// DefaultTemplates returns the built-in cloud-init templates
func DefaultTemplates() []Template {
	return []Template{
		{
			ID:          "cit-basic",
			Name:        "Basic User Setup",
			Description: "Creates a user with sudo access and SSH key",
			Content: `#cloud-config
hostname: {{.Hostname}}
manage_etc_hosts: true
users:
  - name: {{.Username}}
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - {{.SSHKey}}
chpasswd:
  list: |
    {{.Username}}:{{.Password}}
  expire: False
package_update: true
packages:
  - qemu-guest-agent`,
			Variables: []string{"Hostname", "Username", "SSHKey", "Password"},
		},
		{
			ID:          "cit-docker",
			Name:        "Docker Ready",
			Description: "Ubuntu with Docker pre-installed",
			Content: `#cloud-config
package_update: true
packages:
  - docker.io
  - qemu-guest-agent
users:
  - name: {{.Username}}
    groups: docker
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - {{.SSHKey}}
runcmd:
  - systemctl enable docker
  - systemctl start docker`,
			Variables: []string{"Username", "SSHKey"},
		},
		{
			ID:          "cit-kubernetes",
			Name:        "Kubernetes Node",
			Description: "Ubuntu with containerd and Kubernetes tools",
			Content: `#cloud-config
package_update: true
packages:
  - apt-transport-https
  - ca-certificates
  - curl
  - gnupg
  - lsb-release
  - qemu-guest-agent
users:
  - name: {{.Username}}
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - {{.SSHKey}}
runcmd:
  - sysctl -w net.ipv4.ip_forward=1
  - echo "net.ipv4.ip_forward=1" >> /etc/sysctl.conf`,
			Variables: []string{"Username", "SSHKey"},
		},
	}
}

// RenderTemplate renders a template with the given variables
func RenderTemplate(templateContent string, variables map[string]string) (string, error) {
	if templateContent == "" {
		return "", nil
	}

	// Parse the template
	tmpl, err := template.New("cloudinit").Parse(templateContent)
	if err != nil {
		return "", fmt.Errorf("failed to parse template: %w", err)
	}

	// Execute with variables
	var buf strings.Builder
	if err := tmpl.Execute(&buf, variables); err != nil {
		return "", fmt.Errorf("failed to execute template: %w", err)
	}

	return buf.String(), nil
}

// ExtractVariables extracts variable names from a template string
// Looks for patterns like {{.VariableName}}
func ExtractVariables(content string) []string {
	var variables []string
	seen := make(map[string]bool)

	// Regex to match {{.VariableName}} or {{.VariableName | ... }}
	re := regexp.MustCompile(`\{\{\s*\.([A-Za-z][A-Za-z0-9_]*)\s*\}\}`)
	matches := re.FindAllStringSubmatch(content, -1)

	for _, match := range matches {
		if len(match) > 1 && !seen[match[1]] {
			seen[match[1]] = true
			variables = append(variables, match[1])
		}
	}

	return variables
}
