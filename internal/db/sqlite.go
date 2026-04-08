package db

import (
	"context"
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"time"

	"github.com/chv/chv/internal/config"
	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
	_ "modernc.org/sqlite"
)

type Repository struct {
	db *sql.DB
}

func Open(path string) (*Repository, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return nil, err
	}

	db, err := sql.Open("sqlite", path)
	if err != nil {
		return nil, err
	}

	repo := &Repository{db: db}
	if err := repo.initialize(); err != nil {
		db.Close()
		return nil, err
	}
	return repo, nil
}

func (r *Repository) Close() error {
	return r.db.Close()
}

func (r *Repository) initialize() error {
	pragmas := []string{
		"PRAGMA journal_mode = WAL;",
		"PRAGMA foreign_keys = ON;",
		"PRAGMA busy_timeout = 5000;",
		"PRAGMA synchronous = NORMAL;",
	}
	for _, pragma := range pragmas {
		if _, err := r.db.Exec(pragma); err != nil {
			return err
		}
	}

	schema, err := os.ReadFile(schemaPath())
	if err != nil {
		return err
	}
	if _, err = r.db.Exec(string(schema)); err != nil {
		return err
	}

	// Run migrations for existing databases
	if err := r.migrateAddImageFormat(); err != nil {
		return fmt.Errorf("failed to migrate images table: %w", err)
	}

	return nil
}

func schemaPath() string {
	_, file, _, _ := runtime.Caller(0)
	candidates := []string{
		filepath.Join(filepath.Dir(file), "..", "..", "configs", "schema_sqlite.sql"),
		"/app/configs/schema_sqlite.sql",
		"./configs/schema_sqlite.sql",
	}

	for _, candidate := range candidates {
		if _, err := os.Stat(candidate); err == nil {
			return candidate
		}
	}

	return candidates[0]
}

func nowUTC() string {
	return time.Now().UTC().Format(time.RFC3339)
}

func boolInt(value bool) int {
	if value {
		return 1
	}
	return 0
}

func (r *Repository) CreateToken(ctx context.Context, token *models.APIToken) error {
	_, err := r.db.ExecContext(
		ctx,
		`INSERT INTO api_tokens (id, name, token_hash, created_at, expires_at, revoked_at)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		token.ID,
		token.Name,
		token.TokenHash,
		token.CreatedAt,
		token.ExpiresAt,
		token.RevokedAt,
	)
	return err
}

func (r *Repository) GetAPITokenByHash(ctx context.Context, hash string) (*models.APIToken, error) {
	row := r.db.QueryRowContext(ctx, `SELECT id, name, token_hash, created_at, expires_at, revoked_at FROM api_tokens WHERE token_hash = ?`, hash)

	var token models.APIToken
	var expiresAt sql.NullString
	var revokedAt sql.NullString
	if err := row.Scan(&token.ID, &token.Name, &token.TokenHash, &token.CreatedAt, &expiresAt, &revokedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}

	if expiresAt.Valid {
		token.ExpiresAt = &expiresAt.String
	}
	if revokedAt.Valid {
		token.RevokedAt = &revokedAt.String
	}

	return &token, nil
}

// User methods

func (r *Repository) CreateUser(ctx context.Context, user *models.User) error {
	_, err := r.db.ExecContext(
		ctx,
		`INSERT INTO users (id, username, password_hash, email, role, is_active, last_login_at, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		user.ID,
		user.Username,
		user.PasswordHash,
		nullable(user.Email),
		user.Role,
		boolInt(user.IsActive),
		nullablePtr(user.LastLoginAt),
		user.CreatedAt,
		user.UpdatedAt,
	)
	return err
}

func (r *Repository) GetUserByUsername(ctx context.Context, username string) (*models.User, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, username, password_hash, email, role, is_active, last_login_at, created_at, updated_at
		 FROM users WHERE username = ?`, username)

	var user models.User
	var email sql.NullString
	var lastLoginAt sql.NullString
	var isActive int

	if err := row.Scan(&user.ID, &user.Username, &user.PasswordHash, &email, &user.Role, &isActive, &lastLoginAt, &user.CreatedAt, &user.UpdatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}

	user.IsActive = isActive == 1
	if email.Valid {
		user.Email = email.String
	}
	if lastLoginAt.Valid {
		user.LastLoginAt = &lastLoginAt.String
	}

	return &user, nil
}

func (r *Repository) GetUserByID(ctx context.Context, id string) (*models.User, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, username, password_hash, email, role, is_active, last_login_at, created_at, updated_at
		 FROM users WHERE id = ?`, id)

	var user models.User
	var email sql.NullString
	var lastLoginAt sql.NullString
	var isActive int

	if err := row.Scan(&user.ID, &user.Username, &user.PasswordHash, &email, &user.Role, &isActive, &lastLoginAt, &user.CreatedAt, &user.UpdatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}

	user.IsActive = isActive == 1
	if email.Valid {
		user.Email = email.String
	}
	if lastLoginAt.Valid {
		user.LastLoginAt = &lastLoginAt.String
	}

	return &user, nil
}

func (r *Repository) UpdateUserLastLogin(ctx context.Context, userID string, loginTime string) error {
	_, err := r.db.ExecContext(ctx,
		`UPDATE users SET last_login_at = ?, updated_at = ? WHERE id = ?`,
		loginTime, nowUTC(), userID)
	return err
}

func nullablePtr(s *string) any {
	if s == nil {
		return nil
	}
	return *s
}

func (r *Repository) UpsertInstallStatus(ctx context.Context, status *models.InstallStatus) error {
	_, err := r.db.ExecContext(
		ctx,
		`INSERT INTO install_status (
			id, data_root, database_path, bridge_name, bridge_exists, bridge_ip_expected, bridge_ip_actual,
			bridge_up, localdisk_path, localdisk_ready, cloud_hypervisor_path, cloud_hypervisor_found,
			cloudinit_supported, overall_state, last_checked_at, last_bootstrapped_at, last_error
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(id) DO UPDATE SET
			data_root = excluded.data_root,
			database_path = excluded.database_path,
			bridge_name = excluded.bridge_name,
			bridge_exists = excluded.bridge_exists,
			bridge_ip_expected = excluded.bridge_ip_expected,
			bridge_ip_actual = excluded.bridge_ip_actual,
			bridge_up = excluded.bridge_up,
			localdisk_path = excluded.localdisk_path,
			localdisk_ready = excluded.localdisk_ready,
			cloud_hypervisor_path = excluded.cloud_hypervisor_path,
			cloud_hypervisor_found = excluded.cloud_hypervisor_found,
			cloudinit_supported = excluded.cloudinit_supported,
			overall_state = excluded.overall_state,
			last_checked_at = excluded.last_checked_at,
			last_bootstrapped_at = excluded.last_bootstrapped_at,
			last_error = excluded.last_error`,
		status.ID,
		status.DataRoot,
		status.DatabasePath,
		status.BridgeName,
		boolInt(status.BridgeExists),
		status.BridgeIPExpected,
		nullable(status.BridgeIPActual),
		boolInt(status.BridgeUp),
		status.LocaldiskPath,
		boolInt(status.LocaldiskReady),
		status.CloudHypervisorPath,
		boolInt(status.CloudHypervisorFound),
		boolInt(status.CloudInitSupported),
		status.OverallState,
		status.LastCheckedAt,
		nullable(status.LastBootstrappedAt),
		nullable(status.LastError),
	)
	return err
}

func (r *Repository) GetInstallStatus(ctx context.Context) (*models.InstallStatus, error) {
	row := r.db.QueryRowContext(
		ctx,
		`SELECT id, data_root, database_path, bridge_name, bridge_exists, bridge_ip_expected, bridge_ip_actual,
		        bridge_up, localdisk_path, localdisk_ready, cloud_hypervisor_path, cloud_hypervisor_found,
		        cloudinit_supported, overall_state, last_checked_at, last_bootstrapped_at, last_error
		   FROM install_status
		  ORDER BY last_checked_at DESC
		  LIMIT 1`,
	)

	var status models.InstallStatus
	var bridgeExists, bridgeUp, localdiskReady, hypervisorFound, cloudInitSupported int
	var bridgeIPActual, lastBootstrappedAt, lastError sql.NullString
	if err := row.Scan(
		&status.ID,
		&status.DataRoot,
		&status.DatabasePath,
		&status.BridgeName,
		&bridgeExists,
		&status.BridgeIPExpected,
		&bridgeIPActual,
		&bridgeUp,
		&status.LocaldiskPath,
		&localdiskReady,
		&status.CloudHypervisorPath,
		&hypervisorFound,
		&cloudInitSupported,
		&status.OverallState,
		&status.LastCheckedAt,
		&lastBootstrappedAt,
		&lastError,
	); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}

	status.BridgeExists = bridgeExists == 1
	status.BridgeUp = bridgeUp == 1
	status.LocaldiskReady = localdiskReady == 1
	status.CloudHypervisorFound = hypervisorFound == 1
	status.CloudInitSupported = cloudInitSupported == 1
	if bridgeIPActual.Valid {
		status.BridgeIPActual = bridgeIPActual.String
	}
	if lastBootstrappedAt.Valid {
		status.LastBootstrappedAt = lastBootstrappedAt.String
	}
	if lastError.Valid {
		status.LastError = lastError.String
	}

	return &status, nil
}

func (r *Repository) EnsureDefaultNetwork(ctx context.Context) error {
	_, err := r.db.ExecContext(
		ctx,
		`INSERT OR IGNORE INTO networks (id, name, mode, bridge_name, cidr, gateway_ip, is_system_managed, status, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		uuid.NewString(),
		"default",
		"bridge",
		config.DefaultBridgeName,
		config.DefaultNetworkCIDR,
		config.DefaultBridgeGateway,
		1,
		"active",
		nowUTC(),
	)
	return err
}

func (r *Repository) EnsureDefaultStoragePool(ctx context.Context, path string) error {
	_, err := r.db.ExecContext(
		ctx,
		`INSERT OR IGNORE INTO storage_pools (id, name, pool_type, path, is_default, status, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`,
		uuid.NewString(),
		"localdisk",
		"localdisk",
		path,
		1,
		"ready",
		nowUTC(),
	)
	return err
}

func (r *Repository) GetNetworkByName(ctx context.Context, name string) (*models.Network, error) {
	row := r.db.QueryRowContext(ctx, `SELECT id, name, mode, bridge_name, cidr, gateway_ip, is_system_managed, status, created_at FROM networks WHERE name = ?`, name)
	var item models.Network
	var isSystemManaged int
	if err := row.Scan(&item.ID, &item.Name, &item.Mode, &item.BridgeName, &item.CIDR, &item.GatewayIP, &isSystemManaged, &item.Status, &item.CreatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	item.IsSystemManaged = isSystemManaged == 1
	return &item, nil
}

func (r *Repository) GetNetworkByID(ctx context.Context, id string) (*models.Network, error) {
	row := r.db.QueryRowContext(ctx, `SELECT id, name, mode, bridge_name, cidr, gateway_ip, is_system_managed, status, created_at FROM networks WHERE id = ?`, id)
	var item models.Network
	var isSystemManaged int
	if err := row.Scan(&item.ID, &item.Name, &item.Mode, &item.BridgeName, &item.CIDR, &item.GatewayIP, &isSystemManaged, &item.Status, &item.CreatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	item.IsSystemManaged = isSystemManaged == 1
	return &item, nil
}

func (r *Repository) CreateNetwork(ctx context.Context, network *models.Network) error {
	_, err := r.db.ExecContext(
		ctx,
		`INSERT INTO networks (id, name, mode, bridge_name, cidr, gateway_ip, is_system_managed, status, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		network.ID,
		network.Name,
		network.Mode,
		network.BridgeName,
		network.CIDR,
		network.GatewayIP,
		boolInt(network.IsSystemManaged),
		network.Status,
		network.CreatedAt,
	)
	return err
}

func (r *Repository) ListNetworks(ctx context.Context) ([]models.Network, error) {
	rows, err := r.db.QueryContext(ctx, `SELECT id, name, mode, bridge_name, cidr, gateway_ip, is_system_managed, status, created_at FROM networks ORDER BY created_at ASC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.Network
	for rows.Next() {
		var item models.Network
		var isSystemManaged int
		if err := rows.Scan(&item.ID, &item.Name, &item.Mode, &item.BridgeName, &item.CIDR, &item.GatewayIP, &isSystemManaged, &item.Status, &item.CreatedAt); err != nil {
			return nil, err
		}
		item.IsSystemManaged = isSystemManaged == 1
		out = append(out, item)
	}
	return out, rows.Err()
}

func (r *Repository) GetStoragePoolByName(ctx context.Context, name string) (*models.StoragePool, error) {
	row := r.db.QueryRowContext(ctx, `SELECT id, name, pool_type, path, is_default, status, COALESCE(capacity_bytes, 0), COALESCE(allocatable_bytes, 0), created_at FROM storage_pools WHERE name = ?`, name)
	var item models.StoragePool
	var isDefault int
	if err := row.Scan(&item.ID, &item.Name, &item.PoolType, &item.Path, &isDefault, &item.Status, &item.CapacityBytes, &item.AllocatableBytes, &item.CreatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	item.IsDefault = isDefault == 1
	return &item, nil
}

func (r *Repository) GetStoragePoolByID(ctx context.Context, id string) (*models.StoragePool, error) {
	row := r.db.QueryRowContext(ctx, `SELECT id, name, pool_type, path, is_default, status, COALESCE(capacity_bytes, 0), COALESCE(allocatable_bytes, 0), created_at FROM storage_pools WHERE id = ?`, id)
	var item models.StoragePool
	var isDefault int
	if err := row.Scan(&item.ID, &item.Name, &item.PoolType, &item.Path, &isDefault, &item.Status, &item.CapacityBytes, &item.AllocatableBytes, &item.CreatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	item.IsDefault = isDefault == 1
	return &item, nil
}

func (r *Repository) CreateStoragePool(ctx context.Context, pool *models.StoragePool) error {
	_, err := r.db.ExecContext(
		ctx,
		`INSERT INTO storage_pools (id, name, pool_type, path, is_default, status, capacity_bytes, allocatable_bytes, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		pool.ID,
		pool.Name,
		pool.PoolType,
		pool.Path,
		boolInt(pool.IsDefault),
		pool.Status,
		pool.CapacityBytes,
		pool.AllocatableBytes,
		pool.CreatedAt,
	)
	return err
}

func (r *Repository) ListStoragePools(ctx context.Context) ([]models.StoragePool, error) {
	rows, err := r.db.QueryContext(ctx, `SELECT id, name, pool_type, path, is_default, status, COALESCE(capacity_bytes, 0), COALESCE(allocatable_bytes, 0), created_at FROM storage_pools ORDER BY created_at ASC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.StoragePool
	for rows.Next() {
		var item models.StoragePool
		var isDefault int
		if err := rows.Scan(&item.ID, &item.Name, &item.PoolType, &item.Path, &isDefault, &item.Status, &item.CapacityBytes, &item.AllocatableBytes, &item.CreatedAt); err != nil {
			return nil, err
		}
		item.IsDefault = isDefault == 1
		out = append(out, item)
	}
	return out, rows.Err()
}

func (r *Repository) ListImages(ctx context.Context) ([]models.Image, error) {
	rows, err := r.db.QueryContext(ctx, `SELECT id, name, os_family, architecture, format, source_format, normalized_format, COALESCE(source_url, ''), COALESCE(checksum, ''), COALESCE(local_path, ''), cloud_init_supported, status, created_at FROM images ORDER BY created_at ASC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.Image
	for rows.Next() {
		var item models.Image
		var cloudInitSupported int
		if err := rows.Scan(&item.ID, &item.Name, &item.OSFamily, &item.Architecture, &item.Format, &item.SourceFormat, &item.NormalizedFormat, &item.SourceURL, &item.Checksum, &item.LocalPath, &cloudInitSupported, &item.Status, &item.CreatedAt); err != nil {
			return nil, err
		}
		item.CloudInitSupported = cloudInitSupported == 1
		out = append(out, item)
	}
	return out, rows.Err()
}

func (r *Repository) CreateImage(ctx context.Context, image *models.Image) error {
	_, err := r.db.ExecContext(
		ctx,
		`INSERT INTO images (id, name, os_family, architecture, format, source_format, normalized_format, source_url, checksum, local_path, cloud_init_supported, status, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		image.ID,
		image.Name,
		image.OSFamily,
		image.Architecture,
		image.Format,
		image.SourceFormat,
		image.NormalizedFormat,
		image.SourceURL,
		nullable(image.Checksum),
		image.LocalPath,
		boolInt(image.CloudInitSupported),
		image.Status,
		image.CreatedAt,
	)
	return err
}

func (r *Repository) GetImageByID(ctx context.Context, id string) (*models.Image, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, name, os_family, architecture, format, source_format, normalized_format, COALESCE(source_url, ''), COALESCE(checksum, ''), COALESCE(local_path, ''), cloud_init_supported, status, created_at
		 FROM images WHERE id = ?`, id)

	var image models.Image
	var cloudInitSupported int
	if err := row.Scan(&image.ID, &image.Name, &image.OSFamily, &image.Architecture, &image.Format, &image.SourceFormat, &image.NormalizedFormat,
		&image.SourceURL, &image.Checksum, &image.LocalPath, &cloudInitSupported, &image.Status, &image.CreatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	image.CloudInitSupported = cloudInitSupported == 1
	return &image, nil
}

func (r *Repository) UpdateImage(ctx context.Context, image *models.Image) error {
	_, err := r.db.ExecContext(
		ctx,
		`UPDATE images SET name = ?, os_family = ?, architecture = ?, format = ?, source_format = ?, normalized_format = ?, source_url = ?,
		 checksum = ?, local_path = ?, cloud_init_supported = ?, status = ?
		 WHERE id = ?`,
		image.Name,
		image.OSFamily,
		image.Architecture,
		image.Format,
		image.SourceFormat,
		image.NormalizedFormat,
		image.SourceURL,
		nullable(image.Checksum),
		image.LocalPath,
		boolInt(image.CloudInitSupported),
		image.Status,
		image.ID,
	)
	return err
}

func (r *Repository) ListVMs(ctx context.Context) ([]models.VirtualMachine, error) {
	rows, err := r.db.QueryContext(ctx, `SELECT id, name, image_id, storage_pool_id, network_id, desired_state, actual_state, vcpu, memory_mb, disk_path, COALESCE(seed_iso_path, ''), workspace_path, COALESCE(cloud_hypervisor_pid, 0), COALESCE(ip_address, ''), COALESCE(mac_address, ''), COALESCE(last_error, ''), created_at, updated_at FROM virtual_machines ORDER BY created_at ASC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.VirtualMachine
	for rows.Next() {
		var item models.VirtualMachine
		if err := rows.Scan(&item.ID, &item.Name, &item.ImageID, &item.StoragePoolID, &item.NetworkID, &item.DesiredState, &item.ActualState, &item.VCPU, &item.MemoryMB, &item.DiskPath, &item.SeedISOPath, &item.WorkspacePath, &item.CloudHypervisorPID, &item.IPAddress, &item.MACAddress, &item.LastError, &item.CreatedAt, &item.UpdatedAt); err != nil {
			return nil, err
		}
		out = append(out, item)
	}
	return out, rows.Err()
}

func (r *Repository) GetVMByID(ctx context.Context, id string) (*models.VirtualMachine, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, name, image_id, storage_pool_id, network_id, desired_state, actual_state, vcpu, memory_mb,
		        disk_path, COALESCE(seed_iso_path, ''), workspace_path, COALESCE(cloud_hypervisor_pid, 0),
		        COALESCE(ip_address, ''), COALESCE(mac_address, ''), COALESCE(last_error, ''), created_at, updated_at
		 FROM virtual_machines WHERE id = ?`, id)

	var v models.VirtualMachine
	if err := row.Scan(&v.ID, &v.Name, &v.ImageID, &v.StoragePoolID, &v.NetworkID,
		&v.DesiredState, &v.ActualState, &v.VCPU, &v.MemoryMB, &v.DiskPath, &v.SeedISOPath,
		&v.WorkspacePath, &v.CloudHypervisorPID, &v.IPAddress, &v.MACAddress, &v.LastError, &v.CreatedAt, &v.UpdatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	return &v, nil
}

func (r *Repository) CreateVM(ctx context.Context, vm *models.VirtualMachine) error {
	_, err := r.db.ExecContext(
		ctx,
		`INSERT INTO virtual_machines (
			id, name, image_id, storage_pool_id, network_id, desired_state, actual_state,
			vcpu, memory_mb, disk_path, seed_iso_path, workspace_path, last_error, created_at, updated_at
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		vm.ID, vm.Name, vm.ImageID, vm.StoragePoolID, vm.NetworkID,
		vm.DesiredState, vm.ActualState, vm.VCPU, vm.MemoryMB,
		vm.DiskPath, nullable(vm.SeedISOPath), vm.WorkspacePath,
		nullable(vm.LastError), vm.CreatedAt, vm.UpdatedAt,
	)
	return err
}

func (r *Repository) UpdateVM(ctx context.Context, vm *models.VirtualMachine) error {
	_, err := r.db.ExecContext(
		ctx,
		`UPDATE virtual_machines SET
			desired_state = ?, actual_state = ?, last_error = ?, updated_at = ?
		 WHERE id = ?`,
		vm.DesiredState, vm.ActualState, nullable(vm.LastError), vm.UpdatedAt, vm.ID,
	)
	return err
}

func (r *Repository) DeleteVM(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM virtual_machines WHERE id = ?`, id)
	return err
}

func (r *Repository) ListOperations(ctx context.Context) ([]models.Operation, error) {
	rows, err := r.db.QueryContext(ctx, `SELECT id, resource_type, resource_id, operation_type, state, COALESCE(request_payload, ''), COALESCE(result_payload, ''), COALESCE(error_payload, ''), COALESCE(started_at, ''), COALESCE(finished_at, ''), created_at FROM operations ORDER BY created_at DESC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.Operation
	for rows.Next() {
		var item models.Operation
		if err := rows.Scan(&item.ID, &item.ResourceType, &item.ResourceID, &item.OperationType, &item.State, &item.RequestPayload, &item.ResultPayload, &item.ErrorPayload, &item.StartedAt, &item.FinishedAt, &item.CreatedAt); err != nil {
			return nil, err
		}
		out = append(out, item)
	}
	return out, rows.Err()
}

func (r *Repository) CreateOperation(ctx context.Context, op *models.Operation) error {
	_, err := r.db.ExecContext(
		ctx,
		`INSERT INTO operations (id, resource_type, resource_id, operation_type, state, request_payload, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`,
		op.ID,
		op.ResourceType,
		op.ResourceID,
		op.OperationType,
		op.State,
		op.RequestPayload,
		op.CreatedAt,
	)
	return err
}

func (r *Repository) UpdateOperation(ctx context.Context, op *models.Operation) error {
	_, err := r.db.ExecContext(
		ctx,
		`UPDATE operations SET state = ?, result_payload = ?, error_payload = ?, started_at = ?, finished_at = ?
		 WHERE id = ?`,
		op.State,
		nullable(op.ResultPayload),
		nullable(op.ErrorPayload),
		nullable(op.StartedAt),
		op.FinishedAt,
		op.ID,
	)
	return err
}

// migrateAddImageFormat adds missing columns to images table
func (r *Repository) migrateAddImageFormat() error {
	columns := []struct {
		name string
		def  string
	}{
		{"format", "TEXT DEFAULT 'qcow2'"},
		{"source_url", "TEXT"},
		{"local_path", "TEXT"},
	}

	for _, col := range columns {
		var count int
		err := r.db.QueryRow(`
			SELECT COUNT(*) FROM pragma_table_info('images') WHERE name = ?
		`, col.name).Scan(&count)
		if err != nil {
			return err
		}

		if count == 0 {
			_, err = r.db.Exec(fmt.Sprintf(`ALTER TABLE images ADD COLUMN %s %s`, col.name, col.def))
			if err != nil {
				return fmt.Errorf("failed to add %s column: %w", col.name, err)
			}
		}
	}
	return nil
}

func nullable(value string) any {
	if value == "" {
		return nil
	}
	return value
}
