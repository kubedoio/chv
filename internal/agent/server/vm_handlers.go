// Package server provides the gRPC server implementation for the CHV Agent.
package server

import (
	"context"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"time"

	"github.com/chv/chv/internal/agent/cloudinit"
	"github.com/chv/chv/internal/agent/metadata"
	"github.com/chv/chv/internal/hypervisor"
	"github.com/chv/chv/internal/pb/agent"
	"github.com/chv/chv/internal/validation"
)

// ProvisionVM provisions a new VM on the agent.
// It prepares the disk, creates cloud-init config if provided, and stores the VM configuration.
func (s *Server) ProvisionVM(ctx context.Context, req *agent.ProvisionVMRequest) (*agent.VMStateResponse, error) {
	log.Printf("Provisioning VM %s (%s)", req.VmName, req.VmId)
	
	// Debug: log cloud-init config
	if req.CloudInit != nil {
		log.Printf("CloudInit received - UserData: %v, MetaData: %v, NetworkConfig: %v", 
			req.CloudInit.UserData != "", req.CloudInit.MetaData != "", req.CloudInit.NetworkConfig != "")
		if req.CloudInit.NetworkConfig != "" {
			log.Printf("NetworkConfig content: %s", req.CloudInit.NetworkConfig)
		}
	} else {
		log.Printf("CloudInit is nil")
	}

	// Check if VM is already provisioned or running
	instance := s.launcher.GetInstance(req.VmId)
	if instance != nil {
		return &agent.VMStateResponse{
			VmId:  req.VmId,
			State: "running",
			Pid:   fmt.Sprintf("%d", instance.PID),
		}, nil
	}

	// Check if VM exists in state
	state, _ := s.launcher.GetVMState(req.VmId)
	if state != "" && state != "unknown" {
		return &agent.VMStateResponse{
			VmId:  req.VmId,
			State: state,
		}, nil
	}

	// Determine volume path from disk attachments
	var volumePath string
	if len(req.Disks) > 0 {
		volumePath = req.Disks[0].Path
	}

	// Create volume from backing image if specified
	if req.Boot != nil && req.Boot.BackingImageId != "" && volumePath != "" {
		// Validate backing image ID
		if err := validation.ValidateID(req.Boot.BackingImageId); err != nil {
			return &agent.VMStateResponse{
				VmId:  req.VmId,
				State: "error",
				Error: &agent.ErrorDetail{
					Code:    "INVALID_IMAGE_ID",
					Message: fmt.Sprintf("Invalid backing image ID: %v", err),
				},
			}, nil
		}
		
		// Use provided image path or construct from image ID
		imagePath := req.Boot.BackingImagePath
		if imagePath == "" {
			// Try .raw first, then .qcow2
			rawPath := filepath.Join(s.config.ImageDir, req.Boot.BackingImageId+".raw")
			qcow2Path := filepath.Join(s.config.ImageDir, req.Boot.BackingImageId+".qcow2")
			
			if _, err := os.Stat(rawPath); err == nil {
				imagePath = rawPath
			} else if _, err := os.Stat(qcow2Path); err == nil {
				imagePath = qcow2Path
			} else {
				// Default to .raw for error message clarity
				imagePath = rawPath
			}
		}
		if err := s.createVolumeFromImage(volumePath, imagePath); err != nil {
			log.Printf("Failed to create volume from image: %v", err)
			return &agent.VMStateResponse{
				VmId:  req.VmId,
				State: "error",
				Error: &agent.ErrorDetail{
					Code:    "VOLUME_CREATE_FAILED",
					Message: err.Error(),
				},
			}, nil
		}
	}

	// Create cloud-init ISO (for backwards compatibility) and register with metadata server
	var isoPath string
	var cloudInitConfig *cloudinit.Config

	if req.CloudInit != nil && (req.CloudInit.UserData != "" || req.CloudInit.MetaData != "" || req.CloudInit.NetworkConfig != "") {
		// Use provided cloud-init config
		cloudInitConfig = &cloudinit.Config{
			UserData:      req.CloudInit.UserData,
			MetaData:      req.CloudInit.MetaData,
			NetworkConfig: req.CloudInit.NetworkConfig,
		}
		var err error
		isoPath, err = s.isoGenerator.GenerateISO(req.VmId, cloudInitConfig)
		if err != nil {
			log.Printf("Failed to generate cloud-init ISO: %v", err)
		}
	} else if req.Boot != nil && req.Boot.Mode == "cloud_image" {
		// Auto-generate cloud-init for cloud_image mode with SSH key
		sshKey, err := s.getSSHPublicKey()
		if err == nil && sshKey != "" {
			userData := fmt.Sprintf(`#cloud-config
users:
  - name: ubuntu
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - %s
    shell: /bin/bash
chpasswd:
  list: |
    ubuntu:ubuntu
  expire: False
ssh_pwauth: False
`, sshKey)
			metaData := fmt.Sprintf("instance-id: %s\nlocal-hostname: %s\n", req.VmId, req.VmName)
			cloudInitConfig = &cloudinit.Config{
				UserData:      userData,
				MetaData:      metaData,
				NetworkConfig: req.CloudInit.GetNetworkConfig(),
			}
			isoPath, err = s.isoGenerator.GenerateISO(req.VmId, cloudInitConfig)
			if err != nil {
				log.Printf("Failed to generate cloud-init ISO: %v", err)
			} else {
				log.Printf("Generated cloud-init ISO for VM %s", req.VmId)
			}
		}
	}

	// Register VM with metadata server
	if s.metadataServer != nil && cloudInitConfig != nil {
		metaConfig := &metadata.Config{
			InstanceID:    req.VmId,
			Hostname:      req.VmName,
			NetworkConfig: cloudInitConfig.NetworkConfig,
			UserData:      cloudInitConfig.UserData,
			MetaData:      cloudInitConfig.MetaData,
		}
		if metaConfig.MetaData == "" {
			metaConfig.MetaData = fmt.Sprintf("instance-id: %s\nlocal-hostname: %s\n", req.VmId, req.VmName)
		}
		s.metadataServer.RegisterVM(req.VmId, metaConfig)
		log.Printf("Registered VM %s with metadata server", req.VmId)
	}

	// Create VM config for later start
	vmConfig := &hypervisor.VMConfig{
		VMID:         req.VmId,
		Name:         req.VmName,
		VCPU:         int(req.Vcpu),
		MemoryMB:     int(req.MemoryMb),
		VolumePath:   volumePath,
		CloudInitISO: isoPath,
	}

	// Store config for later use
	if err := s.storeVMConfig(req.VmId, vmConfig); err != nil {
		log.Printf("Failed to store VM config: %v", err)
	}

	return &agent.VMStateResponse{
		VmId:  req.VmId,
		State: "provisioned",
	}, nil
}

// StartVM starts a provisioned VM.
// It boots the VM using the stored configuration from the provision step.
func (s *Server) StartVM(ctx context.Context, req *agent.VMStateRequest) (*agent.VMStateResponse, error) {
	log.Printf("Starting VM %s", req.VmId)

	// Check if already running
	instance := s.launcher.GetInstance(req.VmId)
	if instance != nil {
		return &agent.VMStateResponse{
			VmId:  req.VmId,
			State: "running",
			Pid:   fmt.Sprintf("%d", instance.PID),
		}, nil
	}

	// Check if VM is already running in state
	state, err := s.launcher.GetVMState(req.VmId)
	if err == nil && state == "running" {
		return &agent.VMStateResponse{
			VmId:  req.VmId,
			State: "running",
		}, nil
	}

	// Load VM config from provision step
	vmConfig, err := s.loadVMConfig(req.VmId)
	if err != nil {
		return &agent.VMStateResponse{
			VmId:  req.VmId,
			State: "error",
			Error: &agent.ErrorDetail{
				Code:    "VM_NOT_PROVISIONED",
				Message: "VM must be provisioned before starting: " + err.Error(),
			},
		}, nil
	}

	// Verify that required files exist
	if _, err := os.Stat(vmConfig.VolumePath); os.IsNotExist(err) {
		return &agent.VMStateResponse{
			VmId:  req.VmId,
			State: "error",
			Error: &agent.ErrorDetail{
				Code:    "VM_NOT_PROVISIONED",
				Message: "VM disk not found, provision required: " + vmConfig.VolumePath,
			},
		}, nil
	}
	if vmConfig.CloudInitISO != "" {
		if _, err := os.Stat(vmConfig.CloudInitISO); os.IsNotExist(err) {
			return &agent.VMStateResponse{
				VmId:  req.VmId,
				State: "error",
				Error: &agent.ErrorDetail{
					Code:    "VM_NOT_PROVISIONED",
					Message: "Cloud-init ISO not found, provision required: " + vmConfig.CloudInitISO,
				},
			}, nil
		}
	}

	// Start the VM
	operationID := fmt.Sprintf("start-%d", time.Now().UnixNano())
	startedInstance, err := s.launcher.StartVM(vmConfig, operationID)
	if err != nil {
		return &agent.VMStateResponse{
			VmId:  req.VmId,
			State: "error",
			Error: &agent.ErrorDetail{
				Code:    "START_FAILED",
				Message: err.Error(),
			},
		}, nil
	}

	return &agent.VMStateResponse{
		VmId:  req.VmId,
		State: "running",
		Pid:   fmt.Sprintf("%d", startedInstance.PID),
	}, nil
}

// StopVM stops a running VM.
// It performs a graceful shutdown unless force is specified.
func (s *Server) StopVM(ctx context.Context, req *agent.VMStateRequest) (*agent.VMStateResponse, error) {
	log.Printf("Stopping VM %s", req.VmId)

	// Check if VM exists
	instance := s.launcher.GetInstance(req.VmId)
	if instance == nil {
		// Check if VM is in state but not in memory
		state, _ := s.launcher.GetVMState(req.VmId)
		if state == "" || state == "unknown" {
			// VM doesn't exist, consider it already stopped
			return &agent.VMStateResponse{
				VmId:  req.VmId,
				State: "stopped",
			}, nil
		}
		// VM exists in state but not running
		if state == "stopped" {
			return &agent.VMStateResponse{
				VmId:  req.VmId,
				State: "stopped",
			}, nil
		}
	}

	// Generate operation ID for idempotency
	operationID := fmt.Sprintf("stop-%d", time.Now().UnixNano())

	if err := s.launcher.StopVM(req.VmId, false, operationID); err != nil {
		return &agent.VMStateResponse{
			VmId:  req.VmId,
			State: "error",
			Error: &agent.ErrorDetail{
				Code:    "STOP_FAILED",
				Message: err.Error(),
			},
		}, nil
	}

	// Unregister VM from metadata server when stopped
	if s.metadataServer != nil {
		s.metadataServer.UnregisterVM(req.VmId)
		log.Printf("Unregistered stopped VM %s from metadata server", req.VmId)
	}

	return &agent.VMStateResponse{
		VmId:  req.VmId,
		State: "stopped",
	}, nil
}

// RebootVM reboots a running VM.
// It sends a reboot signal to the VM via the Cloud Hypervisor API.
func (s *Server) RebootVM(ctx context.Context, req *agent.VMStateRequest) (*agent.VMStateResponse, error) {
	log.Printf("Rebooting VM %s", req.VmId)

	operationID := fmt.Sprintf("reboot-%d", time.Now().UnixNano())

	if err := s.launcher.RebootVM(req.VmId, operationID); err != nil {
		return &agent.VMStateResponse{
			VmId:  req.VmId,
			State: "error",
			Error: &agent.ErrorDetail{
				Code:    "REBOOT_FAILED",
				Message: err.Error(),
			},
		}, nil
	}

	return &agent.VMStateResponse{
		VmId:  req.VmId,
		State: "running",
	}, nil
}

// DeleteVM deletes a VM and optionally its volumes.
// It first stops the VM if running, then cleans up resources.
func (s *Server) DeleteVM(ctx context.Context, req *agent.VMDeleteRequest) (*agent.VMStateResponse, error) {
	log.Printf("Deleting VM %s", req.VmId)

	// First stop if running
	instance := s.launcher.GetInstance(req.VmId)
	if instance != nil {
		operationID := fmt.Sprintf("delete-stop-%d", time.Now().UnixNano())
		if err := s.launcher.StopVM(req.VmId, req.Force, operationID); err != nil {
			return &agent.VMStateResponse{
				VmId:  req.VmId,
				State: "error",
				Error: &agent.ErrorDetail{
					Code:    "DELETE_FAILED",
					Message: err.Error(),
				},
			}, nil
		}
	}

	// Delete volumes if requested
	if req.DeleteVolumes {
		// Load VM config to get volume paths
		vmConfig, err := s.loadVMConfig(req.VmId)
		if err == nil && vmConfig != nil && vmConfig.VolumePath != "" {
			if err := s.storage.DeleteVolume(vmConfig.VolumePath); err != nil {
				log.Printf("Warning: failed to delete volume %s: %v", vmConfig.VolumePath, err)
			}
		}
	}

	// Unregister VM from metadata server
	if s.metadataServer != nil {
		s.metadataServer.UnregisterVM(req.VmId)
		log.Printf("Unregistered VM %s from metadata server", req.VmId)
	}

	// Clean up stored config
	configPath := filepath.Join(s.config.DataDir, "configs", req.VmId+".json")
	if err := s.deleteFileIfExists(configPath); err != nil {
		log.Printf("Warning: failed to delete VM config: %v", err)
	}

	return &agent.VMStateResponse{
		VmId:  req.VmId,
		State: "deleted",
	}, nil
}

// GetVMState returns the current state of a VM.
func (s *Server) GetVMState(ctx context.Context, req *agent.VMStateRequest) (*agent.VMStateResponse, error) {
	// First check if VM is running (has an active instance)
	instance := s.launcher.GetInstance(req.VmId)
	if instance != nil {
		return &agent.VMStateResponse{
			VmId:  req.VmId,
			State: "running",
			Pid:   fmt.Sprintf("%d", instance.PID),
		}, nil
	}

	// Check persisted state from state manager
	state, err := s.launcher.GetVMState(req.VmId)
	if err == nil && state != "" {
		return &agent.VMStateResponse{
			VmId:  req.VmId,
			State: state,
		}, nil
	}

	// VM not found in state - check if it exists in config
	// If config exists but no state, VM was stopped
	_, configErr := s.loadVMConfig(req.VmId)
	if configErr == nil {
		// Config exists but no running instance or state file
		// VM was provisioned but is now stopped
		return &agent.VMStateResponse{
			VmId:  req.VmId,
			State: "stopped",
		}, nil
	}

	// VM not found anywhere
	return &agent.VMStateResponse{
		VmId:  req.VmId,
		State: "unknown",
	}, nil
}

// deleteFileIfExists deletes a file if it exists, ignoring "not found" errors.
func (s *Server) deleteFileIfExists(path string) error {
	err := s.storage.DeleteVolume(path)
	if err != nil && err.Error() == "volume not found" {
		return nil
	}
	return err
}

// getSSHPublicKey reads the SSH public key for cloud-init.
// The key is generated at build time and stored in /root/.ssh/
func (s *Server) getSSHPublicKey() (string, error) {
	keyPath := "/root/.ssh/chv_id_ed25519.pub"
	data, err := os.ReadFile(keyPath)
	if err != nil {
		return "", fmt.Errorf("failed to read SSH public key: %w", err)
	}
	return string(data), nil
}
