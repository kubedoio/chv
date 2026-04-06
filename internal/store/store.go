// Package store provides database storage for CHV models.
package store

import (
	"context"

	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgconn"
	"github.com/jackc/pgx/v5/pgxpool"
)

// Store defines the database operations.
type Store interface {
	// Nodes
	CreateNode(ctx context.Context, node *models.Node) error
	GetNode(ctx context.Context, id uuid.UUID) (*models.Node, error)
	GetNodeByHostname(ctx context.Context, hostname string) (*models.Node, error)
	UpdateNode(ctx context.Context, node *models.Node) error
	UpdateNodeHeartbeat(ctx context.Context, id uuid.UUID, status models.NodeState) error
	ListNodes(ctx context.Context) ([]*models.Node, error)
	SetNodeMaintenance(ctx context.Context, id uuid.UUID, enabled bool) error
	
	// Networks
	CreateNetwork(ctx context.Context, network *models.Network) error
	GetNetwork(ctx context.Context, id uuid.UUID) (*models.Network, error)
	ListNetworks(ctx context.Context) ([]*models.Network, error)
	DeleteNetwork(ctx context.Context, id uuid.UUID) error
	
	// Storage Pools
	CreateStoragePool(ctx context.Context, pool *models.StoragePool) error
	GetStoragePool(ctx context.Context, id uuid.UUID) (*models.StoragePool, error)
	ListStoragePools(ctx context.Context) ([]*models.StoragePool, error)
	ListStoragePoolsByNode(ctx context.Context, nodeID uuid.UUID) ([]*models.StoragePool, error)
	DeleteStoragePool(ctx context.Context, id uuid.UUID) error
	
	// Images
	CreateImage(ctx context.Context, image *models.Image) error
	GetImage(ctx context.Context, id uuid.UUID) (*models.Image, error)
	UpdateImage(ctx context.Context, image *models.Image) error
	ListImages(ctx context.Context) ([]*models.Image, error)
	DeleteImage(ctx context.Context, id uuid.UUID) error
	
	// VMs
	CreateVM(ctx context.Context, vm *models.VirtualMachine) error
	GetVM(ctx context.Context, id uuid.UUID) (*models.VirtualMachine, error)
	GetVMByName(ctx context.Context, name string) (*models.VirtualMachine, error)
	UpdateVM(ctx context.Context, vm *models.VirtualMachine) error
	UpdateVMActualState(ctx context.Context, id uuid.UUID, state models.VMActualState, lastError []byte) error
	ListVMs(ctx context.Context) ([]*models.VirtualMachine, error)
	ListVMsByNode(ctx context.Context, nodeID uuid.UUID) ([]*models.VirtualMachine, error)
	ListVMsNeedingReconciliation(ctx context.Context) ([]*models.VirtualMachine, error)
	DeleteVM(ctx context.Context, id uuid.UUID) error
	
	// Volumes
	CreateVolume(ctx context.Context, volume *models.Volume) error
	GetVolume(ctx context.Context, id uuid.UUID) (*models.Volume, error)
	UpdateVolume(ctx context.Context, volume *models.Volume) error
	ListVolumesByVM(ctx context.Context, vmID uuid.UUID) ([]*models.Volume, error)
	
	// VM Network Attachments
	CreateVMNetworkAttachment(ctx context.Context, attachment *models.VMNetworkAttachment) error
	ListVMNetworkAttachments(ctx context.Context, vmID uuid.UUID) ([]*models.VMNetworkAttachment, error)
	
	// API Tokens
	CreateAPIToken(ctx context.Context, token *models.APIToken) error
	GetAPITokenByHash(ctx context.Context, hash string) (*models.APIToken, error)
	RevokeAPIToken(ctx context.Context, id uuid.UUID) error
	
	// Operations
	CreateOperation(ctx context.Context, op *models.Operation) error
	GetOperation(ctx context.Context, id uuid.UUID) (*models.Operation, error)
	UpdateOperation(ctx context.Context, op *models.Operation) error
	ListOperations(ctx context.Context, filters map[string]interface{}) ([]*models.Operation, error)
	CreateOperationLog(ctx context.Context, log *models.OperationLog) error
	GetOperationLogs(ctx context.Context, operationID uuid.UUID) ([]*models.OperationLog, error)
	
	// Transactions
	WithTx(ctx context.Context, fn func(Store) error) error
}

// PostgresStore implements Store using PostgreSQL.
type PostgresStore struct {
	pool *pgxpool.Pool
}

// NewPostgresStore creates a new PostgreSQL store.
func NewPostgresStore(pool *pgxpool.Pool) *PostgresStore {
	return &PostgresStore{pool: pool}
}

// WithTx executes a function within a transaction.
func (s *PostgresStore) WithTx(ctx context.Context, fn func(Store) error) error {
	tx, err := s.pool.Begin(ctx)
	if err != nil {
		return err
	}
	
	defer tx.Rollback(ctx)
	
	txStore := &txStore{tx: tx}
	if err := fn(txStore); err != nil {
		return err
	}
	
	return tx.Commit(ctx)
}

// txStore is a Store implementation that uses a transaction.
type txStore struct {
	tx pgx.Tx
}

// Implement Store methods for txStore...
// (These delegate to the main implementation with the tx)

func (s *txStore) CreateNode(ctx context.Context, node *models.Node) error {
	return createNode(ctx, s.tx, node)
}

func (s *txStore) GetNode(ctx context.Context, id uuid.UUID) (*models.Node, error) {
	return getNode(ctx, s.tx, id)
}

func (s *txStore) GetNodeByHostname(ctx context.Context, hostname string) (*models.Node, error) {
	return getNodeByHostname(ctx, s.tx, hostname)
}

func (s *txStore) UpdateNode(ctx context.Context, node *models.Node) error {
	return updateNode(ctx, s.tx, node)
}

func (s *txStore) UpdateNodeHeartbeat(ctx context.Context, id uuid.UUID, status models.NodeState) error {
	return updateNodeHeartbeat(ctx, s.tx, id, status)
}

func (s *txStore) ListNodes(ctx context.Context) ([]*models.Node, error) {
	return listNodes(ctx, s.tx)
}

func (s *txStore) SetNodeMaintenance(ctx context.Context, id uuid.UUID, enabled bool) error {
	return setNodeMaintenance(ctx, s.tx, id, enabled)
}

func (s *txStore) CreateNetwork(ctx context.Context, network *models.Network) error {
	return createNetwork(ctx, s.tx, network)
}

func (s *txStore) GetNetwork(ctx context.Context, id uuid.UUID) (*models.Network, error) {
	return getNetwork(ctx, s.tx, id)
}

func (s *txStore) ListNetworks(ctx context.Context) ([]*models.Network, error) {
	return listNetworks(ctx, s.tx)
}

func (s *txStore) DeleteNetwork(ctx context.Context, id uuid.UUID) error {
	return deleteNetwork(ctx, s.tx, id)
}

func (s *txStore) CreateStoragePool(ctx context.Context, pool *models.StoragePool) error {
	return createStoragePool(ctx, s.tx, pool)
}

func (s *txStore) GetStoragePool(ctx context.Context, id uuid.UUID) (*models.StoragePool, error) {
	return getStoragePool(ctx, s.tx, id)
}

func (s *txStore) ListStoragePools(ctx context.Context) ([]*models.StoragePool, error) {
	return listStoragePools(ctx, s.tx)
}

func (s *txStore) ListStoragePoolsByNode(ctx context.Context, nodeID uuid.UUID) ([]*models.StoragePool, error) {
	return listStoragePoolsByNode(ctx, s.tx, nodeID)
}

func (s *txStore) DeleteStoragePool(ctx context.Context, id uuid.UUID) error {
	return deleteStoragePool(ctx, s.tx, id)
}

func (s *txStore) CreateImage(ctx context.Context, image *models.Image) error {
	return createImage(ctx, s.tx, image)
}

func (s *txStore) GetImage(ctx context.Context, id uuid.UUID) (*models.Image, error) {
	return getImage(ctx, s.tx, id)
}

func (s *txStore) UpdateImage(ctx context.Context, image *models.Image) error {
	return updateImage(ctx, s.tx, image)
}

func (s *txStore) ListImages(ctx context.Context) ([]*models.Image, error) {
	return listImages(ctx, s.tx)
}

func (s *txStore) DeleteImage(ctx context.Context, id uuid.UUID) error {
	return deleteImage(ctx, s.tx, id)
}

func (s *txStore) CreateVM(ctx context.Context, vm *models.VirtualMachine) error {
	return createVM(ctx, s.tx, vm)
}

func (s *txStore) GetVM(ctx context.Context, id uuid.UUID) (*models.VirtualMachine, error) {
	return getVM(ctx, s.tx, id)
}

func (s *txStore) GetVMByName(ctx context.Context, name string) (*models.VirtualMachine, error) {
	return getVMByName(ctx, s.tx, name)
}

func (s *txStore) UpdateVM(ctx context.Context, vm *models.VirtualMachine) error {
	return updateVM(ctx, s.tx, vm)
}

func (s *txStore) UpdateVMActualState(ctx context.Context, id uuid.UUID, state models.VMActualState, lastError []byte) error {
	return updateVMActualState(ctx, s.tx, id, state, lastError)
}

func (s *txStore) ListVMs(ctx context.Context) ([]*models.VirtualMachine, error) {
	return listVMs(ctx, s.tx)
}

func (s *txStore) ListVMsByNode(ctx context.Context, nodeID uuid.UUID) ([]*models.VirtualMachine, error) {
	return listVMsByNode(ctx, s.tx, nodeID)
}

func (s *txStore) ListVMsNeedingReconciliation(ctx context.Context) ([]*models.VirtualMachine, error) {
	return listVMsNeedingReconciliation(ctx, s.tx)
}

func (s *txStore) DeleteVM(ctx context.Context, id uuid.UUID) error {
	return deleteVM(ctx, s.tx, id)
}

func (s *txStore) CreateVolume(ctx context.Context, volume *models.Volume) error {
	return createVolume(ctx, s.tx, volume)
}

func (s *txStore) GetVolume(ctx context.Context, id uuid.UUID) (*models.Volume, error) {
	return getVolume(ctx, s.tx, id)
}

func (s *txStore) UpdateVolume(ctx context.Context, volume *models.Volume) error {
	return updateVolume(ctx, s.tx, volume)
}

func (s *txStore) ListVolumesByVM(ctx context.Context, vmID uuid.UUID) ([]*models.Volume, error) {
	return listVolumesByVM(ctx, s.tx, vmID)
}

func (s *txStore) CreateVMNetworkAttachment(ctx context.Context, attachment *models.VMNetworkAttachment) error {
	return createVMNetworkAttachment(ctx, s.tx, attachment)
}

func (s *txStore) ListVMNetworkAttachments(ctx context.Context, vmID uuid.UUID) ([]*models.VMNetworkAttachment, error) {
	return listVMNetworkAttachments(ctx, s.tx, vmID)
}

func (s *txStore) CreateAPIToken(ctx context.Context, token *models.APIToken) error {
	return createAPIToken(ctx, s.tx, token)
}

func (s *txStore) GetAPITokenByHash(ctx context.Context, hash string) (*models.APIToken, error) {
	return getAPITokenByHash(ctx, s.tx, hash)
}

func (s *txStore) RevokeAPIToken(ctx context.Context, id uuid.UUID) error {
	return revokeAPIToken(ctx, s.tx, id)
}

func (s *txStore) CreateOperation(ctx context.Context, op *models.Operation) error {
	return createOperation(ctx, s.tx, op)
}

func (s *txStore) GetOperation(ctx context.Context, id uuid.UUID) (*models.Operation, error) {
	return getOperation(ctx, s.tx, id)
}

func (s *txStore) UpdateOperation(ctx context.Context, op *models.Operation) error {
	return updateOperation(ctx, s.tx, op)
}

func (s *txStore) ListOperations(ctx context.Context, filters map[string]interface{}) ([]*models.Operation, error) {
	return listOperations(ctx, s.tx, filters)
}

func (s *txStore) CreateOperationLog(ctx context.Context, log *models.OperationLog) error {
	return createOperationLog(ctx, s.tx, log)
}

func (s *txStore) GetOperationLogs(ctx context.Context, operationID uuid.UUID) ([]*models.OperationLog, error) {
	return getOperationLogs(ctx, s.tx, operationID)
}

func (s *txStore) WithTx(ctx context.Context, fn func(Store) error) error {
	return fn(s)
}

// Helper type for query operations
type querier interface {
	Query(ctx context.Context, sql string, args ...interface{}) (pgx.Rows, error)
	QueryRow(ctx context.Context, sql string, args ...interface{}) pgx.Row
	Exec(ctx context.Context, sql string, args ...interface{}) (pgconn.CommandTag, error)
}
