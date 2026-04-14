package api

import (
	"net/http"
	"strings"

	"github.com/chv/chv/internal/vm"
	"github.com/go-chi/chi/v5"
)

// VM Template handlers

type createVMTemplateRequest struct {
	SourceVMID      string   `json:"source_vm_id,omitempty"`
	Name            string   `json:"name"`
	Description     string   `json:"description,omitempty"`
	VCPU            int      `json:"vcpu,omitempty"`
	MemoryMB        int      `json:"memory_mb,omitempty"`
	CloudInitConfig string   `json:"cloud_init_config,omitempty"`
	Tags            []string `json:"tags,omitempty"`
}

func (h *Handler) listVMTemplates(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)

	templates, err := h.vmService.ListTemplates(ctx, "")
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "list_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, templates)
}

func (h *Handler) createVMTemplate(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)

	var req createVMTemplateRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON",
			Retryable: false,
		})
		return
	}

	// Validation
	if req.Name == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Template name is required",
			Retryable: false,
		})
		return
	}

	if h.vmService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:      "service_unavailable",
			Message:   "VM service not available",
			Retryable: true,
		})
		return
	}

	template, err := h.vmService.CreateTemplate(ctx, vm.CreateTemplateInput{
		SourceVMID:      req.SourceVMID,
		Name:            req.Name,
		Description:     req.Description,
		VCPU:            req.VCPU,
		MemoryMB:        req.MemoryMB,
		CloudInitConfig: req.CloudInitConfig,
		Tags:            req.Tags,
	})
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "template_create_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusCreated, template)
}

func (h *Handler) getVMTemplate(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	templateID := chi.URLParam(r, "id")

	template, err := h.vmService.GetTemplate(ctx, templateID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "template_get_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	if template == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:      "not_found",
			Message:   "Template not found",
			Retryable: false,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, template)
}

func (h *Handler) deleteVMTemplate(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	templateID := chi.URLParam(r, "id")

	if err := h.vmService.DeleteTemplate(ctx, templateID); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "template_delete_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"message": "Template deleted successfully",
	})
}

type cloneFromTemplateRequest struct {
	Name           string            `json:"name"`
	Variables      map[string]string `json:"variables,omitempty"`
	CustomUserData string            `json:"custom_user_data,omitempty"`
}

func (h *Handler) cloneFromTemplate(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	templateID := chi.URLParam(r, "id")

	var req cloneFromTemplateRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON",
			Retryable: false,
		})
		return
	}

	if req.Name == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "VM name is required",
			Retryable: false,
		})
		return
	}

	if h.vmService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:      "service_unavailable",
			Message:   "VM service not available",
			Retryable: true,
		})
		return
	}

	vm, err := h.vmService.CloneFromTemplate(ctx, templateID, vm.CloneFromTemplateInput{
		Name:           req.Name,
		CloudInitVars:  req.Variables,
		CustomUserData: req.CustomUserData,
	})
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "clone_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusCreated, vm)
}

// Cloud-init Template handlers

func (h *Handler) listCloudInitTemplates(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)

	templates, err := h.vmService.ListCloudInitTemplates(ctx)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "list_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, templates)
}

func (h *Handler) getCloudInitTemplate(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	templateID := chi.URLParam(r, "id")

	template, err := h.vmService.GetCloudInitTemplate(ctx, templateID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "template_get_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	if template == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:      "not_found",
			Message:   "Template not found",
			Retryable: false,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, template)
}

type createCloudInitTemplateRequest struct {
	Name        string `json:"name"`
	Description string `json:"description,omitempty"`
	Content     string `json:"content"`
}

func (h *Handler) createCloudInitTemplate(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)

	var req createCloudInitTemplateRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON",
			Retryable: false,
		})
		return
	}

	if req.Name == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Template name is required",
			Retryable: false,
		})
		return
	}

	if req.Content == "" {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Template content is required",
			Retryable: false,
		})
		return
	}

	if h.vmService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:      "service_unavailable",
			Message:   "VM service not available",
			Retryable: true,
		})
		return
	}

	template, err := h.vmService.CreateCloudInitTemplate(ctx, req.Name, req.Description, req.Content)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "template_create_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusCreated, template)
}

func (h *Handler) deleteCloudInitTemplate(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	templateID := chi.URLParam(r, "id")

	if err := h.vmService.DeleteCloudInitTemplate(ctx, templateID); err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "template_delete_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"message": "Template deleted successfully",
	})
}

type renderCloudInitTemplateRequest struct {
	Variables map[string]string `json:"variables"`
}

type renderCloudInitTemplateResponse struct {
	TemplateID string            `json:"template_id"`
	Rendered   string            `json:"rendered"`
	Variables  map[string]string `json:"variables"`
}

func (h *Handler) renderCloudInitTemplate(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	templateID := chi.URLParam(r, "id")

	var req renderCloudInitTemplateRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON",
			Retryable: false,
		})
		return
	}

	if h.vmService == nil {
		h.writeError(w, http.StatusServiceUnavailable, apiError{
			Code:      "service_unavailable",
			Message:   "VM service not available",
			Retryable: true,
		})
		return
	}

	rendered, err := h.vmService.RenderCloudInitTemplateByID(ctx, templateID, req.Variables)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "render_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, renderCloudInitTemplateResponse{
		TemplateID: templateID,
		Rendered:   rendered,
		Variables:  req.Variables,
	})
}

func (h *Handler) previewVMTemplate(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	templateID := chi.URLParam(r, "id")

	preview, err := h.vmService.TemplatePreview(ctx, templateID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "preview_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, preview)
}

// Cloud-init apply request
type applyCloudInitRequest struct {
	TemplateID string            `json:"template_id,omitempty"`
	Variables  map[string]string `json:"variables,omitempty"`
	UserData   string            `json:"user_data,omitempty"`
}

// applyCloudInit applies a cloud-init configuration to an existing VM
// Note: This regenerates the cloud-init ISO but requires VM restart to take effect
func (h *Handler) applyCloudInit(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)
	vmID := chi.URLParam(r, "id")

	var req applyCloudInitRequest
	if err := decodeJSON(r, &req); err != nil {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Request body must be valid JSON",
			Retryable: false,
		})
		return
	}

	// Get the VM
	vmModel, err := h.vmService.GetVM(ctx, vmID)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "vm_get_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}
	if vmModel == nil {
		h.writeError(w, http.StatusNotFound, apiError{
			Code:      "not_found",
			Message:   "VM not found",
			Retryable: false,
		})
		return
	}

	// Build cloud-init config
	var userData string
	if req.UserData != "" {
		userData = req.UserData
	} else if req.TemplateID != "" {
		// Render template with variables
		rendered, err := h.vmService.RenderCloudInitTemplateByID(ctx, req.TemplateID, req.Variables)
		if err != nil {
			h.writeError(w, http.StatusInternalServerError, apiError{
				Code:      "render_failed",
				Message:   err.Error(),
				Retryable: true,
			})
			return
		}
		userData = rendered
	} else {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_request",
			Message:   "Either template_id or user_data must be provided",
			Retryable: false,
		})
		return
	}

	// Validate cloud-init content
	if !strings.Contains(userData, "#cloud-config") {
		h.writeError(w, http.StatusBadRequest, apiError{
			Code:      "invalid_cloudinit",
			Message:   "Cloud-init content must contain '#cloud-config' header",
			Retryable: false,
		})
		return
	}

	// Get template cloud-init vars for username/ssh keys
	username := ""
	var sshKeys []string
	if req.Variables != nil {
		username = req.Variables["Username"]
		if sshKey := req.Variables["SSHKey"]; sshKey != "" {
			sshKeys = []string{sshKey}
		}
	}

	// Update VM with new cloud-init config
	// Note: This updates the stored config but requires VM restart to apply
	err = h.vmService.UpdateVMCloudInit(ctx, vmID, vm.UpdateCloudInitInput{
		UserData:          userData,
		Username:          username,
		SSHAuthorizedKeys: sshKeys,
	})
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "apply_failed",
			Message:   err.Error(),
			Retryable: true,
		})
		return
	}

	h.writeJSON(w, http.StatusOK, map[string]any{
		"message": "Cloud-init configuration applied. Restart the VM to apply changes.",
		"vm_id":   vmID,
		"warning": "VM must be restarted for changes to take effect",
	})
}
