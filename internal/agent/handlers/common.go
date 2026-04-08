package handlers

import (
	"encoding/json"
	"net/http"

	"github.com/chv/chv/internal/agentapi"
)

func respondJSON(w http.ResponseWriter, status int, payload any) {
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(payload)
}

func respondError(w http.ResponseWriter, status int, code, message string, retryable bool) {
	respondJSON(w, status, map[string]any{
		"error": agentapi.Error{
			Code:      code,
			Message:   message,
			Retryable: retryable,
		},
	})
}
