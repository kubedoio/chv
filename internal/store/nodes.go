package store

import (
	"context"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
)

func (s *PostgresStore) CreateNode(ctx context.Context, node *models.Node) error {
	return createNode(ctx, s.pool, node)
}

func (s *PostgresStore) GetNode(ctx context.Context, id uuid.UUID) (*models.Node, error) {
	return getNode(ctx, s.pool, id)
}

func (s *PostgresStore) GetNodeByHostname(ctx context.Context, hostname string) (*models.Node, error) {
	return getNodeByHostname(ctx, s.pool, hostname)
}

func (s *PostgresStore) UpdateNode(ctx context.Context, node *models.Node) error {
	return updateNode(ctx, s.pool, node)
}

func (s *PostgresStore) UpdateNodeHeartbeat(ctx context.Context, id uuid.UUID, status models.NodeState) error {
	return updateNodeHeartbeat(ctx, s.pool, id, status)
}

func (s *PostgresStore) ListNodes(ctx context.Context) ([]*models.Node, error) {
	return listNodes(ctx, s.pool)
}

func (s *PostgresStore) SetNodeMaintenance(ctx context.Context, id uuid.UUID, enabled bool) error {
	return setNodeMaintenance(ctx, s.pool, id, enabled)
}

func createNode(ctx context.Context, q querier, node *models.Node) error {
	sql := `
		INSERT INTO nodes (
			id, hostname, management_ip, status, maintenance_mode,
			total_cpu_cores, total_ram_mb, allocatable_cpu_cores, allocatable_ram_mb,
			labels, capabilities, agent_version, hypervisor_version,
			last_heartbeat_at, created_at, updated_at
		) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
	`
	
	_, err := q.Exec(ctx, sql,
		node.ID, node.Hostname, node.ManagementIP, node.Status, node.MaintenanceMode,
		node.TotalCPUcores, node.TotalRAMMB, node.AllocatableCPUCores, node.AllocatableRAMMB,
		node.Labels, node.Capabilities, node.AgentVersion, node.HypervisorVersion,
		node.LastHeartbeatAt, node.CreatedAt, node.UpdatedAt,
	)
	return err
}

func getNode(ctx context.Context, q querier, id uuid.UUID) (*models.Node, error) {
	sql := `
		SELECT id, hostname, management_ip::text, status, maintenance_mode,
			total_cpu_cores, total_ram_mb, allocatable_cpu_cores, allocatable_ram_mb,
			labels, capabilities, agent_version, hypervisor_version,
			last_heartbeat_at, created_at, updated_at
		FROM nodes WHERE id = $1
	`
	
	node := &models.Node{}
	err := q.QueryRow(ctx, sql, id).Scan(
		&node.ID, &node.Hostname, &node.ManagementIP, &node.Status, &node.MaintenanceMode,
		&node.TotalCPUcores, &node.TotalRAMMB, &node.AllocatableCPUCores, &node.AllocatableRAMMB,
		&node.Labels, &node.Capabilities, &node.AgentVersion, &node.HypervisorVersion,
		&node.LastHeartbeatAt, &node.CreatedAt, &node.UpdatedAt,
	)
	if err == pgx.ErrNoRows {
		return nil, nil
	}
	return node, err
}

func getNodeByHostname(ctx context.Context, q querier, hostname string) (*models.Node, error) {
	sql := `
		SELECT id, hostname, management_ip::text, status, maintenance_mode,
			total_cpu_cores, total_ram_mb, allocatable_cpu_cores, allocatable_ram_mb,
			labels, capabilities, agent_version, hypervisor_version,
			last_heartbeat_at, created_at, updated_at
		FROM nodes WHERE hostname = $1
	`
	
	node := &models.Node{}
	err := q.QueryRow(ctx, sql, hostname).Scan(
		&node.ID, &node.Hostname, &node.ManagementIP, &node.Status, &node.MaintenanceMode,
		&node.TotalCPUcores, &node.TotalRAMMB, &node.AllocatableCPUCores, &node.AllocatableRAMMB,
		&node.Labels, &node.Capabilities, &node.AgentVersion, &node.HypervisorVersion,
		&node.LastHeartbeatAt, &node.CreatedAt, &node.UpdatedAt,
	)
	if err == pgx.ErrNoRows {
		return nil, nil
	}
	return node, err
}

func updateNode(ctx context.Context, q querier, node *models.Node) error {
	sql := `
		UPDATE nodes SET
			hostname = $2, management_ip = $3, status = $4, maintenance_mode = $5,
			total_cpu_cores = $6, total_ram_mb = $7, allocatable_cpu_cores = $8, allocatable_ram_mb = $9,
			labels = $10, capabilities = $11, agent_version = $12, hypervisor_version = $13,
			last_heartbeat_at = $14, updated_at = $15
		WHERE id = $1
	`
	
	node.UpdatedAt = time.Now()
	_, err := q.Exec(ctx, sql,
		node.ID, node.Hostname, node.ManagementIP, node.Status, node.MaintenanceMode,
		node.TotalCPUcores, node.TotalRAMMB, node.AllocatableCPUCores, node.AllocatableRAMMB,
		node.Labels, node.Capabilities, node.AgentVersion, node.HypervisorVersion,
		node.LastHeartbeatAt, node.UpdatedAt,
	)
	return err
}

func updateNodeHeartbeat(ctx context.Context, q querier, id uuid.UUID, status models.NodeState) error {
	sql := `
		UPDATE nodes SET
			status = $2,
			last_heartbeat_at = $3,
			updated_at = $3
		WHERE id = $1
	`
	
	now := time.Now()
	_, err := q.Exec(ctx, sql, id, status, now)
	return err
}

func listNodes(ctx context.Context, q querier) ([]*models.Node, error) {
	sql := `
		SELECT id, hostname, management_ip::text, status, maintenance_mode,
			total_cpu_cores, total_ram_mb, allocatable_cpu_cores, allocatable_ram_mb,
			labels, capabilities, agent_version, hypervisor_version,
			last_heartbeat_at, created_at, updated_at
		FROM nodes ORDER BY hostname
	`
	
	rows, err := q.Query(ctx, sql)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	
	var nodes []*models.Node
	for rows.Next() {
		node := &models.Node{}
		err := rows.Scan(
			&node.ID, &node.Hostname, &node.ManagementIP, &node.Status, &node.MaintenanceMode,
			&node.TotalCPUcores, &node.TotalRAMMB, &node.AllocatableCPUCores, &node.AllocatableRAMMB,
			&node.Labels, &node.Capabilities, &node.AgentVersion, &node.HypervisorVersion,
			&node.LastHeartbeatAt, &node.CreatedAt, &node.UpdatedAt,
		)
		if err != nil {
			return nil, err
		}
		nodes = append(nodes, node)
	}
	
	return nodes, rows.Err()
}

func setNodeMaintenance(ctx context.Context, q querier, id uuid.UUID, enabled bool) error {
	sql := `
		UPDATE nodes SET
			maintenance_mode = $2,
			updated_at = $3
		WHERE id = $1
	`
	
	now := time.Now()
	_, err := q.Exec(ctx, sql, id, enabled, now)
	return err
}
