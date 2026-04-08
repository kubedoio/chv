package api

import (
	"net/http"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/operations"
)

// listEvents returns operational events
func (h *Handler) listEvents(w http.ResponseWriter, r *http.Request) {
	ctx := requestContext(r)

	// Get optional filter params
	resourceType := r.URL.Query().Get("resource_type")

	// Create operations service
	opService := operations.NewService(h.repo)

	// List operations
	opList, err := opService.ListOperations(ctx, resourceType)
	if err != nil {
		h.writeError(w, http.StatusInternalServerError, apiError{
			Code:      "events_fetch_failed",
			Message:   "Could not fetch events",
			Retryable: true,
		})
		return
	}

	// Transform operations to event format
	events := make([]eventResponse, 0, len(opList))
	for _, op := range opList {
		events = append(events, operationToEvent(op))
	}

	h.writeJSON(w, http.StatusOK, events)
}

// eventResponse is the external API format for events
type eventResponse struct {
	ID          string `json:"id"`
	Timestamp   string `json:"timestamp"`
	Operation   string `json:"operation"`
	Status      string `json:"status"`
	Resource    string `json:"resource"`
	ResourceID  string `json:"resource_id,omitempty"`
	Message     string `json:"message,omitempty"`
}

// operationToEvent converts an internal Operation to an eventResponse
func operationToEvent(op models.Operation) eventResponse {
	event := eventResponse{
		ID:         op.ID,
		Timestamp:  op.CreatedAt,
		Operation:  op.OperationType,
		Status:     op.State,
		Resource:   op.ResourceType,
		ResourceID: op.ResourceID,
	}

	// Build message from payload
	switch op.State {
	case operations.StatePending:
		event.Message = "Operation queued"
	case operations.StateRunning:
		event.Message = "Operation in progress"
	case operations.StateCompleted:
		event.Message = "Operation completed successfully"
	case operations.StateFailed:
		event.Message = "Operation failed"
		if op.ErrorPayload != "" {
			event.Message = op.ErrorPayload
		}
	}

	return event
}
