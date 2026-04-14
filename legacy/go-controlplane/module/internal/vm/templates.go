package vm

import (
	"context"
	"fmt"
	"regexp"
	"strings"
	"text/template"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
)

// Re-export types from models for backwards compatibility
type VMTemplate = models.VMTemplate
type CloudInitTemplate = models.CloudInitTemplate

// CreateTemplateInput holds parameters for creating a VM template
type CreateTemplateInput struct {
	SourceVMID      string
	Name            string
	Description     string
	VCPU            int
	MemoryMB        int
	CloudInitConfig string
	Tags            []string
}

// CloneFromTemplateInput holds parameters for cloning a VM from a template
type CloneFromTemplateInput struct {
	Name            string
	CloudInitVars   map[string]string
	CustomUserData  string
}

// CreateTemplate creates a new VM template from an existing VM or configuration
func (s *Service) CreateTemplate(ctx context.Context, input CreateTemplateInput) (*VMTemplate, error) {
	// Validate inputs
	if input.Name == "" {
		return nil, fmt.Errorf("template name is required")
	}

	// Get local node ID
	localNode, err := s.repo.GetLocalNode(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to get local node: %w", err)
	}
	if localNode == nil {
		return nil, fmt.Errorf("local node not found")
	}

	var imageID, networkID, storagePoolID string
	var vcpu, memoryMB int

	// If source VM provided, copy its configuration
	if input.SourceVMID != "" {
		sourceVM, err := s.repo.GetVMByID(ctx, input.SourceVMID)
		if err != nil {
			return nil, fmt.Errorf("failed to get source VM: %w", err)
		}
		if sourceVM == nil {
			return nil, fmt.Errorf("source VM not found: %s", input.SourceVMID)
		}

		imageID = sourceVM.ImageID
		networkID = sourceVM.NetworkID
		storagePoolID = sourceVM.StoragePoolID
		vcpu = sourceVM.VCPU
		memoryMB = sourceVM.MemoryMB
	}

	// Override with explicit values if provided
	if input.VCPU > 0 {
		vcpu = input.VCPU
	}
	if input.MemoryMB > 0 {
		memoryMB = input.MemoryMB
	}

	// Validate required fields
	if vcpu == 0 {
		vcpu = 2 // Default
	}
	if memoryMB == 0 {
		memoryMB = 2048 // Default
	}

	template := &VMTemplate{
		ID:              uuid.NewString(),
		NodeID:          localNode.ID,
		Name:            input.Name,
		Description:     input.Description,
		VCPU:            vcpu,
		MemoryMB:        memoryMB,
		ImageID:         imageID,
		NetworkID:       networkID,
		StoragePoolID:   storagePoolID,
		CloudInitConfig: input.CloudInitConfig,
		Tags:            input.Tags,
		CreatedAt:       time.Now().UTC().Format(time.RFC3339),
	}

	if err := s.repo.CreateVMTemplate(ctx, template); err != nil {
		return nil, fmt.Errorf("failed to create template: %w", err)
	}

	return template, nil
}

// CloneFromTemplate creates a new VM from a template
func (s *Service) CloneFromTemplate(ctx context.Context, templateID string, input CloneFromTemplateInput) (*models.VirtualMachine, error) {
	// Get the template
	template, err := s.repo.GetVMTemplate(ctx, templateID)
	if err != nil {
		return nil, fmt.Errorf("failed to get template: %w", err)
	}
	if template == nil {
		return nil, fmt.Errorf("template not found: %s", templateID)
	}

	// Validate name
	if input.Name == "" {
		return nil, fmt.Errorf("VM name is required")
	}

	// Build cloud-init config
	var userData string
	if input.CustomUserData != "" {
		userData = input.CustomUserData
	} else if template.CloudInitConfig != "" {
		// Render template with variables
		rendered, err := RenderCloudInitTemplate(template.CloudInitConfig, input.CloudInitVars)
		if err != nil {
			return nil, fmt.Errorf("failed to render cloud-init template: %w", err)
		}
		userData = rendered
	}

	// Get template cloud-init vars for username/ssh keys
	username := ""
	var sshKeys []string
	if input.CloudInitVars != nil {
		username = input.CloudInitVars["Username"]
		if sshKey := input.CloudInitVars["SSHKey"]; sshKey != "" {
			sshKeys = []string{sshKey}
		}
	}

	// Create the VM using the template configuration
	vmInput := CreateVMInput{
		Name:              input.Name,
		ImageID:           template.ImageID,
		StoragePoolID:     template.StoragePoolID,
		NetworkID:         template.NetworkID,
		VCPU:              template.VCPU,
		MemoryMB:          template.MemoryMB,
		UserData:          userData,
		Username:          username,
		SSHAuthorizedKeys: sshKeys,
	}

	return s.CreateVM(ctx, vmInput)
}

// ListTemplates returns all VM templates for a node
func (s *Service) ListTemplates(ctx context.Context, nodeID string) ([]VMTemplate, error) {
	return s.repo.ListVMTemplates(ctx, nodeID)
}

// GetTemplate returns a single VM template by ID
func (s *Service) GetTemplate(ctx context.Context, id string) (*VMTemplate, error) {
	return s.repo.GetVMTemplate(ctx, id)
}

// DeleteTemplate deletes a VM template
func (s *Service) DeleteTemplate(ctx context.Context, id string) error {
	return s.repo.DeleteVMTemplate(ctx, id)
}

// UpdateTemplate updates a VM template
func (s *Service) UpdateTemplate(ctx context.Context, template *VMTemplate) error {
	return s.repo.UpdateVMTemplate(ctx, template)
}

// RenderCloudInitTemplate renders a cloud-init template with the given variables
func RenderCloudInitTemplate(content string, variables map[string]string) (string, error) {
	if content == "" {
		return "", nil
	}

	// Parse the template
	tmpl, err := template.New("cloudinit").Parse(content)
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

// DefaultCloudInitTemplates returns the built-in cloud-init templates
func DefaultCloudInitTemplates() []CloudInitTemplate {
	return []CloudInitTemplate{
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

// ListCloudInitTemplates returns all cloud-init templates
func (s *Service) ListCloudInitTemplates(ctx context.Context) ([]CloudInitTemplate, error) {
	return s.repo.ListCloudInitTemplates(ctx)
}

// GetCloudInitTemplate returns a single cloud-init template by ID
func (s *Service) GetCloudInitTemplate(ctx context.Context, id string) (*CloudInitTemplate, error) {
	return s.repo.GetCloudInitTemplate(ctx, id)
}

// CreateCloudInitTemplate creates a new cloud-init template
func (s *Service) CreateCloudInitTemplate(ctx context.Context, name, description, content string) (*CloudInitTemplate, error) {
	if name == "" {
		return nil, fmt.Errorf("template name is required")
	}
	if content == "" {
		return nil, fmt.Errorf("template content is required")
	}

	// Extract variables from content
	variables := ExtractVariables(content)

	template := &CloudInitTemplate{
		ID:          uuid.NewString(),
		Name:        name,
		Description: description,
		Content:     content,
		Variables:   variables,
		CreatedAt:   time.Now().UTC().Format(time.RFC3339),
	}

	if err := s.repo.CreateCloudInitTemplate(ctx, template); err != nil {
		return nil, fmt.Errorf("failed to create cloud-init template: %w", err)
	}

	return template, nil
}

// DeleteCloudInitTemplate deletes a cloud-init template
func (s *Service) DeleteCloudInitTemplate(ctx context.Context, id string) error {
	return s.repo.DeleteCloudInitTemplate(ctx, id)
}

// RenderCloudInitTemplateByID renders a cloud-init template by ID with variables
func (s *Service) RenderCloudInitTemplateByID(ctx context.Context, id string, variables map[string]string) (string, error) {
	template, err := s.repo.GetCloudInitTemplate(ctx, id)
	if err != nil {
		return "", fmt.Errorf("failed to get cloud-init template: %w", err)
	}
	if template == nil {
		return "", fmt.Errorf("cloud-init template not found: %s", id)
	}

	return RenderCloudInitTemplate(template.Content, variables)
}

// PreviewCloudInitTemplate creates a preview of what the rendered cloud-init will look like
func (s *Service) PreviewCloudInitTemplate(ctx context.Context, templateID string, variables map[string]string) (map[string]any, error) {
	rendered, err := s.RenderCloudInitTemplateByID(ctx, templateID, variables)
	if err != nil {
		return nil, err
	}

	// Parse to validate YAML
	// Note: We don't actually parse as YAML here, just return the rendered content
	// The cloud-init service will validate when creating the VM
	return map[string]any{
		"template_id": templateID,
		"rendered":    rendered,
		"variables":   variables,
	}, nil
}

// TemplatePreview generates a preview of a VM that would be created from a template
func (s *Service) TemplatePreview(ctx context.Context, templateID string) (*VMTemplate, error) {
	template, err := s.repo.GetVMTemplate(ctx, templateID)
	if err != nil {
		return nil, err
	}
	if template == nil {
		return nil, fmt.Errorf("template not found")
	}

	// Get related resources for display
	image, _ := s.repo.GetImageByID(ctx, template.ImageID)
	network, _ := s.repo.GetNetworkByID(ctx, template.NetworkID)
	pool, _ := s.repo.GetStoragePoolByID(ctx, template.StoragePoolID)

	// Create a preview with resource names
	preview := &VMTemplate{
		ID:            template.ID,
		NodeID:        template.NodeID,
		Name:          template.Name,
		Description:   template.Description,
		VCPU:          template.VCPU,
		MemoryMB:      template.MemoryMB,
		ImageID:       template.ImageID,
		NetworkID:     template.NetworkID,
		StoragePoolID: template.StoragePoolID,
		Tags:          template.Tags,
		CreatedAt:     template.CreatedAt,
	}

	// Add resource names as additional context
	preview.Tags = append(preview.Tags, fmt.Sprintf("image:%s", template.ImageID))
	if image != nil {
		preview.Tags = append(preview.Tags, fmt.Sprintf("image_name:%s", image.Name))
	}
	if network != nil {
		preview.Tags = append(preview.Tags, fmt.Sprintf("network_name:%s", network.Name))
	}
	if pool != nil {
		preview.Tags = append(preview.Tags, fmt.Sprintf("pool_name:%s", pool.Name))
	}

	return preview, nil
}

// Helper function to ensure templates exist (called during initialization)
func (s *Service) EnsureDefaultCloudInitTemplates(ctx context.Context) error {
	templates := DefaultCloudInitTemplates()
	for _, t := range templates {
		// Check if template exists
		existing, err := s.repo.GetCloudInitTemplate(ctx, t.ID)
		if err != nil {
			return err
		}
		if existing == nil {
			// Create the default template
			if err := s.repo.CreateCloudInitTemplate(ctx, &t); err != nil {
				return err
			}
		}
	}
	return nil
}
