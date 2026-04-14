package vm

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/chv/chv/internal/db"
	"github.com/chv/chv/internal/models"
)

func TestGatekeeper_CheckAll_WorkspaceMissing(t *testing.T) {
	tmpDir := t.TempDir()
	repo, err := db.Open(filepath.Join(tmpDir, "test.db"))
	if err != nil {
		t.Fatalf("failed to open db: %v", err)
	}
	defer repo.Close()

	ctx := context.Background()

	// Create a VM with non-existent workspace
	vm := &models.VirtualMachine{
		ID:              "vm-1",
		Name:            "test-vm",
		ImageID:         "img-1",
		StoragePoolID:   "pool-1",
		NetworkID:       "net-1",
		WorkspacePath:   "/nonexistent/path",
		DesiredState:    "stopped",
		ActualState:     "stopped",
		VCPU:            2,
		MemoryMB:        2048,
	}

	gatekeeper := NewGatekeeper(repo)
	result := gatekeeper.CheckAll(ctx, vm)

	if result.Passed {
		t.Error("expected gate checks to fail, but they passed")
	}

	if len(result.Errors) == 0 {
		t.Fatal("expected at least one gate error")
	}

	// Should have workspace_missing error
	foundWorkspaceError := false
	for _, e := range result.Errors {
		if e.Code == "workspace_missing" {
			foundWorkspaceError = true
			break
		}
	}
	if !foundWorkspaceError {
		t.Errorf("expected workspace_missing error, got: %+v", result.Errors)
	}
}

func TestGatekeeper_CheckAll_SeedISOMissing(t *testing.T) {
	tmpDir := t.TempDir()
	repo, err := db.Open(filepath.Join(tmpDir, "test.db"))
	if err != nil {
		t.Fatalf("failed to open db: %v", err)
	}
	defer repo.Close()

	ctx := context.Background()

	// Create workspace directory but no seed.iso
	workspaceDir := filepath.Join(tmpDir, "workspace")
	if err := os.MkdirAll(workspaceDir, 0755); err != nil {
		t.Fatalf("failed to create workspace: %v", err)
	}

	// Create disk.qcow2 but not seed.iso
	diskPath := filepath.Join(workspaceDir, "disk.qcow2")
	if err := os.WriteFile(diskPath, []byte("disk"), 0644); err != nil {
		t.Fatalf("failed to create disk: %v", err)
	}

	vm := &models.VirtualMachine{
		ID:              "vm-1",
		Name:            "test-vm",
		ImageID:         "img-1",
		StoragePoolID:   "pool-1",
		NetworkID:       "net-1",
		WorkspacePath:   workspaceDir,
		DesiredState:    "stopped",
		ActualState:     "stopped",
		VCPU:            2,
		MemoryMB:        2048,
	}

	gatekeeper := NewGatekeeper(repo)
	result := gatekeeper.CheckAll(ctx, vm)

	if result.Passed {
		t.Error("expected gate checks to fail, but they passed")
	}

	// Should have seed_iso_missing error
	foundSeedError := false
	for _, e := range result.Errors {
		if e.Code == "seed_iso_missing" {
			foundSeedError = true
			break
		}
	}
	if !foundSeedError {
		t.Errorf("expected seed_iso_missing error, got: %+v", result.Errors)
	}
}

func TestGatekeeper_CheckAll_DiskMissing(t *testing.T) {
	tmpDir := t.TempDir()
	repo, err := db.Open(filepath.Join(tmpDir, "test.db"))
	if err != nil {
		t.Fatalf("failed to open db: %v", err)
	}
	defer repo.Close()

	ctx := context.Background()

	// Create workspace directory but no disk.qcow2
	workspaceDir := filepath.Join(tmpDir, "workspace")
	if err := os.MkdirAll(workspaceDir, 0755); err != nil {
		t.Fatalf("failed to create workspace: %v", err)
	}

	// Create seed.iso but not disk.qcow2
	seedPath := filepath.Join(workspaceDir, "seed.iso")
	if err := os.WriteFile(seedPath, []byte("seed"), 0644); err != nil {
		t.Fatalf("failed to create seed.iso: %v", err)
	}

	vm := &models.VirtualMachine{
		ID:              "vm-1",
		Name:            "test-vm",
		ImageID:         "img-1",
		StoragePoolID:   "pool-1",
		NetworkID:       "net-1",
		WorkspacePath:   workspaceDir,
		DesiredState:    "stopped",
		ActualState:     "stopped",
		VCPU:            2,
		MemoryMB:        2048,
	}

	gatekeeper := NewGatekeeper(repo)
	result := gatekeeper.CheckAll(ctx, vm)

	if result.Passed {
		t.Error("expected gate checks to fail, but they passed")
	}

	// Should have disk_missing error
	foundDiskError := false
	for _, e := range result.Errors {
		if e.Code == "disk_missing" {
			foundDiskError = true
			break
		}
	}
	if !foundDiskError {
		t.Errorf("expected disk_missing error, got: %+v", result.Errors)
	}
}

func TestGatekeeper_CheckAll_AllGatesPass(t *testing.T) {
	tmpDir := t.TempDir()
	repo, err := db.Open(filepath.Join(tmpDir, "test.db"))
	if err != nil {
		t.Fatalf("failed to open db: %v", err)
	}
	defer repo.Close()

	ctx := context.Background()

	// Create a test node first (required for foreign key constraints)
	node := &models.Node{
		ID:        "node-1",
		Name:      "test-node",
		Hostname:  "test",
		IPAddress: "127.0.0.1",
		Status:    "online",
		IsLocal:   true,
		CreatedAt: "2024-01-01T00:00:00Z",
		UpdatedAt: "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateNode(ctx, node); err != nil {
		t.Fatalf("failed to create node: %v", err)
	}

	// Create necessary resources in DB
	// 1. Create network
	network := &models.Network{
		ID:              "net-1",
		NodeID:          "node-1",
		Name:            "test-net",
		Mode:            "bridge",
		BridgeName:      "br0",
		CIDR:            "10.0.0.0/24",
		GatewayIP:       "10.0.0.1",
		IsSystemManaged: false,
		Status:          "active",
		CreatedAt:       "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateNetwork(ctx, network); err != nil {
		t.Fatalf("failed to create network: %v", err)
	}

	// 2. Create storage pool
	pool := &models.StoragePool{
		ID:        "pool-1",
		NodeID:      "node-1",
		Name:      "test-pool",
		PoolType:  "local",
		Path:      tmpDir,
		IsDefault: false,
		Status:    "ready",
		CreatedAt: "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateStoragePool(ctx, pool); err != nil {
		t.Fatalf("failed to create pool: %v", err)
	}

	// 3. Create image
	image := &models.Image{
		ID:                 "img-1",
		NodeID:             "node-1",
		Name:               "test-image",
		OSFamily:           "ubuntu",
		Architecture:       "x86_64",
		Format:             "qcow2",
		SourceURL:          "http://example.com/image.qcow2",
		LocalPath:          filepath.Join(tmpDir, "image.qcow2"),
		CloudInitSupported: true,
		Status:             "ready",
		CreatedAt:          "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateImage(ctx, image); err != nil {
		t.Fatalf("failed to create image: %v", err)
	}

	// 4. Create workspace with all required files
	workspaceDir := filepath.Join(tmpDir, "workspace")
	if err := os.MkdirAll(workspaceDir, 0755); err != nil {
		t.Fatalf("failed to create workspace: %v", err)
	}

	// Create seed.iso
	seedPath := filepath.Join(workspaceDir, "seed.iso")
	if err := os.WriteFile(seedPath, []byte("seed"), 0644); err != nil {
		t.Fatalf("failed to create seed.iso: %v", err)
	}

	// Create disk.qcow2
	diskPath := filepath.Join(workspaceDir, "disk.qcow2")
	if err := os.WriteFile(diskPath, []byte("disk"), 0644); err != nil {
		t.Fatalf("failed to create disk.qcow2: %v", err)
	}

	vm := &models.VirtualMachine{
		ID:              "vm-1",
		Name:            "test-vm",
		ImageID:         "img-1",
		StoragePoolID:   "pool-1",
		NetworkID:       "net-1",
		WorkspacePath:   workspaceDir,
		DesiredState:    "stopped",
		ActualState:     "stopped",
		VCPU:            2,
		MemoryMB:        2048,
	}

	gatekeeper := NewGatekeeper(repo)
	result := gatekeeper.CheckAll(ctx, vm)

	if !result.Passed {
		t.Errorf("expected all gate checks to pass, but got errors: %+v", result.Errors)
	}

	if len(result.Errors) != 0 {
		t.Errorf("expected no errors, got: %+v", result.Errors)
	}
}

func TestGatekeeper_CheckAll_ImageNotReady(t *testing.T) {
	tmpDir := t.TempDir()
	repo, err := db.Open(filepath.Join(tmpDir, "test.db"))
	if err != nil {
		t.Fatalf("failed to open db: %v", err)
	}
	defer repo.Close()

	ctx := context.Background()

	// Create a test node first (required for foreign key constraints)
	node := &models.Node{
		ID:        "node-1",
		Name:      "test-node",
		Hostname:  "test",
		IPAddress: "127.0.0.1",
		Status:    "online",
		IsLocal:   true,
		CreatedAt: "2024-01-01T00:00:00Z",
		UpdatedAt: "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateNode(ctx, node); err != nil {
		t.Fatalf("failed to create node: %v", err)
	}

	// Create network and pool (ready)
	network := &models.Network{
		ID:              "net-1",
		NodeID:          "node-1",
		Name:            "test-net",
		Mode:            "bridge",
		BridgeName:      "br0",
		CIDR:            "10.0.0.0/24",
		GatewayIP:       "10.0.0.1",
		IsSystemManaged: false,
		Status:          "active",
		CreatedAt:       "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateNetwork(ctx, network); err != nil {
		t.Fatalf("failed to create network: %v", err)
	}

	pool := &models.StoragePool{
		ID:        "pool-1",
		NodeID:      "node-1",
		Name:      "test-pool",
		PoolType:  "local",
		Path:      tmpDir,
		IsDefault: false,
		Status:    "ready",
		CreatedAt: "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateStoragePool(ctx, pool); err != nil {
		t.Fatalf("failed to create pool: %v", err)
	}

	// Create image with status "downloading" (not ready)
	image := &models.Image{
		ID:                 "img-1",
		NodeID:             "node-1",
		Name:               "test-image",
		OSFamily:           "ubuntu",
		Architecture:       "x86_64",
		Format:             "qcow2",
		SourceURL:          "http://example.com/image.qcow2",
		LocalPath:          filepath.Join(tmpDir, "image.qcow2"),
		CloudInitSupported: true,
		Status:             "downloading", // Not ready
		CreatedAt:          "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateImage(ctx, image); err != nil {
		t.Fatalf("failed to create image: %v", err)
	}

	// Create workspace with required files
	workspaceDir := filepath.Join(tmpDir, "workspace")
	if err := os.MkdirAll(workspaceDir, 0755); err != nil {
		t.Fatalf("failed to create workspace: %v", err)
	}
	os.WriteFile(filepath.Join(workspaceDir, "seed.iso"), []byte("seed"), 0644)
	os.WriteFile(filepath.Join(workspaceDir, "disk.qcow2"), []byte("disk"), 0644)

	vm := &models.VirtualMachine{
		ID:              "vm-1",
		Name:            "test-vm",
		ImageID:         "img-1",
		StoragePoolID:   "pool-1",
		NetworkID:       "net-1",
		WorkspacePath:   workspaceDir,
		DesiredState:    "stopped",
		ActualState:     "stopped",
		VCPU:            2,
		MemoryMB:        2048,
	}

	gatekeeper := NewGatekeeper(repo)
	result := gatekeeper.CheckAll(ctx, vm)

	if result.Passed {
		t.Error("expected gate checks to fail due to image not ready")
	}

	foundImageError := false
	for _, e := range result.Errors {
		if e.Code == "image_not_ready" {
			foundImageError = true
			if e.Message == "" {
				t.Error("expected error message for image_not_ready")
			}
			break
		}
	}
	if !foundImageError {
		t.Errorf("expected image_not_ready error, got: %+v", result.Errors)
	}
}

func TestGatekeeper_CheckAll_StorageNotReady(t *testing.T) {
	tmpDir := t.TempDir()
	repo, err := db.Open(filepath.Join(tmpDir, "test.db"))
	if err != nil {
		t.Fatalf("failed to open db: %v", err)
	}
	defer repo.Close()

	ctx := context.Background()

	// Create a test node first (required for foreign key constraints)
	node := &models.Node{
		ID:        "node-1",
		Name:      "test-node",
		Hostname:  "test",
		IPAddress: "127.0.0.1",
		Status:    "online",
		IsLocal:   true,
		CreatedAt: "2024-01-01T00:00:00Z",
		UpdatedAt: "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateNode(ctx, node); err != nil {
		t.Fatalf("failed to create node: %v", err)
	}

	// Create network
	network := &models.Network{
		ID:              "net-1",
		NodeID:          "node-1",
		Name:            "test-net",
		Mode:            "bridge",
		BridgeName:      "br0",
		CIDR:            "10.0.0.0/24",
		GatewayIP:       "10.0.0.1",
		IsSystemManaged: false,
		Status:          "active",
		CreatedAt:       "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateNetwork(ctx, network); err != nil {
		t.Fatalf("failed to create network: %v", err)
	}

	// Create storage pool with status "offline" (not ready)
	pool := &models.StoragePool{
		ID:        "pool-1",
		NodeID:      "node-1",
		Name:      "test-pool",
		PoolType:  "local",
		Path:      tmpDir,
		IsDefault: false,
		Status:    "offline", // Not ready
		CreatedAt: "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateStoragePool(ctx, pool); err != nil {
		t.Fatalf("failed to create pool: %v", err)
	}

	// Create ready image
	image := &models.Image{
		ID:                 "img-1",
		NodeID:             "node-1",
		Name:               "test-image",
		OSFamily:           "ubuntu",
		Architecture:       "x86_64",
		Format:             "qcow2",
		SourceURL:          "http://example.com/image.qcow2",
		LocalPath:          filepath.Join(tmpDir, "image.qcow2"),
		CloudInitSupported: true,
		Status:             "ready",
		CreatedAt:          "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateImage(ctx, image); err != nil {
		t.Fatalf("failed to create image: %v", err)
	}

	// Create workspace with required files
	workspaceDir := filepath.Join(tmpDir, "workspace")
	if err := os.MkdirAll(workspaceDir, 0755); err != nil {
		t.Fatalf("failed to create workspace: %v", err)
	}
	os.WriteFile(filepath.Join(workspaceDir, "seed.iso"), []byte("seed"), 0644)
	os.WriteFile(filepath.Join(workspaceDir, "disk.qcow2"), []byte("disk"), 0644)

	vm := &models.VirtualMachine{
		ID:              "vm-1",
		Name:            "test-vm",
		ImageID:         "img-1",
		StoragePoolID:   "pool-1",
		NetworkID:       "net-1",
		WorkspacePath:   workspaceDir,
		DesiredState:    "stopped",
		ActualState:     "stopped",
		VCPU:            2,
		MemoryMB:        2048,
	}

	gatekeeper := NewGatekeeper(repo)
	result := gatekeeper.CheckAll(ctx, vm)

	if result.Passed {
		t.Error("expected gate checks to fail due to storage not ready")
	}

	foundStorageError := false
	for _, e := range result.Errors {
		if e.Code == "storage_not_ready" {
			foundStorageError = true
			break
		}
	}
	if !foundStorageError {
		t.Errorf("expected storage_not_ready error, got: %+v", result.Errors)
	}
}

func TestGatekeeper_CheckAll_NetworkNotFound(t *testing.T) {
	tmpDir := t.TempDir()
	repo, err := db.Open(filepath.Join(tmpDir, "test.db"))
	if err != nil {
		t.Fatalf("failed to open db: %v", err)
	}
	defer repo.Close()

	ctx := context.Background()

	// Create a test node first (required for foreign key constraints)
	node := &models.Node{
		ID:        "node-1",
		Name:      "test-node",
		Hostname:  "test",
		IPAddress: "127.0.0.1",
		Status:    "online",
		IsLocal:   true,
		CreatedAt: "2024-01-01T00:00:00Z",
		UpdatedAt: "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateNode(ctx, node); err != nil {
		t.Fatalf("failed to create node: %v", err)
	}

	// Create storage pool (ready)
	pool := &models.StoragePool{
		ID:        "pool-1",
		NodeID:      "node-1",
		Name:      "test-pool",
		PoolType:  "local",
		Path:      tmpDir,
		IsDefault: false,
		Status:    "ready",
		CreatedAt: "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateStoragePool(ctx, pool); err != nil {
		t.Fatalf("failed to create pool: %v", err)
	}

	// Create ready image
	image := &models.Image{
		ID:                 "img-1",
		NodeID:             "node-1",
		Name:               "test-image",
		OSFamily:           "ubuntu",
		Architecture:       "x86_64",
		Format:             "qcow2",
		SourceURL:          "http://example.com/image.qcow2",
		LocalPath:          filepath.Join(tmpDir, "image.qcow2"),
		CloudInitSupported: true,
		Status:             "ready",
		CreatedAt:          "2024-01-01T00:00:00Z",
	}
	if err := repo.CreateImage(ctx, image); err != nil {
		t.Fatalf("failed to create image: %v", err)
	}

	// Create workspace with required files
	workspaceDir := filepath.Join(tmpDir, "workspace")
	if err := os.MkdirAll(workspaceDir, 0755); err != nil {
		t.Fatalf("failed to create workspace: %v", err)
	}
	os.WriteFile(filepath.Join(workspaceDir, "seed.iso"), []byte("seed"), 0644)
	os.WriteFile(filepath.Join(workspaceDir, "disk.qcow2"), []byte("disk"), 0644)

	// VM references a network that doesn't exist
	vm := &models.VirtualMachine{
		ID:              "vm-1",
		Name:            "test-vm",
		ImageID:         "img-1",
		StoragePoolID:   "pool-1",
		NetworkID:       "nonexistent-net",
		WorkspacePath:   workspaceDir,
		DesiredState:    "stopped",
		ActualState:     "stopped",
		VCPU:            2,
		MemoryMB:        2048,
	}

	gatekeeper := NewGatekeeper(repo)
	result := gatekeeper.CheckAll(ctx, vm)

	if result.Passed {
		t.Error("expected gate checks to fail due to network not found")
	}

	foundNetworkError := false
	for _, e := range result.Errors {
		if e.Code == "network_not_ready" {
			foundNetworkError = true
			break
		}
	}
	if !foundNetworkError {
		t.Errorf("expected network_not_ready error, got: %+v", result.Errors)
	}
}

func TestGateError_Error(t *testing.T) {
	err := GateError{
		Gate:    "seed_iso_exists",
		Code:    "seed_iso_missing",
		Message: "seed.iso not found at /path/to/seed.iso",
	}

	expected := "seed_iso_exists: seed.iso not found at /path/to/seed.iso"
	if err.Error() != expected {
		t.Errorf("expected %q, got %q", expected, err.Error())
	}
}
