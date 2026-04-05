// Package server provides the gRPC server implementation for the CHV Agent.
package server

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net"
	"os"
	"path/filepath"
	"time"

	"github.com/chv/chv/internal/agent/cloudinit"
	"github.com/chv/chv/internal/agent/console"
	"github.com/chv/chv/internal/hypervisor"
	"github.com/chv/chv/internal/network"
	"github.com/chv/chv/internal/nodevalidate"
	"github.com/chv/chv/internal/pb/agent"
	"github.com/chv/chv/internal/storage"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

// Config holds the gRPC server configuration.
type Config struct {
	NodeID            string
	ControllerAddr    string
	ListenAddr        string
	DataDir           string
	ImageDir          string
	VolumeDir         string
	CloudHypervisor   string
	HeartbeatInterval time.Duration
}

// Server implements the AgentService gRPC server.
type Server struct {
	agent.UnimplementedAgentServiceServer

	config         *Config
	validator      *nodevalidate.Validator
	storage        *storage.Manager
	launcher       *hypervisor.Launcher
	isoGenerator   *cloudinit.ISOGenerator
	consoleManager *console.Manager
	nodeID         string
	hostname       string
	grpcServer     *grpc.Server
	httpServer     *HTTPServer
}

// New creates a new gRPC server instance.
func New(cfg *Config) (*Server, error) {
	hostname, _ := os.Hostname()

	// Initialize directories
	for _, dir := range []string{cfg.DataDir, cfg.ImageDir, cfg.VolumeDir} {
		if err := os.MkdirAll(dir, 0755); err != nil {
			return nil, fmt.Errorf("failed to create directory %s: %w", dir, err)
		}
	}

	// Initialize supporting components
	stateManager := hypervisor.NewStateManager(filepath.Join(cfg.DataDir, "instances"))
	if err := stateManager.Initialize(); err != nil {
		return nil, fmt.Errorf("failed to initialize state manager: %w", err)
	}

	tapManager := network.NewTAPManager("br0")
	isoGenerator := cloudinit.NewISOGenerator(cfg.DataDir)

	// Initialize validator
	validator := nodevalidate.NewValidator(cfg.CloudHypervisor)

	// Initialize launcher
	launcher := hypervisor.NewLauncher(
		cfg.CloudHypervisor,
		filepath.Join(cfg.DataDir, "instances"),
		filepath.Join(cfg.DataDir, "logs"),
		filepath.Join(cfg.DataDir, "sockets"),
		stateManager,
		tapManager,
		isoGenerator,
	)
	if err := launcher.Initialize(); err != nil {
		return nil, fmt.Errorf("failed to initialize launcher: %w", err)
	}

	// Recover any running VMs from previous session
	if err := launcher.Recover(); err != nil {
		log.Printf("Warning: failed to recover VM state: %v", err)
	}

	return &Server{
		config:         cfg,
		validator:      validator,
		storage:        storage.NewManager(cfg.VolumeDir),
		launcher:       launcher,
		isoGenerator:   isoGenerator,
		consoleManager: console.NewManager(filepath.Join(cfg.DataDir, "logs")),
		nodeID:         cfg.NodeID,
		hostname:       hostname,
	}, nil
}

// Start starts the gRPC server and HTTP server.
func (s *Server) Start() error {
	lis, err := net.Listen("tcp", s.config.ListenAddr)
	if err != nil {
		return fmt.Errorf("failed to listen on %s: %w", s.config.ListenAddr, err)
	}

	s.grpcServer = grpc.NewServer()
	agent.RegisterAgentServiceServer(s.grpcServer, s)

	log.Printf("Starting CHV Agent gRPC server on %s", s.config.ListenAddr)
	
	go func() {
		if err := s.grpcServer.Serve(lis); err != nil {
			log.Fatalf("gRPC server error: %v", err)
		}
	}()

	// Start HTTP server for WebSocket console access
	// Use a different port than gRPC (gRPC port + 1)
	httpAddr := s.config.ListenAddr
	if len(httpAddr) > 0 && httpAddr[0] == ':' {
		// Extract port and add 1000
		var port int
		fmt.Sscanf(httpAddr, ":%d", &port)
		httpAddr = fmt.Sprintf(":%d", port+1000)
	}

	s.httpServer = NewHTTPServer(httpAddr, s.consoleManager)
	if err := s.httpServer.Start(); err != nil {
		return fmt.Errorf("failed to start HTTP server: %w", err)
	}

	return nil
}

// Stop gracefully stops the gRPC server.
func (s *Server) Stop() {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	if s.httpServer != nil {
		s.httpServer.Stop(ctx)
	}
	if s.consoleManager != nil {
		s.consoleManager.Close()
	}
	if s.grpcServer != nil {
		s.grpcServer.GracefulStop()
	}
}

// ForceStop force stops the gRPC server.
func (s *Server) ForceStop() {
	if s.grpcServer != nil {
		s.grpcServer.Stop()
	}
}

// Ping implements health check.
func (s *Server) Ping(ctx context.Context, req *agent.Empty) (*agent.PingResponse, error) {
	return &agent.PingResponse{
		Ok:      true,
		Version: "0.1.0",
	}, nil
}

// ReportNodeStatus returns node status.
func (s *Server) ReportNodeStatus(ctx context.Context, req *agent.Empty) (*agent.NodeStatus, error) {
	// Get system info
	result, _ := s.validator.Validate(ctx)

	var caps []*agent.NodeCapability
	for k, v := range result.Capabilities {
		caps = append(caps, &agent.NodeCapability{
			Key:   k,
			Value: v,
		})
	}

	return &agent.NodeStatus{
		NodeId:              s.nodeID,
		Hostname:            s.hostname,
		State:               "online",
		TotalCpuCores:       result.TotalCPUCores,
		TotalRamMb:          result.TotalRAMMB,
		AllocatableCpuCores: result.TotalCPUCores,
		AllocatableRamMb:    result.TotalRAMMB,
		Capabilities:        caps,
		AgentVersion:        "0.1.0",
		HypervisorVersion:   result.HypervisorVersion,
		LastHeartbeatUnix:   time.Now().Unix(),
	}, nil
}

// ValidateNode validates the node.
func (s *Server) ValidateNode(ctx context.Context, req *agent.Empty) (*agent.NodeValidateResponse, error) {
	result, err := s.validator.Validate(ctx)
	if err != nil {
		return &agent.NodeValidateResponse{
			Ok: false,
			Errors: []*agent.ErrorDetail{
				{
					Code:    "VALIDATION_ERROR",
					Message: err.Error(),
				},
			},
		}, nil
	}

	var errors []*agent.ErrorDetail
	for _, e := range result.Errors {
		errors = append(errors, &agent.ErrorDetail{
			Code:      e.Code,
			Message:   e.Message,
			Retryable: e.Retryable,
			Hint:      e.Hint,
		})
	}

	return &agent.NodeValidateResponse{
		Ok:     result.OK,
		Errors: errors,
	}, nil
}

// EnsureBridge ensures a bridge exists.
func (s *Server) EnsureBridge(ctx context.Context, req *agent.EnsureBridgeRequest) (*agent.NodeValidateResponse, error) {
	tapManager := network.NewTAPManager(req.BridgeName)
	if err := tapManager.EnsureBridge(req.BridgeName); err != nil {
		return &agent.NodeValidateResponse{
			Ok: false,
			Errors: []*agent.ErrorDetail{
				{
					Code:      "BRIDGE_CREATE_FAILED",
					Message:   err.Error(),
					Retryable: true,
				},
			},
		}, nil
	}

	return &agent.NodeValidateResponse{Ok: true}, nil
}

// ImportImage imports and normalizes an image.
func (s *Server) ImportImage(ctx context.Context, req *agent.ImageImportRequest) (*agent.NodeValidateResponse, error) {
	imagePath := filepath.Join(s.config.ImageDir, req.ImageId+".raw")

	// Check if already imported
	if _, err := os.Stat(imagePath); err == nil {
		return &agent.NodeValidateResponse{Ok: true}, nil
	}

	log.Printf("Importing image %s from %s to %s", req.ImageId, req.SourceUrl, imagePath)

	// For MVP-1, we simulate image import
	// In production, this would download and convert the image
	f, err := os.Create(imagePath)
	if err != nil {
		return &agent.NodeValidateResponse{
			Ok: false,
			Errors: []*agent.ErrorDetail{
				{
					Code:      "IMPORT_FAILED",
					Message:   fmt.Sprintf("Failed to create image file: %v", err),
					Retryable: true,
				},
			},
		}, nil
	}
	f.Close()

	return &agent.NodeValidateResponse{Ok: true}, nil
}

// CreateVolume creates a volume.
func (s *Server) CreateVolume(ctx context.Context, req *agent.VolumeCreateRequest) (*agent.NodeValidateResponse, error) {
	volumePath := filepath.Join(req.PoolPath, req.VolumeId+".raw")

	// Check if already exists
	if _, err := os.Stat(volumePath); err == nil {
		return &agent.NodeValidateResponse{Ok: true}, nil
	}

	// Create raw disk file
	if err := s.storage.CreateRawVolume(volumePath, int64(req.SizeBytes)); err != nil {
		return &agent.NodeValidateResponse{
			Ok: false,
			Errors: []*agent.ErrorDetail{
				{
					Code:      "VOLUME_CREATE_FAILED",
					Message:   fmt.Sprintf("Failed to create volume: %v", err),
					Retryable: true,
				},
			},
		}, nil
	}

	return &agent.NodeValidateResponse{Ok: true}, nil
}

// ResizeVolume resizes a volume.
func (s *Server) ResizeVolume(ctx context.Context, req *agent.VolumeResizeRequest) (*agent.NodeValidateResponse, error) {
	volumePath := filepath.Join(req.PoolPath, req.VolumeId+".raw")

	if err := s.storage.ResizeRawVolume(volumePath, int64(req.NewSizeBytes)); err != nil {
		return &agent.NodeValidateResponse{
			Ok: false,
			Errors: []*agent.ErrorDetail{
				{
					Code:      "RESIZE_FAILED",
					Message:   fmt.Sprintf("Failed to resize volume: %v", err),
					Retryable: false,
				},
			},
		}, nil
	}

	return &agent.NodeValidateResponse{Ok: true}, nil
}

// DeleteVolume deletes a volume.
func (s *Server) DeleteVolume(ctx context.Context, req *agent.VolumeCreateRequest) (*agent.NodeValidateResponse, error) {
	volumePath := filepath.Join(req.PoolPath, req.VolumeId+".raw")

	if err := s.storage.DeleteVolume(volumePath); err != nil && !os.IsNotExist(err) {
		return &agent.NodeValidateResponse{
			Ok: false,
			Errors: []*agent.ErrorDetail{
				{
					Code:      "DELETE_FAILED",
					Message:   fmt.Sprintf("Failed to delete volume: %v", err),
					Retryable: true,
				},
			},
		}, nil
	}

	return &agent.NodeValidateResponse{Ok: true}, nil
}

// ListHostVMs lists VMs on this host.
func (s *Server) ListHostVMs(ctx context.Context, req *agent.Empty) (*agent.ListVMsResponse, error) {
	instances := s.launcher.ListInstances()

	// Always return a non-nil slice
	vmInfos := make([]*agent.ListVMsResponse_VMInfo, 0, len(instances))
	for _, instance := range instances {
		vmInfos = append(vmInfos, &agent.ListVMsResponse_VMInfo{
			VmId: instance.VMID,
			Pid:  fmt.Sprintf("%d", instance.PID),
		})
	}

	return &agent.ListVMsResponse{
		Vms: vmInfos,
	}, nil
}

// PrepareDrain prepares node for maintenance.
func (s *Server) PrepareDrain(ctx context.Context, req *agent.DrainRequest) (*agent.DrainResponse, error) {
	// List active VMs
	instances := s.launcher.ListInstances()

	// Always return a non-nil slice
	activeVMs := make([]string, 0, len(instances))
	for _, instance := range instances {
		activeVMs = append(activeVMs, instance.VMID)
	}

	return &agent.DrainResponse{
		Ok:        len(activeVMs) == 0,
		ActiveVms: activeVMs,
	}, nil
}

// Helper methods

// storeVMConfig stores VM configuration for later retrieval.
func (s *Server) storeVMConfig(vmID string, config *hypervisor.VMConfig) error {
	configPath := filepath.Join(s.config.DataDir, "configs", vmID+".json")

	// Ensure directory exists
	if err := os.MkdirAll(filepath.Dir(configPath), 0750); err != nil {
		return err
	}

	data, err := json.Marshal(config)
	if err != nil {
		return err
	}

	return os.WriteFile(configPath, data, 0640)
}

// loadVMConfig loads stored VM configuration.
func (s *Server) loadVMConfig(vmID string) (*hypervisor.VMConfig, error) {
	configPath := filepath.Join(s.config.DataDir, "configs", vmID+".json")

	data, err := os.ReadFile(configPath)
	if err != nil {
		return nil, err
	}

	var config hypervisor.VMConfig
	if err := json.Unmarshal(data, &config); err != nil {
		return nil, err
	}

	return &config, nil
}

// createVolumeFromImage creates a volume by copying an image.
func (s *Server) createVolumeFromImage(volumePath, imagePath string) error {
	// Check if image exists
	if _, err := os.Stat(imagePath); os.IsNotExist(err) {
		return fmt.Errorf("backing image not found: %s", imagePath)
	}

	// Copy image to volume path
	input, err := os.ReadFile(imagePath)
	if err != nil {
		return fmt.Errorf("failed to read image: %w", err)
	}

	if err := os.WriteFile(volumePath, input, 0640); err != nil {
		return fmt.Errorf("failed to write volume: %w", err)
	}

	return nil
}

// grpcError converts an error to a gRPC status error.
func grpcError(code codes.Code, msg string, err error) error {
	if err != nil {
		return status.Errorf(code, "%s: %v", msg, err)
	}
	return status.Errorf(code, "%s", msg)
}

// StreamConsole streams console data bidirectionally between controller and VM.
func (s *Server) StreamConsole(stream agent.AgentService_StreamConsoleServer) error {
	// First message should contain VM ID
	msg, err := stream.Recv()
	if err != nil {
		return status.Errorf(codes.InvalidArgument, "failed to receive initial message: %v", err)
	}

	vmID := msg.GetVmId()
	if vmID == "" {
		return status.Errorf(codes.InvalidArgument, "VM ID required in initial message")
	}

	// Get or create session
	session, err := s.consoleManager.GetOrCreateSession(vmID)
	if err != nil {
		return status.Errorf(codes.Internal, "failed to create console session: %v", err)
	}

	// Create a unique client for this stream
	clientID := fmt.Sprintf("grpc-%p", stream)
	client := session.AddClient(clientID)
	defer session.RemoveClient(clientID)

	ctx := stream.Context()

	// Send initial history
	history := session.GetHistory()
	if len(history) > 0 {
		if err := stream.Send(&agent.ConsoleStreamResponse{
			Type: agent.ConsoleStreamResponse_HISTORY,
			Data: history,
		}); err != nil {
			return status.Errorf(codes.Internal, "failed to send history: %v", err)
		}
	}

	// Create error channel
	errChan := make(chan error, 2)

	// Goroutine to read from console and send to gRPC stream
	go func() {
		for {
			select {
			case <-ctx.Done():
				errChan <- ctx.Err()
				return
			case data, ok := <-client.Send:
				if !ok {
					errChan <- io.EOF
					return
				}

				resp := &agent.ConsoleStreamResponse{
					Type: agent.ConsoleStreamResponse_OUTPUT,
					Data: data,
				}

				if err := stream.Send(resp); err != nil {
					errChan <- err
					return
				}
			}
		}
	}()

	// Goroutine to read from gRPC stream and send to console
	go func() {
		for {
			msg, err := stream.Recv()
			if err != nil {
				errChan <- err
				return
			}

			switch msg.GetType() {
			case agent.ConsoleStreamRequest_INPUT:
				// Write input to session
				if err := session.WriteInput(msg.GetData()); err != nil {
					// Input error - send error response
					stream.Send(&agent.ConsoleStreamResponse{
						Type:    agent.ConsoleStreamResponse_ERROR,
						Message: err.Error(),
					})
				}

			case agent.ConsoleStreamRequest_RESIZE:
				// Handle resize - acknowledged but not fully implemented in MVP-1
				stream.Send(&agent.ConsoleStreamResponse{
					Type:    agent.ConsoleStreamResponse_STATUS,
					Message: "Resize acknowledged",
				})

			case agent.ConsoleStreamRequest_PING:
				// Send pong
				stream.Send(&agent.ConsoleStreamResponse{
					Type:    agent.ConsoleStreamResponse_STATUS,
					Message: "pong",
				})
			}
		}
	}()

	// Wait for error or context cancellation
	return <-errChan
}

// GetConsoleStatus returns the status of a VM console session.
func (s *Server) GetConsoleStatus(ctx context.Context, req *agent.ConsoleStatusRequest) (*agent.ConsoleStatusResponse, error) {
	vmID := req.GetVmId()
	if vmID == "" {
		return nil, status.Errorf(codes.InvalidArgument, "VM ID required")
	}

	session, exists := s.consoleManager.GetSession(vmID)
	if !exists {
		return &agent.ConsoleStatusResponse{
			VmId:      vmID,
			Active:    false,
			Connected: false,
		}, nil
	}

	return &agent.ConsoleStatusResponse{
		VmId:        vmID,
		Active:      session.IsActive(),
		Connected:   session.ClientCount() > 0,
		ClientCount: int32(session.ClientCount()),
		LogPath:     session.LogPath,
	}, nil
}

// CloseConsole closes a console session.
func (s *Server) CloseConsole(ctx context.Context, req *agent.ConsoleCloseRequest) (*agent.ConsoleCloseResponse, error) {
	vmID := req.GetVmId()
	if vmID == "" {
		return nil, status.Errorf(codes.InvalidArgument, "VM ID required")
	}

	s.consoleManager.RemoveSession(vmID)

	return &agent.ConsoleCloseResponse{
		VmId:   vmID,
		Closed: true,
	}, nil
}
