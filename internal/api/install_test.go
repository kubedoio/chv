package api

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"testing"

	"github.com/chv/chv/internal/auth"
	"github.com/chv/chv/internal/bootstrap"
	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/network"
)

type fakeBridgeManager struct {
	status  network.BridgeStatus
	actions []string
	err     error
}

func (f *fakeBridgeManager) Inspect(_ context.Context, _ string, _ string) (network.BridgeStatus, error) {
	return f.status, f.err
}

func (f *fakeBridgeManager) Ensure(_ context.Context, _ string, _ string) ([]string, error) {
	return f.actions, f.err
}

func (f *fakeBridgeManager) Repair(_ context.Context, _ string, _ string) ([]string, error) {
	return f.actions, f.err
}

type fakePrereqChecker struct {
	hypervisorPath string
	hypervisorErr  error
	isoToolPath    string
	isoToolErr     error
}

func (f fakePrereqChecker) FindCloudHypervisor() (string, error) {
	if f.hypervisorErr != nil {
		return "", f.hypervisorErr
	}
	return f.hypervisorPath, nil
}

func (f fakePrereqChecker) FindCloudInitTool() (string, error) {
	if f.isoToolErr != nil {
		return "", f.isoToolErr
	}
	return f.isoToolPath, nil
}

func TestInstallStatusEndpointReturnsStructuredStatus(t *testing.T) {
	root := t.TempDir()
	repo, err := db.Open(filepath.Join(root, "chv.db"))
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}
	defer repo.Close()

	service, err := bootstrap.NewService(bootstrap.Config{
		DataRoot:      root,
		DatabasePath:  filepath.Join(root, "chv.db"),
		BridgeName:    "chvbr0",
		BridgeCIDR:    "10.0.0.1/24",
		LocaldiskPath: filepath.Join(root, "storage", "localdisk"),
		BridgeManager: &fakeBridgeManager{
			status: network.BridgeStatus{
				Name:     "chvbr0",
				Exists:   true,
				ActualIP: "10.0.0.1/24",
				Up:       true,
			},
		},
		PrereqChecker:    fakePrereqChecker{hypervisorPath: "/usr/bin/cloud-hypervisor", isoToolPath: "/usr/bin/xorrisofs"},
		Repository:       repo,
		SkipDBConnection: true,
	})
	if err != nil {
		t.Fatalf("NewService() error = %v", err)
	}

	handler := NewHandler(repo, auth.NewService(repo), service)
	req := httptest.NewRequest(http.MethodGet, "/api/v1/install/status", nil)
	rr := httptest.NewRecorder()
	handler.Router().ServeHTTP(rr, req)

	if rr.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rr.Code)
	}

	var body map[string]any
	if err := json.Unmarshal(rr.Body.Bytes(), &body); err != nil {
		t.Fatalf("json.Unmarshal() error = %v", err)
	}

	if body["overall_state"] == nil {
		t.Fatalf("expected overall_state in response")
	}
}

func TestLoginValidateRequiresToken(t *testing.T) {
	root := t.TempDir()
	repo, err := db.Open(filepath.Join(root, "chv.db"))
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}
	defer repo.Close()

	service, err := bootstrap.NewService(bootstrap.Config{
		DataRoot:         root,
		DatabasePath:     filepath.Join(root, "chv.db"),
		BridgeName:       "chvbr0",
		BridgeCIDR:       "10.0.0.1/24",
		LocaldiskPath:    filepath.Join(root, "storage", "localdisk"),
		BridgeManager:    &fakeBridgeManager{},
		PrereqChecker:    fakePrereqChecker{hypervisorErr: errors.New("missing"), isoToolErr: errors.New("missing")},
		Repository:       repo,
		SkipDBConnection: true,
	})
	if err != nil {
		t.Fatalf("NewService() error = %v", err)
	}

	authService := auth.NewService(repo)
	result, err := authService.CreateToken(context.Background(), "admin")
	if err != nil {
		t.Fatalf("CreateToken() error = %v", err)
	}

	handler := NewHandler(repo, authService, service)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/login/validate", nil)
	req.Header.Set("Authorization", "Bearer "+result.Token)
	rr := httptest.NewRecorder()
	handler.Router().ServeHTTP(rr, req)

	if rr.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d body=%s", rr.Code, rr.Body.String())
	}
}
