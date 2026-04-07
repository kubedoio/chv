package quota

import (
	"context"
	"testing"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/store"
	"github.com/google/uuid"
)

// mockStore is a mock implementation of store.Store for testing
type mockStore struct {
	quotas map[string]*models.ResourceQuota
	usages map[string]*models.ResourceUsage
}

func newMockStore() *mockStore {
	return &mockStore{
		quotas: make(map[string]*models.ResourceQuota),
		usages: make(map[string]*models.ResourceUsage),
	}
}

func (m *mockStore) GetQuota(ctx context.Context, userID string) (*models.ResourceQuota, error) {
	return m.quotas[userID], nil
}

func (m *mockStore) SetQuota(ctx context.Context, quota *models.ResourceQuota) error {
	m.quotas[quota.UserID.String()] = quota
	return nil
}

func (m *mockStore) GetUsage(ctx context.Context, userID string) (*models.ResourceUsage, error) {
	if usage, ok := m.usages[userID]; ok {
		return usage, nil
	}
	uid, _ := uuid.Parse(userID)
	return &models.ResourceUsage{UserID: uid}, nil
}

func (m *mockStore) UpdateUsage(ctx context.Context, userID string, delta models.ResourceUsage) error {
	if _, ok := m.usages[userID]; !ok {
		uid, _ := uuid.Parse(userID)
		m.usages[userID] = &models.ResourceUsage{UserID: uid}
	}
	m.usages[userID].CPUsUsed += delta.CPUsUsed
	m.usages[userID].MemoryMBUsed += delta.MemoryMBUsed
	m.usages[userID].VMCount += delta.VMCount
	m.usages[userID].DiskGBUsed += delta.DiskGBUsed
	m.usages[userID].UpdatedAt = time.Now()
	return nil
}

func (m *mockStore) EnsureQuota(ctx context.Context, userID string) error {
	if _, ok := m.quotas[userID]; !ok {
		uid, _ := uuid.Parse(userID)
		m.quotas[userID] = models.DefaultQuota(uid)
	}
	if _, ok := m.usages[userID]; !ok {
		uid, _ := uuid.Parse(userID)
		m.usages[userID] = &models.ResourceUsage{UserID: uid}
	}
	return nil
}

// Implement other required methods with no-ops
func (m *mockStore) CreateNode(ctx context.Context, node *models.Node) error { return nil }
func (m *mockStore) GetNode(ctx context.Context, id uuid.UUID) (*models.Node, error) { return nil, nil }
func (m *mockStore) GetNodeByHostname(ctx context.Context, hostname string) (*models.Node, error) { return nil, nil }
func (m *mockStore) UpdateNode(ctx context.Context, node *models.Node) error { return nil }
func (m *mockStore) UpdateNodeHeartbeat(ctx context.Context, id uuid.UUID, status models.NodeState) error { return nil }
func (m *mockStore) ListNodes(ctx context.Context) ([]*models.Node, error) { return nil, nil }
func (m *mockStore) SetNodeMaintenance(ctx context.Context, id uuid.UUID, enabled bool) error { return nil }
func (m *mockStore) CreateNetwork(ctx context.Context, network *models.Network) error { return nil }
func (m *mockStore) GetNetwork(ctx context.Context, id uuid.UUID) (*models.Network, error) { return nil, nil }
func (m *mockStore) ListNetworks(ctx context.Context) ([]*models.Network, error) { return nil, nil }
func (m *mockStore) DeleteNetwork(ctx context.Context, id uuid.UUID) error { return nil }
func (m *mockStore) CreateStoragePool(ctx context.Context, pool *models.StoragePool) error { return nil }
func (m *mockStore) GetStoragePool(ctx context.Context, id uuid.UUID) (*models.StoragePool, error) { return nil, nil }
func (m *mockStore) ListStoragePools(ctx context.Context) ([]*models.StoragePool, error) { return nil, nil }
func (m *mockStore) ListStoragePoolsByNode(ctx context.Context, nodeID uuid.UUID) ([]*models.StoragePool, error) { return nil, nil }
func (m *mockStore) DeleteStoragePool(ctx context.Context, id uuid.UUID) error { return nil }
func (m *mockStore) CreateImage(ctx context.Context, image *models.Image) error { return nil }
func (m *mockStore) GetImage(ctx context.Context, id uuid.UUID) (*models.Image, error) { return nil, nil }
func (m *mockStore) UpdateImage(ctx context.Context, image *models.Image) error { return nil }
func (m *mockStore) ListImages(ctx context.Context) ([]*models.Image, error) { return nil, nil }
func (m *mockStore) DeleteImage(ctx context.Context, id uuid.UUID) error { return nil }
func (m *mockStore) CreateVM(ctx context.Context, vm *models.VirtualMachine) error { return nil }
func (m *mockStore) GetVM(ctx context.Context, id uuid.UUID) (*models.VirtualMachine, error) { return nil, nil }
func (m *mockStore) GetVMByName(ctx context.Context, name string) (*models.VirtualMachine, error) { return nil, nil }
func (m *mockStore) UpdateVM(ctx context.Context, vm *models.VirtualMachine) error { return nil }
func (m *mockStore) UpdateVMActualState(ctx context.Context, id uuid.UUID, state models.VMActualState, lastError []byte) error { return nil }
func (m *mockStore) ListVMs(ctx context.Context) ([]*models.VirtualMachine, error) { return nil, nil }
func (m *mockStore) ListVMsByNode(ctx context.Context, nodeID uuid.UUID) ([]*models.VirtualMachine, error) { return nil, nil }
func (m *mockStore) ListVMsNeedingReconciliation(ctx context.Context) ([]*models.VirtualMachine, error) { return nil, nil }
func (m *mockStore) DeleteVM(ctx context.Context, id uuid.UUID) error { return nil }
func (m *mockStore) CreateVolume(ctx context.Context, volume *models.Volume) error { return nil }
func (m *mockStore) GetVolume(ctx context.Context, id uuid.UUID) (*models.Volume, error) { return nil, nil }
func (m *mockStore) UpdateVolume(ctx context.Context, volume *models.Volume) error { return nil }
func (m *mockStore) ListVolumesByVM(ctx context.Context, vmID uuid.UUID) ([]*models.Volume, error) { return nil, nil }
func (m *mockStore) CreateSnapshot(ctx context.Context, snapshot *models.Snapshot) error { return nil }
func (m *mockStore) GetSnapshot(ctx context.Context, id uuid.UUID) (*models.Snapshot, error) { return nil, nil }
func (m *mockStore) UpdateSnapshot(ctx context.Context, snapshot *models.Snapshot) error { return nil }
func (m *mockStore) ListSnapshotsByVM(ctx context.Context, vmID uuid.UUID) ([]*models.Snapshot, error) { return nil, nil }
func (m *mockStore) DeleteSnapshot(ctx context.Context, id uuid.UUID) error { return nil }
func (m *mockStore) CreateVMNetworkAttachment(ctx context.Context, attachment *models.VMNetworkAttachment) error { return nil }
func (m *mockStore) ListVMNetworkAttachments(ctx context.Context, vmID uuid.UUID) ([]*models.VMNetworkAttachment, error) { return nil, nil }
func (m *mockStore) CreateAPIToken(ctx context.Context, token *models.APIToken) error { return nil }
func (m *mockStore) GetAPITokenByHash(ctx context.Context, hash string) (*models.APIToken, error) { return nil, nil }
func (m *mockStore) ListAPITokens(ctx context.Context) ([]*models.APIToken, error) { return nil, nil }
func (m *mockStore) RevokeAPIToken(ctx context.Context, id uuid.UUID) error { return nil }
func (m *mockStore) CreateOperation(ctx context.Context, op *models.Operation) error { return nil }
func (m *mockStore) GetOperation(ctx context.Context, id uuid.UUID) (*models.Operation, error) { return nil, nil }
func (m *mockStore) UpdateOperation(ctx context.Context, op *models.Operation) error { return nil }
func (m *mockStore) ListOperations(ctx context.Context, filters map[string]interface{}) ([]*models.Operation, error) { return nil, nil }
func (m *mockStore) CreateOperationLog(ctx context.Context, log *models.OperationLog) error { return nil }
func (m *mockStore) GetOperationLogs(ctx context.Context, operationID uuid.UUID) ([]*models.OperationLog, error) { return nil, nil }
func (m *mockStore) WithTx(ctx context.Context, fn func(store.Store) error) error { return fn(m) }

func TestCheckQuota_NoQuota_ReturnsError(t *testing.T) {
	mock := newMockStore()
	svc := NewService(mock)
	ctx := context.Background()

	// EnsureQuota should create a default quota
	mock.EnsureQuota(ctx, "user1")

	err := svc.CheckQuota(ctx, "user1", 10, 1024, 1, 10)
	if err == nil {
		t.Error("Expected quota error for exceeding default quota, got nil")
	}
}

func TestCheckQuota_WithinQuota_ReturnsNil(t *testing.T) {
	mock := newMockStore()
	svc := NewService(mock)
	ctx := context.Background()

	uid := uuid.New()
	mock.SetQuota(ctx, &models.ResourceQuota{
		UserID:      uid,
		MaxCPUs:     10,
		MaxMemoryMB: 10000,
		MaxVMCount:  10,
		MaxDiskGB:   100,
	})
	mock.usages[uid.String()] = &models.ResourceUsage{UserID: uid}

	err := svc.CheckQuota(ctx, uid.String(), 2, 2048, 1, 10)
	if err != nil {
		t.Errorf("Expected no error, got: %v", err)
	}
}

func TestCheckQuota_AnonymousUser_ReturnsNil(t *testing.T) {
	mock := newMockStore()
	svc := NewService(mock)
	ctx := context.Background()

	// Anonymous user should bypass quota check
	err := svc.CheckQuota(ctx, "anonymous", 100, 100000, 100, 1000)
	if err != nil {
		t.Errorf("Expected no error for anonymous user, got: %v", err)
	}
}

func TestUpdateUsageForVMCreation(t *testing.T) {
	mock := newMockStore()
	svc := NewService(mock)
	ctx := context.Background()

	uid := uuid.New()
	spec := &models.VMSpec{CPU: 2, MemoryMB: 2048}

	err := svc.UpdateUsageForVMCreation(ctx, uid.String(), spec, 10)
	if err != nil {
		t.Errorf("Expected no error, got: %v", err)
	}

	usage := mock.usages[uid.String()]
	if usage == nil {
		t.Fatal("Usage should be set")
	}
	if usage.CPUsUsed != 2 {
		t.Errorf("Expected CPUsUsed=2, got %d", usage.CPUsUsed)
	}
	if usage.MemoryMBUsed != 2048 {
		t.Errorf("Expected MemoryMBUsed=2048, got %d", usage.MemoryMBUsed)
	}
	if usage.VMCount != 1 {
		t.Errorf("Expected VMCount=1, got %d", usage.VMCount)
	}
	if usage.DiskGBUsed != 10 {
		t.Errorf("Expected DiskGBUsed=10, got %d", usage.DiskGBUsed)
	}
}

func TestUpdateUsageForVMDeletion(t *testing.T) {
	mock := newMockStore()
	svc := NewService(mock)
	ctx := context.Background()

	uid := uuid.New()
	// Set initial usage
	mock.usages[uid.String()] = &models.ResourceUsage{
		UserID:       uid,
		CPUsUsed:     4,
		MemoryMBUsed: 4096,
		VMCount:      2,
		DiskGBUsed:   20,
	}

	spec := &models.VMSpec{CPU: 2, MemoryMB: 2048}
	err := svc.UpdateUsageForVMDeletion(ctx, uid.String(), spec, 10)
	if err != nil {
		t.Errorf("Expected no error, got: %v", err)
	}

	usage := mock.usages[uid.String()]
	if usage.CPUsUsed != 2 {
		t.Errorf("Expected CPUsUsed=2, got %d", usage.CPUsUsed)
	}
	if usage.MemoryMBUsed != 2048 {
		t.Errorf("Expected MemoryMBUsed=2048, got %d", usage.MemoryMBUsed)
	}
	if usage.VMCount != 1 {
		t.Errorf("Expected VMCount=1, got %d", usage.VMCount)
	}
	if usage.DiskGBUsed != 10 {
		t.Errorf("Expected DiskGBUsed=10, got %d", usage.DiskGBUsed)
	}
}

func TestQuotaExceededError_Error(t *testing.T) {
	err := &QuotaExceededError{
		Resource: "CPU",
		Used:     4,
		Limit:    8,
		Delta:    6,
	}
	expected := "CPU quota exceeded: using 4 of 8 (requested 6)"
	if err.Error() != expected {
		t.Errorf("Expected error message %q, got %q", expected, err.Error())
	}
}

func TestDefaultQuota(t *testing.T) {
	uid := uuid.New()
	quota := models.DefaultQuota(uid)

	if quota.UserID != uid {
		t.Errorf("Expected UserID=%v, got %v", uid, quota.UserID)
	}
	if quota.MaxCPUs != 8 {
		t.Errorf("Expected MaxCPUs=8, got %d", quota.MaxCPUs)
	}
	if quota.MaxMemoryMB != 16384 {
		t.Errorf("Expected MaxMemoryMB=16384, got %d", quota.MaxMemoryMB)
	}
	if quota.MaxVMCount != 5 {
		t.Errorf("Expected MaxVMCount=5, got %d", quota.MaxVMCount)
	}
	if quota.MaxDiskGB != 100 {
		t.Errorf("Expected MaxDiskGB=100, got %d", quota.MaxDiskGB)
	}
}
