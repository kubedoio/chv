package store

import (
	"context"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
)

// Network operations
func (s *PostgresStore) CreateNetwork(ctx context.Context, network *models.Network) error {
	return createNetwork(ctx, s.pool, network)
}

func (s *PostgresStore) GetNetwork(ctx context.Context, id uuid.UUID) (*models.Network, error) {
	return getNetwork(ctx, s.pool, id)
}

func (s *PostgresStore) ListNetworks(ctx context.Context) ([]*models.Network, error) {
	return listNetworks(ctx, s.pool)
}

func createNetwork(ctx context.Context, q querier, network *models.Network) error {
	sql := `INSERT INTO networks (id, name, bridge_name, cidr, gateway_ip, dns_servers, mtu, mode, status, created_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)`
	_, err := q.Exec(ctx, sql, network.ID, network.Name, network.BridgeName, network.CIDR,
		network.GatewayIP, network.DNSServers, network.MTU, network.Mode, network.Status, network.CreatedAt)
	return err
}

func getNetwork(ctx context.Context, q querier, id uuid.UUID) (*models.Network, error) {
	sql := `SELECT id, name, bridge_name, cidr, gateway_ip, dns_servers, mtu, mode, status, created_at
		FROM networks WHERE id = $1`
	n := &models.Network{}
	err := q.QueryRow(ctx, sql, id).Scan(&n.ID, &n.Name, &n.BridgeName, &n.CIDR, &n.GatewayIP,
		&n.DNSServers, &n.MTU, &n.Mode, &n.Status, &n.CreatedAt)
	if err == pgx.ErrNoRows {
		return nil, nil
	}
	return n, err
}

func listNetworks(ctx context.Context, q querier) ([]*models.Network, error) {
	sql := `SELECT id, name, bridge_name, cidr, gateway_ip, dns_servers, mtu, mode, status, created_at
		FROM networks ORDER BY name`
	rows, err := q.Query(ctx, sql)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	
	var networks []*models.Network
	for rows.Next() {
		n := &models.Network{}
		if err := rows.Scan(&n.ID, &n.Name, &n.BridgeName, &n.CIDR, &n.GatewayIP,
			&n.DNSServers, &n.MTU, &n.Mode, &n.Status, &n.CreatedAt); err != nil {
			return nil, err
		}
		networks = append(networks, n)
	}
	return networks, rows.Err()
}

// Storage Pool operations
func (s *PostgresStore) CreateStoragePool(ctx context.Context, pool *models.StoragePool) error {
	return createStoragePool(ctx, s.pool, pool)
}

func (s *PostgresStore) GetStoragePool(ctx context.Context, id uuid.UUID) (*models.StoragePool, error) {
	return getStoragePool(ctx, s.pool, id)
}

func (s *PostgresStore) ListStoragePools(ctx context.Context) ([]*models.StoragePool, error) {
	return listStoragePools(ctx, s.pool)
}

func (s *PostgresStore) ListStoragePoolsByNode(ctx context.Context, nodeID uuid.UUID) ([]*models.StoragePool, error) {
	return listStoragePoolsByNode(ctx, s.pool, nodeID)
}

func createStoragePool(ctx context.Context, q querier, pool *models.StoragePool) error {
	sql := `INSERT INTO storage_pools (id, node_id, name, pool_type, path_or_export, capacity_bytes,
		allocatable_bytes, status, supports_online_resize, supports_clone, supports_snapshot, created_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)`
	_, err := q.Exec(ctx, sql, pool.ID, pool.NodeID, pool.Name, pool.PoolType, pool.PathOrExport,
		pool.CapacityBytes, pool.AllocatableBytes, pool.Status, pool.SupportsOnlineResize,
		pool.SupportsClone, pool.SupportsSnapshot, pool.CreatedAt)
	return err
}

func getStoragePool(ctx context.Context, q querier, id uuid.UUID) (*models.StoragePool, error) {
	sql := `SELECT id, node_id, name, pool_type, path_or_export, capacity_bytes,
		allocatable_bytes, status, supports_online_resize, supports_clone, supports_snapshot, created_at
		FROM storage_pools WHERE id = $1`
	p := &models.StoragePool{}
	err := q.QueryRow(ctx, sql, id).Scan(&p.ID, &p.NodeID, &p.Name, &p.PoolType, &p.PathOrExport,
		&p.CapacityBytes, &p.AllocatableBytes, &p.Status, &p.SupportsOnlineResize,
		&p.SupportsClone, &p.SupportsSnapshot, &p.CreatedAt)
	if err == pgx.ErrNoRows {
		return nil, nil
	}
	return p, err
}

func listStoragePools(ctx context.Context, q querier) ([]*models.StoragePool, error) {
	sql := `SELECT id, node_id, name, pool_type, path_or_export, capacity_bytes,
		allocatable_bytes, status, supports_online_resize, supports_clone, supports_snapshot, created_at
		FROM storage_pools ORDER BY name`
	rows, err := q.Query(ctx, sql)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	
	var pools []*models.StoragePool
	for rows.Next() {
		p := &models.StoragePool{}
		if err := rows.Scan(&p.ID, &p.NodeID, &p.Name, &p.PoolType, &p.PathOrExport,
			&p.CapacityBytes, &p.AllocatableBytes, &p.Status, &p.SupportsOnlineResize,
			&p.SupportsClone, &p.SupportsSnapshot, &p.CreatedAt); err != nil {
			return nil, err
		}
		pools = append(pools, p)
	}
	return pools, rows.Err()
}

func listStoragePoolsByNode(ctx context.Context, q querier, nodeID uuid.UUID) ([]*models.StoragePool, error) {
	sql := `SELECT id, node_id, name, pool_type, path_or_export, capacity_bytes,
		allocatable_bytes, status, supports_online_resize, supports_clone, supports_snapshot, created_at
		FROM storage_pools WHERE node_id = $1 ORDER BY name`
	rows, err := q.Query(ctx, sql, nodeID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	
	var pools []*models.StoragePool
	for rows.Next() {
		p := &models.StoragePool{}
		if err := rows.Scan(&p.ID, &p.NodeID, &p.Name, &p.PoolType, &p.PathOrExport,
			&p.CapacityBytes, &p.AllocatableBytes, &p.Status, &p.SupportsOnlineResize,
			&p.SupportsClone, &p.SupportsSnapshot, &p.CreatedAt); err != nil {
			return nil, err
		}
		pools = append(pools, p)
	}
	return pools, rows.Err()
}

// Image operations
func (s *PostgresStore) CreateImage(ctx context.Context, image *models.Image) error {
	return createImage(ctx, s.pool, image)
}

func (s *PostgresStore) GetImage(ctx context.Context, id uuid.UUID) (*models.Image, error) {
	return getImage(ctx, s.pool, id)
}

func (s *PostgresStore) UpdateImage(ctx context.Context, image *models.Image) error {
	return updateImage(ctx, s.pool, image)
}

func (s *PostgresStore) ListImages(ctx context.Context) ([]*models.Image, error) {
	return listImages(ctx, s.pool)
}

func createImage(ctx context.Context, q querier, image *models.Image) error {
	sql := `INSERT INTO images (id, name, os_family, source_format, normalized_format, architecture,
		cloud_init_supported, default_username, checksum, status, metadata, created_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)`
	_, err := q.Exec(ctx, sql, image.ID, image.Name, image.OSFamily, image.SourceFormat,
		image.NormalizedFormat, image.Architecture, image.CloudInitSupported, image.DefaultUsername,
		image.Checksum, image.Status, image.Metadata, image.CreatedAt)
	return err
}

func getImage(ctx context.Context, q querier, id uuid.UUID) (*models.Image, error) {
	sql := `SELECT id, name, os_family, source_format, normalized_format, architecture,
		cloud_init_supported, default_username, checksum, status, metadata, created_at
		FROM images WHERE id = $1`
	i := &models.Image{}
	err := q.QueryRow(ctx, sql, id).Scan(&i.ID, &i.Name, &i.OSFamily, &i.SourceFormat,
		&i.NormalizedFormat, &i.Architecture, &i.CloudInitSupported, &i.DefaultUsername,
		&i.Checksum, &i.Status, &i.Metadata, &i.CreatedAt)
	if err == pgx.ErrNoRows {
		return nil, nil
	}
	return i, err
}

func updateImage(ctx context.Context, q querier, image *models.Image) error {
	sql := `UPDATE images SET name = $2, os_family = $3, source_format = $4, normalized_format = $5,
		architecture = $6, cloud_init_supported = $7, default_username = $8, checksum = $9,
		status = $10, metadata = $11 WHERE id = $1`
	_, err := q.Exec(ctx, sql, image.ID, image.Name, image.OSFamily, image.SourceFormat,
		image.NormalizedFormat, image.Architecture, image.CloudInitSupported, image.DefaultUsername,
		image.Checksum, image.Status, image.Metadata)
	return err
}

func listImages(ctx context.Context, q querier) ([]*models.Image, error) {
	sql := `SELECT id, name, os_family, source_format, normalized_format, architecture,
		cloud_init_supported, default_username, checksum, status, metadata, created_at
		FROM images ORDER BY name`
	rows, err := q.Query(ctx, sql)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	
	var images []*models.Image
	for rows.Next() {
		i := &models.Image{}
		if err := rows.Scan(&i.ID, &i.Name, &i.OSFamily, &i.SourceFormat,
			&i.NormalizedFormat, &i.Architecture, &i.CloudInitSupported, &i.DefaultUsername,
			&i.Checksum, &i.Status, &i.Metadata, &i.CreatedAt); err != nil {
			return nil, err
		}
		images = append(images, i)
	}
	return images, rows.Err()
}

// Volume operations
func (s *PostgresStore) CreateVolume(ctx context.Context, volume *models.Volume) error {
	return createVolume(ctx, s.pool, volume)
}

func (s *PostgresStore) GetVolume(ctx context.Context, id uuid.UUID) (*models.Volume, error) {
	return getVolume(ctx, s.pool, id)
}

func (s *PostgresStore) UpdateVolume(ctx context.Context, volume *models.Volume) error {
	return updateVolume(ctx, s.pool, volume)
}

func (s *PostgresStore) ListVolumesByVM(ctx context.Context, vmID uuid.UUID) ([]*models.Volume, error) {
	return listVolumesByVM(ctx, s.pool, vmID)
}

func createVolume(ctx context.Context, q querier, volume *models.Volume) error {
	sql := `INSERT INTO volumes (id, vm_id, pool_id, backing_image_id, format, size_bytes, path,
		attachment_state, resize_state, metadata, created_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)`
	_, err := q.Exec(ctx, sql, volume.ID, volume.VMID, volume.PoolID, volume.BackingImageID,
		volume.Format, volume.SizeBytes, volume.Path, volume.AttachmentState, volume.ResizeState,
		volume.Metadata, volume.CreatedAt)
	return err
}

func getVolume(ctx context.Context, q querier, id uuid.UUID) (*models.Volume, error) {
	sql := `SELECT id, vm_id, pool_id, backing_image_id, format, size_bytes, path,
		attachment_state, resize_state, metadata, created_at FROM volumes WHERE id = $1`
	v := &models.Volume{}
	err := q.QueryRow(ctx, sql, id).Scan(&v.ID, &v.VMID, &v.PoolID, &v.BackingImageID,
		&v.Format, &v.SizeBytes, &v.Path, &v.AttachmentState, &v.ResizeState, &v.Metadata, &v.CreatedAt)
	if err == pgx.ErrNoRows {
		return nil, nil
	}
	return v, err
}

func updateVolume(ctx context.Context, q querier, volume *models.Volume) error {
	sql := `UPDATE volumes SET vm_id = $2, pool_id = $3, backing_image_id = $4, format = $5,
		size_bytes = $6, path = $7, attachment_state = $8, resize_state = $9, metadata = $10
		WHERE id = $1`
	_, err := q.Exec(ctx, sql, volume.ID, volume.VMID, volume.PoolID, volume.BackingImageID,
		volume.Format, volume.SizeBytes, volume.Path, volume.AttachmentState, volume.ResizeState, volume.Metadata)
	return err
}

func listVolumesByVM(ctx context.Context, q querier, vmID uuid.UUID) ([]*models.Volume, error) {
	sql := `SELECT id, vm_id, pool_id, backing_image_id, format, size_bytes, path,
		attachment_state, resize_state, metadata, created_at FROM volumes WHERE vm_id = $1`
	rows, err := q.Query(ctx, sql, vmID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	
	var volumes []*models.Volume
	for rows.Next() {
		v := &models.Volume{}
		if err := rows.Scan(&v.ID, &v.VMID, &v.PoolID, &v.BackingImageID,
			&v.Format, &v.SizeBytes, &v.Path, &v.AttachmentState, &v.ResizeState, &v.Metadata, &v.CreatedAt); err != nil {
			return nil, err
		}
		volumes = append(volumes, v)
	}
	return volumes, rows.Err()
}

// VM Network Attachment operations
func (s *PostgresStore) CreateVMNetworkAttachment(ctx context.Context, attachment *models.VMNetworkAttachment) error {
	return createVMNetworkAttachment(ctx, s.pool, attachment)
}

func (s *PostgresStore) ListVMNetworkAttachments(ctx context.Context, vmID uuid.UUID) ([]*models.VMNetworkAttachment, error) {
	return listVMNetworkAttachments(ctx, s.pool, vmID)
}

func createVMNetworkAttachment(ctx context.Context, q querier, a *models.VMNetworkAttachment) error {
	sql := `INSERT INTO vm_network_attachments (id, vm_id, network_id, mac_address, ip_address, nic_index, created_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7)`
	_, err := q.Exec(ctx, sql, a.ID, a.VMID, a.NetworkID, a.MACAddress, a.IPAddress, a.NICIndex, a.CreatedAt)
	return err
}

func listVMNetworkAttachments(ctx context.Context, q querier, vmID uuid.UUID) ([]*models.VMNetworkAttachment, error) {
	sql := `SELECT id, vm_id, network_id, mac_address, ip_address, nic_index, created_at
		FROM vm_network_attachments WHERE vm_id = $1 ORDER BY nic_index`
	rows, err := q.Query(ctx, sql, vmID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	
	var attachments []*models.VMNetworkAttachment
	for rows.Next() {
		a := &models.VMNetworkAttachment{}
		if err := rows.Scan(&a.ID, &a.VMID, &a.NetworkID, &a.MACAddress, &a.IPAddress, &a.NICIndex, &a.CreatedAt); err != nil {
			return nil, err
		}
		attachments = append(attachments, a)
	}
	return attachments, rows.Err()
}

// API Token operations
func (s *PostgresStore) CreateAPIToken(ctx context.Context, token *models.APIToken) error {
	return createAPIToken(ctx, s.pool, token)
}

func (s *PostgresStore) GetAPITokenByHash(ctx context.Context, hash string) (*models.APIToken, error) {
	return getAPITokenByHash(ctx, s.pool, hash)
}

func (s *PostgresStore) RevokeAPIToken(ctx context.Context, id uuid.UUID) error {
	return revokeAPIToken(ctx, s.pool, id)
}

func createAPIToken(ctx context.Context, q querier, token *models.APIToken) error {
	sql := `INSERT INTO api_tokens (id, name, token_hash, role_id, expires_at, revoked_at, created_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7)`
	_, err := q.Exec(ctx, sql, token.ID, token.Name, token.TokenHash, token.RoleID,
		token.ExpiresAt, token.RevokedAt, token.CreatedAt)
	return err
}

func getAPITokenByHash(ctx context.Context, q querier, hash string) (*models.APIToken, error) {
	sql := `SELECT id, name, token_hash, role_id, expires_at, revoked_at, created_at
		FROM api_tokens WHERE token_hash = $1`
	t := &models.APIToken{}
	err := q.QueryRow(ctx, sql, hash).Scan(&t.ID, &t.Name, &t.TokenHash, &t.RoleID,
		&t.ExpiresAt, &t.RevokedAt, &t.CreatedAt)
	if err == pgx.ErrNoRows {
		return nil, nil
	}
	return t, err
}

func revokeAPIToken(ctx context.Context, q querier, id uuid.UUID) error {
	sql := `UPDATE api_tokens SET revoked_at = $2 WHERE id = $1`
	_, err := q.Exec(ctx, sql, id, time.Now())
	return err
}

// Operation operations
func (s *PostgresStore) CreateOperation(ctx context.Context, op *models.Operation) error {
	return createOperation(ctx, s.pool, op)
}

func (s *PostgresStore) GetOperation(ctx context.Context, id uuid.UUID) (*models.Operation, error) {
	return getOperation(ctx, s.pool, id)
}

func (s *PostgresStore) UpdateOperation(ctx context.Context, op *models.Operation) error {
	return updateOperation(ctx, s.pool, op)
}

func createOperation(ctx context.Context, q querier, op *models.Operation) error {
	sql := `INSERT INTO operations (id, resource_type, resource_id, operation_type, status,
		request_payload, result_payload, error_payload, started_at, finished_at, created_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)`
	_, err := q.Exec(ctx, sql, op.ID, op.ResourceType, op.ResourceID, op.OperationType,
		op.Status, op.RequestPayload, op.ResultPayload, op.ErrorPayload, op.StartedAt, op.FinishedAt, op.CreatedAt)
	return err
}

func getOperation(ctx context.Context, q querier, id uuid.UUID) (*models.Operation, error) {
	sql := `SELECT id, resource_type, resource_id, operation_type, status,
		request_payload, result_payload, error_payload, started_at, finished_at, created_at
		FROM operations WHERE id = $1`
	o := &models.Operation{}
	err := q.QueryRow(ctx, sql, id).Scan(&o.ID, &o.ResourceType, &o.ResourceID, &o.OperationType,
		&o.Status, &o.RequestPayload, &o.ResultPayload, &o.ErrorPayload, &o.StartedAt, &o.FinishedAt, &o.CreatedAt)
	if err == pgx.ErrNoRows {
		return nil, nil
	}
	return o, err
}

func updateOperation(ctx context.Context, q querier, op *models.Operation) error {
	sql := `UPDATE operations SET resource_type = $2, resource_id = $3, operation_type = $4,
		status = $5, request_payload = $6, result_payload = $7, error_payload = $8,
		started_at = $9, finished_at = $10 WHERE id = $1`
	_, err := q.Exec(ctx, sql, op.ID, op.ResourceType, op.ResourceID, op.OperationType,
		op.Status, op.RequestPayload, op.ResultPayload, op.ErrorPayload, op.StartedAt, op.FinishedAt)
	return err
}
