package store

import (
	"context"
	"time"

	"github.com/chv/chv/internal/models"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
)

func (s *PostgresStore) CreateVM(ctx context.Context, vm *models.VirtualMachine) error {
	return createVM(ctx, s.pool, vm)
}

func (s *PostgresStore) GetVM(ctx context.Context, id uuid.UUID) (*models.VirtualMachine, error) {
	return getVM(ctx, s.pool, id)
}

func (s *PostgresStore) GetVMByName(ctx context.Context, name string) (*models.VirtualMachine, error) {
	return getVMByName(ctx, s.pool, name)
}

func (s *PostgresStore) UpdateVM(ctx context.Context, vm *models.VirtualMachine) error {
	return updateVM(ctx, s.pool, vm)
}

func (s *PostgresStore) UpdateVMActualState(ctx context.Context, id uuid.UUID, state models.VMActualState, lastError []byte) error {
	return updateVMActualState(ctx, s.pool, id, state, lastError)
}

func (s *PostgresStore) ListVMs(ctx context.Context) ([]*models.VirtualMachine, error) {
	return listVMs(ctx, s.pool)
}

func (s *PostgresStore) ListVMsByNode(ctx context.Context, nodeID uuid.UUID) ([]*models.VirtualMachine, error) {
	return listVMsByNode(ctx, s.pool, nodeID)
}

func (s *PostgresStore) ListVMsNeedingReconciliation(ctx context.Context) ([]*models.VirtualMachine, error) {
	return listVMsNeedingReconciliation(ctx, s.pool)
}

func (s *PostgresStore) DeleteVM(ctx context.Context, id uuid.UUID) error {
	return deleteVM(ctx, s.pool, id)
}

func createVM(ctx context.Context, q querier, vm *models.VirtualMachine) error {
	sql := `
		INSERT INTO virtual_machines (
			id, name, node_id, desired_state, actual_state, placement_status,
			spec, last_error, created_at, updated_at
		) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
	`
	
	_, err := q.Exec(ctx, sql,
		vm.ID, vm.Name, vm.NodeID, vm.DesiredState, vm.ActualState, vm.PlacementStatus,
		vm.Spec, vm.LastError, vm.CreatedAt, vm.UpdatedAt,
	)
	return err
}

func getVM(ctx context.Context, q querier, id uuid.UUID) (*models.VirtualMachine, error) {
	sql := `
		SELECT id, name, node_id, desired_state, actual_state, placement_status,
			spec, last_error, created_at, updated_at
		FROM virtual_machines WHERE id = $1
	`
	
	vm := &models.VirtualMachine{}
	err := q.QueryRow(ctx, sql, id).Scan(
		&vm.ID, &vm.Name, &vm.NodeID, &vm.DesiredState, &vm.ActualState, &vm.PlacementStatus,
		&vm.Spec, &vm.LastError, &vm.CreatedAt, &vm.UpdatedAt,
	)
	if err == pgx.ErrNoRows {
		return nil, nil
	}
	return vm, err
}

func getVMByName(ctx context.Context, q querier, name string) (*models.VirtualMachine, error) {
	sql := `
		SELECT id, name, node_id, desired_state, actual_state, placement_status,
			spec, last_error, created_at, updated_at
		FROM virtual_machines WHERE name = $1
	`
	
	vm := &models.VirtualMachine{}
	err := q.QueryRow(ctx, sql, name).Scan(
		&vm.ID, &vm.Name, &vm.NodeID, &vm.DesiredState, &vm.ActualState, &vm.PlacementStatus,
		&vm.Spec, &vm.LastError, &vm.CreatedAt, &vm.UpdatedAt,
	)
	if err == pgx.ErrNoRows {
		return nil, nil
	}
	return vm, err
}

func updateVM(ctx context.Context, q querier, vm *models.VirtualMachine) error {
	sql := `
		UPDATE virtual_machines SET
			name = $2, node_id = $3, desired_state = $4, actual_state = $5,
			placement_status = $6, spec = $7, last_error = $8, updated_at = $9
		WHERE id = $1
	`
	
	vm.UpdatedAt = time.Now()
	_, err := q.Exec(ctx, sql,
		vm.ID, vm.Name, vm.NodeID, vm.DesiredState, vm.ActualState,
		vm.PlacementStatus, vm.Spec, vm.LastError, vm.UpdatedAt,
	)
	return err
}

func updateVMActualState(ctx context.Context, q querier, id uuid.UUID, state models.VMActualState, lastError []byte) error {
	sql := `
		UPDATE virtual_machines SET
			actual_state = $2,
			last_error = $3,
			updated_at = $4
		WHERE id = $1
	`
	
	now := time.Now()
	_, err := q.Exec(ctx, sql, id, state, lastError, now)
	return err
}

func listVMs(ctx context.Context, q querier) ([]*models.VirtualMachine, error) {
	sql := `
		SELECT id, name, node_id, desired_state, actual_state, placement_status,
			spec, last_error, created_at, updated_at
		FROM virtual_machines ORDER BY name
	`
	
	rows, err := q.Query(ctx, sql)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	
	var vms []*models.VirtualMachine
	for rows.Next() {
		vm := &models.VirtualMachine{}
		err := rows.Scan(
			&vm.ID, &vm.Name, &vm.NodeID, &vm.DesiredState, &vm.ActualState, &vm.PlacementStatus,
			&vm.Spec, &vm.LastError, &vm.CreatedAt, &vm.UpdatedAt,
		)
		if err != nil {
			return nil, err
		}
		vms = append(vms, vm)
	}
	
	return vms, rows.Err()
}

func listVMsByNode(ctx context.Context, q querier, nodeID uuid.UUID) ([]*models.VirtualMachine, error) {
	sql := `
		SELECT id, name, node_id, desired_state, actual_state, placement_status,
			spec, last_error, created_at, updated_at
		FROM virtual_machines WHERE node_id = $1 ORDER BY name
	`
	
	rows, err := q.Query(ctx, sql, nodeID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	
	var vms []*models.VirtualMachine
	for rows.Next() {
		vm := &models.VirtualMachine{}
		err := rows.Scan(
			&vm.ID, &vm.Name, &vm.NodeID, &vm.DesiredState, &vm.ActualState, &vm.PlacementStatus,
			&vm.Spec, &vm.LastError, &vm.CreatedAt, &vm.UpdatedAt,
		)
		if err != nil {
			return nil, err
		}
		vms = append(vms, vm)
	}
	
	return vms, rows.Err()
}

func listVMsNeedingReconciliation(ctx context.Context, q querier) ([]*models.VirtualMachine, error) {
	sql := `
		SELECT id, name, node_id, desired_state, actual_state, placement_status,
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
	
	rows, err := q.Query(ctx, sql)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	
	var vms []*models.VirtualMachine
	for rows.Next() {
		vm := &models.VirtualMachine{}
		err := rows.Scan(
			&vm.ID, &vm.Name, &vm.NodeID, &vm.DesiredState, &vm.ActualState, &vm.PlacementStatus,
			&vm.Spec, &vm.LastError, &vm.CreatedAt, &vm.UpdatedAt,
		)
		if err != nil {
			return nil, err
		}
		vms = append(vms, vm)
	}
	
	return vms, rows.Err()
}

func deleteVM(ctx context.Context, q querier, id uuid.UUID) error {
	sql := `DELETE FROM virtual_machines WHERE id = $1`
	_, err := q.Exec(ctx, sql, id)
	return err
}
