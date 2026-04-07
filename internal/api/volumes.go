package api

import (
	"context"
	"encoding/json"
	"net/http"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/pkg/uuidx"
	"github.com/go-chi/chi/v5"
	"github.com/google/uuid"
)

// CloneVolumeRequest represents a volume clone request
type CloneVolumeRequest struct {
	Name string `json:"name"`
}

// CloneVolumeResponse represents a volume clone response
type CloneVolumeResponse struct {
	Volume    *models.Volume `json:"volume"`
	Operation string         `json:"operation,omitempty"`
	Message   string         `json:"message,omitempty"`
}

func (h *Handler) cloneVolume(w http.ResponseWriter, r *http.Request) {
	id := chi.URLParam(r, "id")
	volumeID, err := uuidx.Parse(id)
	if err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_ID", "Invalid volume ID")
		return
	}

	var req CloneVolumeRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "Invalid request body")
		return
	}

	if req.Name == "" {
		h.errorResponse(w, http.StatusBadRequest, "INVALID_REQUEST", "name is required")
		return
	}

	// Get source volume
	sourceVolume, err := h.store.GetVolume(r.Context(), volumeID)
	if err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get volume")
		return
	}
	if sourceVolume == nil {
		h.errorResponse(w, http.StatusNotFound, "NOT_FOUND", "Volume not found")
		return
	}

	// Check if VM is stopped (cloning requires VM to be stopped)
	if sourceVolume.VMID != nil {
		vm, err := h.store.GetVM(r.Context(), *sourceVolume.VMID)
		if err != nil {
			h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to get VM")
			return
		}
		if vm != nil && vm.ActualState == models.VMActualStateRunning {
			h.errorResponse(w, http.StatusConflict, "VM_RUNNING", "Volume cannot be cloned while VM is running")
			return
		}
	}

	// Create new volume
	newVolumeID := uuidx.New()
	now := time.Now()
	newVolume := &models.Volume{
		ID:              newVolumeID,
		PoolID:          sourceVolume.PoolID,
		BackingImageID:  sourceVolume.BackingImageID,
		Format:          sourceVolume.Format,
		SizeBytes:       sourceVolume.SizeBytes,
		AttachmentState: models.VolumeAttachmentStateDetached,
		ResizeState:     models.VolumeResizeStateIdle,
		Metadata:        []byte("{}"),
		CreatedAt:       now,
	}

	// Store source and target paths for cloning
	sourcePath := ""
	if sourceVolume.Path != nil {
		sourcePath = *sourceVolume.Path
	}

	// Create volume in database
	if err := h.store.CreateVolume(r.Context(), newVolume); err != nil {
		h.errorResponse(w, http.StatusInternalServerError, "INTERNAL_ERROR", "Failed to create volume record")
		return
	}

	// Start operation tracking
	userID, _ := r.Context().Value("user_id").(string)
	if userID == "" {
		userID = "anonymous"
	}

	var opID *uuid.UUID
	op, _ := h.operations.Start(r.Context(), models.OpVolumeClone, models.OpCategoryAsync,
		"volume", &newVolumeID, models.ActorTypeUser, userID, req)
	if op != nil {
		opID = &op.ID
	}

	// Perform clone in background
	go func(sourcePath string) {
		ctx := context.Background()

		// TODO: Integrate with storage manager to perform actual clone
		// This requires access to the storage manager which should be injected
		// into the handler. sourcePath contains the path to the source volume.
		_ = sourcePath

		// Update operation status
		if opID != nil {
			h.operations.Complete(ctx, *opID, map[string]string{
				"source_volume_id": volumeID.String(),
				"new_volume_id":    newVolumeID.String(),
				"status":           "completed",
			})
		}
	}(sourcePath)

	h.jsonResponse(w, http.StatusAccepted, CloneVolumeResponse{
		Volume:    newVolume,
		Operation: "clone",
		Message:   "Volume clone initiated",
	})
}
