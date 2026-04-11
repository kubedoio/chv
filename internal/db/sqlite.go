package db

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"time"

	"github.com/chv/chv/internal/config"
	"github.com/chv/chv/internal/models"
	"github.com/chv/chv/internal/networking"
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

func (r *Repository) PingContext(ctx context.Context) error {
	return r.db.PingContext(ctx)
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

	// Run multi-node support migrations
	if err := r.migrateAddNodesTable(); err != nil {
		return fmt.Errorf("failed to migrate nodes table: %w", err)
	}
	if err := r.migrateAddNodeIDColumns(); err != nil {
		return fmt.Errorf("failed to migrate node_id columns: %w", err)
	}

	// Run agent token migrations
	if err := r.migrateAddAgentTokenColumns(); err != nil {
		return fmt.Errorf("failed to migrate agent token columns: %w", err)
	}

	// Run VM templates migrations
	if err := r.migrateAddVMTemplatesTable(); err != nil {
		return fmt.Errorf("failed to migrate vm_templates table: %w", err)
	}

	// Run Cloud-init templates migrations
	if err := r.migrateAddCloudInitTemplatesTable(); err != nil {
		return fmt.Errorf("failed to migrate cloud_init_templates table: %w", err)
	}

	// Run Phase 3 migrations
	if err := r.migrateAddUserIDToVMs(); err != nil {
		return fmt.Errorf("failed to migrate user_id to vms: %w", err)
	}
	if err := r.migrateAddQuotasTable(); err != nil {
		return fmt.Errorf("failed to migrate quotas table: %w", err)
	}
	if err := r.migrateAddUsageCacheTable(); err != nil {
		return fmt.Errorf("failed to migrate usage_cache table: %w", err)
	}
	if err := r.migrateAddVLANNetworksTable(); err != nil {
		return fmt.Errorf("failed to migrate vlan_networks table: %w", err)
	}
	if err := r.migrateAddDHCPServersTable(); err != nil {
		return fmt.Errorf("failed to migrate dhcp_servers table: %w", err)
	}
	if err := r.migrateAddDHCPLeasesTable(); err != nil {
		return fmt.Errorf("failed to migrate dhcp_leases table: %w", err)
	}
	if err := r.migrateAddFirewallRulesTable(); err != nil {
		return fmt.Errorf("failed to migrate firewall_rules table: %w", err)
	}
	if err := r.migrateAddBackupJobsTable(); err != nil {
		return fmt.Errorf("failed to migrate backup_jobs table: %w", err)
	}
	if err := r.migrateAddBackupHistoryTable(); err != nil {
		return fmt.Errorf("failed to migrate backup_history table: %w", err)
	}
	if err := r.migrateAddNodeMetricsTable(); err != nil {
		return fmt.Errorf("failed to migrate node_metrics table: %w", err)
	}

	// Run VNC console migration
	if err := r.migrateAddConsoleTypeToVMs(); err != nil {
		return fmt.Errorf("failed to migrate console_type to vms: %w", err)
	}

	// Ensure local node exists
	if err := r.ensureLocalNode(context.Background()); err != nil {
		return fmt.Errorf("failed to ensure local node: %w", err)
	}

	// Ensure default cloud-init templates exist
	if err := r.ensureDefaultCloudInitTemplates(context.Background()); err != nil {
		return fmt.Errorf("failed to ensure default cloud-init templates: %w", err)
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
	row := r.db.QueryRowContext(ctx, `SELECT id, node_id, name, mode, bridge_name, cidr, gateway_ip, is_system_managed, status, created_at FROM networks WHERE id = ?`, id)
	var item models.Network
	var isSystemManaged int
	if err := row.Scan(&item.ID, &item.NodeID, &item.Name, &item.Mode, &item.BridgeName, &item.CIDR, &item.GatewayIP, &isSystemManaged, &item.Status, &item.CreatedAt); err != nil {
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
		`INSERT INTO networks (id, node_id, name, mode, bridge_name, cidr, gateway_ip, is_system_managed, status, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		network.ID,
		network.NodeID,
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
	rows, err := r.db.QueryContext(ctx, `SELECT id, node_id, name, mode, bridge_name, cidr, gateway_ip, is_system_managed, status, created_at FROM networks ORDER BY created_at ASC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.Network
	for rows.Next() {
		var item models.Network
		var isSystemManaged int
		if err := rows.Scan(&item.ID, &item.NodeID, &item.Name, &item.Mode, &item.BridgeName, &item.CIDR, &item.GatewayIP, &isSystemManaged, &item.Status, &item.CreatedAt); err != nil {
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
	row := r.db.QueryRowContext(ctx, `SELECT id, node_id, name, pool_type, path, is_default, status, COALESCE(capacity_bytes, 0), COALESCE(allocatable_bytes, 0), created_at FROM storage_pools WHERE id = ?`, id)
	var item models.StoragePool
	var isDefault int
	if err := row.Scan(&item.ID, &item.NodeID, &item.Name, &item.PoolType, &item.Path, &isDefault, &item.Status, &item.CapacityBytes, &item.AllocatableBytes, &item.CreatedAt); err != nil {
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
		`INSERT INTO storage_pools (id, node_id, name, pool_type, path, is_default, status, capacity_bytes, allocatable_bytes, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		pool.ID,
		pool.NodeID,
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
	rows, err := r.db.QueryContext(ctx, `SELECT id, node_id, name, pool_type, path, is_default, status, COALESCE(capacity_bytes, 0), COALESCE(allocatable_bytes, 0), created_at FROM storage_pools ORDER BY created_at ASC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.StoragePool
	for rows.Next() {
		var item models.StoragePool
		var isDefault int
		if err := rows.Scan(&item.ID, &item.NodeID, &item.Name, &item.PoolType, &item.Path, &isDefault, &item.Status, &item.CapacityBytes, &item.AllocatableBytes, &item.CreatedAt); err != nil {
			return nil, err
		}
		item.IsDefault = isDefault == 1
		out = append(out, item)
	}
	return out, rows.Err()
}

func (r *Repository) ListImages(ctx context.Context) ([]models.Image, error) {
	rows, err := r.db.QueryContext(ctx, `SELECT id, node_id, name, os_family, architecture, format, source_format, normalized_format, source_url, checksum, local_path, cloud_init_supported, status, created_at FROM images ORDER BY created_at ASC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.Image
	for rows.Next() {
		var item models.Image
		var cloudInitSupported int
		var checksum sql.NullString
		if err := rows.Scan(&item.ID, &item.NodeID, &item.Name, &item.OSFamily, &item.Architecture, &item.Format, &item.SourceFormat, &item.NormalizedFormat, &item.SourceURL, &checksum, &item.LocalPath, &cloudInitSupported, &item.Status, &item.CreatedAt); err != nil {
			return nil, err
		}
		item.CloudInitSupported = cloudInitSupported == 1
		if checksum.Valid {
			item.Checksum = checksum.String
		}
		out = append(out, item)
	}
	return out, rows.Err()
}

func (r *Repository) CreateImage(ctx context.Context, image *models.Image) error {
	_, err := r.db.ExecContext(
		ctx,
		`INSERT INTO images (id, node_id, name, os_family, architecture, format, source_format, normalized_format, source_url, checksum, local_path, cloud_init_supported, status, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		image.ID,
		image.NodeID,
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
		`SELECT id, node_id, name, os_family, architecture, format, source_format, normalized_format, source_url, checksum, local_path, cloud_init_supported, status, created_at
		 FROM images WHERE id = ?`, id)

	var image models.Image
	var cloudInitSupported int
	var checksum sql.NullString
	if err := row.Scan(&image.ID, &image.NodeID, &image.Name, &image.OSFamily, &image.Architecture, &image.Format, &image.SourceFormat, &image.NormalizedFormat,
		&image.SourceURL, &checksum, &image.LocalPath, &cloudInitSupported, &image.Status, &image.CreatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	image.CloudInitSupported = cloudInitSupported == 1
	if checksum.Valid {
		image.Checksum = checksum.String
	}
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
	rows, err := r.db.QueryContext(ctx, `SELECT id, node_id, name, image_id, storage_pool_id, network_id, desired_state, actual_state, vcpu, memory_mb, disk_path, COALESCE(seed_iso_path, ''), workspace_path, COALESCE(cloud_hypervisor_pid, 0), COALESCE(ip_address, ''), COALESCE(mac_address, ''), COALESCE(console_type, 'pty'), COALESCE(last_error, ''), created_at, updated_at FROM virtual_machines ORDER BY created_at ASC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.VirtualMachine
	for rows.Next() {
		var item models.VirtualMachine
		if err := rows.Scan(&item.ID, &item.NodeID, &item.Name, &item.ImageID, &item.StoragePoolID, &item.NetworkID, &item.DesiredState, &item.ActualState, &item.VCPU, &item.MemoryMB, &item.DiskPath, &item.SeedISOPath, &item.WorkspacePath, &item.CloudHypervisorPID, &item.IPAddress, &item.MACAddress, &item.ConsoleType, &item.LastError, &item.CreatedAt, &item.UpdatedAt); err != nil {
			return nil, err
		}
		out = append(out, item)
	}
	return out, rows.Err()
}

func (r *Repository) GetVMByID(ctx context.Context, id string) (*models.VirtualMachine, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, node_id, name, image_id, storage_pool_id, network_id, desired_state, actual_state, vcpu, memory_mb,
		        disk_path, COALESCE(seed_iso_path, ''), workspace_path, COALESCE(cloud_hypervisor_pid, 0),
		        COALESCE(ip_address, ''), COALESCE(mac_address, ''), COALESCE(console_type, 'pty'), COALESCE(last_error, ''), created_at, updated_at
		 FROM virtual_machines WHERE id = ?`, id)

	var v models.VirtualMachine
	if err := row.Scan(&v.ID, &v.NodeID, &v.Name, &v.ImageID, &v.StoragePoolID, &v.NetworkID,
		&v.DesiredState, &v.ActualState, &v.VCPU, &v.MemoryMB, &v.DiskPath, &v.SeedISOPath,
		&v.WorkspacePath, &v.CloudHypervisorPID, &v.IPAddress, &v.MACAddress, &v.ConsoleType, &v.LastError, &v.CreatedAt, &v.UpdatedAt); err != nil {
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
			id, node_id, name, image_id, storage_pool_id, network_id, desired_state, actual_state,
			vcpu, memory_mb, disk_path, seed_iso_path, workspace_path, console_type, last_error, created_at, updated_at
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		vm.ID, vm.NodeID, vm.Name, vm.ImageID, vm.StoragePoolID, vm.NetworkID,
		vm.DesiredState, vm.ActualState, vm.VCPU, vm.MemoryMB,
		vm.DiskPath, nullable(vm.SeedISOPath), vm.WorkspacePath,
		nullable(vm.ConsoleType), nullable(vm.LastError), vm.CreatedAt, vm.UpdatedAt,
	)
	return err
}

func (r *Repository) UpdateVM(ctx context.Context, vm *models.VirtualMachine) error {
	_, err := r.db.ExecContext(
		ctx,
		`UPDATE virtual_machines SET
			desired_state = ?, actual_state = ?, last_error = ?, seed_iso_path = ?,
			ip_address = ?, mac_address = ?, console_type = ?, updated_at = ?
		 WHERE id = ?`,
		vm.DesiredState, vm.ActualState, nullable(vm.LastError), nullable(vm.SeedISOPath),
		nullable(vm.IPAddress), nullable(vm.MACAddress), nullable(vm.ConsoleType), vm.UpdatedAt, vm.ID,
	)
	return err
}

func (r *Repository) DeleteVM(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM virtual_machines WHERE id = ?`, id)
	return err
}

// ListVMsByDesiredState returns all VMs with the specified desired state
func (r *Repository) ListVMsByDesiredState(ctx context.Context, desiredState string) ([]models.VirtualMachine, error) {
	rows, err := r.db.QueryContext(ctx, `SELECT id, node_id, name, image_id, storage_pool_id, network_id, desired_state, actual_state, vcpu, memory_mb, disk_path, COALESCE(seed_iso_path, ''), workspace_path, COALESCE(cloud_hypervisor_pid, 0), COALESCE(ip_address, ''), COALESCE(mac_address, ''), COALESCE(console_type, 'pty'), COALESCE(last_error, ''), created_at, updated_at FROM virtual_machines WHERE desired_state = ? ORDER BY created_at ASC`, desiredState)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.VirtualMachine
	for rows.Next() {
		var item models.VirtualMachine
		if err := rows.Scan(&item.ID, &item.NodeID, &item.Name, &item.ImageID, &item.StoragePoolID, &item.NetworkID, &item.DesiredState, &item.ActualState, &item.VCPU, &item.MemoryMB, &item.DiskPath, &item.SeedISOPath, &item.WorkspacePath, &item.CloudHypervisorPID, &item.IPAddress, &item.MACAddress, &item.ConsoleType, &item.LastError, &item.CreatedAt, &item.UpdatedAt); err != nil {
			return nil, err
		}
		out = append(out, item)
	}
	return out, rows.Err()
}

func (r *Repository) ListVMsByNetwork(ctx context.Context, networkID string) ([]models.VirtualMachine, error) {
	rows, err := r.db.QueryContext(ctx, `SELECT id, node_id, name, image_id, storage_pool_id, network_id, desired_state, actual_state, vcpu, memory_mb, disk_path, COALESCE(seed_iso_path, ''), workspace_path, COALESCE(cloud_hypervisor_pid, 0), COALESCE(ip_address, ''), COALESCE(mac_address, ''), COALESCE(console_type, 'pty'), COALESCE(last_error, ''), created_at, updated_at FROM virtual_machines WHERE network_id = ? ORDER BY created_at ASC`, networkID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.VirtualMachine
	for rows.Next() {
		var item models.VirtualMachine
		if err := rows.Scan(&item.ID, &item.NodeID, &item.Name, &item.ImageID, &item.StoragePoolID, &item.NetworkID, &item.DesiredState, &item.ActualState, &item.VCPU, &item.MemoryMB, &item.DiskPath, &item.SeedISOPath, &item.WorkspacePath, &item.CloudHypervisorPID, &item.IPAddress, &item.MACAddress, &item.ConsoleType, &item.LastError, &item.CreatedAt, &item.UpdatedAt); err != nil {
			return nil, err
		}
		out = append(out, item)
	}
	return out, rows.Err()
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
		{"source_format", "TEXT DEFAULT 'qcow2'"},
		{"normalized_format", "TEXT DEFAULT 'qcow2'"},
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

func (r *Repository) CreateVMSnapshot(ctx context.Context, s *models.VMSnapshot) error {
	_, err := r.db.ExecContext(ctx,
		`INSERT INTO vm_snapshots (id, vm_id, name, created_at, status) VALUES (?, ?, ?, ?, ?)`,
		s.ID, s.VMID, s.Name, s.CreatedAt, s.Status)
	return err
}

func (r *Repository) GetVMSnapshot(ctx context.Context, id string) (*models.VMSnapshot, error) {
	row := r.db.QueryRowContext(ctx, `SELECT id, vm_id, name, created_at, status FROM vm_snapshots WHERE id = ?`, id)
	var s models.VMSnapshot
	if err := row.Scan(&s.ID, &s.VMID, &s.Name, &s.CreatedAt, &s.Status); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	return &s, nil
}

func (r *Repository) ListVMSnapshots(ctx context.Context, vmID string) ([]models.VMSnapshot, error) {
	rows, err := r.db.QueryContext(ctx, `SELECT id, vm_id, name, created_at, status FROM vm_snapshots WHERE vm_id = ? ORDER BY created_at DESC`, vmID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.VMSnapshot
	for rows.Next() {
		var s models.VMSnapshot
		if err := rows.Scan(&s.ID, &s.VMID, &s.Name, &s.CreatedAt, &s.Status); err != nil {
			return nil, err
		}
		out = append(out, s)
	}
	return out, rows.Err()
}

func (r *Repository) DeleteVMSnapshot(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM vm_snapshots WHERE id = ?`, id)
	return err
}

func nullable(value string) any {
	if value == "" {
		return nil
	}
	return value
}

// migrateAddNodesTable creates the nodes table if it doesn't exist
func (r *Repository) migrateAddNodesTable() error {
	// Check if nodes table exists
	var count int
	err := r.db.QueryRow(`SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='nodes'`).Scan(&count)
	if err != nil {
		return err
	}

	if count == 0 {
		// Create nodes table
		_, err = r.db.Exec(`
			CREATE TABLE nodes (
				id TEXT PRIMARY KEY,
				name TEXT NOT NULL,
				hostname TEXT NOT NULL,
				ip_address TEXT NOT NULL,
				status TEXT NOT NULL DEFAULT 'offline',
				is_local INTEGER NOT NULL DEFAULT 1,
				agent_url TEXT NULL,
				agent_token TEXT NULL,
				agent_token_hash TEXT NULL,
				capabilities TEXT NULL,
				last_seen_at TEXT NULL,
				created_at TEXT NOT NULL,
				updated_at TEXT NOT NULL
			)
		`)
		if err != nil {
			return fmt.Errorf("failed to create nodes table: %w", err)
		}

		// Create index on node status
		_, err = r.db.Exec(`CREATE INDEX idx_nodes_status ON nodes(status)`)
		if err != nil {
			return fmt.Errorf("failed to create nodes status index: %w", err)
		}
	}

	return nil
}

// migrateAddAgentTokenColumns adds agent_token_hash and capabilities columns if they don't exist
func (r *Repository) migrateAddAgentTokenColumns() error {
	columns := []struct {
		name string
		def  string
	}{
		{"agent_token_hash", "TEXT NULL"},
		{"capabilities", "TEXT NULL"},
	}

	for _, col := range columns {
		var count int
		err := r.db.QueryRow(`
			SELECT COUNT(*) FROM pragma_table_info('nodes') WHERE name = ?
		`, col.name).Scan(&count)
		if err != nil {
			return err
		}

		if count == 0 {
			_, err = r.db.Exec(fmt.Sprintf(`ALTER TABLE nodes ADD COLUMN %s %s`, col.name, col.def))
			if err != nil {
				return fmt.Errorf("failed to add %s column to nodes: %w", col.name, err)
			}
		}
	}

	return nil
}

// migrateAddNodeIDColumns adds node_id columns to existing tables
func (r *Repository) migrateAddNodeIDColumns() error {
	tables := []struct {
		name    string
		columns []struct {
			name string
			def  string
		}
		createIndex bool
	}{
		{
			name: "networks",
			columns: []struct {
				name string
				def  string
			}{
				{"node_id", "TEXT DEFAULT 'local'"},
			},
			createIndex: true,
		},
		{
			name: "storage_pools",
			columns: []struct {
				name string
				def  string
			}{
				{"node_id", "TEXT DEFAULT 'local'"},
			},
			createIndex: true,
		},
		{
			name: "images",
			columns: []struct {
				name string
				def  string
			}{
				{"node_id", "TEXT DEFAULT 'local'"},
			},
			createIndex: true,
		},
		{
			name: "virtual_machines",
			columns: []struct {
				name string
				def  string
			}{
				{"node_id", "TEXT DEFAULT 'local'"},
			},
			createIndex: true,
		},
	}

	for _, table := range tables {
		for _, col := range table.columns {
			var count int
			err := r.db.QueryRow(`
				SELECT COUNT(*) FROM pragma_table_info(?) WHERE name = ?
			`, table.name, col.name).Scan(&count)
			if err != nil {
				return err
			}

			if count == 0 {
				_, err = r.db.Exec(fmt.Sprintf(`ALTER TABLE %s ADD COLUMN %s %s`, table.name, col.name, col.def))
				if err != nil {
					return fmt.Errorf("failed to add %s column to %s: %w", col.name, table.name, err)
				}
			}
		}

		// Create index if needed
		if table.createIndex {
			indexName := fmt.Sprintf("idx_%s_node_id", table.name)
			_, err := r.db.Exec(fmt.Sprintf(`CREATE INDEX IF NOT EXISTS %s ON %s(node_id)`, indexName, table.name))
			if err != nil {
				return fmt.Errorf("failed to create index %s: %w", indexName, err)
			}
		}
	}

	return nil
}

// ensureLocalNode creates the local node if it doesn't exist
func (r *Repository) ensureLocalNode(ctx context.Context) error {
	// Check if local node exists (by is_local flag)
	var count int
	err := r.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM nodes WHERE is_local = 1`).Scan(&count)
	if err != nil {
		return err
	}

	if count == 0 {
		// Check if there's a legacy "local" node (for backward compatibility)
		var legacyCount int
		err = r.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM nodes WHERE id = 'local'`).Scan(&legacyCount)
		if err != nil {
			return err
		}

		if legacyCount == 0 {
			// Create default local node with a proper UUID
			now := nowUTC()
			localNodeID := uuid.New().String()
			_, err = r.db.ExecContext(ctx, `
				INSERT INTO nodes (id, name, hostname, ip_address, status, is_local, created_at, updated_at)
				VALUES (?, ?, ?, ?, ?, ?, ?, ?)
			`, localNodeID, "Local Node", "localhost", "127.0.0.1", models.NodeStatusOnline, 1, now, now)
			if err != nil {
				return fmt.Errorf("failed to create local node: %w", err)
			}
		} else {
			// Mark legacy "local" node as is_local
			_, err = r.db.ExecContext(ctx, `UPDATE nodes SET is_local = 1 WHERE id = 'local'`)
			if err != nil {
				return fmt.Errorf("failed to update legacy local node: %w", err)
			}
		}
	}

	return nil
}

// Node repository methods

func (r *Repository) CreateNode(ctx context.Context, node *models.Node) error {
	_, err := r.db.ExecContext(
		ctx,
		`INSERT INTO nodes (id, name, hostname, ip_address, status, is_local, agent_url, agent_token, agent_token_hash, capabilities, last_seen_at, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		node.ID,
		node.Name,
		node.Hostname,
		node.IPAddress,
		node.Status,
		boolInt(node.IsLocal),
		nullable(node.AgentURL),
		nullable(node.AgentToken),
		nullable(node.AgentTokenHash),
		nullable(node.Capabilities),
		nullable(node.LastSeenAt),
		node.CreatedAt,
		node.UpdatedAt,
	)
	return err
}

func (r *Repository) GetNode(ctx context.Context, id string) (*models.Node, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, name, hostname, ip_address, status, is_local, agent_url, agent_token_hash, capabilities, last_seen_at, created_at, updated_at FROM nodes WHERE id = ?`, id)
	var node models.Node
	var isLocal int
	var agentURL, agentTokenHash, capabilities, lastSeenAt sql.NullString
	err := row.Scan(&node.ID, &node.Name, &node.Hostname, &node.IPAddress, &node.Status, &isLocal, &agentURL, &agentTokenHash, &capabilities, &lastSeenAt, &node.CreatedAt, &node.UpdatedAt)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	node.IsLocal = isLocal == 1
	if agentURL.Valid {
		node.AgentURL = agentURL.String
	}
	if agentTokenHash.Valid {
		node.AgentTokenHash = agentTokenHash.String
	}
	if capabilities.Valid {
		node.Capabilities = capabilities.String
	}
	if lastSeenAt.Valid {
		node.LastSeenAt = lastSeenAt.String
	}
	return &node, nil
}

func (r *Repository) GetLocalNode(ctx context.Context) (*models.Node, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, name, hostname, ip_address, status, is_local, agent_url, agent_token_hash, capabilities, last_seen_at, created_at, updated_at FROM nodes WHERE is_local = 1 LIMIT 1`)
	var node models.Node
	var isLocal int
	var agentURL, agentTokenHash, capabilities, lastSeenAt sql.NullString
	err := row.Scan(&node.ID, &node.Name, &node.Hostname, &node.IPAddress, &node.Status, &isLocal, &agentURL, &agentTokenHash, &capabilities, &lastSeenAt, &node.CreatedAt, &node.UpdatedAt)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	node.IsLocal = isLocal == 1
	if agentURL.Valid {
		node.AgentURL = agentURL.String
	}
	if agentTokenHash.Valid {
		node.AgentTokenHash = agentTokenHash.String
	}
	if capabilities.Valid {
		node.Capabilities = capabilities.String
	}
	if lastSeenAt.Valid {
		node.LastSeenAt = lastSeenAt.String
	}
	return &node, nil
}

func (r *Repository) ListNodes(ctx context.Context) ([]models.Node, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, name, hostname, ip_address, status, is_local, agent_url, capabilities, last_seen_at, created_at, updated_at FROM nodes ORDER BY is_local DESC, name ASC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.Node
	for rows.Next() {
		var node models.Node
		var isLocal int
		var agentURL, capabilities, lastSeenAt sql.NullString
		if err := rows.Scan(&node.ID, &node.Name, &node.Hostname, &node.IPAddress, &node.Status, &isLocal, &agentURL, &capabilities, &lastSeenAt, &node.CreatedAt, &node.UpdatedAt); err != nil {
			return nil, err
		}
		node.IsLocal = isLocal == 1
		if agentURL.Valid {
			node.AgentURL = agentURL.String
		}
		if capabilities.Valid {
			node.Capabilities = capabilities.String
		}
		if lastSeenAt.Valid {
			node.LastSeenAt = lastSeenAt.String
		}
		out = append(out, node)
	}
	return out, rows.Err()
}

func (r *Repository) UpdateNode(ctx context.Context, node *models.Node) error {
	_, err := r.db.ExecContext(ctx,
		`UPDATE nodes SET name = ?, hostname = ?, ip_address = ?, agent_url = ?, capabilities = ?, updated_at = ? WHERE id = ?`,
		node.Name,
		node.Hostname,
		node.IPAddress,
		nullable(node.AgentURL),
		nullable(node.Capabilities),
		node.UpdatedAt,
		node.ID,
	)
	return err
}

func (r *Repository) UpdateNodeStatus(ctx context.Context, id string, status string) error {
	_, err := r.db.ExecContext(ctx,
		`UPDATE nodes SET status = ?, updated_at = ? WHERE id = ?`,
		status, nowUTC(), id)
	return err
}

func (r *Repository) UpdateNodeLastSeen(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx,
		`UPDATE nodes SET last_seen_at = ?, updated_at = ? WHERE id = ?`,
		nowUTC(), nowUTC(), id)
	return err
}

func (r *Repository) DeleteNode(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM nodes WHERE id = ?`, id)
	return err
}

// ValidateAgentToken checks if an agent token is valid
func (r *Repository) ValidateAgentToken(ctx context.Context, tokenHash string) error {
	row := r.db.QueryRowContext(ctx,
		`SELECT COUNT(*) FROM nodes WHERE agent_token_hash = ?`, tokenHash)
	var count int
	if err := row.Scan(&count); err != nil {
		return err
	}
	if count == 0 {
		return fmt.Errorf("invalid agent token")
	}
	return nil
}

// GetNodeByAgentToken retrieves a node by its agent token hash
func (r *Repository) GetNodeByAgentToken(ctx context.Context, tokenHash string) (*models.Node, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, name, hostname, ip_address, status, is_local, agent_url, agent_token_hash, capabilities, last_seen_at, created_at, updated_at FROM nodes WHERE agent_token_hash = ?`, tokenHash)
	var node models.Node
	var isLocal int
	var agentURL, agentTokenHash, capabilities, lastSeenAt sql.NullString
	err := row.Scan(&node.ID, &node.Name, &node.Hostname, &node.IPAddress, &node.Status, &isLocal, &agentURL, &agentTokenHash, &capabilities, &lastSeenAt, &node.CreatedAt, &node.UpdatedAt)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	node.IsLocal = isLocal == 1
	if agentURL.Valid {
		node.AgentURL = agentURL.String
	}
	if agentTokenHash.Valid {
		node.AgentTokenHash = agentTokenHash.String
	}
	if capabilities.Valid {
		node.Capabilities = capabilities.String
	}
	if lastSeenAt.Valid {
		node.LastSeenAt = lastSeenAt.String
	}
	return &node, nil
}

// GetNodesByStatus returns all nodes with the specified status
func (r *Repository) GetNodesByStatus(ctx context.Context, status string) ([]models.Node, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, name, hostname, ip_address, status, is_local, agent_url, capabilities, last_seen_at, created_at, updated_at FROM nodes WHERE status = ? ORDER BY name ASC`, status)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.Node
	for rows.Next() {
		var node models.Node
		var isLocal int
		var agentURL, capabilities, lastSeenAt sql.NullString
		if err := rows.Scan(&node.ID, &node.Name, &node.Hostname, &node.IPAddress, &node.Status, &isLocal, &agentURL, &capabilities, &lastSeenAt, &node.CreatedAt, &node.UpdatedAt); err != nil {
			return nil, err
		}
		node.IsLocal = isLocal == 1
		if agentURL.Valid {
			node.AgentURL = agentURL.String
		}
		if capabilities.Valid {
			node.Capabilities = capabilities.String
		}
		if lastSeenAt.Valid {
			node.LastSeenAt = lastSeenAt.String
		}
		out = append(out, node)
	}
	return out, rows.Err()
}

// Node-scoped resource queries

func (r *Repository) ListNetworksByNode(ctx context.Context, nodeID string) ([]models.Network, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, node_id, name, mode, bridge_name, cidr, gateway_ip, is_system_managed, status, created_at FROM networks WHERE node_id = ? ORDER BY created_at ASC`, nodeID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.Network
	for rows.Next() {
		var item models.Network
		var isSystemManaged int
		if err := rows.Scan(&item.ID, &item.NodeID, &item.Name, &item.Mode, &item.BridgeName, &item.CIDR, &item.GatewayIP, &isSystemManaged, &item.Status, &item.CreatedAt); err != nil {
			return nil, err
		}
		item.IsSystemManaged = isSystemManaged == 1
		out = append(out, item)
	}
	return out, rows.Err()
}

func (r *Repository) ListStoragePoolsByNode(ctx context.Context, nodeID string) ([]models.StoragePool, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, node_id, name, pool_type, path, is_default, status, COALESCE(capacity_bytes, 0), COALESCE(allocatable_bytes, 0), created_at FROM storage_pools WHERE node_id = ? ORDER BY created_at ASC`, nodeID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.StoragePool
	for rows.Next() {
		var item models.StoragePool
		var isDefault int
		if err := rows.Scan(&item.ID, &item.NodeID, &item.Name, &item.PoolType, &item.Path, &isDefault, &item.Status, &item.CapacityBytes, &item.AllocatableBytes, &item.CreatedAt); err != nil {
			return nil, err
		}
		item.IsDefault = isDefault == 1
		out = append(out, item)
	}
	return out, rows.Err()
}

func (r *Repository) ListImagesByNode(ctx context.Context, nodeID string) ([]models.Image, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, node_id, name, os_family, architecture, format, source_format, normalized_format, source_url, checksum, local_path, cloud_init_supported, status, created_at FROM images WHERE node_id = ? ORDER BY created_at ASC`, nodeID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.Image
	for rows.Next() {
		var item models.Image
		var cloudInitSupported int
		var checksum sql.NullString
		if err := rows.Scan(&item.ID, &item.NodeID, &item.Name, &item.OSFamily, &item.Architecture, &item.Format, &item.SourceFormat, &item.NormalizedFormat, &item.SourceURL, &checksum, &item.LocalPath, &cloudInitSupported, &item.Status, &item.CreatedAt); err != nil {
			return nil, err
		}
		item.CloudInitSupported = cloudInitSupported == 1
		if checksum.Valid {
			item.Checksum = checksum.String
		}
		out = append(out, item)
	}
	return out, rows.Err()
}

func (r *Repository) ListVMsByNode(ctx context.Context, nodeID string) ([]models.VirtualMachine, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, node_id, name, image_id, storage_pool_id, network_id, desired_state, actual_state, vcpu, memory_mb, disk_path, seed_iso_path, workspace_path, cloud_hypervisor_pid, ip_address, mac_address, console_type, last_error, created_at, updated_at FROM virtual_machines WHERE node_id = ? ORDER BY created_at DESC`, nodeID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.VirtualMachine
	for rows.Next() {
		var vm models.VirtualMachine
		var pid sql.NullInt64
		var seedISO sql.NullString
		var workspace sql.NullString
		var ip sql.NullString
		var mac sql.NullString
		var consoleType sql.NullString
		var lastError sql.NullString
		if err := rows.Scan(&vm.ID, &vm.NodeID, &vm.Name, &vm.ImageID, &vm.StoragePoolID, &vm.NetworkID, &vm.DesiredState, &vm.ActualState, &vm.VCPU, &vm.MemoryMB, &vm.DiskPath, &seedISO, &workspace, &pid, &ip, &mac, &consoleType, &lastError, &vm.CreatedAt, &vm.UpdatedAt); err != nil {
			return nil, err
		}
		if seedISO.Valid {
			vm.SeedISOPath = seedISO.String
		}
		if workspace.Valid {
			vm.WorkspacePath = workspace.String
		}
		if pid.Valid {
			vm.CloudHypervisorPID = int(pid.Int64)
		}
		if ip.Valid {
			vm.IPAddress = ip.String
		}
		if mac.Valid {
			vm.MACAddress = mac.String
		}
		if consoleType.Valid {
			vm.ConsoleType = consoleType.String
		}
		if lastError.Valid {
			vm.LastError = lastError.String
		}
		out = append(out, vm)
	}
	return out, rows.Err()
}

// Count methods for node resources

func (r *Repository) CountVMsByNode(ctx context.Context, nodeID string) (int, error) {
	var count int
	err := r.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM virtual_machines WHERE node_id = ?`, nodeID).Scan(&count)
	return count, err
}

func (r *Repository) CountImagesByNode(ctx context.Context, nodeID string) (int, error) {
	var count int
	err := r.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM images WHERE node_id = ?`, nodeID).Scan(&count)
	return count, err
}

func (r *Repository) CountStoragePoolsByNode(ctx context.Context, nodeID string) (int, error) {
	var count int
	err := r.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM storage_pools WHERE node_id = ?`, nodeID).Scan(&count)
	return count, err
}

func (r *Repository) CountNetworksByNode(ctx context.Context, nodeID string) (int, error) {
	var count int
	err := r.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM networks WHERE node_id = ?`, nodeID).Scan(&count)
	return count, err
}

// CreateVMBootLog stores a boot log entry
func (r *Repository) CreateVMBootLog(ctx context.Context, vmID string, entry *models.VMBootLogEntry) error {
	_, err := r.db.ExecContext(ctx,
		`INSERT INTO vm_boot_logs (id, vm_id, line_number, content, timestamp)
		 VALUES (?, ?, ?, ?, ?)`,
		uuid.NewString(),
		vmID,
		entry.LineNumber,
		entry.Content,
		entry.Timestamp.UTC().Format(time.RFC3339),
	)
	return err
}

// GetVMBootLogs retrieves boot logs for a VM
func (r *Repository) GetVMBootLogs(ctx context.Context, vmID string, limit int) ([]models.VMBootLogEntry, error) {
	var query string
	var rows *sql.Rows
	var err error

	if limit > 0 {
		query = `SELECT line_number, content, timestamp FROM vm_boot_logs 
				 WHERE vm_id = ? ORDER BY line_number DESC LIMIT ?`
		rows, err = r.db.QueryContext(ctx, query, vmID, limit)
	} else {
		query = `SELECT line_number, content, timestamp FROM vm_boot_logs 
				 WHERE vm_id = ? ORDER BY line_number ASC`
		rows, err = r.db.QueryContext(ctx, query, vmID)
	}

	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.VMBootLogEntry
	for rows.Next() {
		var entry models.VMBootLogEntry
		var ts string
		if err := rows.Scan(&entry.LineNumber, &entry.Content, &ts); err != nil {
			return nil, err
		}
		entry.Timestamp, _ = time.Parse(time.RFC3339, ts)
		out = append(out, entry)
	}

	// Reverse if we used DESC order for limit
	if limit > 0 {
		for i, j := 0, len(out)-1; i < j; i, j = i+1, j-1 {
			out[i], out[j] = out[j], out[i]
		}
	}

	return out, rows.Err()
}

// ClearVMBootLogs removes all boot logs for a VM
func (r *Repository) ClearVMBootLogs(ctx context.Context, vmID string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM vm_boot_logs WHERE vm_id = ?`, vmID)
	return err
}

// NodeMetrics methods

// RecordNodeMetrics records metrics for a node
func (r *Repository) RecordNodeMetrics(ctx context.Context, nodeID string, metrics interface {
	GetCPUPercent() float64
	GetMemoryUsedMB() int
	GetMemoryTotalMB() int
	GetDiskUsedGB() int
	GetDiskTotalGB() int
	GetTimestamp() string
}) error {
	_, err := r.db.ExecContext(ctx,
		`INSERT INTO node_metrics (id, node_id, cpu_percent, memory_used_mb, memory_total_mb, disk_used_gb, disk_total_gb, timestamp)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
		uuid.NewString(),
		nodeID,
		metrics.GetCPUPercent(),
		metrics.GetMemoryUsedMB(),
		metrics.GetMemoryTotalMB(),
		metrics.GetDiskUsedGB(),
		metrics.GetDiskTotalGB(),
		metrics.GetTimestamp(),
	)
	return err
}

// GetNodeMetrics retrieves metrics for a node within a time range
func (r *Repository) GetNodeMetrics(ctx context.Context, nodeID string, since string) ([]NodeMetricsRecord, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, node_id, cpu_percent, memory_used_mb, memory_total_mb, disk_used_gb, disk_total_gb, timestamp
		 FROM node_metrics
		 WHERE node_id = ? AND timestamp >= ?
		 ORDER BY timestamp DESC`,
		nodeID, since)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []NodeMetricsRecord
	for rows.Next() {
		var m NodeMetricsRecord
		if err := rows.Scan(&m.ID, &m.NodeID, &m.CPUPercent, &m.MemoryUsedMB, &m.MemoryTotalMB, &m.DiskUsedGB, &m.DiskTotalGB, &m.Timestamp); err != nil {
			return nil, err
		}
		out = append(out, m)
	}
	return out, rows.Err()
}

// GetLatestNodeMetrics retrieves the most recent metrics for a node
func (r *Repository) GetLatestNodeMetrics(ctx context.Context, nodeID string) (*NodeMetricsRecord, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, node_id, cpu_percent, memory_used_mb, memory_total_mb, disk_used_gb, disk_total_gb, timestamp
		 FROM node_metrics
		 WHERE node_id = ?
		 ORDER BY timestamp DESC
		 LIMIT 1`,
		nodeID)

	var m NodeMetricsRecord
	if err := row.Scan(&m.ID, &m.NodeID, &m.CPUPercent, &m.MemoryUsedMB, &m.MemoryTotalMB, &m.DiskUsedGB, &m.DiskTotalGB, &m.Timestamp); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	return &m, nil
}

// NodeMetricsRecord represents a node metrics record
type NodeMetricsRecord struct {
	ID            string
	NodeID        string
	CPUPercent    float64
	MemoryUsedMB  int
	MemoryTotalMB int
	DiskUsedGB    int
	DiskTotalGB   int
	Timestamp     string
}

// GetCPUPercent returns CPU percent for interface compatibility
func (m NodeMetricsRecord) GetCPUPercent() float64 { return m.CPUPercent }

// GetMemoryUsedMB returns memory used MB for interface compatibility
func (m NodeMetricsRecord) GetMemoryUsedMB() int { return m.MemoryUsedMB }

// GetMemoryTotalMB returns memory total MB for interface compatibility
func (m NodeMetricsRecord) GetMemoryTotalMB() int { return m.MemoryTotalMB }

// GetDiskUsedGB returns disk used GB for interface compatibility
func (m NodeMetricsRecord) GetDiskUsedGB() int { return m.DiskUsedGB }

// GetDiskTotalGB returns disk total GB for interface compatibility
func (m NodeMetricsRecord) GetDiskTotalGB() int { return m.DiskTotalGB }

// GetTimestamp returns timestamp for interface compatibility
func (m NodeMetricsRecord) GetTimestamp() string { return m.Timestamp }

// Role repository methods

func (r *Repository) GetRole(ctx context.Context, id string) (*models.Role, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, name, permissions, created_at FROM roles WHERE id = ?`, id)

	var role models.Role
	var permissionsJSON string
	if err := row.Scan(&role.ID, &role.Name, &permissionsJSON, &role.CreatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}

	// Parse permissions JSON
	if err := json.Unmarshal([]byte(permissionsJSON), &role.Permissions); err != nil {
		return nil, err
	}

	return &role, nil
}

func (r *Repository) GetRoleByName(ctx context.Context, name string) (*models.Role, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, name, permissions, created_at FROM roles WHERE name = ?`, name)

	var role models.Role
	var permissionsJSON string
	if err := row.Scan(&role.ID, &role.Name, &permissionsJSON, &role.CreatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}

	// Parse permissions JSON
	if err := json.Unmarshal([]byte(permissionsJSON), &role.Permissions); err != nil {
		return nil, err
	}

	return &role, nil
}

func (r *Repository) ListRoles(ctx context.Context) ([]models.Role, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, name, permissions, created_at FROM roles ORDER BY name`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var roles []models.Role
	for rows.Next() {
		var role models.Role
		var permissionsJSON string
		if err := rows.Scan(&role.ID, &role.Name, &permissionsJSON, &role.CreatedAt); err != nil {
			return nil, err
		}
		if err := json.Unmarshal([]byte(permissionsJSON), &role.Permissions); err != nil {
			return nil, err
		}
		roles = append(roles, role)
	}
	return roles, rows.Err()
}

// Audit log repository methods

func (r *Repository) CreateAuditLog(ctx context.Context, log *models.AuditLog) error {
	_, err := r.db.ExecContext(ctx,
		`INSERT INTO audit_logs (id, user_id, user_name, action, resource_type, resource_id, details, ip_address, success, error, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		log.ID,
		log.UserID,
		log.UserName,
		log.Action,
		log.ResourceType,
		nullable(log.ResourceID),
		nullable(log.Details),
		nullable(log.IPAddress),
		boolInt(log.Success),
		nullable(log.Error),
		log.CreatedAt,
	)
	return err
}

type AuditLogFilters struct {
	UserID       string
	ResourceType string
	ResourceID   string
	Action       string
	From         string
	To           string
	Limit        int
	Offset       int
}

func (r *Repository) ListAuditLogs(ctx context.Context, filters AuditLogFilters) ([]models.AuditLog, error) {
	query := `SELECT id, user_id, user_name, action, resource_type, resource_id, details, ip_address, success, error, created_at FROM audit_logs WHERE 1=1`
	args := []any{}

	if filters.UserID != "" {
		query += ` AND user_id = ?`
		args = append(args, filters.UserID)
	}
	if filters.ResourceType != "" {
		query += ` AND resource_type = ?`
		args = append(args, filters.ResourceType)
	}
	if filters.ResourceID != "" {
		query += ` AND resource_id = ?`
		args = append(args, filters.ResourceID)
	}
	if filters.Action != "" {
		query += ` AND action = ?`
		args = append(args, filters.Action)
	}
	if filters.From != "" {
		query += ` AND created_at >= ?`
		args = append(args, filters.From)
	}
	if filters.To != "" {
		query += ` AND created_at <= ?`
		args = append(args, filters.To)
	}

	query += ` ORDER BY created_at DESC`

	if filters.Limit > 0 {
		query += ` LIMIT ?`
		args = append(args, filters.Limit)
	}
	if filters.Offset > 0 {
		query += ` OFFSET ?`
		args = append(args, filters.Offset)
	}

	rows, err := r.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var logs []models.AuditLog
	for rows.Next() {
		var log models.AuditLog
		var resourceID, details, ipAddress, errStr sql.NullString
		var success int
		if err := rows.Scan(&log.ID, &log.UserID, &log.UserName, &log.Action, &log.ResourceType,
			&resourceID, &details, &ipAddress, &success, &errStr, &log.CreatedAt); err != nil {
			return nil, err
		}
		log.Success = success == 1
		if resourceID.Valid {
			log.ResourceID = resourceID.String
		}
		if details.Valid {
			log.Details = details.String
		}
		if ipAddress.Valid {
			log.IPAddress = ipAddress.String
		}
		if errStr.Valid {
			log.Error = errStr.String
		}
		logs = append(logs, log)
	}
	return logs, rows.Err()
}

// User management repository methods

func (r *Repository) ListUsers(ctx context.Context) ([]models.User, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, username, password_hash, email, role, is_active, last_login_at, created_at, updated_at FROM users ORDER BY created_at`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var users []models.User
	for rows.Next() {
		var user models.User
		var email, lastLoginAt sql.NullString
		var isActive int
		if err := rows.Scan(&user.ID, &user.Username, &user.PasswordHash, &email, &user.Role,
			&isActive, &lastLoginAt, &user.CreatedAt, &user.UpdatedAt); err != nil {
			return nil, err
		}
		user.IsActive = isActive == 1
		if email.Valid {
			user.Email = email.String
		}
		if lastLoginAt.Valid {
			user.LastLoginAt = &lastLoginAt.String
		}
		users = append(users, user)
	}
	return users, rows.Err()
}

func (r *Repository) UpdateUser(ctx context.Context, user *models.User) error {
	_, err := r.db.ExecContext(ctx,
		`UPDATE users SET username = ?, email = ?, role = ?, is_active = ?, updated_at = ? WHERE id = ?`,
		user.Username,
		nullable(user.Email),
		user.Role,
		boolInt(user.IsActive),
		nowUTC(),
		user.ID,
	)
	return err
}

func (r *Repository) UpdateUserPassword(ctx context.Context, userID, passwordHash string) error {
	_, err := r.db.ExecContext(ctx,
		`UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?`,
		passwordHash,
		nowUTC(),
		userID,
	)
	return err
}

func (r *Repository) DeleteUser(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM users WHERE id = ?`, id)
	return err
}

// VM Template repository methods

// CreateVMTemplate creates a new VM template
func (r *Repository) CreateVMTemplate(ctx context.Context, template *models.VMTemplate) error {
	tagsJSON, err := json.Marshal(template.Tags)
	if err != nil {
		return err
	}

	_, err = r.db.ExecContext(ctx,
		`INSERT INTO vm_templates (id, node_id, name, description, vcpu, memory_mb, image_id, network_id, storage_pool_id, cloud_init_config, tags, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		template.ID,
		template.NodeID,
		template.Name,
		nullable(template.Description),
		template.VCPU,
		template.MemoryMB,
		template.ImageID,
		template.NetworkID,
		template.StoragePoolID,
		nullable(template.CloudInitConfig),
		string(tagsJSON),
		template.CreatedAt,
	)
	return err
}

// GetVMTemplate retrieves a VM template by ID
func (r *Repository) GetVMTemplate(ctx context.Context, id string) (*models.VMTemplate, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, node_id, name, description, vcpu, memory_mb, image_id, network_id, storage_pool_id, cloud_init_config, tags, created_at
		 FROM vm_templates WHERE id = ?`, id)

	var t models.VMTemplate
	var description, cloudInitConfig, tagsJSON sql.NullString

	if err := row.Scan(&t.ID, &t.NodeID, &t.Name, &description, &t.VCPU, &t.MemoryMB,
		&t.ImageID, &t.NetworkID, &t.StoragePoolID, &cloudInitConfig, &tagsJSON, &t.CreatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}

	if description.Valid {
		t.Description = description.String
	}
	if cloudInitConfig.Valid {
		t.CloudInitConfig = cloudInitConfig.String
	}
	if tagsJSON.Valid {
		json.Unmarshal([]byte(tagsJSON.String), &t.Tags)
	}

	return &t, nil
}

// ListVMTemplates returns all VM templates for a node
func (r *Repository) ListVMTemplates(ctx context.Context, nodeID string) ([]models.VMTemplate, error) {
	var query string
	var args []any

	if nodeID != "" {
		query = `SELECT id, node_id, name, description, vcpu, memory_mb, image_id, network_id, storage_pool_id, cloud_init_config, tags, created_at
				 FROM vm_templates WHERE node_id = ? ORDER BY created_at DESC`
		args = append(args, nodeID)
	} else {
		query = `SELECT id, node_id, name, description, vcpu, memory_mb, image_id, network_id, storage_pool_id, cloud_init_config, tags, created_at
				 FROM vm_templates ORDER BY created_at DESC`
	}

	rows, err := r.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.VMTemplate
	for rows.Next() {
		var t models.VMTemplate
		var description, cloudInitConfig, tagsJSON sql.NullString

		if err := rows.Scan(&t.ID, &t.NodeID, &t.Name, &description, &t.VCPU, &t.MemoryMB,
			&t.ImageID, &t.NetworkID, &t.StoragePoolID, &cloudInitConfig, &tagsJSON, &t.CreatedAt); err != nil {
			return nil, err
		}

		if description.Valid {
			t.Description = description.String
		}
		if cloudInitConfig.Valid {
			t.CloudInitConfig = cloudInitConfig.String
		}
		if tagsJSON.Valid {
			json.Unmarshal([]byte(tagsJSON.String), &t.Tags)
		}
		out = append(out, t)
	}
	return out, rows.Err()
}

// UpdateVMTemplate updates a VM template
func (r *Repository) UpdateVMTemplate(ctx context.Context, template *models.VMTemplate) error {
	tagsJSON, err := json.Marshal(template.Tags)
	if err != nil {
		return err
	}

	_, err = r.db.ExecContext(ctx,
		`UPDATE vm_templates SET name = ?, description = ?, vcpu = ?, memory_mb = ?, image_id = ?, 
		 network_id = ?, storage_pool_id = ?, cloud_init_config = ?, tags = ?
		 WHERE id = ?`,
		template.Name,
		nullable(template.Description),
		template.VCPU,
		template.MemoryMB,
		template.ImageID,
		template.NetworkID,
		template.StoragePoolID,
		nullable(template.CloudInitConfig),
		string(tagsJSON),
		template.ID,
	)
	return err
}

// DeleteVMTemplate deletes a VM template
func (r *Repository) DeleteVMTemplate(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM vm_templates WHERE id = ?`, id)
	return err
}

// Cloud-init Template repository methods

// CreateCloudInitTemplate creates a new cloud-init template
func (r *Repository) CreateCloudInitTemplate(ctx context.Context, template *models.CloudInitTemplate) error {
	variablesJSON, err := json.Marshal(template.Variables)
	if err != nil {
		return err
	}

	_, err = r.db.ExecContext(ctx,
		`INSERT INTO cloud_init_templates (id, name, description, content, variables, created_at)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		template.ID,
		template.Name,
		nullable(template.Description),
		template.Content,
		string(variablesJSON),
		template.CreatedAt,
	)
	return err
}

// GetCloudInitTemplate retrieves a cloud-init template by ID
func (r *Repository) GetCloudInitTemplate(ctx context.Context, id string) (*models.CloudInitTemplate, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, name, description, content, variables, created_at
		 FROM cloud_init_templates WHERE id = ?`, id)

	var t models.CloudInitTemplate
	var description, variablesJSON sql.NullString

	if err := row.Scan(&t.ID, &t.Name, &description, &t.Content, &variablesJSON, &t.CreatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}

	if description.Valid {
		t.Description = description.String
	}
	if variablesJSON.Valid {
		json.Unmarshal([]byte(variablesJSON.String), &t.Variables)
	}

	return &t, nil
}

// ListCloudInitTemplates returns all cloud-init templates
func (r *Repository) ListCloudInitTemplates(ctx context.Context) ([]models.CloudInitTemplate, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, name, description, content, variables, created_at
		 FROM cloud_init_templates ORDER BY name`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []models.CloudInitTemplate
	for rows.Next() {
		var t models.CloudInitTemplate
		var description, variablesJSON sql.NullString

		if err := rows.Scan(&t.ID, &t.Name, &description, &t.Content, &variablesJSON, &t.CreatedAt); err != nil {
			return nil, err
		}

		if description.Valid {
			t.Description = description.String
		}
		if variablesJSON.Valid {
			json.Unmarshal([]byte(variablesJSON.String), &t.Variables)
		}
		out = append(out, t)
	}
	return out, rows.Err()
}

// DeleteCloudInitTemplate deletes a cloud-init template
func (r *Repository) DeleteCloudInitTemplate(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM cloud_init_templates WHERE id = ?`, id)
	return err
}

// migrateAddVMTemplatesTable creates the vm_templates table if it doesn't exist
func (r *Repository) migrateAddVMTemplatesTable() error {
	var count int
	err := r.db.QueryRow(`SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='vm_templates'`).Scan(&count)
	if err != nil {
		return err
	}

	if count == 0 {
		_, err = r.db.Exec(`
			CREATE TABLE vm_templates (
				id TEXT PRIMARY KEY,
				node_id TEXT NOT NULL,
				name TEXT NOT NULL,
				description TEXT,
				vcpu INTEGER NOT NULL,
				memory_mb INTEGER NOT NULL,
				image_id TEXT NOT NULL,
				network_id TEXT NOT NULL,
				storage_pool_id TEXT NOT NULL,
				cloud_init_config TEXT,
				tags TEXT,
				created_at TEXT NOT NULL,
				FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE,
				FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE RESTRICT,
				FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE RESTRICT,
				FOREIGN KEY(storage_pool_id) REFERENCES storage_pools(id) ON DELETE RESTRICT,
				UNIQUE(node_id, name)
			)
		`)
		if err != nil {
			return fmt.Errorf("failed to create vm_templates table: %w", err)
		}

		_, err = r.db.Exec(`CREATE INDEX idx_vm_templates_node_id ON vm_templates(node_id)`)
		if err != nil {
			return fmt.Errorf("failed to create vm_templates index: %w", err)
		}
	}

	return nil
}

// migrateAddCloudInitTemplatesTable creates the cloud_init_templates table if it doesn't exist
func (r *Repository) migrateAddCloudInitTemplatesTable() error {
	var count int
	err := r.db.QueryRow(`SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='cloud_init_templates'`).Scan(&count)
	if err != nil {
		return err
	}

	if count == 0 {
		_, err = r.db.Exec(`
			CREATE TABLE cloud_init_templates (
				id TEXT PRIMARY KEY,
				name TEXT NOT NULL UNIQUE,
				description TEXT,
				content TEXT NOT NULL,
				variables TEXT,
				created_at TEXT NOT NULL
			)
		`)
		if err != nil {
			return fmt.Errorf("failed to create cloud_init_templates table: %w", err)
		}
	}

	return nil
}

// ensureDefaultCloudInitTemplates inserts default templates if they don't exist
func (r *Repository) ensureDefaultCloudInitTemplates(ctx context.Context) error {
	templates := models.DefaultCloudInitTemplates()
	for _, t := range templates {
		// Check if template exists
		var count int
		err := r.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM cloud_init_templates WHERE id = ?`, t.ID).Scan(&count)
		if err != nil {
			return err
		}

		if count == 0 {
			// Insert the default template
			if err := r.CreateCloudInitTemplate(ctx, &t); err != nil {
				return err
			}
		}
	}
	return nil
}

// DeleteNetwork removes a network from the database
func (r *Repository) DeleteNetwork(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM networks WHERE id = ?`, id)
	return err
}

// VLAN repository methods

func (r *Repository) CreateVLAN(ctx context.Context, vlan *networking.VLANNetwork) error {
	_, err := r.db.ExecContext(ctx,
		`INSERT INTO vlan_networks (id, network_id, vlan_id, name, cidr, gateway_ip, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`,
		vlan.ID, vlan.NetworkID, vlan.VLANID, vlan.Name, vlan.CIDR, vlan.GatewayIP, vlan.CreatedAt,
	)
	return err
}

func (r *Repository) GetVLANByID(ctx context.Context, id string) (*networking.VLANNetwork, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, network_id, vlan_id, name, cidr, gateway_ip, created_at 
		 FROM vlan_networks WHERE id = ?`, id)
	
	var vlan networking.VLANNetwork
	var createdAt sql.NullString
	err := row.Scan(&vlan.ID, &vlan.NetworkID, &vlan.VLANID, &vlan.Name, &vlan.CIDR, &vlan.GatewayIP, &createdAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if createdAt.Valid {
		vlan.CreatedAt = createdAt.String
	}
	return &vlan, nil
}

func (r *Repository) GetVLANByNetworkAndVLANID(ctx context.Context, networkID string, vlanID int) (*networking.VLANNetwork, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, network_id, vlan_id, name, cidr, gateway_ip, created_at 
		 FROM vlan_networks WHERE network_id = ? AND vlan_id = ?`,
		networkID, vlanID)
	
	var vlan networking.VLANNetwork
	var createdAt sql.NullString
	err := row.Scan(&vlan.ID, &vlan.NetworkID, &vlan.VLANID, &vlan.Name, &vlan.CIDR, &vlan.GatewayIP, &createdAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if createdAt.Valid {
		vlan.CreatedAt = createdAt.String
	}
	return &vlan, nil
}

func (r *Repository) DeleteVLAN(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM vlan_networks WHERE id = ?`, id)
	return err
}

func (r *Repository) ListVLANsByNetwork(ctx context.Context, networkID string) ([]networking.VLANNetwork, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, network_id, vlan_id, name, cidr, gateway_ip, created_at 
		 FROM vlan_networks WHERE network_id = ? ORDER BY vlan_id ASC`, networkID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var vlans []networking.VLANNetwork
	for rows.Next() {
		var vlan networking.VLANNetwork
		var createdAt sql.NullString
		if err := rows.Scan(&vlan.ID, &vlan.NetworkID, &vlan.VLANID, &vlan.Name, &vlan.CIDR, &vlan.GatewayIP, &createdAt); err != nil {
			return nil, err
		}
		if createdAt.Valid {
			vlan.CreatedAt = createdAt.String
		}
		vlans = append(vlans, vlan)
	}
	return vlans, rows.Err()
}

// DHCP repository methods

func (r *Repository) CreateDHCPServer(ctx context.Context, server *networking.DHCPServer) error {
	leaseTimeSeconds := int64(server.LeaseTime.Seconds())
	_, err := r.db.ExecContext(ctx,
		`INSERT INTO dhcp_servers (id, network_id, range_start, range_end, lease_time_seconds, is_running, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
		server.ID, server.NetworkID, server.RangeStart, server.RangeEnd, leaseTimeSeconds, boolInt(server.IsRunning),
		server.CreatedAt, server.UpdatedAt,
	)
	return err
}

func (r *Repository) GetDHCPServerByNetwork(ctx context.Context, networkID string) (*networking.DHCPServer, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, network_id, range_start, range_end, lease_time_seconds, is_running, created_at, updated_at
		 FROM dhcp_servers WHERE network_id = ?`, networkID)
	
	var server networking.DHCPServer
	var leaseTimeSeconds int64
	var isRunning int
	err := row.Scan(&server.ID, &server.NetworkID, &server.RangeStart, &server.RangeEnd, &leaseTimeSeconds, 
		&isRunning, &server.CreatedAt, &server.UpdatedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	server.IsRunning = isRunning != 0
	server.LeaseTime = time.Duration(leaseTimeSeconds) * time.Second
	return &server, nil
}

func (r *Repository) UpdateDHCPServer(ctx context.Context, server *networking.DHCPServer) error {
	leaseTimeSeconds := int64(server.LeaseTime.Seconds())
	_, err := r.db.ExecContext(ctx,
		`UPDATE dhcp_servers SET range_start = ?, range_end = ?, lease_time_seconds = ?, is_running = ?, updated_at = ?
		 WHERE id = ?`,
		server.RangeStart, server.RangeEnd, leaseTimeSeconds, boolInt(server.IsRunning), server.UpdatedAt, server.ID,
	)
	return err
}

func (r *Repository) DeleteDHCPServer(ctx context.Context, networkID string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM dhcp_servers WHERE network_id = ?`, networkID)
	return err
}

func (r *Repository) CreateDHCPLease(ctx context.Context, lease *networking.DHCPLease) error {
	_, err := r.db.ExecContext(ctx,
		`INSERT INTO dhcp_leases (id, network_id, mac_address, ip_address, hostname, lease_start, lease_end)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`,
		lease.ID, lease.NetworkID, lease.MACAddress, lease.IPAddress, nullable(lease.Hostname), lease.LeaseStart, lease.LeaseEnd,
	)
	return err
}

func (r *Repository) GetDHCPLeaseByMAC(ctx context.Context, networkID, macAddress string) (*networking.DHCPLease, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, network_id, mac_address, ip_address, hostname, lease_start, lease_end
		 FROM dhcp_leases WHERE network_id = ? AND mac_address = ?`, networkID, macAddress)
	
	return r.scanDHCPLease(row)
}

func (r *Repository) GetDHCPLeaseByIP(ctx context.Context, networkID, ipAddress string) (*networking.DHCPLease, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, network_id, mac_address, ip_address, hostname, lease_start, lease_end
		 FROM dhcp_leases WHERE network_id = ? AND ip_address = ?`, networkID, ipAddress)
	
	return r.scanDHCPLease(row)
}

func (r *Repository) ListDHCPLeases(ctx context.Context, networkID string) ([]networking.DHCPLease, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, network_id, mac_address, ip_address, hostname, lease_start, lease_end
		 FROM dhcp_leases WHERE network_id = ? ORDER BY ip_address ASC`, networkID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var leases []networking.DHCPLease
	for rows.Next() {
		var lease networking.DHCPLease
		var hostname sql.NullString
		err := rows.Scan(&lease.ID, &lease.NetworkID, &lease.MACAddress, &lease.IPAddress, &hostname, &lease.LeaseStart, &lease.LeaseEnd)
		if err != nil {
			return nil, err
		}
		if hostname.Valid {
			lease.Hostname = hostname.String
		}
		leases = append(leases, lease)
	}
	return leases, rows.Err()
}

func (r *Repository) DeleteDHCPLease(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM dhcp_leases WHERE id = ?`, id)
	return err
}

func (r *Repository) UpdateDHCPLease(ctx context.Context, lease *networking.DHCPLease) error {
	_, err := r.db.ExecContext(ctx,
		`UPDATE dhcp_leases SET ip_address = ?, hostname = ?, lease_start = ?, lease_end = ?
		 WHERE id = ?`,
		lease.IPAddress, nullable(lease.Hostname), lease.LeaseStart, lease.LeaseEnd, lease.ID,
	)
	return err
}

func (r *Repository) scanDHCPLease(row *sql.Row) (*networking.DHCPLease, error) {
	var lease networking.DHCPLease
	var hostname sql.NullString
	err := row.Scan(&lease.ID, &lease.NetworkID, &lease.MACAddress, &lease.IPAddress, &hostname, &lease.LeaseStart, &lease.LeaseEnd)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if hostname.Valid {
		lease.Hostname = hostname.String
	}
	return &lease, nil
}

// Firewall repository methods

func (r *Repository) CreateFirewallRule(ctx context.Context, rule *networking.FirewallRule) error {
	_, err := r.db.ExecContext(ctx,
		`INSERT INTO firewall_rules (id, vm_id, direction, protocol, port_range, source_cidr, action, priority, description, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		rule.ID, rule.VMID, rule.Direction, rule.Protocol, nullable(rule.PortRange), 
		rule.SourceCIDR, rule.Action, rule.Priority, nullable(rule.Description), rule.CreatedAt,
	)
	return err
}

func (r *Repository) GetFirewallRuleByID(ctx context.Context, id string) (*networking.FirewallRule, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, vm_id, direction, protocol, port_range, source_cidr, action, priority, description, created_at
		 FROM firewall_rules WHERE id = ?`, id)
	
	return r.scanFirewallRule(row)
}

func (r *Repository) ListFirewallRulesByVM(ctx context.Context, vmID string) ([]networking.FirewallRule, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, vm_id, direction, protocol, port_range, source_cidr, action, priority, description, created_at
		 FROM firewall_rules WHERE vm_id = ? ORDER BY priority ASC`, vmID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var rules []networking.FirewallRule
	for rows.Next() {
		var rule networking.FirewallRule
		var portRange, description sql.NullString
		err := rows.Scan(&rule.ID, &rule.VMID, &rule.Direction, &rule.Protocol, &portRange, 
			&rule.SourceCIDR, &rule.Action, &rule.Priority, &description, &rule.CreatedAt)
		if err != nil {
			return nil, err
		}
		if portRange.Valid {
			rule.PortRange = portRange.String
		}
		if description.Valid {
			rule.Description = description.String
		}
		rules = append(rules, rule)
	}
	return rules, rows.Err()
}

func (r *Repository) UpdateFirewallRule(ctx context.Context, rule *networking.FirewallRule) error {
	_, err := r.db.ExecContext(ctx,
		`UPDATE firewall_rules SET direction = ?, protocol = ?, port_range = ?, source_cidr = ?, action = ?, priority = ?, description = ?
		 WHERE id = ?`,
		rule.Direction, rule.Protocol, nullable(rule.PortRange), rule.SourceCIDR, 
		rule.Action, rule.Priority, nullable(rule.Description), rule.ID,
	)
	return err
}

func (r *Repository) DeleteFirewallRule(ctx context.Context, id string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM firewall_rules WHERE id = ?`, id)
	return err
}

func (r *Repository) DeleteFirewallRulesByVM(ctx context.Context, vmID string) error {
	_, err := r.db.ExecContext(ctx, `DELETE FROM firewall_rules WHERE vm_id = ?`, vmID)
	return err
}

func (r *Repository) scanFirewallRule(row *sql.Row) (*networking.FirewallRule, error) {
	var rule networking.FirewallRule
	var portRange, description sql.NullString
	err := row.Scan(&rule.ID, &rule.VMID, &rule.Direction, &rule.Protocol, &portRange, 
		&rule.SourceCIDR, &rule.Action, &rule.Priority, &description, &rule.CreatedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if portRange.Valid {
		rule.PortRange = portRange.String
	}
	if description.Valid {
		rule.Description = description.String
	}
	return &rule, nil
}




// Type definitions for networking


// Quota repository methods

// CreateQuota creates a quota for a user
func (r *Repository) CreateQuota(ctx context.Context, userID string, quota *models.Quota) error {
	now := time.Now().UTC().Format(time.RFC3339)
	_, err := r.db.ExecContext(ctx,
		`INSERT INTO quotas (id, user_id, max_vms, max_cpu, max_memory_gb, max_storage_gb, max_networks, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		quota.ID, userID, quota.MaxVMs, quota.MaxCPUs, quota.MaxMemoryGB, quota.MaxStorageGB, quota.MaxNetworks, now, now,
	)
	return err
}

// GetQuota retrieves a user's quota
func (r *Repository) GetQuota(ctx context.Context, userID string) (*models.Quota, error) {
	row := r.db.QueryRowContext(ctx,
		`SELECT id, user_id, max_vms, max_cpu, max_memory_gb, max_storage_gb, max_networks, created_at, updated_at 
		 FROM quotas WHERE user_id = ?`, userID)

	var q models.Quota
	var createdAt, updatedAt string
	err := row.Scan(&q.ID, &q.UserID, &q.MaxVMs, &q.MaxCPUs, &q.MaxMemoryGB, &q.MaxStorageGB, &q.MaxNetworks, &createdAt, &updatedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &q, nil
}

// UpsertQuota creates or updates a user's quota
func (r *Repository) UpsertQuota(ctx context.Context, q *models.Quota) error {
	existing, err := r.GetQuota(ctx, q.UserID)
	if err != nil {
		return err
	}

	now := time.Now().UTC().Format(time.RFC3339)
	if existing == nil {
		// Create new
		if q.ID == "" {
			q.ID = uuid.NewString()
		}
		return r.CreateQuota(ctx, q.UserID, q)
	}

	// Update existing
	_, err = r.db.ExecContext(ctx,
		`UPDATE quotas SET max_vms = ?, max_cpu = ?, max_memory_gb = ?, max_storage_gb = ?, max_networks = ?, updated_at = ? WHERE user_id = ?`,
		q.MaxVMs, q.MaxCPUs, q.MaxMemoryGB, q.MaxStorageGB, q.MaxNetworks, now, q.UserID,
	)
	return err
}

// GetUserUsage retrieves resource usage for a user
func (r *Repository) GetUserUsage(ctx context.Context, userID string) (*models.ResourceUsage, error) {
	// Count VMs owned by user
	var vmCount int
	err := r.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM vms WHERE owner_id = ?`, userID).Scan(&vmCount)
	if err != nil {
		return nil, err
	}

	// Sum CPU cores
	var totalCPUs int
	err = r.db.QueryRowContext(ctx, `SELECT COALESCE(SUM(vcpu), 0) FROM vms WHERE owner_id = ?`, userID).Scan(&totalCPUs)
	if err != nil {
		return nil, err
	}

	// Sum memory
	var totalMemoryMB int
	err = r.db.QueryRowContext(ctx, `SELECT COALESCE(SUM(memory_mb), 0) FROM vms WHERE owner_id = ?`, userID).Scan(&totalMemoryMB)
	if err != nil {
		return nil, err
	}

	// Sum storage
	var totalStorageMB int
	err = r.db.QueryRowContext(ctx, `SELECT COALESCE(SUM(disk_gb), 0) FROM vms WHERE owner_id = ?`, userID).Scan(&totalStorageMB)
	if err != nil {
		return nil, err
	}

	// Count networks (no per-user ownership yet, so count all)
	var networkCount int
	err = r.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM networks`).Scan(&networkCount)
	if err != nil {
		return nil, err
	}

	return &models.ResourceUsage{
		VMs:        vmCount,
		CPUs:       totalCPUs,
		MemoryGB:   totalMemoryMB / 1024,
		StorageGB:  totalStorageMB,
		Networks:   networkCount,
	}, nil
}

// ListQuotas retrieves all quotas
func (r *Repository) ListQuotas(ctx context.Context) ([]models.Quota, error) {
	rows, err := r.db.QueryContext(ctx,
		`SELECT id, user_id, max_vms, max_cpu, max_memory_gb, max_storage_gb, max_networks, created_at, updated_at 
		 FROM quotas ORDER BY created_at DESC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var quotas []models.Quota
	for rows.Next() {
		var q models.Quota
		var createdAt, updatedAt string
		if err := rows.Scan(&q.ID, &q.UserID, &q.MaxVMs, &q.MaxCPUs, &q.MaxMemoryGB, &q.MaxStorageGB, &q.MaxNetworks, &createdAt, &updatedAt); err != nil {
			return nil, err
		}
		quotas = append(quotas, q)
	}
	return quotas, rows.Err()
}

// UpdateQuota updates a quota
func (r *Repository) UpdateQuota(ctx context.Context, quota *models.Quota) error {
	now := time.Now().UTC().Format(time.RFC3339)
	_, err := r.db.ExecContext(ctx,
		`UPDATE quotas SET max_vms = ?, max_cpu = ?, max_memory_gb = ?, max_storage_gb = ?, max_networks = ?, updated_at = ? WHERE id = ?`,
		quota.MaxVMs, quota.MaxCPUs, quota.MaxMemoryGB, quota.MaxStorageGB, quota.MaxNetworks, now, quota.ID,
	)
	return err
}

// RefreshUserUsageCache refreshes the cached usage data for a user
func (r *Repository) RefreshUserUsageCache(ctx context.Context, userID string) error {
	// In this implementation, we don't cache usage - calculate fresh each time
	// This method exists for interface compatibility with cached implementations
	return nil
}


// Phase 3 Migration Functions

// migrateAddQuotasTable creates the quotas table if it doesn't exist
func (r *Repository) migrateAddQuotasTable() error {
	_, err := r.db.Exec(`
		CREATE TABLE IF NOT EXISTS quotas (
			id TEXT PRIMARY KEY,
			user_id TEXT NOT NULL UNIQUE,
			max_vms INTEGER NOT NULL DEFAULT 10,
			max_cpu INTEGER NOT NULL DEFAULT 20,
			max_memory_gb INTEGER NOT NULL DEFAULT 64,
			max_storage_gb INTEGER NOT NULL DEFAULT 500,
			max_networks INTEGER NOT NULL DEFAULT 5,
			created_at TEXT NOT NULL,
			updated_at TEXT NOT NULL,
			FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
		)
	`)
	if err != nil {
		return err
	}
	_, err = r.db.Exec(`CREATE INDEX IF NOT EXISTS idx_quotas_user_id ON quotas(user_id)`)
	return err
}

// migrateAddUsageCacheTable creates the usage_cache table if it doesn't exist
func (r *Repository) migrateAddUsageCacheTable() error {
	_, err := r.db.Exec(`
		CREATE TABLE IF NOT EXISTS usage_cache (
			user_id TEXT PRIMARY KEY,
			vms INTEGER NOT NULL DEFAULT 0,
			cpu INTEGER NOT NULL DEFAULT 0,
			memory_gb INTEGER NOT NULL DEFAULT 0,
			storage_gb INTEGER NOT NULL DEFAULT 0,
			networks INTEGER NOT NULL DEFAULT 0,
			updated_at TEXT NOT NULL,
			FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
		)
	`)
	if err != nil {
		return err
	}
	_, err = r.db.Exec(`CREATE INDEX IF NOT EXISTS idx_usage_cache_user_id ON usage_cache(user_id)`)
	return err
}

// migrateAddVLANNetworksTable creates the vlan_networks table if it doesn't exist
func (r *Repository) migrateAddVLANNetworksTable() error {
	_, err := r.db.Exec(`
		CREATE TABLE IF NOT EXISTS vlan_networks (
			id TEXT PRIMARY KEY,
			network_id TEXT NOT NULL,
			vlan_id INTEGER NOT NULL,
			name TEXT NOT NULL,
			cidr TEXT NOT NULL,
			gateway_ip TEXT NOT NULL,
			created_at TEXT,
			FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE CASCADE,
			UNIQUE(network_id, vlan_id)
		)
	`)
	if err != nil {
		return err
	}
	_, err = r.db.Exec(`CREATE INDEX IF NOT EXISTS idx_vlan_networks_network_id ON vlan_networks(network_id)`)
	return err
}

// migrateAddDHCPServersTable creates the dhcp_servers table if it doesn't exist
func (r *Repository) migrateAddDHCPServersTable() error {
	_, err := r.db.Exec(`
		CREATE TABLE IF NOT EXISTS dhcp_servers (
			id TEXT PRIMARY KEY,
			network_id TEXT NOT NULL UNIQUE,
			range_start TEXT NOT NULL,
			range_end TEXT NOT NULL,
			lease_time_seconds INTEGER NOT NULL DEFAULT 3600,
			is_running INTEGER NOT NULL DEFAULT 0,
			created_at TEXT NOT NULL,
			updated_at TEXT NOT NULL,
			FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE CASCADE
		)
	`)
	if err != nil {
		return err
	}
	_, err = r.db.Exec(`CREATE INDEX IF NOT EXISTS idx_dhcp_servers_network_id ON dhcp_servers(network_id)`)
	return err
}

// migrateAddDHCPLeasesTable creates the dhcp_leases table if it doesn't exist
func (r *Repository) migrateAddDHCPLeasesTable() error {
	_, err := r.db.Exec(`
		CREATE TABLE IF NOT EXISTS dhcp_leases (
			id TEXT PRIMARY KEY,
			network_id TEXT NOT NULL,
			mac_address TEXT NOT NULL,
			ip_address TEXT NOT NULL,
			hostname TEXT,
			lease_start TEXT NOT NULL,
			lease_end TEXT NOT NULL,
			FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE CASCADE,
			UNIQUE(network_id, mac_address)
		)
	`)
	if err != nil {
		return err
	}
	_, err = r.db.Exec(`CREATE INDEX IF NOT EXISTS idx_dhcp_leases_network_id ON dhcp_leases(network_id)`)
	return err
}

// migrateAddFirewallRulesTable creates the firewall_rules table if it doesn't exist
func (r *Repository) migrateAddFirewallRulesTable() error {
	_, err := r.db.Exec(`
		CREATE TABLE IF NOT EXISTS firewall_rules (
			id TEXT PRIMARY KEY,
			vm_id TEXT NOT NULL,
			direction TEXT NOT NULL,
			protocol TEXT NOT NULL,
			port_range TEXT,
			source_cidr TEXT NOT NULL,
			action TEXT NOT NULL,
			priority INTEGER NOT NULL,
			description TEXT,
			created_at TEXT NOT NULL,
			FOREIGN KEY(vm_id) REFERENCES vms(id) ON DELETE CASCADE
		)
	`)
	if err != nil {
		return err
	}
	_, err = r.db.Exec(`CREATE INDEX IF NOT EXISTS idx_firewall_rules_vm_id ON firewall_rules(vm_id)`)
	return err
}

// migrateAddBackupJobsTable creates the backup_jobs table if it doesn't exist
func (r *Repository) migrateAddBackupJobsTable() error {
	_, err := r.db.Exec(`
		CREATE TABLE IF NOT EXISTS backup_jobs (
			id TEXT PRIMARY KEY,
			vm_id TEXT NOT NULL,
			name TEXT NOT NULL,
			schedule TEXT NOT NULL,
			retention INTEGER NOT NULL DEFAULT 7,
			destination TEXT NOT NULL,
			enabled INTEGER NOT NULL DEFAULT 1,
			created_at TEXT NOT NULL,
			updated_at TEXT NOT NULL,
			FOREIGN KEY(vm_id) REFERENCES vms(id) ON DELETE CASCADE
		)
	`)
	if err != nil {
		return err
	}
	_, err = r.db.Exec(`CREATE INDEX IF NOT EXISTS idx_backup_jobs_vm_id ON backup_jobs(vm_id)`)
	return err
}

// migrateAddBackupHistoryTable creates the backup_history table if it doesn't exist
func (r *Repository) migrateAddBackupHistoryTable() error {
	_, err := r.db.Exec(`
		CREATE TABLE IF NOT EXISTS backup_history (
			id TEXT PRIMARY KEY,
			job_id TEXT,
			vm_id TEXT NOT NULL,
			snapshot_id TEXT,
			status TEXT NOT NULL,
			size_bytes INTEGER,
			started_at TEXT NOT NULL,
			completed_at TEXT,
			error_message TEXT,
			FOREIGN KEY(job_id) REFERENCES backup_jobs(id) ON DELETE SET NULL,
			FOREIGN KEY(vm_id) REFERENCES vms(id) ON DELETE CASCADE
		)
	`)
	if err != nil {
		return err
	}
	_, err = r.db.Exec(`CREATE INDEX IF NOT EXISTS idx_backup_history_job_id ON backup_history(job_id)`)
	if err != nil {
		return err
	}
	_, err = r.db.Exec(`CREATE INDEX IF NOT EXISTS idx_backup_history_vm_id ON backup_history(vm_id)`)
	return err
}

// migrateAddNodeMetricsTable creates the node_metrics table if it doesn't exist
func (r *Repository) migrateAddNodeMetricsTable() error {
	_, err := r.db.Exec(`
		CREATE TABLE IF NOT EXISTS node_metrics (
			id TEXT PRIMARY KEY,
			node_id TEXT NOT NULL,
			cpu_percent REAL NOT NULL,
			memory_used_mb INTEGER NOT NULL,
			memory_total_mb INTEGER NOT NULL,
			disk_used_gb INTEGER NOT NULL,
			disk_total_gb INTEGER NOT NULL,
			timestamp TEXT NOT NULL,
			FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE
		)
	`)
	if err != nil {
		return err
	}
	_, err = r.db.Exec(`CREATE INDEX IF NOT EXISTS idx_node_metrics_node_id ON node_metrics(node_id)`)
	if err != nil {
		return err
	}
	_, err = r.db.Exec(`CREATE INDEX IF NOT EXISTS idx_node_metrics_timestamp ON node_metrics(timestamp)`)
	return err
}


// migrateAddUserIDToVMs adds user_id column to virtual_machines table
func (r *Repository) migrateAddUserIDToVMs() error {
	// Check if column exists
	var count int
	err := r.db.QueryRow(`SELECT COUNT(*) FROM pragma_table_info('virtual_machines') WHERE name = 'user_id'`).Scan(&count)
	if err != nil {
		return err
	}
	if count == 0 {
		_, err = r.db.Exec(`ALTER TABLE virtual_machines ADD COLUMN user_id TEXT NULL`)
		if err != nil {
			return fmt.Errorf("failed to add user_id column: %w", err)
		}
	}
	// Create index if it doesn't exist
	_, err = r.db.Exec(`CREATE INDEX IF NOT EXISTS idx_vms_user_id ON virtual_machines(user_id)`)
	return err
}

// migrateAddConsoleTypeToVMs adds console_type column to virtual_machines table
func (r *Repository) migrateAddConsoleTypeToVMs() error {
	// Check if column exists
	var count int
	err := r.db.QueryRow(`SELECT COUNT(*) FROM pragma_table_info('virtual_machines') WHERE name = 'console_type'`).Scan(&count)
	if err != nil {
		return err
	}
	if count == 0 {
		_, err = r.db.Exec(`ALTER TABLE virtual_machines ADD COLUMN console_type TEXT NULL DEFAULT 'pty'`)
		if err != nil {
			return fmt.Errorf("failed to add console_type column: %w", err)
		}
	}
	return nil
}
