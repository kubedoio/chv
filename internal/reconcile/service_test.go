package reconcile

import (
	"context"
	"encoding/json"
	"errors"
	"sync"
	"testing"
	"time"

	pb "github.com/chv/chv/internal/pb/agent"
	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/scheduler"
	"github.com/chv/chv/internal/store"
	"github.com/chv/chv/pkg/errorsx"
	"github.com/google/uuid"
)

// mockStore implements store.Store for testing
type mockStore struct {
	mu                 sync.RWMutex
	vms                map[uuid.UUID]*models.VirtualMachine
	nodes              map[uuid.UUID]*models.Node
	volumes            map[uuid.UUID]*models.Volume
	getVMCalled        bool
	updateVMCalled     bool
	deleteVMCalled     bool
	getNodeCalled      bool
	listVMsCalled      bool
	createVolumeCalled bool
	withTxCalled       bool
	getVMError         error
	updateVMError      error
	deleteVMError      error
	getNodeError       error
	listVMsError       error
	withTxError        error
	updateNodeCalled   bool
	updateNodeError    error
}

func newMockStore() *mockStore {
	return &mockStore{
		vms:     make(map[uuid.UUID]*models.VirtualMachine),
		nodes:   make(map[uuid.UUID]*models.Node),
		volumes: make(map[uuid.UUID]*models.Volume),
	}
}

// Test helper methods for safe concurrent access
func (m *mockStore) addVM(vm *models.VirtualMachine) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.vms[vm.ID] = vm
}

func (m *mockStore) addNode(node *models.Node) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.nodes[node.ID] = node
}

func (m *mockStore) CreateNode(ctx context.Context, node *models.Node) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.nodes[node.ID] = node
	return nil
}

func (m *mockStore) GetNode(ctx context.Context, id uuid.UUID) (*models.Node, error) {
	m.mu.Lock()
	m.getNodeCalled = true
	m.mu.Unlock()
	if m.getNodeError != nil {
		return nil, m.getNodeError
	}
	m.mu.RLock()
	defer m.mu.RUnlock()
	node, exists := m.nodes[id]
	if !exists {
		return nil, nil
	}
	return node, nil
}

func (m *mockStore) GetNodeByHostname(ctx context.Context, hostname string) (*models.Node, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	for _, node := range m.nodes {
		if node.Hostname == hostname {
			return node, nil
		}
	}
	return nil, nil
}

func (m *mockStore) UpdateNode(ctx context.Context, node *models.Node) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.updateNodeCalled = true
	if m.updateNodeError != nil {
		return m.updateNodeError
	}
	m.nodes[node.ID] = node
	return nil
}

func (m *mockStore) UpdateNodeHeartbeat(ctx context.Context, id uuid.UUID, status models.NodeState) error {
	return nil
}

func (m *mockStore) ListNodes(ctx context.Context) ([]*models.Node, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	nodes := make([]*models.Node, 0, len(m.nodes))
	for _, node := range m.nodes {
		nodes = append(nodes, node)
	}
	return nodes, nil
}

func (m *mockStore) SetNodeMaintenance(ctx context.Context, id uuid.UUID, enabled bool) error {
	return nil
}

func (m *mockStore) CreateNetwork(ctx context.Context, network *models.Network) error {
	return nil
}

func (m *mockStore) GetNetwork(ctx context.Context, id uuid.UUID) (*models.Network, error) {
	return nil, nil
}

func (m *mockStore) ListNetworks(ctx context.Context) ([]*models.Network, error) {
	return nil, nil
}

func (m *mockStore) DeleteNetwork(ctx context.Context, id uuid.UUID) error {
	return nil
}

func (m *mockStore) CreateStoragePool(ctx context.Context, pool *models.StoragePool) error {
	return nil
}

func (m *mockStore) GetStoragePool(ctx context.Context, id uuid.UUID) (*models.StoragePool, error) {
	return nil, nil
}

func (m *mockStore) ListStoragePools(ctx context.Context) ([]*models.StoragePool, error) {
	return nil, nil
}

func (m *mockStore) ListStoragePoolsByNode(ctx context.Context, nodeID uuid.UUID) ([]*models.StoragePool, error) {
	return nil, nil
}

func (m *mockStore) CreateImage(ctx context.Context, image *models.Image) error {
	return nil
}

func (m *mockStore) GetImage(ctx context.Context, id uuid.UUID) (*models.Image, error) {
	return nil, nil
}

func (m *mockStore) UpdateImage(ctx context.Context, image *models.Image) error {
	return nil
}

func (m *mockStore) ListImages(ctx context.Context) ([]*models.Image, error) {
	return nil, nil
}

func (m *mockStore) DeleteImage(ctx context.Context, id uuid.UUID) error {
	return nil
}

func (m *mockStore) CreateVM(ctx context.Context, vm *models.VirtualMachine) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.vms[vm.ID] = vm
	return nil
}

func (m *mockStore) GetVM(ctx context.Context, id uuid.UUID) (*models.VirtualMachine, error) {
	m.mu.Lock()
	m.getVMCalled = true
	m.mu.Unlock()
	if m.getVMError != nil {
		return nil, m.getVMError
	}
	m.mu.RLock()
	defer m.mu.RUnlock()
	vm, exists := m.vms[id]
	if !exists {
		return nil, nil
	}
	return vm, nil
}

func (m *mockStore) GetVMByName(ctx context.Context, name string) (*models.VirtualMachine, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	for _, vm := range m.vms {
		if vm.Name == name {
			return vm, nil
		}
	}
	return nil, nil
}

func (m *mockStore) UpdateVM(ctx context.Context, vm *models.VirtualMachine) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.updateVMCalled = true
	if m.updateVMError != nil {
		return m.updateVMError
	}
	m.vms[vm.ID] = vm
	return nil
}

func (m *mockStore) UpdateVMActualState(ctx context.Context, id uuid.UUID, state models.VMActualState, lastError []byte) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if vm, exists := m.vms[id]; exists {
		vm.ActualState = state
		vm.LastError = lastError
	}
	return nil
}

func (m *mockStore) ListVMs(ctx context.Context) ([]*models.VirtualMachine, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	vms := make([]*models.VirtualMachine, 0, len(m.vms))
	for _, vm := range m.vms {
		vms = append(vms, vm)
	}
	return vms, nil
}

func (m *mockStore) ListVMsByNode(ctx context.Context, nodeID uuid.UUID) ([]*models.VirtualMachine, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	var result []*models.VirtualMachine
	for _, vm := range m.vms {
		if vm.NodeID != nil && *vm.NodeID == nodeID {
			result = append(result, vm)
		}
	}
	return result, nil
}

func (m *mockStore) ListVMsNeedingReconciliation(ctx context.Context) ([]*models.VirtualMachine, error) {
	m.mu.Lock()
	m.listVMsCalled = true
	m.mu.Unlock()
	if m.listVMsError != nil {
		return nil, m.listVMsError
	}
	m.mu.RLock()
	defer m.mu.RUnlock()
	var result []*models.VirtualMachine
	for _, vm := range m.vms {
		if vm.NeedsReconciliation() {
			result = append(result, vm)
		}
	}
	return result, nil
}

func (m *mockStore) DeleteVM(ctx context.Context, id uuid.UUID) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.deleteVMCalled = true
	if m.deleteVMError != nil {
		return m.deleteVMError
	}
	delete(m.vms, id)
	return nil
}

func (m *mockStore) CreateVolume(ctx context.Context, volume *models.Volume) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.createVolumeCalled = true
	m.volumes[volume.ID] = volume
	return nil
}

func (m *mockStore) GetVolume(ctx context.Context, id uuid.UUID) (*models.Volume, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	vol, exists := m.volumes[id]
	if !exists {
		return nil, nil
	}
	return vol, nil
}

func (m *mockStore) UpdateVolume(ctx context.Context, volume *models.Volume) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.volumes[volume.ID] = volume
	return nil
}

func (m *mockStore) ListVolumesByVM(ctx context.Context, vmID uuid.UUID) ([]*models.Volume, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	var result []*models.Volume
	for _, vol := range m.volumes {
		if vol.VMID != nil && *vol.VMID == vmID {
			result = append(result, vol)
		}
	}
	return result, nil
}

func (m *mockStore) CreateVMNetworkAttachment(ctx context.Context, attachment *models.VMNetworkAttachment) error {
	return nil
}

func (m *mockStore) ListVMNetworkAttachments(ctx context.Context, vmID uuid.UUID) ([]*models.VMNetworkAttachment, error) {
	return nil, nil
}

func (m *mockStore) CreateAPIToken(ctx context.Context, token *models.APIToken) error {
	return nil
}

func (m *mockStore) GetAPITokenByHash(ctx context.Context, hash string) (*models.APIToken, error) {
	return nil, nil
}

func (m *mockStore) RevokeAPIToken(ctx context.Context, id uuid.UUID) error {
	return nil
}

func (m *mockStore) CreateOperation(ctx context.Context, op *models.Operation) error {
	return nil
}

func (m *mockStore) GetOperation(ctx context.Context, id uuid.UUID) (*models.Operation, error) {
	return nil, nil
}

func (m *mockStore) UpdateOperation(ctx context.Context, op *models.Operation) error {
	return nil
}

func (m *mockStore) ListOperations(ctx context.Context, filters map[string]interface{}) ([]*models.Operation, error) {
	return nil, nil
}

func (m *mockStore) CreateOperationLog(ctx context.Context, log *models.OperationLog) error {
	return nil
}

func (m *mockStore) GetOperationLogs(ctx context.Context, operationID uuid.UUID) ([]*models.OperationLog, error) {
	return nil, nil
}

func (m *mockStore) WithTx(ctx context.Context, fn func(store.Store) error) error {
	m.mu.Lock()
	m.withTxCalled = true
	m.mu.Unlock()
	if m.withTxError != nil {
		return m.withTxError
	}
	return fn(m)
}

func (m *mockStore) DeleteStoragePool(ctx context.Context, id uuid.UUID) error {
	return nil
}

func (m *mockStore) ListAPITokens(ctx context.Context) ([]*models.APIToken, error) {
	return nil, nil
}

func (m *mockStore) CreateSnapshot(ctx context.Context, snapshot *models.Snapshot) error {
	return nil
}

func (m *mockStore) GetSnapshot(ctx context.Context, id uuid.UUID) (*models.Snapshot, error) {
	return nil, nil
}

func (m *mockStore) UpdateSnapshot(ctx context.Context, snapshot *models.Snapshot) error {
	return nil
}

func (m *mockStore) ListSnapshotsByVM(ctx context.Context, vmID uuid.UUID) ([]*models.Snapshot, error) {
	return nil, nil
}

func (m *mockStore) DeleteSnapshot(ctx context.Context, id uuid.UUID) error {
	return nil
}

func (m *mockStore) GetQuota(ctx context.Context, userID string) (*models.ResourceQuota, error) {
	return nil, nil
}

func (m *mockStore) SetQuota(ctx context.Context, quota *models.ResourceQuota) error {
	return nil
}

func (m *mockStore) GetUsage(ctx context.Context, userID string) (*models.ResourceUsage, error) {
	return nil, nil
}

func (m *mockStore) UpdateUsage(ctx context.Context, userID string, delta models.ResourceUsage) error {
	return nil
}

func (m *mockStore) EnsureQuota(ctx context.Context, userID string) error {
	return nil
}

// Test helper methods for accessing tracking fields safely
func (m *mockStore) wasGetVMCalled() bool {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return m.getVMCalled
}

// mockAgentClient implements agent.Client for testing
type mockAgentClient struct {
	startVMCalled    bool
	stopVMCalled     bool
	deleteVMCalled   bool
	getVMStateCalled bool
	pingCalled       bool
	closeCalled      bool
	startVMError     error
	stopVMError      error
	deleteVMError    error
	getVMStateResp   *pb.VMStateResponse
	getVMStateError  error
	pingError        error
	closeError       error
}

func newMockAgentClient() *mockAgentClient {
	return &mockAgentClient{}
}

func (m *mockAgentClient) Ping(ctx context.Context, nodeID string) error {
	m.pingCalled = true
	return m.pingError
}

func (m *mockAgentClient) ProvisionVM(ctx context.Context, nodeID string, req *pb.ProvisionVMRequest) error {
	return nil
}

func (m *mockAgentClient) StartVM(ctx context.Context, nodeID string, vmID string) error {
	m.startVMCalled = true
	return m.startVMError
}

func (m *mockAgentClient) StopVM(ctx context.Context, nodeID string, vmID string) error {
	m.stopVMCalled = true
	return m.stopVMError
}

func (m *mockAgentClient) RebootVM(ctx context.Context, nodeID string, vmID string) error {
	return nil
}

func (m *mockAgentClient) DeleteVM(ctx context.Context, nodeID string, vmID string) error {
	m.deleteVMCalled = true
	return m.deleteVMError
}

func (m *mockAgentClient) GetVMState(ctx context.Context, nodeID string, vmID string) (*pb.VMStateResponse, error) {
	m.getVMStateCalled = true
	return m.getVMStateResp, m.getVMStateError
}

func (m *mockAgentClient) CreateVolume(ctx context.Context, nodeID string, req *pb.VolumeCreateRequest) error {
	return nil
}

func (m *mockAgentClient) ResizeVolume(ctx context.Context, nodeID string, req *pb.VolumeResizeRequest) error {
	return nil
}

func (m *mockAgentClient) EnsureBridge(ctx context.Context, nodeID string, req *pb.EnsureBridgeRequest) error {
	return nil
}

func (m *mockAgentClient) ImportImage(ctx context.Context, nodeID string, req *pb.ImageImportRequest) error {
	return nil
}

func (m *mockAgentClient) StreamConsole(ctx context.Context, nodeID string) (pb.AgentService_StreamConsoleClient, error) {
	return nil, nil
}

func (m *mockAgentClient) Close() error {
	m.closeCalled = true
	return m.closeError
}

// Helper functions to create test data
func createTestVM(id uuid.UUID, name string, desired models.VMDesiredState, actual models.VMActualState) *models.VirtualMachine {
	spec := &models.VMSpec{
		CPU:      2,
		MemoryMB: 4096,
	}
	specJSON, _ := json.Marshal(spec)
	return &models.VirtualMachine{
		ID:           id,
		Name:         name,
		DesiredState: desired,
		ActualState:  actual,
		Spec:         specJSON,
		CreatedAt:    time.Now(),
		UpdatedAt:    time.Now(),
	}
}

func createTestNode(id uuid.UUID, hostname string, ip string) *models.Node {
	return &models.Node{
		ID:                   id,
		Hostname:             hostname,
		ManagementIP:         ip,
		Status:               models.NodeStateOnline,
		TotalCPUcores:        8,
		TotalRAMMB:           16384,
		AllocatableCPUCores:  8,
		AllocatableRAMMB:     16384,
		CreatedAt:            time.Now(),
		UpdatedAt:            time.Now(),
	}
}

// TestService_StartStop tests service lifecycle
func TestService_StartStop(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Start the service
	svc.Start(ctx)

	// Give it a moment to start
	time.Sleep(50 * time.Millisecond)

	// Stop the service
	svc.Stop()

	if !mockAgent.closeCalled {
		t.Error("Expected Close to be called on agent client")
	}
}

// TestService_TriggerVM tests manual VM trigger
func TestService_TriggerVM(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	svc.Start(ctx)
	defer svc.Stop()

	// Create a test VM that needs reconciliation
	vmID := uuid.New()
	nodeID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateRunning, models.VMActualStateStopped)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Create the node
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	mockStore.addNode(node)

	// Trigger reconciliation
	svc.TriggerVM(vmID)

	// Give it time to process
	time.Sleep(100 * time.Millisecond)

	// Verify VM was fetched
	if !mockStore.wasGetVMCalled() {
		t.Error("Expected GetVM to be called")
	}
}

// TestReconcileRunning_ProvisioningToRunning tests VM provisioning to running transition
func TestReconcileRunning_ProvisioningToRunning(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	// Create node first
	nodeID := uuid.New()
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	mockStore.addNode(node)

	svc := NewService(mockStore, sched, mockAgent)

	ctx := context.Background()

	// Create VM in provisioning state without node assignment
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateRunning, models.VMActualStateProvisioning)
	mockStore.addVM(vm)

	// Pre-assign node to VM (simulating scheduler behavior)
	// The scheduler would normally do this, but for this test we do it directly
	vm.NodeID = &nodeID
	vm.PlacementStatus = models.PlacementStatusScheduled
	mockStore.addVM(vm)

	// Call reconcileRunning
	svc.reconcileRunning(ctx, vm)

	// Verify the flow
	if !mockAgent.startVMCalled {
		t.Error("Expected StartVM to be called on agent")
	}

	// Check final state
	updatedVM := mockStore.vms[vmID]
	if updatedVM == nil {
		t.Fatal("VM should still exist")
	}
	if updatedVM.ActualState != models.VMActualStateRunning {
		t.Errorf("Expected VM state to be running, got %s", updatedVM.ActualState)
	}
}

// TestReconcileRunning_StoppedToRunning tests starting a stopped VM
func TestReconcileRunning_StoppedToRunning(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create node
	nodeID := uuid.New()
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	mockStore.addNode(node)

	// Create VM in stopped state with node assigned
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateRunning, models.VMActualStateStopped)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Call reconcileRunning
	svc.reconcileRunning(ctx, vm)

	// Verify StartVM was called
	if !mockAgent.startVMCalled {
		t.Error("Expected StartVM to be called")
	}

	// Check final state
	updatedVM := mockStore.vms[vmID]
	if updatedVM.ActualState != models.VMActualStateRunning {
		t.Errorf("Expected VM state to be running, got %s", updatedVM.ActualState)
	}
}

// TestReconcileRunning_AlreadyRunning tests no-op when VM already running
func TestReconcileRunning_AlreadyRunning(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create VM already running
	vmID := uuid.New()
	nodeID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateRunning, models.VMActualStateRunning)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Call reconcileRunning
	svc.reconcileRunning(ctx, vm)

	// Verify StartVM was NOT called
	if mockAgent.startVMCalled {
		t.Error("Expected StartVM NOT to be called for already running VM")
	}
}

// TestReconcileRunning_SchedulingFails tests error handling when scheduler fails
func TestReconcileRunning_SchedulingFails(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create VM in provisioning state without node
	// Don't create any nodes, so scheduling will fail
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateRunning, models.VMActualStateProvisioning)
	mockStore.addVM(vm)

	// Call reconcileRunning
	svc.reconcileRunning(ctx, vm)

	// VM should be updated with error state
	updatedVM := mockStore.vms[vmID]
	if updatedVM == nil {
		t.Fatal("VM should still exist")
	}
	if updatedVM.ActualState != models.VMActualStateError {
		t.Errorf("Expected VM state to be error, got %s", updatedVM.ActualState)
	}
}

// TestReconcileRunning_StartVMFails tests error handling when agent start fails
func TestReconcileRunning_StartVMFails(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	mockAgent.startVMError = errors.New("start failed")
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create node
	nodeID := uuid.New()
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	mockStore.addNode(node)

	// Create VM in stopped state with node
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateRunning, models.VMActualStateStopped)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Call reconcileRunning - will try twice then fail
	svc.reconcileRunning(ctx, vm)

	// VM should be in error state
	updatedVM := mockStore.vms[vmID]
	if updatedVM == nil {
		t.Fatal("VM should still exist")
	}
	if updatedVM.ActualState != models.VMActualStateError {
		t.Errorf("Expected VM state to be error, got %s", updatedVM.ActualState)
	}
	if updatedVM.LastError == nil {
		t.Error("Expected LastError to be set")
	}
}

// TestReconcileRunning_NodeNotFound tests error when assigned node is missing
func TestReconcileRunning_NodeNotFound(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create VM with non-existent node
	vmID := uuid.New()
	nodeID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateRunning, models.VMActualStateStopped)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Don't create the node - it doesn't exist

	// Call reconcileRunning
	svc.reconcileRunning(ctx, vm)

	// VM should be in error state
	updatedVM := mockStore.vms[vmID]
	if updatedVM == nil {
		t.Fatal("VM should still exist")
	}
	if updatedVM.ActualState != models.VMActualStateError {
		t.Errorf("Expected VM state to be error, got %s", updatedVM.ActualState)
	}
}

// TestReconcileStopped_RunningToStopped tests stopping a running VM
func TestReconcileStopped_RunningToStopped(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create node
	nodeID := uuid.New()
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	mockStore.addNode(node)

	// Create running VM
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateStopped, models.VMActualStateRunning)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Call reconcileStopped
	svc.reconcileStopped(ctx, vm)

	// Verify StopVM was called
	if !mockAgent.stopVMCalled {
		t.Error("Expected StopVM to be called")
	}

	// Check final state
	updatedVM := mockStore.vms[vmID]
	if updatedVM.ActualState != models.VMActualStateStopped {
		t.Errorf("Expected VM state to be stopped, got %s", updatedVM.ActualState)
	}
}

// TestReconcileStopped_AlreadyStopped tests no-op when VM already stopped
func TestReconcileStopped_AlreadyStopped(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create already stopped VM
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateStopped, models.VMActualStateStopped)
	mockStore.addVM(vm)

	// Call reconcileStopped
	svc.reconcileStopped(ctx, vm)

	// Verify StopVM was NOT called
	if mockAgent.stopVMCalled {
		t.Error("Expected StopVM NOT to be called for already stopped VM")
	}
}

// TestReconcileStopped_NoNode tests marking stopped when no node assigned
func TestReconcileStopped_NoNode(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create running VM without node
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateStopped, models.VMActualStateRunning)
	// NodeID is nil
	mockStore.addVM(vm)

	// Call reconcileStopped
	svc.reconcileStopped(ctx, vm)

	// Verify StopVM was NOT called (no node to stop on)
	if mockAgent.stopVMCalled {
		t.Error("Expected StopVM NOT to be called when no node assigned")
	}

	// VM should be marked as stopped
	updatedVM := mockStore.vms[vmID]
	if updatedVM.ActualState != models.VMActualStateStopped {
		t.Errorf("Expected VM state to be stopped, got %s", updatedVM.ActualState)
	}
}

// TestReconcileStopped_NodeGone tests handling when node is deleted
func TestReconcileStopped_NodeGone(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create running VM with non-existent node
	vmID := uuid.New()
	nodeID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateStopped, models.VMActualStateRunning)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Don't create the node

	// Call reconcileStopped
	svc.reconcileStopped(ctx, vm)

	// VM should be marked as stopped
	updatedVM := mockStore.vms[vmID]
	if updatedVM.ActualState != models.VMActualStateStopped {
		t.Errorf("Expected VM state to be stopped, got %s", updatedVM.ActualState)
	}
}

// TestReconcileStopped_StopFails tests retry on stop failure
func TestReconcileStopped_StopFails(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	mockAgent.stopVMError = errors.New("stop failed")
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create node
	nodeID := uuid.New()
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	mockStore.addNode(node)

	// Create running VM
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateStopped, models.VMActualStateRunning)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Call reconcileStopped - will try stop twice
	svc.reconcileStopped(ctx, vm)

	// Even if stop fails, VM should still be marked as stopped
	updatedVM := mockStore.vms[vmID]
	if updatedVM.ActualState != models.VMActualStateStopped {
		t.Errorf("Expected VM state to be stopped, got %s", updatedVM.ActualState)
	}
}

// TestReconcileDeleted_RunningToDeleted tests stopping and deleting a running VM
func TestReconcileDeleted_RunningToDeleted(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create node
	nodeID := uuid.New()
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	mockStore.addNode(node)

	// Create running VM
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateDeleted, models.VMActualStateRunning)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Call reconcileDeleted
	svc.reconcileDeleted(ctx, vm)

	// Verify StopVM and DeleteVM were called
	if !mockAgent.stopVMCalled {
		t.Error("Expected StopVM to be called")
	}
	if !mockAgent.deleteVMCalled {
		t.Error("Expected DeleteVM to be called")
	}

	// VM should be deleted
	if _, exists := mockStore.vms[vmID]; exists {
		t.Error("VM should have been deleted")
	}
}

// TestReconcileDeleted_StoppedToDeleted tests deleting a stopped VM
func TestReconcileDeleted_StoppedToDeleted(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create node
	nodeID := uuid.New()
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	mockStore.addNode(node)

	// Create stopped VM
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateDeleted, models.VMActualStateStopped)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Call reconcileDeleted
	svc.reconcileDeleted(ctx, vm)

	// Verify DeleteVM was called (but not StopVM since already stopped)
	if mockAgent.stopVMCalled {
		t.Error("Expected StopVM NOT to be called for already stopped VM")
	}
	if !mockAgent.deleteVMCalled {
		t.Error("Expected DeleteVM to be called")
	}

	// VM should be deleted
	if _, exists := mockStore.vms[vmID]; exists {
		t.Error("VM should have been deleted")
	}
}

// TestReconcileDeleted_AgentDeleteFails tests continuing even if agent delete fails
func TestReconcileDeleted_AgentDeleteFails(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	mockAgent.deleteVMError = errors.New("delete failed")
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create node
	nodeID := uuid.New()
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	mockStore.addNode(node)

	// Create stopped VM
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateDeleted, models.VMActualStateStopped)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Call reconcileDeleted - delete will fail but should continue
	svc.reconcileDeleted(ctx, vm)

	// VM should still be deleted from database
	if _, exists := mockStore.vms[vmID]; exists {
		t.Error("VM should have been deleted from database even if agent delete failed")
	}
}

// TestReconcileDeleted_ResourcesReleased tests that resources are released
func TestReconcileDeleted_ResourcesReleased(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create node with some allocated resources
	nodeID := uuid.New()
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	node.AllocatableCPUCores = 6  // 2 cores allocated to VM
	node.AllocatableRAMMB = 12288 // 4GB allocated to VM
	mockStore.addNode(node)

	// Create VM
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateDeleted, models.VMActualStateStopped)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Call reconcileDeleted
	svc.reconcileDeleted(ctx, vm)

	// Verify resources were released
	updatedNode := mockStore.nodes[nodeID]
	if updatedNode.AllocatableCPUCores != 8 {
		t.Errorf("Expected CPU to be 8 after release, got %d", updatedNode.AllocatableCPUCores)
	}
	if updatedNode.AllocatableRAMMB != 16384 {
		t.Errorf("Expected RAM to be 16384 after release, got %d", updatedNode.AllocatableRAMMB)
	}
}

// TestReconcileVM_GetVMFails tests handling DB error
func TestReconcileVM_GetVMFails(t *testing.T) {
	mockStore := newMockStore()
	mockStore.getVMError = errors.New("database error")
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	vmID := uuid.New()

	// Call reconcileVM - should handle error gracefully
	svc.reconcileVM(ctx, vmID)

	// Should not panic and should log error
	if !mockStore.getVMCalled {
		t.Error("Expected GetVM to be called")
	}
}

// TestReconcileVM_VMNotFound tests handling missing VM
func TestReconcileVM_VMNotFound(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	vmID := uuid.New()

	// Call reconcileVM for non-existent VM
	svc.reconcileVM(ctx, vmID)

	// Should return without error
	if !mockStore.getVMCalled {
		t.Error("Expected GetVM to be called")
	}
}

// TestReconcileVM_NoReconciliationNeeded tests skip when desired == actual
func TestReconcileVM_NoReconciliationNeeded(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create VM already in desired state
	vmID := uuid.New()
	nodeID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateRunning, models.VMActualStateRunning)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Create node
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	mockStore.addNode(node)

	// Call reconcileVM
	svc.reconcileVM(ctx, vmID)

	// Should not call any agent methods since VM is already in desired state
	if mockAgent.startVMCalled {
		t.Error("Expected StartVM NOT to be called when already running")
	}
}

// TestReconcileVM_DesiredRunning tests reconciliation for running desired state
func TestReconcileVM_DesiredRunning(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create node
	nodeID := uuid.New()
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	mockStore.addNode(node)

	// Create VM that needs to be started
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateRunning, models.VMActualStateStopped)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Call reconcileVM
	svc.reconcileVM(ctx, vmID)

	// Should call StartVM
	if !mockAgent.startVMCalled {
		t.Error("Expected StartVM to be called")
	}
}

// TestReconcileVM_DesiredStopped tests reconciliation for stopped desired state
func TestReconcileVM_DesiredStopped(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create node
	nodeID := uuid.New()
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	mockStore.addNode(node)

	// Create VM that needs to be stopped
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateStopped, models.VMActualStateRunning)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Call reconcileVM
	svc.reconcileVM(ctx, vmID)

	// Should call StopVM
	if !mockAgent.stopVMCalled {
		t.Error("Expected StopVM to be called")
	}
}

// TestReconcileVM_DesiredDeleted tests reconciliation for deleted desired state
func TestReconcileVM_DesiredDeleted(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create node
	nodeID := uuid.New()
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	mockStore.addNode(node)

	// Create VM that needs to be deleted
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateDeleted, models.VMActualStateStopped)
	vm.NodeID = &nodeID
	mockStore.addVM(vm)

	// Call reconcileVM
	svc.reconcileVM(ctx, vmID)

	// Should delete VM
	if !mockStore.deleteVMCalled {
		t.Error("Expected DeleteVM to be called on store")
	}
	if _, exists := mockStore.vms[vmID]; exists {
		t.Error("VM should have been deleted")
	}
}

// TestReconcileAll tests the reconcileAll method
func TestReconcileAll(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create node
	nodeID := uuid.New()
	node := createTestNode(nodeID, "test-node", "10.0.0.1")
	mockStore.addNode(node)

	// Create VMs needing reconciliation
	vm1ID := uuid.New()
	vm1 := createTestVM(vm1ID, "test-vm-1", models.VMDesiredStateRunning, models.VMActualStateStopped)
	vm1.NodeID = &nodeID
	mockStore.vms[vm1ID] = vm1

	vm2ID := uuid.New()
	vm2 := createTestVM(vm2ID, "test-vm-2", models.VMDesiredStateStopped, models.VMActualStateRunning)
	vm2.NodeID = &nodeID
	mockStore.vms[vm2ID] = vm2

	// VM already reconciled - should not be processed
	vm3ID := uuid.New()
	vm3 := createTestVM(vm3ID, "test-vm-3", models.VMDesiredStateRunning, models.VMActualStateRunning)
	vm3.NodeID = &nodeID
	mockStore.vms[vm3ID] = vm3

	// Call reconcileAll
	svc.reconcileAll(ctx)

	// Verify all VMs were processed
	if !mockStore.listVMsCalled {
		t.Error("Expected ListVMsNeedingReconciliation to be called")
	}

	// vm1 should be started
	if !mockAgent.startVMCalled {
		t.Error("Expected StartVM to be called for vm1")
	}

	// vm2 should be stopped
	if !mockAgent.stopVMCalled {
		t.Error("Expected StopVM to be called for vm2")
	}
}

// TestReconcileAll_ListFails tests handling list error
func TestReconcileAll_ListFails(t *testing.T) {
	mockStore := newMockStore()
	mockStore.listVMsError = errors.New("database error")
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Call reconcileAll - should handle error gracefully
	svc.reconcileAll(ctx)

	// Should have called list but not panic
	if !mockStore.listVMsCalled {
		t.Error("Expected ListVMsNeedingReconciliation to be called")
	}
}

// TestSetError tests the setError helper
func TestSetError(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create VM
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateRunning, models.VMActualStateProvisioning)
	mockStore.addVM(vm)

	// Test with errorsx.Error
	appErr := errorsx.New(errorsx.ErrInternal, "test error")
	svc.setError(ctx, vm, appErr)

	// Check error was set
	updatedVM := mockStore.vms[vmID]
	if updatedVM.ActualState != models.VMActualStateError {
		t.Errorf("Expected state to be error, got %s", updatedVM.ActualState)
	}
	if updatedVM.LastError == nil {
		t.Fatal("Expected LastError to be set")
	}

	// Parse error JSON
	var errData map[string]interface{}
	if err := json.Unmarshal(updatedVM.LastError, &errData); err != nil {
		t.Fatalf("Failed to unmarshal error: %v", err)
	}
	if errData["code"] != "INTERNAL_ERROR" {
		t.Errorf("Expected code INTERNAL_ERROR, got %v", errData["code"])
	}
}

// TestSetError_GenericError tests setError with generic error
func TestSetError_GenericError(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create VM
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateRunning, models.VMActualStateProvisioning)
	mockStore.addVM(vm)

	// Test with generic error
	genericErr := errors.New("generic test error")
	svc.setError(ctx, vm, genericErr)

	// Check error was set
	updatedVM := mockStore.vms[vmID]
	if updatedVM.ActualState != models.VMActualStateError {
		t.Errorf("Expected state to be error, got %s", updatedVM.ActualState)
	}
	if updatedVM.LastError == nil {
		t.Fatal("Expected LastError to be set")
	}

	// Parse error JSON
	var errData map[string]interface{}
	if err := json.Unmarshal(updatedVM.LastError, &errData); err != nil {
		t.Fatalf("Failed to unmarshal error: %v", err)
	}
	if errData["code"] != "INTERNAL_ERROR" {
		t.Errorf("Expected code INTERNAL_ERROR, got %v", errData["code"])
	}
}

// TestReconcileDeleted_NoNode tests deleting VM without node
func TestReconcileDeleted_NoNode(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)
	ctx := context.Background()

	// Create stopped VM without node
	vmID := uuid.New()
	vm := createTestVM(vmID, "test-vm", models.VMDesiredStateDeleted, models.VMActualStateStopped)
	// NodeID is nil
	mockStore.addVM(vm)

	// Call reconcileDeleted
	svc.reconcileDeleted(ctx, vm)

	// Should skip agent calls but still delete from DB
	if mockAgent.stopVMCalled {
		t.Error("Expected StopVM NOT to be called without node")
	}
	if mockAgent.deleteVMCalled {
		t.Error("Expected DeleteVM NOT to be called without node")
	}
	if _, exists := mockStore.vms[vmID]; exists {
		t.Error("VM should have been deleted from database")
	}
}

// TestNewService_WithNilAgent tests creating service with nil agent
func TestNewService_WithNilAgent(t *testing.T) {
	mockStore := newMockStore()
	sched := scheduler.NewService(mockStore)

	// Create service with nil agent - should create default client
	svc := NewService(mockStore, sched, nil)

	if svc.agentClient == nil {
		t.Error("Expected agentClient to be created when nil is passed")
	}
}

// TestTriggerVM_ChannelFull tests trigger when channel is full
func TestTriggerVM_ChannelFull(t *testing.T) {
	mockStore := newMockStore()
	mockAgent := newMockAgentClient()
	sched := scheduler.NewService(mockStore)

	svc := NewService(mockStore, sched, mockAgent)

	// Fill up the channel
	for i := 0; i < 100; i++ {
		svc.TriggerVM(uuid.New())
	}

	// This should not block - just drop the message
	svc.TriggerVM(uuid.New())

	// Success - no panic means we handled the full channel gracefully
}

// Table-driven test for reconcileRunning with various states
func TestReconcileRunning_TableDriven(t *testing.T) {
	nodeID := uuid.New()

	tests := []struct {
		name          string
		actualState   models.VMActualState
		setupVM       func(*models.VirtualMachine)
		setupMocks    func(*mockStore, *mockAgentClient)
		expectStartVM bool
		expectError   bool
		expectedState models.VMActualState
	}{
		{
			name:          "provisioning to running",
			actualState:   models.VMActualStateProvisioning,
			setupVM:       func(vm *models.VirtualMachine) { vm.NodeID = &nodeID },
			setupMocks:    func(s *mockStore, a *mockAgentClient) {},
			expectStartVM: true,
			expectedState: models.VMActualStateRunning,
		},
		{
			name:          "error state to running",
			actualState:   models.VMActualStateError,
			setupVM:       func(vm *models.VirtualMachine) { vm.NodeID = &nodeID },
			setupMocks:    func(s *mockStore, a *mockAgentClient) {},
			expectStartVM: true,
			expectedState: models.VMActualStateRunning,
		},
		{
			name:          "unknown state to running",
			actualState:   models.VMActualStateUnknown,
			setupVM:       func(vm *models.VirtualMachine) { vm.NodeID = &nodeID },
			setupMocks:    func(s *mockStore, a *mockAgentClient) {},
			expectStartVM: true,
			expectedState: models.VMActualStateRunning,
		},
		{
			name:          "already running",
			actualState:   models.VMActualStateRunning,
			setupVM:       func(vm *models.VirtualMachine) { vm.NodeID = &nodeID },
			setupMocks:    func(s *mockStore, a *mockAgentClient) {},
			expectStartVM: false,
			expectedState: models.VMActualStateRunning,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			mockStore := newMockStore()
			mockAgent := newMockAgentClient()
			sched := scheduler.NewService(mockStore)

			// Setup node
			node := createTestNode(nodeID, "test-node", "10.0.0.1")
			mockStore.addNode(node)

			svc := NewService(mockStore, sched, mockAgent)
			ctx := context.Background()

			vmID := uuid.New()
			vm := createTestVM(vmID, "test-vm", models.VMDesiredStateRunning, tt.actualState)
			tt.setupVM(vm)
			mockStore.addVM(vm)

			tt.setupMocks(mockStore, mockAgent)

			svc.reconcileRunning(ctx, vm)

			if tt.expectStartVM && !mockAgent.startVMCalled {
				t.Error("Expected StartVM to be called")
			}
			if !tt.expectStartVM && mockAgent.startVMCalled {
				t.Error("Expected StartVM NOT to be called")
			}

			updatedVM := mockStore.vms[vmID]
			if updatedVM != nil && updatedVM.ActualState != tt.expectedState {
				t.Errorf("Expected state %s, got %s", tt.expectedState, updatedVM.ActualState)
			}
		})
	}
}

// Table-driven test for reconcileStopped
func TestReconcileStopped_TableDriven(t *testing.T) {
	nodeID := uuid.New()

	tests := []struct {
		name          string
		actualState   models.VMActualState
		setupVM       func(*models.VirtualMachine)
		setupMocks    func(*mockStore, *mockAgentClient)
		expectStopVM  bool
		expectedState models.VMActualState
	}{
		{
			name:          "running to stopped",
			actualState:   models.VMActualStateRunning,
			setupVM:       func(vm *models.VirtualMachine) { vm.NodeID = &nodeID },
			setupMocks:    func(s *mockStore, a *mockAgentClient) {},
			expectStopVM:  true,
			expectedState: models.VMActualStateStopped,
		},
		{
			name:          "starting to stopped",
			actualState:   models.VMActualStateStarting,
			setupVM:       func(vm *models.VirtualMachine) { vm.NodeID = &nodeID },
			setupMocks:    func(s *mockStore, a *mockAgentClient) {},
			expectStopVM:  true,
			expectedState: models.VMActualStateStopped,
		},
		{
			name:          "already stopped",
			actualState:   models.VMActualStateStopped,
			setupVM:       func(vm *models.VirtualMachine) {},
			setupMocks:    func(s *mockStore, a *mockAgentClient) {},
			expectStopVM:  false,
			expectedState: models.VMActualStateStopped,
		},
		{
			name:          "provisioning stays provisioning",
			actualState:   models.VMActualStateProvisioning,
			setupVM:       func(vm *models.VirtualMachine) {},
			setupMocks:    func(s *mockStore, a *mockAgentClient) {},
			expectStopVM:  false,
			expectedState: models.VMActualStateProvisioning,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			mockStore := newMockStore()
			mockAgent := newMockAgentClient()
			sched := scheduler.NewService(mockStore)

			// Setup node
			node := createTestNode(nodeID, "test-node", "10.0.0.1")
			mockStore.addNode(node)

			svc := NewService(mockStore, sched, mockAgent)
			ctx := context.Background()

			vmID := uuid.New()
			vm := createTestVM(vmID, "test-vm", models.VMDesiredStateStopped, tt.actualState)
			tt.setupVM(vm)
			mockStore.addVM(vm)

			tt.setupMocks(mockStore, mockAgent)

			svc.reconcileStopped(ctx, vm)

			if tt.expectStopVM && !mockAgent.stopVMCalled {
				t.Error("Expected StopVM to be called")
			}
			if !tt.expectStopVM && mockAgent.stopVMCalled {
				t.Error("Expected StopVM NOT to be called")
			}

			updatedVM := mockStore.vms[vmID]
			if updatedVM != nil && updatedVM.ActualState != tt.expectedState {
				t.Errorf("Expected state %s, got %s", tt.expectedState, updatedVM.ActualState)
			}
		})
	}
}

// Ensure mockStore implements store.Store interface
var _ store.Store = (*mockStore)(nil)
