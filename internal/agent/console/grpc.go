// Package console provides VM serial console streaming for the agent.
package console

import (
	"context"
	"fmt"
	"io"

	"github.com/chv/chv/internal/pb/agent"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

// GRPCServer implements gRPC streaming for console data.
type GRPCServer struct {
	manager *Manager
}

// NewGRPCServer creates a new gRPC console server.
func NewGRPCServer(manager *Manager) *GRPCServer {
	return &GRPCServer{
		manager: manager,
	}
}

// StreamConsole implements the gRPC streaming console endpoint.
// This streams console data bidirectionally between the controller and agent.
func (s *GRPCServer) StreamConsole(stream agent.AgentService_StreamConsoleServer) error {
	// First message should contain VM ID and authentication
	msg, err := stream.Recv()
	if err != nil {
		return status.Errorf(codes.InvalidArgument, "failed to receive initial message: %v", err)
	}

	// Extract VM ID from the initial message
	vmID := msg.GetVmId()
	if vmID == "" {
		return status.Errorf(codes.InvalidArgument, "VM ID required in initial message")
	}

	// Get or create session
	session, err := s.manager.GetOrCreateSession(vmID)
	if err != nil {
		return status.Errorf(codes.Internal, "failed to create console session: %v", err)
	}

	// Create a unique client for this gRPC stream
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
				// Handle resize
				if err := client.handleResize(int(msg.GetCols()), int(msg.GetRows())); err != nil {
					stream.Send(&agent.ConsoleStreamResponse{
						Type:    agent.ConsoleStreamResponse_ERROR,
						Message: fmt.Sprintf("Resize failed: %v", err),
					})
				} else {
					stream.Send(&agent.ConsoleStreamResponse{
						Type:    agent.ConsoleStreamResponse_STATUS,
						Message: "Resize successful",
					})
				}

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
func (s *GRPCServer) GetConsoleStatus(ctx context.Context, req *agent.ConsoleStatusRequest) (*agent.ConsoleStatusResponse, error) {
	vmID := req.GetVmId()
	if vmID == "" {
		return nil, status.Errorf(codes.InvalidArgument, "VM ID required")
	}

	session, exists := s.manager.GetSession(vmID)
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
		TtyEnabled:  session.ttyEnabled,
	}, nil
}

// CloseConsole closes a console session.
func (s *GRPCServer) CloseConsole(ctx context.Context, req *agent.ConsoleCloseRequest) (*agent.ConsoleCloseResponse, error) {
	vmID := req.GetVmId()
	if vmID == "" {
		return nil, status.Errorf(codes.InvalidArgument, "VM ID required")
	}

	s.manager.RemoveSession(vmID)

	return &agent.ConsoleCloseResponse{
		VmId:   vmID,
		Closed: true,
	}, nil
}
