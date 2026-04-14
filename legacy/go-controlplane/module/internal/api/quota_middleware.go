package api

import (
	"context"
	"fmt"
	"net/http"

	"github.com/chv/chv/internal/quota"
)

// QuotaMiddleware creates a middleware that checks quota before allowing an action
func QuotaMiddleware(quotaService *quota.Service, checkFunc func(*http.Request) (userID string, resource string, amount int, errorResponse *apiError)) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			userID, resource, amount, errResp := checkFunc(r)
			if errResp != nil {
				WriteError(w, http.StatusBadRequest, errResp.Code, errResp.Message)
				return
			}

			if err := quotaService.CheckQuota(r.Context(), userID, resource, amount); err != nil {
				WriteError(w, http.StatusForbidden, "quota_exceeded", err.Error())
				return
			}
			next.ServeHTTP(w, r)
		})
	}
}

// CanCreateVMCheck returns a check function for VM creation quota
func CanCreateVMCheck(quotaService *quota.Service, getUserID func(*http.Request) string, getVMResources func(*http.Request) (vcpu int, memoryMB int64, storageGB int64, err error)) func(*http.Request) (string, string, int, *apiError) {
	return func(r *http.Request) (string, string, int, *apiError) {
		userID := getUserID(r)
		if userID == "" {
			return "", "", 0, &apiError{Code: "unauthorized", Message: "User not authenticated"}
		}

		vcpu, memoryMB, storageGB, err := getVMResources(r)
		if err != nil {
			return "", "", 0, &apiError{Code: "invalid_request", Message: err.Error()}
		}

		// Check all resource quotas
		if err := quotaService.CanCreateVM(r.Context(), userID, vcpu, memoryMB, storageGB); err != nil {
			return "", "", 0, &apiError{Code: "quota_exceeded", Message: err.Error()}
		}

		return userID, "", 0, nil
	}
}

// CanCreateNetworkCheck returns a check function for network creation quota
func CanCreateNetworkCheck(quotaService *quota.Service, getUserID func(*http.Request) string) func(*http.Request) (string, string, int, *apiError) {
	return func(r *http.Request) (string, string, int, *apiError) {
		userID := getUserID(r)
		if userID == "" {
			return "", "", 0, &apiError{Code: "unauthorized", Message: "User not authenticated"}
		}

		return userID, "networks", 1, nil
	}
}

// QuotaService interface for middleware (to avoid circular dependencies)
type QuotaService interface {
	CheckQuota(ctx context.Context, userID string, resource string, amount int) error
	CanCreateVM(ctx context.Context, userID string, vcpu int, memoryMB int64, storageGB int64) error
}

// WriteError writes an error response
func WriteError(w http.ResponseWriter, status int, code, message string) {
	type errorResponse struct {
		Error struct {
			Code    string `json:"code"`
			Message string `json:"message"`
		} `json:"error"`
	}

	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	resp := errorResponse{}
	resp.Error.Code = code
	resp.Error.Message = message
	fmt.Fprintf(w, `{"error":{"code":"%s","message":"%s"}}`, code, message)
}
