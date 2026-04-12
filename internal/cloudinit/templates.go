package cloudinit

import (
	"context"
	"fmt"
	"regexp"
	"strings"
	"text/template"
	"time"
)

// CloudInitTemplate represents a reusable cloud-init configuration template
type CloudInitTemplate struct {
	ID          string   `json:"id"`
	Name        string   `json:"name"`
	Description string   `json:"description"`
	Content     string   `json:"content"`
	Variables   []string `json:"variables"`
	CreatedAt   string   `json:"created_at"`
}

// Predefined cloud-init template IDs
const (
	TemplateBasicUser   = "cit-basic"
	TemplateDockerReady = "cit-docker"
	TemplateKubernetes  = "cit-kubernetes"
)

// GetDefaultTemplates returns the built-in cloud-init templates
func GetDefaultTemplates() []CloudInitTemplate {
	return []CloudInitTemplate{
		{
			ID:          TemplateBasicUser,
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
			ID:          TemplateDockerReady,
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
			ID:          TemplateKubernetes,
			Name:        "Kubernetes Node",
			Description: "Ubuntu with containerd and Kubernetes tools prerequisites",
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

// TemplateService provides operations for managing cloud-init templates
type TemplateService struct {
	templates map[string]CloudInitTemplate
}

// NewTemplateService creates a new template service with default templates
func NewTemplateService() *TemplateService {
	svc := &TemplateService{
		templates: make(map[string]CloudInitTemplate),
	}

	// Load default templates
	for _, t := range GetDefaultTemplates() {
		svc.templates[t.ID] = t
	}

	return svc
}

// GetTemplate retrieves a template by ID
func (s *TemplateService) GetTemplate(id string) (*CloudInitTemplate, error) {
	t, ok := s.templates[id]
	if !ok {
		return nil, fmt.Errorf("template not found: %s", id)
	}
	return &t, nil
}

// ListTemplates returns all available templates
func (s *TemplateService) ListTemplates() []CloudInitTemplate {
	result := make([]CloudInitTemplate, 0, len(s.templates))
	for _, t := range s.templates {
		result = append(result, t)
	}
	return result
}

// AddTemplate adds a new custom template
func (s *TemplateService) AddTemplate(name, description, content string) (*CloudInitTemplate, error) {
	if name == "" {
		return nil, fmt.Errorf("template name is required")
	}
	if content == "" {
		return nil, fmt.Errorf("template content is required")
	}

	// Generate ID from name
	id := GenerateTemplateID(name)

	// Extract variables from content
	variables := ExtractTemplateVariables(content)

	t := CloudInitTemplate{
		ID:          id,
		Name:        name,
		Description: description,
		Content:     content,
		Variables:   variables,
		CreatedAt:   time.Now().UTC().Format(time.RFC3339),
	}

	s.templates[id] = t
	return &t, nil
}

// RenderCloudInitTemplate renders a template with the given variables
func RenderCloudInitTemplate(templateContent string, variables map[string]string) (string, error) {
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

// ExtractTemplateVariables extracts variable names from a template string
// Looks for patterns like {{.VariableName}}
func ExtractTemplateVariables(content string) []string {
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

// ValidateCloudInit performs basic validation on cloud-init content
func ValidateCloudInit(content string) error {
	if content == "" {
		return fmt.Errorf("cloud-init content is empty")
	}

	// Check for cloud-config header
	if !strings.Contains(content, "#cloud-config") {
		return fmt.Errorf("cloud-init content must contain '#cloud-config' header")
	}

	return nil
}

// RenderServiceTemplate renders a cloud-init template by ID with variables from the service
func (s *TemplateService) RenderServiceTemplate(id string, variables map[string]string) (string, error) {
	template, err := s.GetTemplate(id)
	if err != nil {
		return "", err
	}

	return RenderCloudInitTemplate(template.Content, variables)
}

// GenerateTemplateID creates a URL-safe ID from a template name
func GenerateTemplateID(name string) string {
	// Convert to lowercase and replace spaces with hyphens
	id := strings.ToLower(name)
	id = regexp.MustCompile(`[^a-z0-9-]+`).ReplaceAllString(id, "-")
	id = strings.Trim(id, "-")

	// Add timestamp to ensure uniqueness
	return fmt.Sprintf("cit-%s-%d", id, time.Now().Unix())
}

// MergeCloudInit merges multiple cloud-init configurations
// Later configurations override earlier ones
func MergeCloudInit(configs ...string) string {
	if len(configs) == 0 {
		return ""
	}

	if len(configs) == 1 {
		return configs[0]
	}

	var result strings.Builder
	result.WriteString("#cloud-config\n")
	result.WriteString("# Merged configuration\n\n")

	for i, cfg := range configs {
		if cfg == "" {
			continue
		}

		// Remove #cloud-config header from subsequent configs
		cleanCfg := strings.TrimPrefix(cfg, "#cloud-config")
		cleanCfg = strings.TrimSpace(cleanCfg)

		if cleanCfg != "" {
			result.WriteString(fmt.Sprintf("# Section %d\n", i+1))
			result.WriteString(cleanCfg)
			result.WriteString("\n\n")
		}
	}

	return result.String()
}

// TemplateRenderer handles template rendering with context
type TemplateRenderer struct {
	service *TemplateService
}

// NewTemplateRenderer creates a new template renderer
func NewTemplateRenderer() *TemplateRenderer {
	return &TemplateRenderer{
		service: NewTemplateService(),
	}
}

// Render renders a template with variables
func (r *TemplateRenderer) Render(ctx context.Context, templateID string, variables map[string]string) (string, error) {
	return r.service.RenderServiceTemplate(templateID, variables)
}

// PreviewCloudInitTemplate creates a preview of a rendered template
func PreviewCloudInitTemplate(templateContent string, variables map[string]string) (map[string]interface{}, error) {
	rendered, err := RenderCloudInitTemplate(templateContent, variables)
	if err != nil {
		return nil, err
	}

	return map[string]interface{}{
		"rendered":  rendered,
		"variables": variables,
	}, nil
}
