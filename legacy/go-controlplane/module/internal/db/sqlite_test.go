package db

import (
	"context"
	"path/filepath"
	"testing"
	"time"

	"github.com/chv/chv/internal/models"
)

func TestRepositoryUpsertsInstallStatus(t *testing.T) {
	repo, err := Open(filepath.Join(t.TempDir(), "chv.db"))
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}
	defer repo.Close()

	status := &models.InstallStatus{
		ID:                   "singleton",
		DataRoot:             "/var/lib/chv",
		DatabasePath:         "/var/lib/chv/chv.db",
		BridgeName:           "chvbr0",
		BridgeExists:         true,
		BridgeIPExpected:     "10.0.0.1/24",
		BridgeIPActual:       "10.0.0.1/24",
		BridgeUp:             true,
		LocaldiskPath:        "/var/lib/chv/storage/localdisk",
		LocaldiskReady:       true,
		CloudHypervisorPath:  "/usr/bin/cloud-hypervisor",
		CloudHypervisorFound: true,
		CloudInitSupported:   true,
		OverallState:         models.InstallStateReady,
		LastCheckedAt:        time.Now().UTC().Format(time.RFC3339),
	}

	if err := repo.UpsertInstallStatus(context.Background(), status); err != nil {
		t.Fatalf("UpsertInstallStatus() error = %v", err)
	}

	got, err := repo.GetInstallStatus(context.Background())
	if err != nil {
		t.Fatalf("GetInstallStatus() error = %v", err)
	}

	if got == nil || got.OverallState != models.InstallStateReady {
		t.Fatalf("expected ready install status, got %#v", got)
	}
}
