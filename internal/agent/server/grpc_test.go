package server

import (
	"context"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/chv/chv/internal/pb/agent"
)

func TestNew(t *testing.T) {
	// Create temp directories
	tempDir := t.TempDir()

	cfg := &Config{
		NodeID:            "test-node",
		ListenAddr:        "127.0.0.1:0",
		DataDir:           filepath.Join(tempDir, "data"),
		ImageDir:          filepath.Join(tempDir, "images"),
		VolumeDir:         filepath.Join(tempDir, "volumes"),
		CloudHypervisor:   "/usr/local/bin/cloud-hypervisor",
		HeartbeatInterval: 30 * time.Second,
	}

	srv, err := New(cfg)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	if srv == nil {
		t.Fatal("New() returned nil server")
	}

	if srv.config != cfg {
		t.Error("Server config mismatch")
	}

	// Verify directories were created
	for _, dir := range []string{cfg.DataDir, cfg.ImageDir, cfg.VolumeDir} {
		if _, err := os.Stat(dir); os.IsNotExist(err) {
			t.Errorf("Directory %s was not created", dir)
		}
	}
}

func TestServer_Ping(t *testing.T) {
	tempDir := t.TempDir()
	cfg := &Config{
		ListenAddr:        "127.0.0.1:0",
		DataDir:           filepath.Join(tempDir, "data"),
		ImageDir:          filepath.Join(tempDir, "images"),
		VolumeDir:         filepath.Join(tempDir, "volumes"),
		CloudHypervisor:   "/usr/local/bin/cloud-hypervisor",
		HeartbeatInterval: 30 * time.Second,
	}

	srv, err := New(cfg)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	ctx := context.Background()
	resp, err := srv.Ping(ctx, &agent.Empty{})
	if err != nil {
		t.Fatalf("Ping() error = %v", err)
	}

	if !resp.Ok {
		t.Error("Ping() Ok = false, want true")
	}

	if resp.Version == "" {
		t.Error("Ping() Version is empty")
	}
}

func TestServer_ReportNodeStatus(t *testing.T) {
	tempDir := t.TempDir()
	cfg := &Config{
		NodeID:            "test-node",
		ListenAddr:        "127.0.0.1:0",
		DataDir:           filepath.Join(tempDir, "data"),
		ImageDir:          filepath.Join(tempDir, "images"),
		VolumeDir:         filepath.Join(tempDir, "volumes"),
		CloudHypervisor:   "/usr/local/bin/cloud-hypervisor",
		HeartbeatInterval: 30 * time.Second,
	}

	srv, err := New(cfg)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	ctx := context.Background()
	resp, err := srv.ReportNodeStatus(ctx, &agent.Empty{})
	if err != nil {
		t.Fatalf("ReportNodeStatus() error = %v", err)
	}

	if resp.NodeId != "test-node" {
		t.Errorf("ReportNodeStatus() NodeId = %v, want test-node", resp.NodeId)
	}

	if resp.State != "online" {
		t.Errorf("ReportNodeStatus() State = %v, want online", resp.State)
	}

	if resp.Hostname == "" {
		t.Error("ReportNodeStatus() Hostname is empty")
	}
}

func TestServer_ValidateNode(t *testing.T) {
	tempDir := t.TempDir()
	cfg := &Config{
		ListenAddr:        "127.0.0.1:0",
		DataDir:           filepath.Join(tempDir, "data"),
		ImageDir:          filepath.Join(tempDir, "images"),
		VolumeDir:         filepath.Join(tempDir, "volumes"),
		CloudHypervisor:   "/usr/local/bin/cloud-hypervisor",
		HeartbeatInterval: 30 * time.Second,
	}

	srv, err := New(cfg)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	ctx := context.Background()
	resp, err := srv.ValidateNode(ctx, &agent.Empty{})
	if err != nil {
		t.Fatalf("ValidateNode() error = %v", err)
	}

	// Should return a response (may be OK or have errors depending on environment)
	if resp == nil {
		t.Fatal("ValidateNode() returned nil response")
	}
}

func TestServer_CreateVolume(t *testing.T) {
	tempDir := t.TempDir()
	cfg := &Config{
		ListenAddr:        "127.0.0.1:0",
		DataDir:           filepath.Join(tempDir, "data"),
		ImageDir:          filepath.Join(tempDir, "images"),
		VolumeDir:         tempDir,
		CloudHypervisor:   "/usr/local/bin/cloud-hypervisor",
		HeartbeatInterval: 30 * time.Second,
	}

	srv, err := New(cfg)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	ctx := context.Background()
	req := &agent.VolumeCreateRequest{
		VolumeId:  "test-vol-1",
		PoolPath:  tempDir,
		SizeBytes: 1024 * 1024 * 100, // 100MB
	}

	resp, err := srv.CreateVolume(ctx, req)
	if err != nil {
		t.Fatalf("CreateVolume() error = %v", err)
	}

	if !resp.Ok {
		t.Errorf("CreateVolume() Ok = false, want true")
	}

	// Verify volume file was created
	volumePath := filepath.Join(tempDir, "test-vol-1.raw")
	if _, err := os.Stat(volumePath); os.IsNotExist(err) {
		t.Error("Volume file was not created")
	}
}

func TestServer_CreateVolume_AlreadyExists(t *testing.T) {
	tempDir := t.TempDir()
	cfg := &Config{
		ListenAddr:        "127.0.0.1:0",
		DataDir:           filepath.Join(tempDir, "data"),
		ImageDir:          filepath.Join(tempDir, "images"),
		VolumeDir:         tempDir,
		CloudHypervisor:   "/usr/local/bin/cloud-hypervisor",
		HeartbeatInterval: 30 * time.Second,
	}

	srv, err := New(cfg)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	// Pre-create the volume file
	volumePath := filepath.Join(tempDir, "existing-vol.raw")
	if err := os.WriteFile(volumePath, []byte{}, 0644); err != nil {
		t.Fatalf("Failed to create test file: %v", err)
	}

	ctx := context.Background()
	req := &agent.VolumeCreateRequest{
		VolumeId:  "existing-vol",
		PoolPath:  tempDir,
		SizeBytes: 1024 * 1024 * 100,
	}

	resp, err := srv.CreateVolume(ctx, req)
	if err != nil {
		t.Fatalf("CreateVolume() error = %v", err)
	}

	// Should return OK for already existing volume
	if !resp.Ok {
		t.Error("CreateVolume() should return Ok=true for existing volume")
	}
}

func TestServer_DeleteVolume(t *testing.T) {
	tempDir := t.TempDir()
	cfg := &Config{
		ListenAddr:        "127.0.0.1:0",
		DataDir:           filepath.Join(tempDir, "data"),
		ImageDir:          filepath.Join(tempDir, "images"),
		VolumeDir:         tempDir,
		CloudHypervisor:   "/usr/local/bin/cloud-hypervisor",
		HeartbeatInterval: 30 * time.Second,
	}

	srv, err := New(cfg)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	// Create a volume first
	volumePath := filepath.Join(tempDir, "delete-me.raw")
	if err := os.WriteFile(volumePath, []byte{}, 0644); err != nil {
		t.Fatalf("Failed to create test file: %v", err)
	}

	ctx := context.Background()
	req := &agent.VolumeCreateRequest{
		VolumeId: "delete-me",
		PoolPath: tempDir,
	}

	resp, err := srv.DeleteVolume(ctx, req)
	if err != nil {
		t.Fatalf("DeleteVolume() error = %v", err)
	}

	if !resp.Ok {
		t.Errorf("DeleteVolume() Ok = false, want true")
	}

	// Verify volume file was deleted
	if _, err := os.Stat(volumePath); !os.IsNotExist(err) {
		t.Error("Volume file was not deleted")
	}
}

func TestServer_DeleteVolume_NotFound(t *testing.T) {
	tempDir := t.TempDir()
	cfg := &Config{
		ListenAddr:        "127.0.0.1:0",
		DataDir:           filepath.Join(tempDir, "data"),
		ImageDir:          filepath.Join(tempDir, "images"),
		VolumeDir:         tempDir,
		CloudHypervisor:   "/usr/local/bin/cloud-hypervisor",
		HeartbeatInterval: 30 * time.Second,
	}

	srv, err := New(cfg)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	ctx := context.Background()
	req := &agent.VolumeCreateRequest{
		VolumeId: "non-existent",
		PoolPath: tempDir,
	}

	resp, err := srv.DeleteVolume(ctx, req)
	if err != nil {
		t.Fatalf("DeleteVolume() error = %v", err)
	}

	// Should return OK for non-existent volume (idempotent)
	if !resp.Ok {
		t.Error("DeleteVolume() should return Ok=true for non-existent volume")
	}
}

func TestServer_ListHostVMs(t *testing.T) {
	tempDir := t.TempDir()
	cfg := &Config{
		ListenAddr:        "127.0.0.1:0",
		DataDir:           filepath.Join(tempDir, "data"),
		ImageDir:          filepath.Join(tempDir, "images"),
		VolumeDir:         filepath.Join(tempDir, "volumes"),
		CloudHypervisor:   "/usr/local/bin/cloud-hypervisor",
		HeartbeatInterval: 30 * time.Second,
	}

	srv, err := New(cfg)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	ctx := context.Background()
	resp, err := srv.ListHostVMs(ctx, &agent.Empty{})
	if err != nil {
		t.Fatalf("ListHostVMs() error = %v", err)
	}

	if resp.Vms == nil {
		t.Error("ListHostVMs() Vms is nil")
	}
}

func TestServer_PrepareDrain(t *testing.T) {
	tempDir := t.TempDir()
	cfg := &Config{
		ListenAddr:        "127.0.0.1:0",
		DataDir:           filepath.Join(tempDir, "data"),
		ImageDir:          filepath.Join(tempDir, "images"),
		VolumeDir:         filepath.Join(tempDir, "volumes"),
		CloudHypervisor:   "/usr/local/bin/cloud-hypervisor",
		HeartbeatInterval: 30 * time.Second,
	}

	srv, err := New(cfg)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	ctx := context.Background()
	req := &agent.DrainRequest{
		Enable: true,
	}

	resp, err := srv.PrepareDrain(ctx, req)
	if err != nil {
		t.Fatalf("PrepareDrain() error = %v", err)
	}

	// Should be OK since no VMs are running
	if !resp.Ok {
		t.Error("PrepareDrain() Ok = false, want true (no VMs running)")
	}

	if resp.ActiveVms == nil {
		t.Error("PrepareDrain() ActiveVms is nil")
	}
}

func TestServer_ImportImage(t *testing.T) {
	tempDir := t.TempDir()
	cfg := &Config{
		ListenAddr:        "127.0.0.1:0",
		DataDir:           filepath.Join(tempDir, "data"),
		ImageDir:          tempDir,
		VolumeDir:         filepath.Join(tempDir, "volumes"),
		CloudHypervisor:   "/usr/local/bin/cloud-hypervisor",
		HeartbeatInterval: 30 * time.Second,
	}

	srv, err := New(cfg)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	ctx := context.Background()
	req := &agent.ImageImportRequest{
		ImageId:   "test-image",
		SourceUrl: "http://example.com/image.raw",
	}

	resp, err := srv.ImportImage(ctx, req)
	if err != nil {
		t.Fatalf("ImportImage() error = %v", err)
	}

	if !resp.Ok {
		t.Errorf("ImportImage() Ok = false, want true")
	}
}

func TestServer_ImportImage_AlreadyExists(t *testing.T) {
	tempDir := t.TempDir()
	cfg := &Config{
		ListenAddr:        "127.0.0.1:0",
		DataDir:           filepath.Join(tempDir, "data"),
		ImageDir:          tempDir,
		VolumeDir:         filepath.Join(tempDir, "volumes"),
		CloudHypervisor:   "/usr/local/bin/cloud-hypervisor",
		HeartbeatInterval: 30 * time.Second,
	}

	srv, err := New(cfg)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	// Pre-create the image file
	imagePath := filepath.Join(tempDir, "existing-image.raw")
	if err := os.WriteFile(imagePath, []byte{}, 0644); err != nil {
		t.Fatalf("Failed to create test file: %v", err)
	}

	ctx := context.Background()
	req := &agent.ImageImportRequest{
		ImageId:   "existing-image",
		SourceUrl: "http://example.com/image.raw",
	}

	resp, err := srv.ImportImage(ctx, req)
	if err != nil {
		t.Fatalf("ImportImage() error = %v", err)
	}

	if !resp.Ok {
		t.Error("ImportImage() should return Ok=true for existing image")
	}
}

func TestServer_StartStop(t *testing.T) {
	tempDir := t.TempDir()
	cfg := &Config{
		ListenAddr:        "127.0.0.1:0",
		DataDir:           filepath.Join(tempDir, "data"),
		ImageDir:          filepath.Join(tempDir, "images"),
		VolumeDir:         filepath.Join(tempDir, "volumes"),
		CloudHypervisor:   "/usr/local/bin/cloud-hypervisor",
		HeartbeatInterval: 30 * time.Second,
	}

	srv, err := New(cfg)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	// Test StartVM for non-provisioned VM
	ctx := context.Background()
	startReq := &agent.VMStateRequest{
		VmId: "non-existent-vm",
	}

	startResp, err := srv.StartVM(ctx, startReq)
	if err != nil {
		t.Fatalf("StartVM() error = %v", err)
	}

	if startResp.State != "error" {
		t.Errorf("StartVM() State = %v, want error", startResp.State)
	}

	if startResp.Error == nil {
		t.Error("StartVM() Error is nil, expected error detail")
	}

	// Test StopVM for non-running VM (should return stopped for non-existent VM)
	stopReq := &agent.VMStateRequest{
		VmId: "non-existent-vm",
	}
	stopResp, err := srv.StopVM(ctx, stopReq)
	if err != nil {
		t.Fatalf("StopVM() error = %v", err)
	}

	if stopResp.State != "stopped" {
		t.Errorf("StopVM() State = %v, want stopped", stopResp.State)
	}

	// Test GetVMState for unknown VM
	stateResp, err := srv.GetVMState(ctx, startReq)
	if err != nil {
		t.Fatalf("GetVMState() error = %v", err)
	}

	if stateResp.State != "unknown" {
		t.Errorf("GetVMState() State = %v, want unknown", stateResp.State)
	}
}
