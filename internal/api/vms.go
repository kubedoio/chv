package api

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/store"
	"github.com/chv/chv/pkg/errorsx"
	"github.com/chv/chv/pkg/uuidx"
	"github.com/go-chi/chi/v5"
)

// VMCreateRequest represents a VM creation request.
type VMCreateRequest struct {
	Name     string                   `json:"name"`
	CPU      int32                    `json:"vcpu"`
	MemoryMB int64                    `json:"memory_mb"`
	ImageID  string                   `json:"image_id"`
	DiskSize int64                    `json:"disk_size_bytes"`
	Networks []VMNetworkRequest       `json:"networks"`
	CloudInit *models.CloudInitSpec   `json:"cloud_init,omitempty"`
}

// VMNetworkRequest represents a VM network attachment request.
type VMNetworkRequest struct {
	NetworkID string `json:"network_id"`
}

func (h *Handler) createVM(w http.ResponseWriter, r *http.Request) {
	var req VMCreateRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "Invalid request body")
		return
	}

	// Get user ID from context for audit trail
	userID, _ := r.Context().Value("user_id").(string)
	if userID == "" {
		userID = "anonymous"
	}

	// Create VM ID early for operation tracking
	vmID := uuidx.New()

	// Start operation tracking
	op, _ := h.operations.Start(r.Context(), models.OpVMCreate, models.OpCategoryAsync,
		"vm", &vmID, models.ActorTypeUser, userID, req)
	
	// Validate required fields
	if req.Name == "" {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "name is required")
		return
	}
	
	// Validate vCPU first (before image_id check - for validation tests)
	if req.CPU <= 0 {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "vcpu must be greater than 0")
		return
	}
	if req.CPU > 64 {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "vcpu exceeds maximum (64)")
		return
	}
	
	// Validate memory first (before image_id check - for validation tests)
	if req.MemoryMB <= 0 {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "memory must be greater than 0")
		return
	}
	if req.MemoryMB > 524288 { // 512 GB
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "memory exceeds maximum (512GB)")
		return
	}
	
	// Validate image_id after vcpu/memory validation
	if req.ImageID == "" {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "image_id is required")
		return
	}
	
	// Check if VM already exists
	existing, err := h.store.GetVMByName(r.Context(), req.Name)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to check existing VM")
		return
	}
	if existing != nil {
		h.errorResponse(w, http.StatusConflict, "ALREADY_EXISTS", "VM with this name already exists")
		return
	}
	
	// Validate image
	imageID, err := uuidx.Parse(req.ImageID)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid image ID")
		return
	}
	
	image, err := h.store.GetImage(r.Context(), imageID)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get image")
		return
	}
	if image == nil {
		h.errorResponse(w, http.StatusNotFound, "IMAGE_NOT_FOUND", "Image not found")
		return
	}
	if !image.IsReady() {
		h.errorResponse(w, http.StatusBadRequest, "IMAGE_NOT_READY", "Image is not ready")
		return
	}
	
	// Set defaults
	if req.CPU == 0 {
		req.CPU = 1
	}
	if req.MemoryMB == 0 {
		req.MemoryMB = 1024
	}
	if req.DiskSize == 0 {
		req.DiskSize = 10 * 1024 * 1024 * 1024 // 10GB default
	}
	
	// Create VM spec
	spec := &models.VMSpec{
		CPU:       req.CPU,
		MemoryMB:  req.MemoryMB,
		Boot:      models.BootSpec{Mode: "cloud_image"},
		CloudInit: req.CloudInit,
	}
	
	// Add networks
	for _, netReq := range req.Networks {
		spec.Networks = append(spec.Networks, models.NetworkAttachment{
			NetworkID: netReq.NetworkID,
			DHCP:      true,
		})
	}
	
	now := time.Now()
	vm := &models.VirtualMachine{
		ID:              vmID,
		Name:            req.Name,
		DesiredState:    models.VMDesiredStateRunning,
		ActualState:     models.VMActualStateProvisioning,
		PlacementStatus: models.PlacementStatusPending,
		CreatedAt:       now,
		UpdatedAt:       now,
	}
	
	if err := vm.SetSpec(spec); err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to serialize spec")
		return
	}
	
	// Create VM in a transaction
	if err := h.store.WithTx(r.Context(), func(s store.Store) error {
		if err := s.CreateVM(r.Context(), vm); err != nil {
			return err
		}
		
		// Create volume
		volume := &models.Volume{
			ID:              uuidx.New(),
			VMID:            &vm.ID,
			BackingImageID:  &imageID,
			Format:          models.VolumeFormatRaw,
			SizeBytes:       req.DiskSize,
			AttachmentState: models.VolumeAttachmentStateDetached,
			ResizeState:     models.VolumeResizeStateIdle,
			Metadata:        []byte("{}"),
			CreatedAt:       now,
		}
		
		if err := s.CreateVolume(r.Context(), volume); err != nil {
			return err
		}
		
		// Add volume to spec
		spec.Disks = []models.DiskAttachment{
			{
				VolumeID: volume.ID.String(),
				Bus:      "virtio-blk",
				Boot:     true,
			},
		}
		
		// Update VM with volume info
		if err := vm.SetSpec(spec); err != nil {
			return err
		}
		
		return s.UpdateVM(r.Context(), vm)
	}); err != nil {
		// Mark operation as failed
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to create VM")
		return
	}

	// Mark operation as completed
	h.operations.Complete(r.Context(), op.ID, vm)

	// Trigger scheduling
	go h.scheduler.ScheduleVM(r.Context(), vm.ID)

	h.jsonResponse(w, http.StatusCreated, vm)
}

func (h *Handler) listVMs(w http.ResponseWriter, r *http.Request) {
	vms, err := h.store.ListVMs(r.Context())
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to list VMs")
		return
	}
	
	h.jsonResponse(w, http.StatusOK, vms)
}

func (h *Handler) getVM(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	vmID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid VM ID")
		return
	}

	// Validate VM ID is safe for path usage
	if err := uuidx.ValidateSafeForPath(id); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID_FORMAT", "Invalid VM ID format")
		return
	}
	
	vm, err := h.store.GetVM(r.Context(), vmID)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get VM")
		return
	}
	
	if vm == nil {
		h.errorResponse(w, http.StatusNotFound, "NOT_FOUND", "VM not found")
		return
	}
	
	h.jsonResponse(w, http.StatusOK, vm)
}

func (h *Handler) startVM(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	vmID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid VM ID")
		return
	}

	// Validate VM ID is safe for path usage
	if err := uuidx.ValidateSafeForPath(id); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID_FORMAT", "Invalid VM ID format")
		return
	}

	// Get user ID for audit trail
	userID, _ := r.Context().Value("user_id").(string)
	if userID == "" {
		userID = "anonymous"
	}

	// Start operation tracking
	op, _ := h.operations.Start(r.Context(), models.OpVMStart, models.OpCategoryAsync,
		"vm", &vmID, models.ActorTypeUser, userID, map[string]string{"vm_id": id})

	vm, err := h.store.GetVM(r.Context(), vmID)
	if err != nil {
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get VM")
		return
	}
	if vm == nil {
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusNotFound, "NOT_FOUND", "VM not found")
		return
	}

	if !vm.CanStart() {
		err := errorsx.New(errorsx.ErrVMInvalidState, "VM cannot be started in current state")
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusBadRequest, "INVALID_STATE", "VM cannot be started in current state")
		return
	}

	vm.DesiredState = models.VMDesiredStateRunning
	if err := h.store.UpdateVM(r.Context(), vm); err != nil {
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to update VM")
		return
	}

	// Mark operation as completed
	h.operations.Complete(r.Context(), op.ID, vm)

	// Trigger reconciliation
	go h.reconciler.TriggerVM(vm.ID)

	h.jsonResponse(w, http.StatusOK, vm)
}

func (h *Handler) stopVM(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	vmID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid VM ID")
		return
	}

	// Validate VM ID is safe for path usage
	if err := uuidx.ValidateSafeForPath(id); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID_FORMAT", "Invalid VM ID format")
		return
	}

	// Get user ID for audit trail
	userID, _ := r.Context().Value("user_id").(string)
	if userID == "" {
		userID = "anonymous"
	}

	// Start operation tracking
	op, _ := h.operations.Start(r.Context(), models.OpVMStop, models.OpCategoryAsync,
		"vm", &vmID, models.ActorTypeUser, userID, map[string]string{"vm_id": id})

	vm, err := h.store.GetVM(r.Context(), vmID)
	if err != nil {
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get VM")
		return
	}
	if vm == nil {
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusNotFound, "NOT_FOUND", "VM not found")
		return
	}

	if !vm.CanStop() {
		err := errorsx.New(errorsx.ErrVMInvalidState, "VM cannot be stopped in current state")
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusBadRequest, "INVALID_STATE", "VM cannot be stopped in current state")
		return
	}

	vm.DesiredState = models.VMDesiredStateStopped
	if err := h.store.UpdateVM(r.Context(), vm); err != nil {
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to update VM")
		return
	}

	// Mark operation as completed
	h.operations.Complete(r.Context(), op.ID, vm)

	go h.reconciler.TriggerVM(vm.ID)

	h.jsonResponse(w, http.StatusOK, vm)
}

func (h *Handler) rebootVM(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	vmID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid VM ID")
		return
	}

	// Validate VM ID is safe for path usage
	if err := uuidx.ValidateSafeForPath(id); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID_FORMAT", "Invalid VM ID format")
		return
	}

	// Get user ID for audit trail
	userID, _ := r.Context().Value("user_id").(string)
	if userID == "" {
		userID = "anonymous"
	}

	// Start operation tracking
	op, _ := h.operations.Start(r.Context(), models.OpVMReboot, models.OpCategoryAsync,
		"vm", &vmID, models.ActorTypeUser, userID, map[string]string{"vm_id": id})

	vm, err := h.store.GetVM(r.Context(), vmID)
	if err != nil {
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get VM")
		return
	}
	if vm == nil {
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusNotFound, "NOT_FOUND", "VM not found")
		return
	}

	if vm.ActualState != models.VMActualStateRunning {
		err := errorsx.New(errorsx.ErrVMInvalidState, "VM must be running to reboot")
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusBadRequest, "INVALID_STATE", "VM must be running to reboot")
		return
	}

	// Reboot is handled as a state transition
	vm.ActualState = models.VMActualStateStopping
	if err := h.store.UpdateVM(r.Context(), vm); err != nil {
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to update VM")
		return
	}

	// Mark operation as completed
	h.operations.Complete(r.Context(), op.ID, vm)

	go h.reconciler.TriggerVM(vm.ID)

	h.jsonResponse(w, http.StatusOK, vm)
}

// ResizeDiskRequest represents a disk resize request.
type ResizeDiskRequest struct {
	NewSizeBytes int64 `json:"new_size_bytes"`
}

func (h *Handler) resizeDisk(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	vmID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid VM ID")
		return
	}

	// Validate VM ID is safe for path usage
	if err := uuidx.ValidateSafeForPath(id); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID_FORMAT", "Invalid VM ID format")
		return
	}
	
	var req ResizeDiskRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "Invalid request body")
		return
	}
	
	if req.NewSizeBytes <= 0 {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "new_size_bytes must be positive")
		return
	}
	
	// Get VM volumes
	volumes, err := h.store.ListVolumesByVM(r.Context(), vmID)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get volumes")
		return
	}
	
	if len(volumes) == 0 {
		h.errorResponse(w, http.StatusNotFound, "NOT_FOUND", "No volumes found for VM")
		return
	}
	
	// For MVP-1, resize the first volume
	volume := volumes[0]
	
	if !volume.IsResizable() {
		appErr := errorsx.New(errorsx.ErrVolumeResizeUnsupported, "Volume cannot be resized")
		h.jsonResponse(w, http.StatusBadRequest, appErr)
		return
	}
	
	if req.NewSizeBytes <= volume.SizeBytes {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "New size must be larger than current size")
		return
	}
	
	volume.SizeBytes = req.NewSizeBytes
	volume.ResizeState = models.VolumeResizeStateResizing
	
	if err := h.store.UpdateVolume(r.Context(), volume); err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to update volume")
		return
	}
	
	// Trigger resize operation via reconciler
	go h.reconciler.TriggerVM(vmID)
	
	h.jsonResponse(w, http.StatusOK, volume)
}

func (h *Handler) deleteVM(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	vmID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid VM ID")
		return
	}

	// Validate VM ID is safe for path usage
	if err := uuidx.ValidateSafeForPath(id); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID_FORMAT", "Invalid VM ID format")
		return
	}

	// Get user ID for audit trail
	userID, _ := r.Context().Value("user_id").(string)
	if userID == "" {
		userID = "anonymous"
	}

	// Start operation tracking
	op, _ := h.operations.Start(r.Context(), models.OpVMDelete, models.OpCategoryAsync,
		"vm", &vmID, models.ActorTypeUser, userID, map[string]string{"vm_id": id})

	vm, err := h.store.GetVM(r.Context(), vmID)
	if err != nil {
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get VM")
		return
	}
	if vm == nil {
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusNotFound, "NOT_FOUND", "VM not found")
		return
	}

	// Set desired state to deleted
	vm.DesiredState = models.VMDesiredStateDeleted
	vm.ActualState = models.VMActualStateDeleting
	if err := h.store.UpdateVM(r.Context(), vm); err != nil {
		h.operations.Fail(r.Context(), op.ID, err)
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to update VM")
		return
	}

	// Mark operation as completed
	h.operations.Complete(r.Context(), op.ID, map[string]string{"vm_id": id, "status": "deleting"})

	go h.reconciler.TriggerVM(vmID)

	w.WriteHeader(http.StatusNoContent)
}
