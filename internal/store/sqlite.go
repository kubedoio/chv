// Package store provides database storage for CHV models.
package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"log"
	"strings"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
	_ "modernc.org/sqlite"
)

// parseTime parses a timestamp from SQLite.
// It handles multiple formats including Go's time.String() format.
func parseTime(s string) (time.Time, error) {
	if s == "" {
		return time.Time{}, nil
	}
	// Remove monotonic reading if present (e.g., " m=+0.000000001")
	if idx := strings.Index(s, " m="); idx != -1 {
		s = s[:idx]
	}
	// Try RFC3339Nano first
	if t, err := time.Parse(time.RFC3339Nano, s); err == nil {
		return t, nil
	}
	// Try RFC3339
	if t, err := time.Parse(time.RFC3339, s); err == nil {
		return t, nil
	}
	// Try Go's time.String() format: "2006-01-02 15:04:05.999999999 -0700 MST"
	if t, err := time.Parse("2006-01-02 15:04:05.999999999 -0700 MST", s); err == nil {
		return t, nil
	}
	// Try simpler format without timezone name: "2006-01-02 15:04:05.999999999 -0700"
	if t, err := time.Parse("2006-01-02 15:04:05.999999999 -0700", s); err == nil {
		return t, nil
	}
	// Try format without fractional seconds: "2006-01-02 15:04:05 -0700 MST"
	if t, err := time.Parse("2006-01-02 15:04:05 -0700 MST", s); err == nil {
		return t, nil
	}
	// Try simple datetime format
	if t, err := time.Parse("2006-01-02 15:04:05", s); err == nil {
		return t, nil
	}
	return time.Time{}, fmt.Errorf("unable to parse time: %s", s)
}

// SQLiteStore implements Store using SQLite.
type SQLiteStore struct {
	db *sql.DB
}

// NewSQLiteStore creates a new SQLite store and initializes it with proper settings.
func NewSQLiteStore(db *sql.DB) (*SQLiteStore, error) {
	store := &SQLiteStore{db: db}
	if err := store.initialize(); err != nil {
		return nil, fmt.Errorf("failed to initialize SQLite store: %w", err)
	}
	return store, nil
}

// initialize sets up SQLite with proper pragmas for concurrent access and performance.
func (s *SQLiteStore) initialize() error {
	// Set busy timeout to handle concurrent access (5 seconds)
	// This prevents "database is locked" errors by making SQLite wait
	_, err := s.db.Exec("PRAGMA busy_timeout = 5000;")
	if err != nil {
		return fmt.Errorf("failed to set busy_timeout: %w", err)
	}

	// Use WAL mode for better concurrent read/write performance
	// WAL allows readers to not block writers and vice versa
	var journalMode string
	err = s.db.QueryRow("PRAGMA journal_mode = WAL;").Scan(&journalMode)
	if err != nil {
		return fmt.Errorf("failed to set journal_mode: %w", err)
	}
	if journalMode != "wal" {
		return fmt.Errorf("failed to enable WAL mode, got: %s", journalMode)
	}
	log.Println("SQLite WAL mode enabled")

	// Set synchronous mode to NORMAL for better performance with safety
	// NORMAL mode ensures data integrity while being faster than FULL
	_, err = s.db.Exec("PRAGMA synchronous = NORMAL;")
	if err != nil {
		return fmt.Errorf("failed to set synchronous: %w", err)
	}

	// Set cache size to 10000 pages (~40MB with 4KB pages)
	_, err = s.db.Exec("PRAGMA cache_size = 10000;")
	if err != nil {
		return fmt.Errorf("failed to set cache_size: %w", err)
	}

	// Enable foreign key constraints
	_, err = s.db.Exec("PRAGMA foreign_keys = ON;")
	if err != nil {
		return fmt.Errorf("failed to enable foreign_keys: %w", err)
	}

	return nil
}

// sqliteQuerier is a helper interface for query operations using database/sql
type sqliteQuerier interface {
	QueryContext(ctx context.Context, query string, args ...interface{}) (*sql.Rows, error)
	QueryRowContext(ctx context.Context, query string, args ...interface{}) *sql.Row
	ExecContext(ctx context.Context, query string, args ...interface{}) (sql.Result, error)
}

// sqliteTx wraps a sql.Tx to implement sqliteQuerier and Store
type sqliteTx struct {
	tx *sql.Tx
}

func (t *sqliteTx) QueryContext(ctx context.Context, query string, args ...interface{}) (*sql.Rows, error) {
	return t.tx.QueryContext(ctx, query, args...)
}

func (t *sqliteTx) QueryRowContext(ctx context.Context, query string, args ...interface{}) *sql.Row {
	return t.tx.QueryRowContext(ctx, query, args...)
}

func (t *sqliteTx) ExecContext(ctx context.Context, query string, args ...interface{}) (sql.Result, error) {
	return t.tx.ExecContext(ctx, query, args...)
}

// WithTx executes a function within a transaction.
func (s *SQLiteStore) WithTx(ctx context.Context, fn func(Store) error) error {
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}

	defer tx.Rollback()

	txStore := &sqliteTx{tx: tx}
	if err := fn(txStore); err != nil {
		return err
	}

	return tx.Commit()
}

// sqliteStore wrapper for passing sqliteQuerier to helper functions
type sqliteStoreWrapper struct {
	q sqliteQuerier
}

// Node operations
func (s *SQLiteStore) CreateNode(ctx context.Context, node *models.Node) error {
	return sqliteCreateNode(ctx, s.db, node)
}

func (s *SQLiteStore) GetNode(ctx context.Context, id uuid.UUID) (*models.Node, error) {
	return sqliteGetNode(ctx, s.db, id)
}

func (s *SQLiteStore) GetNodeByHostname(ctx context.Context, hostname string) (*models.Node, error) {
	return sqliteGetNodeByHostname(ctx, s.db, hostname)
}

func (s *SQLiteStore) UpdateNode(ctx context.Context, node *models.Node) error {
	return sqliteUpdateNode(ctx, s.db, node)
}

func (s *SQLiteStore) UpdateNodeHeartbeat(ctx context.Context, id uuid.UUID, status models.NodeState) error {
	return sqliteUpdateNodeHeartbeat(ctx, s.db, id, status)
}

func (s *SQLiteStore) ListNodes(ctx context.Context) ([]*models.Node, error) {
	return sqliteListNodes(ctx, s.db)
}

func (s *SQLiteStore) SetNodeMaintenance(ctx context.Context, id uuid.UUID, enabled bool) error {
	return sqliteSetNodeMaintenance(ctx, s.db, id, enabled)
}

// VM operations
func (s *SQLiteStore) CreateVM(ctx context.Context, vm *models.VirtualMachine) error {
	return sqliteCreateVM(ctx, s.db, vm)
}

func (s *SQLiteStore) GetVM(ctx context.Context, id uuid.UUID) (*models.VirtualMachine, error) {
	return sqliteGetVM(ctx, s.db, id)
}

func (s *SQLiteStore) GetVMByName(ctx context.Context, name string) (*models.VirtualMachine, error) {
	return sqliteGetVMByName(ctx, s.db, name)
}

func (s *SQLiteStore) UpdateVM(ctx context.Context, vm *models.VirtualMachine) error {
	return sqliteUpdateVM(ctx, s.db, vm)
}

func (s *SQLiteStore) UpdateVMActualState(ctx context.Context, id uuid.UUID, state models.VMActualState, lastError []byte) error {
	return sqliteUpdateVMActualState(ctx, s.db, id, state, lastError)
}

func (s *SQLiteStore) ListVMs(ctx context.Context) ([]*models.VirtualMachine, error) {
	return sqliteListVMs(ctx, s.db)
}

func (s *SQLiteStore) ListVMsByNode(ctx context.Context, nodeID uuid.UUID) ([]*models.VirtualMachine, error) {
	return sqliteListVMsByNode(ctx, s.db, nodeID)
}

func (s *SQLiteStore) ListVMsNeedingReconciliation(ctx context.Context) ([]*models.VirtualMachine, error) {
	return sqliteListVMsNeedingReconciliation(ctx, s.db)
}

func (s *SQLiteStore) DeleteVM(ctx context.Context, id uuid.UUID) error {
	return sqliteDeleteVM(ctx, s.db, id)
}

// Network operations
func (s *SQLiteStore) CreateNetwork(ctx context.Context, network *models.Network) error {
	return sqliteCreateNetwork(ctx, s.db, network)
}

func (s *SQLiteStore) GetNetwork(ctx context.Context, id uuid.UUID) (*models.Network, error) {
	return sqliteGetNetwork(ctx, s.db, id)
}

func (s *SQLiteStore) ListNetworks(ctx context.Context) ([]*models.Network, error) {
	return sqliteListNetworks(ctx, s.db)
}

func (s *SQLiteStore) DeleteNetwork(ctx context.Context, id uuid.UUID) error {
	return sqliteDeleteNetwork(ctx, s.db, id)
}

// Storage Pool operations
func (s *SQLiteStore) CreateStoragePool(ctx context.Context, pool *models.StoragePool) error {
	return sqliteCreateStoragePool(ctx, s.db, pool)
}

func (s *SQLiteStore) GetStoragePool(ctx context.Context, id uuid.UUID) (*models.StoragePool, error) {
	return sqliteGetStoragePool(ctx, s.db, id)
}

func (s *SQLiteStore) ListStoragePools(ctx context.Context) ([]*models.StoragePool, error) {
	return sqliteListStoragePools(ctx, s.db)
}

func (s *SQLiteStore) ListStoragePoolsByNode(ctx context.Context, nodeID uuid.UUID) ([]*models.StoragePool, error) {
	return sqliteListStoragePoolsByNode(ctx, s.db, nodeID)
}

func (s *SQLiteStore) DeleteStoragePool(ctx context.Context, id uuid.UUID) error {
	return sqliteDeleteStoragePool(ctx, s.db, id)
}

// Image operations
func (s *SQLiteStore) CreateImage(ctx context.Context, image *models.Image) error {
	return sqliteCreateImage(ctx, s.db, image)
}

func (s *SQLiteStore) GetImage(ctx context.Context, id uuid.UUID) (*models.Image, error) {
	return sqliteGetImage(ctx, s.db, id)
}

func (s *SQLiteStore) UpdateImage(ctx context.Context, image *models.Image) error {
	return sqliteUpdateImage(ctx, s.db, image)
}

func (s *SQLiteStore) ListImages(ctx context.Context) ([]*models.Image, error) {
	return sqliteListImages(ctx, s.db)
}

func (s *SQLiteStore) DeleteImage(ctx context.Context, id uuid.UUID) error {
	return sqliteDeleteImage(ctx, s.db, id)
}

// Volume operations
func (s *SQLiteStore) CreateVolume(ctx context.Context, volume *models.Volume) error {
	return sqliteCreateVolume(ctx, s.db, volume)
}

func (s *SQLiteStore) GetVolume(ctx context.Context, id uuid.UUID) (*models.Volume, error) {
	return sqliteGetVolume(ctx, s.db, id)
}

func (s *SQLiteStore) UpdateVolume(ctx context.Context, volume *models.Volume) error {
	return sqliteUpdateVolume(ctx, s.db, volume)
}

func (s *SQLiteStore) ListVolumesByVM(ctx context.Context, vmID uuid.UUID) ([]*models.Volume, error) {
	return sqliteListVolumesByVM(ctx, s.db, vmID)
}

// VM Network Attachment operations
func (s *SQLiteStore) CreateVMNetworkAttachment(ctx context.Context, attachment *models.VMNetworkAttachment) error {
	return sqliteCreateVMNetworkAttachment(ctx, s.db, attachment)
}

func (s *SQLiteStore) ListVMNetworkAttachments(ctx context.Context, vmID uuid.UUID) ([]*models.VMNetworkAttachment, error) {
	return sqliteListVMNetworkAttachments(ctx, s.db, vmID)
}

// API Token operations
func (s *SQLiteStore) CreateAPIToken(ctx context.Context, token *models.APIToken) error {
	return sqliteCreateAPIToken(ctx, s.db, token)
}

func (s *SQLiteStore) GetAPITokenByHash(ctx context.Context, hash string) (*models.APIToken, error) {
	return sqliteGetAPITokenByHash(ctx, s.db, hash)
}

func (s *SQLiteStore) ListAPITokens(ctx context.Context) ([]*models.APIToken, error) {
	return sqliteListAPITokens(ctx, s.db)
}

func (s *SQLiteStore) RevokeAPIToken(ctx context.Context, id uuid.UUID) error {
	return sqliteRevokeAPIToken(ctx, s.db, id)
}

// Operation operations
func (s *SQLiteStore) CreateOperation(ctx context.Context, op *models.Operation) error {
	return sqliteCreateOperation(ctx, s.db, op)
}

func (s *SQLiteStore) GetOperation(ctx context.Context, id uuid.UUID) (*models.Operation, error) {
	return sqliteGetOperation(ctx, s.db, id)
}

func (s *SQLiteStore) UpdateOperation(ctx context.Context, op *models.Operation) error {
	return sqliteUpdateOperation(ctx, s.db, op)
}

func (s *SQLiteStore) ListOperations(ctx context.Context, filters map[string]interface{}) ([]*models.Operation, error) {
	return sqliteListOperations(ctx, s.db, filters)
}

func (s *SQLiteStore) CreateOperationLog(ctx context.Context, log *models.OperationLog) error {
	return sqliteCreateOperationLog(ctx, s.db, log)
}

func (s *SQLiteStore) GetOperationLogs(ctx context.Context, operationID uuid.UUID) ([]*models.OperationLog, error) {
	return sqliteGetOperationLogs(ctx, s.db, operationID)
}

// Snapshot operations
func (s *SQLiteStore) CreateSnapshot(ctx context.Context, snapshot *models.Snapshot) error {
	return sqliteCreateSnapshot(ctx, s.db, snapshot)
}

func (s *SQLiteStore) GetSnapshot(ctx context.Context, id uuid.UUID) (*models.Snapshot, error) {
	return sqliteGetSnapshot(ctx, s.db, id)
}

func (s *SQLiteStore) UpdateSnapshot(ctx context.Context, snapshot *models.Snapshot) error {
	return sqliteUpdateSnapshot(ctx, s.db, snapshot)
}

func (s *SQLiteStore) ListSnapshotsByVM(ctx context.Context, vmID uuid.UUID) ([]*models.Snapshot, error) {
	return sqliteListSnapshotsByVM(ctx, s.db, vmID)
}

func (s *SQLiteStore) DeleteSnapshot(ctx context.Context, id uuid.UUID) error {
	return sqliteDeleteSnapshot(ctx, s.db, id)
}

func (s *SQLiteStore) GetQuota(ctx context.Context, userID string) (*models.ResourceQuota, error) {
	return sqliteGetQuota(ctx, s.db, userID)
}

func (s *SQLiteStore) SetQuota(ctx context.Context, quota *models.ResourceQuota) error {
	return sqliteSetQuota(ctx, s.db, quota)
}

func (s *SQLiteStore) GetUsage(ctx context.Context, userID string) (*models.ResourceUsage, error) {
	return sqliteGetUsage(ctx, s.db, userID)
}

func (s *SQLiteStore) UpdateUsage(ctx context.Context, userID string, delta models.ResourceUsage) error {
	return sqliteUpdateUsage(ctx, s.db, userID, delta)
}

func (s *SQLiteStore) EnsureQuota(ctx context.Context, userID string) error {
	return sqliteEnsureQuota(ctx, s.db, userID)
}

// txStore implements Store for transactions
func (t *sqliteTx) CreateNode(ctx context.Context, node *models.Node) error {
	return sqliteCreateNode(ctx, t.tx, node)
}

func (t *sqliteTx) GetNode(ctx context.Context, id uuid.UUID) (*models.Node, error) {
	return sqliteGetNode(ctx, t.tx, id)
}

func (t *sqliteTx) GetNodeByHostname(ctx context.Context, hostname string) (*models.Node, error) {
	return sqliteGetNodeByHostname(ctx, t.tx, hostname)
}

func (t *sqliteTx) UpdateNode(ctx context.Context, node *models.Node) error {
	return sqliteUpdateNode(ctx, t.tx, node)
}

func (t *sqliteTx) UpdateNodeHeartbeat(ctx context.Context, id uuid.UUID, status models.NodeState) error {
	return sqliteUpdateNodeHeartbeat(ctx, t.tx, id, status)
}

func (t *sqliteTx) ListNodes(ctx context.Context) ([]*models.Node, error) {
	return sqliteListNodes(ctx, t.tx)
}

func (t *sqliteTx) SetNodeMaintenance(ctx context.Context, id uuid.UUID, enabled bool) error {
	return sqliteSetNodeMaintenance(ctx, t.tx, id, enabled)
}

func (t *sqliteTx) CreateNetwork(ctx context.Context, network *models.Network) error {
	return sqliteCreateNetwork(ctx, t.tx, network)
}

func (t *sqliteTx) GetNetwork(ctx context.Context, id uuid.UUID) (*models.Network, error) {
	return sqliteGetNetwork(ctx, t.tx, id)
}

func (t *sqliteTx) ListNetworks(ctx context.Context) ([]*models.Network, error) {
	return sqliteListNetworks(ctx, t.tx)
}

func (t *sqliteTx) DeleteNetwork(ctx context.Context, id uuid.UUID) error {
	return sqliteDeleteNetwork(ctx, t.tx, id)
}

func (t *sqliteTx) CreateStoragePool(ctx context.Context, pool *models.StoragePool) error {
	return sqliteCreateStoragePool(ctx, t.tx, pool)
}

func (t *sqliteTx) GetStoragePool(ctx context.Context, id uuid.UUID) (*models.StoragePool, error) {
	return sqliteGetStoragePool(ctx, t.tx, id)
}

func (t *sqliteTx) ListStoragePools(ctx context.Context) ([]*models.StoragePool, error) {
	return sqliteListStoragePools(ctx, t.tx)
}

func (t *sqliteTx) ListStoragePoolsByNode(ctx context.Context, nodeID uuid.UUID) ([]*models.StoragePool, error) {
	return sqliteListStoragePoolsByNode(ctx, t.tx, nodeID)
}

func (t *sqliteTx) DeleteStoragePool(ctx context.Context, id uuid.UUID) error {
	return sqliteDeleteStoragePool(ctx, t.tx, id)
}

func (t *sqliteTx) CreateImage(ctx context.Context, image *models.Image) error {
	return sqliteCreateImage(ctx, t.tx, image)
}

func (t *sqliteTx) GetImage(ctx context.Context, id uuid.UUID) (*models.Image, error) {
	return sqliteGetImage(ctx, t.tx, id)
}

func (t *sqliteTx) UpdateImage(ctx context.Context, image *models.Image) error {
	return sqliteUpdateImage(ctx, t.tx, image)
}

func (t *sqliteTx) ListImages(ctx context.Context) ([]*models.Image, error) {
	return sqliteListImages(ctx, t.tx)
}

func (t *sqliteTx) DeleteImage(ctx context.Context, id uuid.UUID) error {
	return sqliteDeleteImage(ctx, t.tx, id)
}

func (t *sqliteTx) CreateVM(ctx context.Context, vm *models.VirtualMachine) error {
	return sqliteCreateVM(ctx, t.tx, vm)
}

func (t *sqliteTx) GetVM(ctx context.Context, id uuid.UUID) (*models.VirtualMachine, error) {
	return sqliteGetVM(ctx, t.tx, id)
}

func (t *sqliteTx) GetVMByName(ctx context.Context, name string) (*models.VirtualMachine, error) {
	return sqliteGetVMByName(ctx, t.tx, name)
}

func (t *sqliteTx) UpdateVM(ctx context.Context, vm *models.VirtualMachine) error {
	return sqliteUpdateVM(ctx, t.tx, vm)
}

func (t *sqliteTx) UpdateVMActualState(ctx context.Context, id uuid.UUID, state models.VMActualState, lastError []byte) error {
	return sqliteUpdateVMActualState(ctx, t.tx, id, state, lastError)
}

func (t *sqliteTx) ListVMs(ctx context.Context) ([]*models.VirtualMachine, error) {
	return sqliteListVMs(ctx, t.tx)
}

func (t *sqliteTx) ListVMsByNode(ctx context.Context, nodeID uuid.UUID) ([]*models.VirtualMachine, error) {
	return sqliteListVMsByNode(ctx, t.tx, nodeID)
}

func (t *sqliteTx) ListVMsNeedingReconciliation(ctx context.Context) ([]*models.VirtualMachine, error) {
	return sqliteListVMsNeedingReconciliation(ctx, t.tx)
}

func (t *sqliteTx) DeleteVM(ctx context.Context, id uuid.UUID) error {
	return sqliteDeleteVM(ctx, t.tx, id)
}

func (t *sqliteTx) CreateVolume(ctx context.Context, volume *models.Volume) error {
	return sqliteCreateVolume(ctx, t.tx, volume)
}

func (t *sqliteTx) GetVolume(ctx context.Context, id uuid.UUID) (*models.Volume, error) {
	return sqliteGetVolume(ctx, t.tx, id)
}

func (t *sqliteTx) UpdateVolume(ctx context.Context, volume *models.Volume) error {
	return sqliteUpdateVolume(ctx, t.tx, volume)
}

func (t *sqliteTx) ListVolumesByVM(ctx context.Context, vmID uuid.UUID) ([]*models.Volume, error) {
	return sqliteListVolumesByVM(ctx, t.tx, vmID)
}

func (t *sqliteTx) CreateVMNetworkAttachment(ctx context.Context, attachment *models.VMNetworkAttachment) error {
	return sqliteCreateVMNetworkAttachment(ctx, t.tx, attachment)
}

func (t *sqliteTx) ListVMNetworkAttachments(ctx context.Context, vmID uuid.UUID) ([]*models.VMNetworkAttachment, error) {
	return sqliteListVMNetworkAttachments(ctx, t.tx, vmID)
}

func (t *sqliteTx) CreateAPIToken(ctx context.Context, token *models.APIToken) error {
	return sqliteCreateAPIToken(ctx, t.tx, token)
}

func (t *sqliteTx) GetAPITokenByHash(ctx context.Context, hash string) (*models.APIToken, error) {
	return sqliteGetAPITokenByHash(ctx, t.tx, hash)
}

func (t *sqliteTx) ListAPITokens(ctx context.Context) ([]*models.APIToken, error) {
	return sqliteListAPITokens(ctx, t.tx)
}

func (t *sqliteTx) RevokeAPIToken(ctx context.Context, id uuid.UUID) error {
	return sqliteRevokeAPIToken(ctx, t.tx, id)
}

func (t *sqliteTx) CreateOperation(ctx context.Context, op *models.Operation) error {
	return sqliteCreateOperation(ctx, t.tx, op)
}

func (t *sqliteTx) GetOperation(ctx context.Context, id uuid.UUID) (*models.Operation, error) {
	return sqliteGetOperation(ctx, t.tx, id)
}

func (t *sqliteTx) UpdateOperation(ctx context.Context, op *models.Operation) error {
	return sqliteUpdateOperation(ctx, t.tx, op)
}

func (t *sqliteTx) ListOperations(ctx context.Context, filters map[string]interface{}) ([]*models.Operation, error) {
	return sqliteListOperations(ctx, t.tx, filters)
}

func (t *sqliteTx) CreateOperationLog(ctx context.Context, log *models.OperationLog) error {
	return sqliteCreateOperationLog(ctx, t.tx, log)
}

func (t *sqliteTx) GetOperationLogs(ctx context.Context, operationID uuid.UUID) ([]*models.OperationLog, error) {
	return sqliteGetOperationLogs(ctx, t.tx, operationID)
}

func (t *sqliteTx) WithTx(ctx context.Context, fn func(Store) error) error {
	return fn(t)
}

func (t *sqliteTx) GetQuota(ctx context.Context, userID string) (*models.ResourceQuota, error) {
	return sqliteGetQuota(ctx, t.tx, userID)
}

func (t *sqliteTx) SetQuota(ctx context.Context, quota *models.ResourceQuota) error {
	return sqliteSetQuota(ctx, t.tx, quota)
}

func (t *sqliteTx) GetUsage(ctx context.Context, userID string) (*models.ResourceUsage, error) {
	return sqliteGetUsage(ctx, t.tx, userID)
}

func (t *sqliteTx) UpdateUsage(ctx context.Context, userID string, delta models.ResourceUsage) error {
	return sqliteUpdateUsage(ctx, t.tx, userID, delta)
}

func (t *sqliteTx) EnsureQuota(ctx context.Context, userID string) error {
	return sqliteEnsureQuota(ctx, t.tx, userID)
}

// Snapshot operations for transactions
func (t *sqliteTx) CreateSnapshot(ctx context.Context, snapshot *models.Snapshot) error {
	return sqliteCreateSnapshot(ctx, t.tx, snapshot)
}

func (t *sqliteTx) GetSnapshot(ctx context.Context, id uuid.UUID) (*models.Snapshot, error) {
	return sqliteGetSnapshot(ctx, t.tx, id)
}

func (t *sqliteTx) UpdateSnapshot(ctx context.Context, snapshot *models.Snapshot) error {
	return sqliteUpdateSnapshot(ctx, t.tx, snapshot)
}

func (t *sqliteTx) ListSnapshotsByVM(ctx context.Context, vmID uuid.UUID) ([]*models.Snapshot, error) {
	return sqliteListSnapshotsByVM(ctx, t.tx, vmID)
}

func (t *sqliteTx) DeleteSnapshot(ctx context.Context, id uuid.UUID) error {
	return sqliteDeleteSnapshot(ctx, t.tx, id)
}

// ==================== NODE OPERATIONS ====================

func sqliteCreateNode(ctx context.Context, q sqliteQuerier, node *models.Node) error {
	query := `
		INSERT INTO nodes (
			id, hostname, management_ip, status, maintenance_mode,
			total_cpu_cores, total_ram_mb, allocatable_cpu_cores, allocatable_ram_mb,
			labels, capabilities, agent_version, hypervisor_version,
			last_heartbeat_at, created_at, updated_at
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
	`

	_, err := q.ExecContext(ctx, query,
		node.ID, node.Hostname, node.ManagementIP, node.Status, node.MaintenanceMode,
		node.TotalCPUcores, node.TotalRAMMB, node.AllocatableCPUCores, node.AllocatableRAMMB,
		node.Labels, node.Capabilities, node.AgentVersion, node.HypervisorVersion,
		node.LastHeartbeatAt, node.CreatedAt, node.UpdatedAt,
	)
	return err
}

func sqliteGetNode(ctx context.Context, q sqliteQuerier, id uuid.UUID) (*models.Node, error) {
	query := `
		SELECT id, hostname, management_ip, status, maintenance_mode,
			total_cpu_cores, total_ram_mb, allocatable_cpu_cores, allocatable_ram_mb,
			labels, capabilities, agent_version, hypervisor_version,
			last_heartbeat_at, created_at, updated_at
		FROM nodes WHERE id = ?
	`

	node := &models.Node{}
	var lastHeartbeatAtStr, createdAtStr, updatedAtStr sql.NullString
	err := q.QueryRowContext(ctx, query, id).Scan(
		&node.ID, &node.Hostname, &node.ManagementIP, &node.Status, &node.MaintenanceMode,
		&node.TotalCPUcores, &node.TotalRAMMB, &node.AllocatableCPUCores, &node.AllocatableRAMMB,
		&node.Labels, &node.Capabilities, &node.AgentVersion, &node.HypervisorVersion,
		&lastHeartbeatAtStr, &createdAtStr, &updatedAtStr,
	)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if lastHeartbeatAtStr.Valid {
		lastHeartbeatAt, _ := parseTime(lastHeartbeatAtStr.String)
		if !lastHeartbeatAt.IsZero() {
			node.LastHeartbeatAt = &lastHeartbeatAt
		}
	}
	if createdAtStr.Valid {
		node.CreatedAt, _ = parseTime(createdAtStr.String)
	}
	if updatedAtStr.Valid {
		node.UpdatedAt, _ = parseTime(updatedAtStr.String)
	}
	return node, nil
}

func sqliteGetNodeByHostname(ctx context.Context, q sqliteQuerier, hostname string) (*models.Node, error) {
	query := `
		SELECT id, hostname, management_ip, status, maintenance_mode,
			total_cpu_cores, total_ram_mb, allocatable_cpu_cores, allocatable_ram_mb,
			labels, capabilities, agent_version, hypervisor_version,
			last_heartbeat_at, created_at, updated_at
		FROM nodes WHERE hostname = ?
	`

	node := &models.Node{}
	var lastHeartbeatAtStr, createdAtStr, updatedAtStr sql.NullString
	err := q.QueryRowContext(ctx, query, hostname).Scan(
		&node.ID, &node.Hostname, &node.ManagementIP, &node.Status, &node.MaintenanceMode,
		&node.TotalCPUcores, &node.TotalRAMMB, &node.AllocatableCPUCores, &node.AllocatableRAMMB,
		&node.Labels, &node.Capabilities, &node.AgentVersion, &node.HypervisorVersion,
		&lastHeartbeatAtStr, &createdAtStr, &updatedAtStr,
	)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if lastHeartbeatAtStr.Valid {
		lastHeartbeatAt, _ := parseTime(lastHeartbeatAtStr.String)
		if !lastHeartbeatAt.IsZero() {
			node.LastHeartbeatAt = &lastHeartbeatAt
		}
	}
	if createdAtStr.Valid {
		node.CreatedAt, _ = parseTime(createdAtStr.String)
	}
	if updatedAtStr.Valid {
		node.UpdatedAt, _ = parseTime(updatedAtStr.String)
	}
	return node, nil
}

func sqliteUpdateNode(ctx context.Context, q sqliteQuerier, node *models.Node) error {
	query := `
		UPDATE nodes SET
			hostname = ?, management_ip = ?, status = ?, maintenance_mode = ?,
			total_cpu_cores = ?, total_ram_mb = ?, allocatable_cpu_cores = ?, allocatable_ram_mb = ?,
			labels = ?, capabilities = ?, agent_version = ?, hypervisor_version = ?,
			last_heartbeat_at = ?, updated_at = ?
		WHERE id = ?
	`

	node.UpdatedAt = time.Now()
	_, err := q.ExecContext(ctx, query,
		node.Hostname, node.ManagementIP, node.Status, node.MaintenanceMode,
		node.TotalCPUcores, node.TotalRAMMB, node.AllocatableCPUCores, node.AllocatableRAMMB,
		node.Labels, node.Capabilities, node.AgentVersion, node.HypervisorVersion,
		node.LastHeartbeatAt, node.UpdatedAt, node.ID,
	)
	return err
}

func sqliteUpdateNodeHeartbeat(ctx context.Context, q sqliteQuerier, id uuid.UUID, status models.NodeState) error {
	query := `
		UPDATE nodes SET
			status = ?,
			last_heartbeat_at = ?,
			updated_at = ?
		WHERE id = ?
	`

	now := time.Now()
	_, err := q.ExecContext(ctx, query, status, now, now, id)
	return err
}

func sqliteListNodes(ctx context.Context, q sqliteQuerier) ([]*models.Node, error) {
	query := `
		SELECT id, hostname, management_ip, status, maintenance_mode,
			total_cpu_cores, total_ram_mb, allocatable_cpu_cores, allocatable_ram_mb,
			labels, capabilities, agent_version, hypervisor_version,
			last_heartbeat_at, created_at, updated_at
		FROM nodes ORDER BY hostname
	`

	rows, err := q.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var nodes []*models.Node
	for rows.Next() {
		node := &models.Node{}
		var lastHeartbeatAtStr, createdAtStr, updatedAtStr sql.NullString
		err := rows.Scan(
			&node.ID, &node.Hostname, &node.ManagementIP, &node.Status, &node.MaintenanceMode,
			&node.TotalCPUcores, &node.TotalRAMMB, &node.AllocatableCPUCores, &node.AllocatableRAMMB,
			&node.Labels, &node.Capabilities, &node.AgentVersion, &node.HypervisorVersion,
			&lastHeartbeatAtStr, &createdAtStr, &updatedAtStr,
		)
		if err != nil {
			return nil, err
		}
		if lastHeartbeatAtStr.Valid {
			lastHeartbeatAt, _ := parseTime(lastHeartbeatAtStr.String)
			if !lastHeartbeatAt.IsZero() {
				node.LastHeartbeatAt = &lastHeartbeatAt
			}
		}
		if createdAtStr.Valid {
			node.CreatedAt, _ = parseTime(createdAtStr.String)
		}
		if updatedAtStr.Valid {
			node.UpdatedAt, _ = parseTime(updatedAtStr.String)
		}
		nodes = append(nodes, node)
	}

	return nodes, rows.Err()
}

func sqliteSetNodeMaintenance(ctx context.Context, q sqliteQuerier, id uuid.UUID, enabled bool) error {
	query := `
		UPDATE nodes SET
			maintenance_mode = ?,
			updated_at = ?
		WHERE id = ?
	`

	now := time.Now()
	_, err := q.ExecContext(ctx, query, enabled, now, id)
	return err
}

// ==================== VM OPERATIONS ====================

func sqliteCreateVM(ctx context.Context, q sqliteQuerier, vm *models.VirtualMachine) error {
	query := `
		INSERT INTO virtual_machines (
			id, name, node_id, created_by, desired_state, actual_state, placement_status,
			spec, last_error, created_at, updated_at
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
	`

	_, err := q.ExecContext(ctx, query,
		vm.ID, vm.Name, vm.NodeID, vm.CreatedBy, vm.DesiredState, vm.ActualState, vm.PlacementStatus,
		vm.Spec, vm.LastError, vm.CreatedAt, vm.UpdatedAt,
	)
	return err
}

func sqliteGetVM(ctx context.Context, q sqliteQuerier, id uuid.UUID) (*models.VirtualMachine, error) {
	query := `
		SELECT id, name, node_id, created_by, desired_state, actual_state, placement_status,
			spec, last_error, created_at, updated_at
		FROM virtual_machines WHERE id = ?
	`

	vm := &models.VirtualMachine{}
	var createdAtStr, updatedAtStr sql.NullString
	var lastErrorStr sql.NullString
	err := q.QueryRowContext(ctx, query, id).Scan(
		&vm.ID, &vm.Name, &vm.NodeID, &vm.CreatedBy, &vm.DesiredState, &vm.ActualState, &vm.PlacementStatus,
		&vm.Spec, &lastErrorStr, &createdAtStr, &updatedAtStr,
	)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if lastErrorStr.Valid {
		vm.LastError = json.RawMessage(lastErrorStr.String)
	}
	if createdAtStr.Valid {
		vm.CreatedAt, _ = parseTime(createdAtStr.String)
	}
	if updatedAtStr.Valid {
		vm.UpdatedAt, _ = parseTime(updatedAtStr.String)
	}
	return vm, nil
}

func sqliteGetVMByName(ctx context.Context, q sqliteQuerier, name string) (*models.VirtualMachine, error) {
	query := `
		SELECT id, name, node_id, created_by, desired_state, actual_state, placement_status,
			spec, last_error, created_at, updated_at
		FROM virtual_machines WHERE name = ?
	`

	vm := &models.VirtualMachine{}
	var createdAtStr, updatedAtStr sql.NullString
	var lastErrorStr sql.NullString
	var specStr sql.NullString
	err := q.QueryRowContext(ctx, query, name).Scan(
		&vm.ID, &vm.Name, &vm.NodeID, &vm.CreatedBy, &vm.DesiredState, &vm.ActualState, &vm.PlacementStatus,
		&specStr, &lastErrorStr, &createdAtStr, &updatedAtStr,
	)
	if specStr.Valid {
		vm.Spec = json.RawMessage(specStr.String)
	}
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if lastErrorStr.Valid {
		vm.LastError = json.RawMessage(lastErrorStr.String)
	}
	if createdAtStr.Valid {
		vm.CreatedAt, _ = parseTime(createdAtStr.String)
	}
	if updatedAtStr.Valid {
		vm.UpdatedAt, _ = parseTime(updatedAtStr.String)
	}
	return vm, nil
}

func sqliteUpdateVM(ctx context.Context, q sqliteQuerier, vm *models.VirtualMachine) error {
	query := `
		UPDATE virtual_machines SET
			name = ?, node_id = ?, created_by = ?, desired_state = ?, actual_state = ?,
			placement_status = ?, spec = ?, last_error = ?, updated_at = ?
		WHERE id = ?
	`

	vm.UpdatedAt = time.Now()
	_, err := q.ExecContext(ctx, query,
		vm.Name, vm.NodeID, vm.CreatedBy, vm.DesiredState, vm.ActualState,
		vm.PlacementStatus, vm.Spec, vm.LastError, vm.UpdatedAt, vm.ID,
	)
	return err
}

func sqliteUpdateVMActualState(ctx context.Context, q sqliteQuerier, id uuid.UUID, state models.VMActualState, lastError []byte) error {
	query := `
		UPDATE virtual_machines SET
			actual_state = ?,
			last_error = ?,
			updated_at = ?
		WHERE id = ?
	`

	now := time.Now()
	_, err := q.ExecContext(ctx, query, state, lastError, now, id)
	return err
}

func sqliteListVMs(ctx context.Context, q sqliteQuerier) ([]*models.VirtualMachine, error) {
	query := `
		SELECT id, name, node_id, created_by, desired_state, actual_state, placement_status,
			spec, last_error, created_at, updated_at
		FROM virtual_machines ORDER BY name
	`

	rows, err := q.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var vms []*models.VirtualMachine
	for rows.Next() {
		vm := &models.VirtualMachine{}
		var createdAtStr, updatedAtStr sql.NullString
		var lastErrorStr sql.NullString
		var specStr sql.NullString
		err := rows.Scan(
			&vm.ID, &vm.Name, &vm.NodeID, &vm.CreatedBy, &vm.DesiredState, &vm.ActualState, &vm.PlacementStatus,
			&specStr, &lastErrorStr, &createdAtStr, &updatedAtStr,
		)
		if specStr.Valid {
			vm.Spec = json.RawMessage(specStr.String)
		}
		if err != nil {
			return nil, err
		}
		if lastErrorStr.Valid {
			vm.LastError = json.RawMessage(lastErrorStr.String)
		}
		if createdAtStr.Valid {
			vm.CreatedAt, _ = parseTime(createdAtStr.String)
		}
		if updatedAtStr.Valid {
			vm.UpdatedAt, _ = parseTime(updatedAtStr.String)
		}
		vms = append(vms, vm)
	}

	return vms, rows.Err()
}

func sqliteListVMsByNode(ctx context.Context, q sqliteQuerier, nodeID uuid.UUID) ([]*models.VirtualMachine, error) {
	query := `
		SELECT id, name, node_id, created_by, desired_state, actual_state, placement_status,
			spec, last_error, created_at, updated_at
		FROM virtual_machines WHERE node_id = ? ORDER BY name
	`

	rows, err := q.QueryContext(ctx, query, nodeID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var vms []*models.VirtualMachine
	for rows.Next() {
		vm := &models.VirtualMachine{}
		var createdAtStr, updatedAtStr sql.NullString
		var lastErrorStr sql.NullString
		var specStr sql.NullString
		err := rows.Scan(
			&vm.ID, &vm.Name, &vm.NodeID, &vm.CreatedBy, &vm.DesiredState, &vm.ActualState, &vm.PlacementStatus,
			&specStr, &lastErrorStr, &createdAtStr, &updatedAtStr,
		)
		if specStr.Valid {
			vm.Spec = json.RawMessage(specStr.String)
		}
		if err != nil {
			return nil, err
		}
		if lastErrorStr.Valid {
			vm.LastError = json.RawMessage(lastErrorStr.String)
		}
		if createdAtStr.Valid {
			vm.CreatedAt, _ = parseTime(createdAtStr.String)
		}
		if updatedAtStr.Valid {
			vm.UpdatedAt, _ = parseTime(updatedAtStr.String)
		}
		vms = append(vms, vm)
	}

	return vms, rows.Err()
}

func sqliteListVMsNeedingReconciliation(ctx context.Context, q sqliteQuerier) ([]*models.VirtualMachine, error) {
	query := `
		SELECT id, name, node_id, created_by, desired_state, actual_state, placement_status,
			spec, last_error, created_at, updated_at
		FROM virtual_machines
		WHERE (
			(desired_state = 'running' AND actual_state != 'running') OR
			(desired_state = 'stopped' AND actual_state NOT IN ('stopped', 'provisioning')) OR
			(desired_state = 'deleted' AND actual_state != 'deleting')
		)
		AND placement_status != 'failed'
		ORDER BY updated_at
	`

	rows, err := q.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var vms []*models.VirtualMachine
	for rows.Next() {
		vm := &models.VirtualMachine{}
		var createdAtStr, updatedAtStr sql.NullString
		var lastErrorStr sql.NullString
		var specStr sql.NullString
		err := rows.Scan(
			&vm.ID, &vm.Name, &vm.NodeID, &vm.CreatedBy, &vm.DesiredState, &vm.ActualState, &vm.PlacementStatus,
			&specStr, &lastErrorStr, &createdAtStr, &updatedAtStr,
		)
		if specStr.Valid {
			vm.Spec = json.RawMessage(specStr.String)
		}
		if err != nil {
			return nil, err
		}
		if lastErrorStr.Valid {
			vm.LastError = json.RawMessage(lastErrorStr.String)
		}
		if createdAtStr.Valid {
			vm.CreatedAt, _ = parseTime(createdAtStr.String)
		}
		if updatedAtStr.Valid {
			vm.UpdatedAt, _ = parseTime(updatedAtStr.String)
		}
		vms = append(vms, vm)
	}

	return vms, rows.Err()
}

func sqliteDeleteVM(ctx context.Context, q sqliteQuerier, id uuid.UUID) error {
	query := `DELETE FROM virtual_machines WHERE id = ?`
	_, err := q.ExecContext(ctx, query, id)
	return err
}

// ==================== NETWORK OPERATIONS ====================

func sqliteCreateNetwork(ctx context.Context, q sqliteQuerier, network *models.Network) error {
	query := `INSERT INTO networks (id, name, bridge_name, cidr, gateway_ip, dns_servers, mtu, mode, status, created_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
	_, err := q.ExecContext(ctx, query, network.ID, network.Name, network.BridgeName, network.CIDR,
		network.GatewayIP, network.DNSServers, network.MTU, network.Mode, network.Status, network.CreatedAt)
	return err
}

func sqliteGetNetwork(ctx context.Context, q sqliteQuerier, id uuid.UUID) (*models.Network, error) {
	query := `SELECT id, name, bridge_name, cidr, gateway_ip, dns_servers, mtu, mode, status, created_at
		FROM networks WHERE id = ?`
	n := &models.Network{}
	var createdAtStr sql.NullString
	err := q.QueryRowContext(ctx, query, id).Scan(&n.ID, &n.Name, &n.BridgeName, &n.CIDR, &n.GatewayIP,
		&n.DNSServers, &n.MTU, &n.Mode, &n.Status, &createdAtStr)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if createdAtStr.Valid {
		n.CreatedAt, _ = parseTime(createdAtStr.String)
	}
	return n, nil
}

func sqliteListNetworks(ctx context.Context, q sqliteQuerier) ([]*models.Network, error) {
	query := `SELECT id, name, bridge_name, cidr, gateway_ip, dns_servers, mtu, mode, status, created_at
		FROM networks ORDER BY name`
	rows, err := q.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var networks []*models.Network
	for rows.Next() {
		n := &models.Network{}
		var createdAtStr sql.NullString
		if err := rows.Scan(&n.ID, &n.Name, &n.BridgeName, &n.CIDR, &n.GatewayIP,
			&n.DNSServers, &n.MTU, &n.Mode, &n.Status, &createdAtStr); err != nil {
			return nil, err
		}
		if createdAtStr.Valid {
			n.CreatedAt, _ = parseTime(createdAtStr.String)
		}
		networks = append(networks, n)
	}
	return networks, rows.Err()
}

func sqliteDeleteNetwork(ctx context.Context, q sqliteQuerier, id uuid.UUID) error {
	query := `DELETE FROM networks WHERE id = ?`
	_, err := q.ExecContext(ctx, query, id)
	return err
}

// ==================== STORAGE POOL OPERATIONS ====================

func sqliteCreateStoragePool(ctx context.Context, q sqliteQuerier, pool *models.StoragePool) error {
	query := `INSERT INTO storage_pools (id, node_id, name, pool_type, path_or_export, capacity_bytes,
		allocatable_bytes, status, supports_online_resize, supports_clone, supports_snapshot, created_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
	_, err := q.ExecContext(ctx, query, pool.ID, pool.NodeID, pool.Name, pool.PoolType, pool.PathOrExport,
		pool.CapacityBytes, pool.AllocatableBytes, pool.Status, pool.SupportsOnlineResize,
		pool.SupportsClone, pool.SupportsSnapshot, pool.CreatedAt)
	return err
}

func sqliteGetStoragePool(ctx context.Context, q sqliteQuerier, id uuid.UUID) (*models.StoragePool, error) {
	query := `SELECT id, node_id, name, pool_type, path_or_export, capacity_bytes,
		allocatable_bytes, status, supports_online_resize, supports_clone, supports_snapshot, created_at
		FROM storage_pools WHERE id = ?`
	p := &models.StoragePool{}
	var createdAtStr sql.NullString
	err := q.QueryRowContext(ctx, query, id).Scan(&p.ID, &p.NodeID, &p.Name, &p.PoolType, &p.PathOrExport,
		&p.CapacityBytes, &p.AllocatableBytes, &p.Status, &p.SupportsOnlineResize,
		&p.SupportsClone, &p.SupportsSnapshot, &createdAtStr)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if createdAtStr.Valid {
		p.CreatedAt, _ = parseTime(createdAtStr.String)
	}
	return p, nil
}

func sqliteListStoragePools(ctx context.Context, q sqliteQuerier) ([]*models.StoragePool, error) {
	query := `SELECT id, node_id, name, pool_type, path_or_export, capacity_bytes,
		allocatable_bytes, status, supports_online_resize, supports_clone, supports_snapshot, created_at
		FROM storage_pools ORDER BY name`
	rows, err := q.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var pools []*models.StoragePool
	for rows.Next() {
		p := &models.StoragePool{}
		var createdAtStr sql.NullString
		if err := rows.Scan(&p.ID, &p.NodeID, &p.Name, &p.PoolType, &p.PathOrExport,
			&p.CapacityBytes, &p.AllocatableBytes, &p.Status, &p.SupportsOnlineResize,
			&p.SupportsClone, &p.SupportsSnapshot, &createdAtStr); err != nil {
			return nil, err
		}
		if createdAtStr.Valid {
			p.CreatedAt, _ = parseTime(createdAtStr.String)
		}
		pools = append(pools, p)
	}
	return pools, rows.Err()
}

func sqliteListStoragePoolsByNode(ctx context.Context, q sqliteQuerier, nodeID uuid.UUID) ([]*models.StoragePool, error) {
	query := `SELECT id, node_id, name, pool_type, path_or_export, capacity_bytes,
		allocatable_bytes, status, supports_online_resize, supports_clone, supports_snapshot, created_at
		FROM storage_pools WHERE node_id = ? ORDER BY name`
	rows, err := q.QueryContext(ctx, query, nodeID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var pools []*models.StoragePool
	for rows.Next() {
		p := &models.StoragePool{}
		var createdAtStr sql.NullString
		if err := rows.Scan(&p.ID, &p.NodeID, &p.Name, &p.PoolType, &p.PathOrExport,
			&p.CapacityBytes, &p.AllocatableBytes, &p.Status, &p.SupportsOnlineResize,
			&p.SupportsClone, &p.SupportsSnapshot, &createdAtStr); err != nil {
			return nil, err
		}
		if createdAtStr.Valid {
			p.CreatedAt, _ = parseTime(createdAtStr.String)
		}
		pools = append(pools, p)
	}
	return pools, rows.Err()
}

func sqliteDeleteStoragePool(ctx context.Context, q sqliteQuerier, id uuid.UUID) error {
	query := `DELETE FROM storage_pools WHERE id = ?`
	_, err := q.ExecContext(ctx, query, id)
	return err
}

// ==================== IMAGE OPERATIONS ====================

func sqliteCreateImage(ctx context.Context, q sqliteQuerier, image *models.Image) error {
	query := `INSERT INTO images (id, name, os_family, source_format, normalized_format, architecture,
		cloud_init_supported, default_username, checksum, status, metadata, created_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
	_, err := q.ExecContext(ctx, query, image.ID, image.Name, image.OSFamily, image.SourceFormat,
		image.NormalizedFormat, image.Architecture, image.CloudInitSupported, image.DefaultUsername,
		image.Checksum, image.Status, image.Metadata, image.CreatedAt)
	return err
}

func sqliteGetImage(ctx context.Context, q sqliteQuerier, id uuid.UUID) (*models.Image, error) {
	query := `SELECT id, name, os_family, source_format, normalized_format, architecture,
		cloud_init_supported, default_username, checksum, status, size_bytes, metadata, created_at, imported_at
		FROM images WHERE id = ?`
	i := &models.Image{}
	var createdAtStr, importedAtStr sql.NullString
	var sizeBytesNull sql.NullInt64
	err := q.QueryRowContext(ctx, query, id).Scan(&i.ID, &i.Name, &i.OSFamily, &i.SourceFormat,
		&i.NormalizedFormat, &i.Architecture, &i.CloudInitSupported, &i.DefaultUsername,
		&i.Checksum, &i.Status, &sizeBytesNull, &i.Metadata, &createdAtStr, &importedAtStr)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if sizeBytesNull.Valid {
		i.SizeBytes = uint64(sizeBytesNull.Int64)
	}
	if createdAtStr.Valid {
		i.CreatedAt, _ = parseTime(createdAtStr.String)
	}
	if importedAtStr.Valid {
		importedAt, _ := parseTime(importedAtStr.String)
		i.ImportedAt = &importedAt
	}
	return i, nil
}

func sqliteUpdateImage(ctx context.Context, q sqliteQuerier, image *models.Image) error {
	query := `UPDATE images SET name = ?, os_family = ?, source_format = ?, normalized_format = ?,
		architecture = ?, cloud_init_supported = ?, default_username = ?, checksum = ?,
		status = ?, size_bytes = ?, metadata = ?, imported_at = ? WHERE id = ?`
	
	var importedAt interface{}
	if image.ImportedAt != nil {
		importedAt = image.ImportedAt.Format(time.RFC3339Nano)
	} else {
		importedAt = nil
	}
	
	_, err := q.ExecContext(ctx, query, image.Name, image.OSFamily, image.SourceFormat,
		image.NormalizedFormat, image.Architecture, image.CloudInitSupported, image.DefaultUsername,
		image.Checksum, image.Status, image.SizeBytes, image.Metadata, importedAt, image.ID)
	return err
}

func sqliteListImages(ctx context.Context, q sqliteQuerier) ([]*models.Image, error) {
	query := `SELECT id, name, os_family, source_format, normalized_format, architecture,
		cloud_init_supported, default_username, checksum, status, size_bytes, metadata, created_at, imported_at
		FROM images ORDER BY name`
	rows, err := q.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var images []*models.Image
	for rows.Next() {
		i := &models.Image{}
		var createdAtStr, importedAtStr sql.NullString
		var sizeBytesNull sql.NullInt64
		if err := rows.Scan(&i.ID, &i.Name, &i.OSFamily, &i.SourceFormat,
			&i.NormalizedFormat, &i.Architecture, &i.CloudInitSupported, &i.DefaultUsername,
			&i.Checksum, &i.Status, &sizeBytesNull, &i.Metadata, &createdAtStr, &importedAtStr); err != nil {
			return nil, err
		}
		if sizeBytesNull.Valid {
			i.SizeBytes = uint64(sizeBytesNull.Int64)
		}
		if createdAtStr.Valid {
			i.CreatedAt, _ = parseTime(createdAtStr.String)
		}
		if importedAtStr.Valid {
			importedAt, _ := parseTime(importedAtStr.String)
			i.ImportedAt = &importedAt
		}
		images = append(images, i)
	}
	return images, rows.Err()
}

func sqliteDeleteImage(ctx context.Context, q sqliteQuerier, id uuid.UUID) error {
	query := `DELETE FROM images WHERE id = ?`
	_, err := q.ExecContext(ctx, query, id)
	return err
}

// ==================== VOLUME OPERATIONS ====================

func sqliteCreateVolume(ctx context.Context, q sqliteQuerier, volume *models.Volume) error {
	// Ensure metadata is not nil
	metadata := volume.Metadata
	if len(metadata) == 0 {
		metadata = json.RawMessage("{}")
	}
	query := `INSERT INTO volumes (id, vm_id, pool_id, backing_image_id, format, size_bytes, path,
		attachment_state, resize_state, metadata, created_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
	_, err := q.ExecContext(ctx, query, volume.ID, volume.VMID, volume.PoolID, volume.BackingImageID,
		volume.Format, volume.SizeBytes, volume.Path, volume.AttachmentState, volume.ResizeState,
		metadata, volume.CreatedAt)
	return err
}

func sqliteGetVolume(ctx context.Context, q sqliteQuerier, id uuid.UUID) (*models.Volume, error) {
	query := `SELECT id, vm_id, pool_id, backing_image_id, format, size_bytes, path,
		attachment_state, resize_state, metadata, created_at FROM volumes WHERE id = ?`
	v := &models.Volume{}
	var createdAtStr sql.NullString
	err := q.QueryRowContext(ctx, query, id).Scan(&v.ID, &v.VMID, &v.PoolID, &v.BackingImageID,
		&v.Format, &v.SizeBytes, &v.Path, &v.AttachmentState, &v.ResizeState, &v.Metadata, &createdAtStr)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if createdAtStr.Valid {
		v.CreatedAt, _ = parseTime(createdAtStr.String)
	}
	return v, nil
}

func sqliteUpdateVolume(ctx context.Context, q sqliteQuerier, volume *models.Volume) error {
	query := `UPDATE volumes SET vm_id = ?, pool_id = ?, backing_image_id = ?, format = ?,
		size_bytes = ?, path = ?, attachment_state = ?, resize_state = ?, metadata = ?
		WHERE id = ?`
	_, err := q.ExecContext(ctx, query, volume.VMID, volume.PoolID, volume.BackingImageID,
		volume.Format, volume.SizeBytes, volume.Path, volume.AttachmentState, volume.ResizeState, volume.Metadata, volume.ID)
	return err
}

func sqliteListVolumesByVM(ctx context.Context, q sqliteQuerier, vmID uuid.UUID) ([]*models.Volume, error) {
	query := `SELECT id, vm_id, pool_id, backing_image_id, format, size_bytes, path,
		attachment_state, resize_state, metadata, created_at FROM volumes WHERE vm_id = ?`
	rows, err := q.QueryContext(ctx, query, vmID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var volumes []*models.Volume
	for rows.Next() {
		v := &models.Volume{}
		var createdAtStr sql.NullString
		if err := rows.Scan(&v.ID, &v.VMID, &v.PoolID, &v.BackingImageID,
			&v.Format, &v.SizeBytes, &v.Path, &v.AttachmentState, &v.ResizeState, &v.Metadata, &createdAtStr); err != nil {
			return nil, err
		}
		if createdAtStr.Valid {
			v.CreatedAt, _ = parseTime(createdAtStr.String)
		}
		volumes = append(volumes, v)
	}
	return volumes, rows.Err()
}

// ==================== VM NETWORK ATTACHMENT OPERATIONS ====================

func sqliteCreateVMNetworkAttachment(ctx context.Context, q sqliteQuerier, a *models.VMNetworkAttachment) error {
	query := `INSERT INTO vm_network_attachments (id, vm_id, network_id, mac_address, ip_address, nic_index, created_at)
		VALUES (?, ?, ?, ?, ?, ?, ?)`
	_, err := q.ExecContext(ctx, query, a.ID, a.VMID, a.NetworkID, a.MACAddress, a.IPAddress, a.NICIndex, a.CreatedAt)
	return err
}

func sqliteListVMNetworkAttachments(ctx context.Context, q sqliteQuerier, vmID uuid.UUID) ([]*models.VMNetworkAttachment, error) {
	query := `SELECT id, vm_id, network_id, mac_address, ip_address, nic_index, created_at
		FROM vm_network_attachments WHERE vm_id = ? ORDER BY nic_index`
	rows, err := q.QueryContext(ctx, query, vmID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var attachments []*models.VMNetworkAttachment
	for rows.Next() {
		a := &models.VMNetworkAttachment{}
		var createdAtStr sql.NullString
		if err := rows.Scan(&a.ID, &a.VMID, &a.NetworkID, &a.MACAddress, &a.IPAddress, &a.NICIndex, &createdAtStr); err != nil {
			return nil, err
		}
		if createdAtStr.Valid {
			a.CreatedAt, _ = parseTime(createdAtStr.String)
		}
		attachments = append(attachments, a)
	}
	return attachments, rows.Err()
}

// ==================== API TOKEN OPERATIONS ====================

func sqliteCreateAPIToken(ctx context.Context, q sqliteQuerier, token *models.APIToken) error {
	query := `INSERT INTO api_tokens (id, name, token_hash, role_id, expires_at, revoked_at, created_at)
		VALUES (?, ?, ?, ?, ?, ?, ?)`
	
	// Handle RoleID pointer - convert to string or nil
	var roleID interface{}
	if token.RoleID != nil {
		roleID = token.RoleID.String()
	} else {
		roleID = nil
	}
	
	// Convert timestamps to RFC3339 strings for SQLite
	var expiresAt, revokedAt, createdAt interface{}
	if token.ExpiresAt != nil {
		expiresAt = token.ExpiresAt.Format(time.RFC3339Nano)
	}
	if token.RevokedAt != nil {
		revokedAt = token.RevokedAt.Format(time.RFC3339Nano)
	}
	if !token.CreatedAt.IsZero() {
		createdAt = token.CreatedAt.Format(time.RFC3339Nano)
	}
	
	_, err := q.ExecContext(ctx, query, token.ID.String(), token.Name, token.TokenHash, roleID,
		expiresAt, revokedAt, createdAt)
	return err
}

func sqliteGetAPITokenByHash(ctx context.Context, q sqliteQuerier, hash string) (*models.APIToken, error) {
	query := `SELECT id, name, token_hash, role_id, expires_at, revoked_at, created_at
		FROM api_tokens WHERE token_hash = ?`
	t := &models.APIToken{}
	var roleIDStr sql.NullString
	var expiresAtStr, revokedAtStr, createdAtStr sql.NullString
	err := q.QueryRowContext(ctx, query, hash).Scan(&t.ID, &t.Name, &t.TokenHash, &roleIDStr,
		&expiresAtStr, &revokedAtStr, &createdAtStr)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	// Convert role_id string back to UUID pointer
	if roleIDStr.Valid {
		roleID, err := uuid.Parse(roleIDStr.String)
		if err != nil {
			return nil, err
		}
		t.RoleID = &roleID
	}
	// Parse timestamps using parseTime helper
	if expiresAtStr.Valid {
		pt, _ := parseTime(expiresAtStr.String)
		if !pt.IsZero() {
			t.ExpiresAt = &pt
		}
	}
	if revokedAtStr.Valid {
		pt, _ := parseTime(revokedAtStr.String)
		if !pt.IsZero() {
			t.RevokedAt = &pt
		}
	}
	if createdAtStr.Valid {
		t.CreatedAt, _ = parseTime(createdAtStr.String)
	}
	return t, nil
}

func sqliteListAPITokens(ctx context.Context, q sqliteQuerier) ([]*models.APIToken, error) {
	query := `SELECT id, name, token_hash, role_id, expires_at, revoked_at, created_at
		FROM api_tokens ORDER BY created_at DESC`
	rows, err := q.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var tokens []*models.APIToken
	for rows.Next() {
		t := &models.APIToken{}
		var roleIDStr sql.NullString
		var expiresAtStr, revokedAtStr, createdAtStr sql.NullString
		if err := rows.Scan(&t.ID, &t.Name, &t.TokenHash, &roleIDStr,
			&expiresAtStr, &revokedAtStr, &createdAtStr); err != nil {
			return nil, err
		}
		// Convert role_id string back to UUID pointer
		if roleIDStr.Valid {
			roleID, err := uuid.Parse(roleIDStr.String)
			if err != nil {
				return nil, err
			}
			t.RoleID = &roleID
		}
		// Parse timestamps
		if expiresAtStr.Valid {
			pt, _ := parseTime(expiresAtStr.String)
			if !pt.IsZero() {
				t.ExpiresAt = &pt
			}
		}
		if revokedAtStr.Valid {
			pt, _ := parseTime(revokedAtStr.String)
			if !pt.IsZero() {
				t.RevokedAt = &pt
			}
		}
		if createdAtStr.Valid {
			t.CreatedAt, _ = parseTime(createdAtStr.String)
		}
		tokens = append(tokens, t)
	}
	return tokens, rows.Err()
}

func sqliteRevokeAPIToken(ctx context.Context, q sqliteQuerier, id uuid.UUID) error {
	query := `UPDATE api_tokens SET revoked_at = ? WHERE id = ?`
	_, err := q.ExecContext(ctx, query, time.Now(), id)
	return err
}

// ==================== OPERATION OPERATIONS ====================

func sqliteCreateOperation(ctx context.Context, q sqliteQuerier, op *models.Operation) error {
	query := `
		INSERT INTO operations (
			id, operation_type, category, status, status_message,
			resource_type, resource_id, actor_type, actor_id, node_id,
			request_payload, result_payload, error_payload,
			started_at, finished_at, created_at, updated_at
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
	`

	_, err := q.ExecContext(ctx, query,
		op.ID, op.OperationType, op.Category, op.Status, op.StatusMessage,
		op.ResourceType, op.ResourceID, op.ActorType, op.ActorID, op.NodeID,
		op.RequestPayload, op.ResultPayload, op.ErrorPayload,
		op.StartedAt, op.FinishedAt, op.CreatedAt, op.UpdatedAt,
	)
	return err
}

func sqliteGetOperation(ctx context.Context, q sqliteQuerier, id uuid.UUID) (*models.Operation, error) {
	query := `
		SELECT 
			id, operation_type, category, status, status_message,
			resource_type, resource_id, actor_type, actor_id, node_id,
			request_payload, result_payload, error_payload,
			started_at, finished_at, created_at, updated_at
		FROM operations WHERE id = ?
	`

	op := &models.Operation{}
	var startedAtStr, finishedAtStr, createdAtStr, updatedAtStr sql.NullString
	err := q.QueryRowContext(ctx, query, id).Scan(
		&op.ID, &op.OperationType, &op.Category, &op.Status, &op.StatusMessage,
		&op.ResourceType, &op.ResourceID, &op.ActorType, &op.ActorID, &op.NodeID,
		&op.RequestPayload, &op.ResultPayload, &op.ErrorPayload,
		&startedAtStr, &finishedAtStr, &createdAtStr, &updatedAtStr,
	)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if startedAtStr.Valid {
		startedAt, _ := parseTime(startedAtStr.String)
		op.StartedAt = &startedAt
	}
	if finishedAtStr.Valid {
		finishedAt, _ := parseTime(finishedAtStr.String)
		op.FinishedAt = &finishedAt
	}
	if createdAtStr.Valid {
		op.CreatedAt, _ = parseTime(createdAtStr.String)
	}
	if updatedAtStr.Valid {
		op.UpdatedAt, _ = parseTime(updatedAtStr.String)
	}
	return op, nil
}

func sqliteUpdateOperation(ctx context.Context, q sqliteQuerier, op *models.Operation) error {
	query := `
		UPDATE operations SET
			operation_type = ?, category = ?, status = ?, status_message = ?,
			resource_type = ?, resource_id = ?, actor_type = ?, actor_id = ?, node_id = ?,
			request_payload = ?, result_payload = ?, error_payload = ?,
			started_at = ?, finished_at = ?, updated_at = ?
		WHERE id = ?
	`

	op.UpdatedAt = time.Now()
	_, err := q.ExecContext(ctx, query,
		op.OperationType, op.Category, op.Status, op.StatusMessage,
		op.ResourceType, op.ResourceID, op.ActorType, op.ActorID, op.NodeID,
		op.RequestPayload, op.ResultPayload, op.ErrorPayload,
		op.StartedAt, op.FinishedAt, op.UpdatedAt, op.ID,
	)
	return err
}

func sqliteListOperations(ctx context.Context, q sqliteQuerier, filters map[string]interface{}) ([]*models.Operation, error) {
	whereClause := ""
	var args []interface{}
	argIdx := 0

	if filters != nil {
		var conditions []string

		if resourceType, ok := filters["resource_type"].(string); ok && resourceType != "" {
			conditions = append(conditions, "resource_type = ?")
			args = append(args, resourceType)
			argIdx++
		}

		if resourceID, ok := filters["resource_id"].(uuid.UUID); ok {
			conditions = append(conditions, "resource_id = ?")
			args = append(args, resourceID)
			argIdx++
		}

		if status, ok := filters["status"].(string); ok && status != "" {
			conditions = append(conditions, "status = ?")
			args = append(args, status)
			argIdx++
		}

		if opType, ok := filters["operation_type"].(string); ok && opType != "" {
			conditions = append(conditions, "operation_type = ?")
			args = append(args, opType)
			argIdx++
		}

		if nodeID, ok := filters["node_id"].(uuid.UUID); ok {
			conditions = append(conditions, "node_id = ?")
			args = append(args, nodeID)
			argIdx++
		}

		if len(conditions) > 0 {
			whereClause = "WHERE " + strings.Join(conditions, " AND ")
		}
	}

	query := fmt.Sprintf(`
		SELECT 
			id, operation_type, category, status, status_message,
			resource_type, resource_id, actor_type, actor_id, node_id,
			request_payload, result_payload, error_payload,
			started_at, finished_at, created_at, updated_at
		FROM operations
		%s
		ORDER BY created_at DESC
	`, whereClause)

	rows, err := q.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var ops []*models.Operation
	for rows.Next() {
		op := &models.Operation{}
		var startedAtStr, finishedAtStr, createdAtStr, updatedAtStr sql.NullString
		err := rows.Scan(
			&op.ID, &op.OperationType, &op.Category, &op.Status, &op.StatusMessage,
			&op.ResourceType, &op.ResourceID, &op.ActorType, &op.ActorID, &op.NodeID,
			&op.RequestPayload, &op.ResultPayload, &op.ErrorPayload,
			&startedAtStr, &finishedAtStr, &createdAtStr, &updatedAtStr,
		)
		if err != nil {
			return nil, err
		}
		if startedAtStr.Valid {
			startedAt, _ := parseTime(startedAtStr.String)
			op.StartedAt = &startedAt
		}
		if finishedAtStr.Valid {
			finishedAt, _ := parseTime(finishedAtStr.String)
			op.FinishedAt = &finishedAt
		}
		if createdAtStr.Valid {
			op.CreatedAt, _ = parseTime(createdAtStr.String)
		}
		if updatedAtStr.Valid {
			op.UpdatedAt, _ = parseTime(updatedAtStr.String)
		}
		ops = append(ops, op)
	}

	return ops, rows.Err()
}

func sqliteCreateOperationLog(ctx context.Context, q sqliteQuerier, log *models.OperationLog) error {
	query := `
		INSERT INTO operation_logs (
			id, operation_id, level, message, details, created_at
		) VALUES (?, ?, ?, ?, ?, ?)
	`

	_, err := q.ExecContext(ctx, query,
		log.ID, log.OperationID, log.Level, log.Message, log.Details, log.CreatedAt,
	)
	return err
}

func sqliteGetOperationLogs(ctx context.Context, q sqliteQuerier, operationID uuid.UUID) ([]*models.OperationLog, error) {
	query := `
		SELECT 
			id, operation_id, level, message, details, created_at
		FROM operation_logs
		WHERE operation_id = ?
		ORDER BY created_at ASC
	`

	rows, err := q.QueryContext(ctx, query, operationID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var logs []*models.OperationLog
	for rows.Next() {
		log := &models.OperationLog{}
		var createdAtStr sql.NullString
		err := rows.Scan(
			&log.ID, &log.OperationID, &log.Level, &log.Message, &log.Details, &createdAtStr,
		)
		if err != nil {
			return nil, err
		}
		if createdAtStr.Valid {
			log.CreatedAt, _ = parseTime(createdAtStr.String)
		}
		logs = append(logs, log)
	}

	return logs, rows.Err()
}

// ==================== SNAPSHOT OPERATIONS ====================

func sqliteCreateSnapshot(ctx context.Context, q sqliteQuerier, snapshot *models.Snapshot) error {
	query := `INSERT INTO snapshots (id, vm_id, volume_id, name, description, path, status, size_bytes, created_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
	_, err := q.ExecContext(ctx, query, snapshot.ID, snapshot.VMID, snapshot.VolumeID,
		snapshot.Name, snapshot.Description, snapshot.Path, snapshot.Status, snapshot.SizeBytes, snapshot.CreatedAt)
	return err
}

func sqliteGetSnapshot(ctx context.Context, q sqliteQuerier, id uuid.UUID) (*models.Snapshot, error) {
	query := `SELECT id, vm_id, volume_id, name, description, path, status, size_bytes, created_at
		FROM snapshots WHERE id = ?`
	s := &models.Snapshot{}
	var createdAtStr sql.NullString
	err := q.QueryRowContext(ctx, query, id).Scan(&s.ID, &s.VMID, &s.VolumeID,
		&s.Name, &s.Description, &s.Path, &s.Status, &s.SizeBytes, &createdAtStr)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if createdAtStr.Valid {
		s.CreatedAt, _ = parseTime(createdAtStr.String)
	}
	return s, nil
}

func sqliteListSnapshotsByVM(ctx context.Context, q sqliteQuerier, vmID uuid.UUID) ([]*models.Snapshot, error) {
	query := `SELECT id, vm_id, volume_id, name, description, path, status, size_bytes, created_at
		FROM snapshots WHERE vm_id = ? ORDER BY created_at DESC`
	rows, err := q.QueryContext(ctx, query, vmID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var snapshots []*models.Snapshot
	for rows.Next() {
		s := &models.Snapshot{}
		var createdAtStr sql.NullString
		if err := rows.Scan(&s.ID, &s.VMID, &s.VolumeID,
			&s.Name, &s.Description, &s.Path, &s.Status, &s.SizeBytes, &createdAtStr); err != nil {
			return nil, err
		}
		if createdAtStr.Valid {
			s.CreatedAt, _ = parseTime(createdAtStr.String)
		}
		snapshots = append(snapshots, s)
	}
	return snapshots, rows.Err()
}

func sqliteDeleteSnapshot(ctx context.Context, q sqliteQuerier, id uuid.UUID) error {
	query := `DELETE FROM snapshots WHERE id = ?`
	_, err := q.ExecContext(ctx, query, id)
	return err
}

func sqliteUpdateSnapshot(ctx context.Context, q sqliteQuerier, snapshot *models.Snapshot) error {
	query := `UPDATE snapshots SET vm_id = ?, volume_id = ?, name = ?, description = ?,
		path = ?, status = ?, size_bytes = ? WHERE id = ?`
	_, err := q.ExecContext(ctx, query, snapshot.VMID, snapshot.VolumeID, snapshot.Name,
		snapshot.Description, snapshot.Path, snapshot.Status, snapshot.SizeBytes, snapshot.ID)
	return err
}

// ==================== RESOURCE QUOTA OPERATIONS ====================

func sqliteGetQuota(ctx context.Context, q sqliteQuerier, userID string) (*models.ResourceQuota, error) {
	query := `
		SELECT user_id, max_cpus, max_memory_mb, max_vm_count, max_disk_gb, created_at, updated_at
		FROM resource_quotas WHERE user_id = ?
	`
	quota := &models.ResourceQuota{}
	var createdAtStr, updatedAtStr sql.NullString
	err := q.QueryRowContext(ctx, query, userID).Scan(
		&quota.UserID, &quota.MaxCPUs, &quota.MaxMemoryMB, &quota.MaxVMCount, &quota.MaxDiskGB,
		&createdAtStr, &updatedAtStr,
	)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if createdAtStr.Valid {
		quota.CreatedAt, _ = parseTime(createdAtStr.String)
	}
	if updatedAtStr.Valid {
		quota.UpdatedAt, _ = parseTime(updatedAtStr.String)
	}
	return quota, nil
}

func sqliteSetQuota(ctx context.Context, q sqliteQuerier, quota *models.ResourceQuota) error {
	query := `
		INSERT INTO resource_quotas (user_id, max_cpus, max_memory_mb, max_vm_count, max_disk_gb, created_at, updated_at)
		VALUES (?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(user_id) DO UPDATE SET
			max_cpus = excluded.max_cpus,
			max_memory_mb = excluded.max_memory_mb,
			max_vm_count = excluded.max_vm_count,
			max_disk_gb = excluded.max_disk_gb,
			updated_at = excluded.updated_at
	`
	now := time.Now()
	quota.UpdatedAt = now
	if quota.CreatedAt.IsZero() {
		quota.CreatedAt = now
	}
	_, err := q.ExecContext(ctx, query,
		quota.UserID, quota.MaxCPUs, quota.MaxMemoryMB, quota.MaxVMCount, quota.MaxDiskGB,
		quota.CreatedAt, quota.UpdatedAt,
	)
	return err
}

func sqliteGetUsage(ctx context.Context, q sqliteQuerier, userID string) (*models.ResourceUsage, error) {
	query := `
		SELECT user_id, cpus_used, memory_mb_used, vm_count, disk_gb_used, updated_at
		FROM resource_usage WHERE user_id = ?
	`
	usage := &models.ResourceUsage{}
	var updatedAtStr sql.NullString
	err := q.QueryRowContext(ctx, query, userID).Scan(
		&usage.UserID, &usage.CPUsUsed, &usage.MemoryMBUsed, &usage.VMCount, &usage.DiskGBUsed,
		&updatedAtStr,
	)
	if err == sql.ErrNoRows {
		// Return empty usage if not found
		return &models.ResourceUsage{UserID: uuid.MustParse(userID)}, nil
	}
	if err != nil {
		return nil, err
	}
	if updatedAtStr.Valid {
		usage.UpdatedAt, _ = parseTime(updatedAtStr.String)
	}
	return usage, nil
}

func sqliteUpdateUsage(ctx context.Context, q sqliteQuerier, userID string, delta models.ResourceUsage) error {
	query := `
		INSERT INTO resource_usage (user_id, cpus_used, memory_mb_used, vm_count, disk_gb_used, updated_at)
		VALUES (?, ?, ?, ?, ?, ?)
		ON CONFLICT(user_id) DO UPDATE SET
			cpus_used = MAX(0, resource_usage.cpus_used + ?),
			memory_mb_used = MAX(0, resource_usage.memory_mb_used + ?),
			vm_count = MAX(0, resource_usage.vm_count + ?),
			disk_gb_used = MAX(0, resource_usage.disk_gb_used + ?),
			updated_at = excluded.updated_at
	`
	now := time.Now()
	_, err := q.ExecContext(ctx, query,
		userID, delta.CPUsUsed, delta.MemoryMBUsed, delta.VMCount, delta.DiskGBUsed, now,
		delta.CPUsUsed, delta.MemoryMBUsed, delta.VMCount, delta.DiskGBUsed,
	)
	return err
}

func sqliteEnsureQuota(ctx context.Context, q sqliteQuerier, userID string) error {
	// Check if quota exists
	existing, err := sqliteGetQuota(ctx, q, userID)
	if err != nil {
		return err
	}
	if existing == nil {
		// Create default quota
		uid, err := uuid.Parse(userID)
		if err != nil {
			// Use nil UUID for invalid user IDs
			uid = uuid.Nil
		}
		quota := models.DefaultQuota(uid)
		if err := sqliteSetQuota(ctx, q, quota); err != nil {
			return err
		}
	}
	// Ensure usage record exists
	_, err = q.ExecContext(ctx, `
		INSERT INTO resource_usage (user_id, cpus_used, memory_mb_used, vm_count, disk_gb_used, updated_at)
		VALUES (?, 0, 0, 0, 0, ?)
		ON CONFLICT(user_id) DO NOTHING
	`, userID, time.Now())
	return err
}
