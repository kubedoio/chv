// Package store provides database storage for CHV models.
package store

import (
	"context"

	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
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

	// Snapshots
	CreateSnapshot(ctx context.Context, snapshot *models.Snapshot) error
	GetSnapshot(ctx context.Context, id uuid.UUID) (*models.Snapshot, error)
	UpdateSnapshot(ctx context.Context, snapshot *models.Snapshot) error
	ListSnapshotsByVM(ctx context.Context, vmID uuid.UUID) ([]*models.Snapshot, error)
	DeleteSnapshot(ctx context.Context, id uuid.UUID) error

	// VM Network Attachments
	CreateVMNetworkAttachment(ctx context.Context, attachment *models.VMNetworkAttachment) error
	ListVMNetworkAttachments(ctx context.Context, vmID uuid.UUID) ([]*models.VMNetworkAttachment, error)

	// API Tokens
	CreateAPIToken(ctx context.Context, token *models.APIToken) error
	GetAPITokenByHash(ctx context.Context, hash string) (*models.APIToken, error)
	ListAPITokens(ctx context.Context) ([]*models.APIToken, error)
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

	// Resource Quotas
	GetQuota(ctx context.Context, userID string) (*models.ResourceQuota, error)
	SetQuota(ctx context.Context, quota *models.ResourceQuota) error
	GetUsage(ctx context.Context, userID string) (*models.ResourceUsage, error)
	UpdateUsage(ctx context.Context, userID string, delta models.ResourceUsage) error
	EnsureQuota(ctx context.Context, userID string) error
}
