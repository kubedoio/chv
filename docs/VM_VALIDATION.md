# VM Validation Mechanism

The CHV (Cloud Hypervisor Virtualization) system includes a comprehensive VM validation mechanism that can scan running cloud-hypervisor processes from the command line and validate them against the expected state.

## Overview

The validation mechanism provides:

1. **Process Discovery**: Scans `/proc` to find all running cloud-hypervisor processes
2. **VM Information Extraction**: Parses command line arguments to extract VM configuration
3. **State Comparison**: Compares running VMs against expected state from the database
4. **Issue Detection**: Identifies orphan VMs (running but not managed) and missing VMs (expected but not running)

## API Endpoints

### Controller API

#### POST `/api/v1/vms/validate`

Validates running VMs by comparing them against the VMs marked as "running" in the controller database.

**Authentication**: Bearer token required

**Response**:
```json
{
  "validation": {
    "running_vms": [...],
    "orphan_vms": [...],
    "missing_vm_ids": [...],
    "valid_vms": [...],
    "summary": {
      "total_running": 2,
      "valid": 2,
      "orphans": 0,
      "missing": 0
    }
  },
  "expected": ["vm-id-1", "vm-id-2"]
}
```

### Agent API

#### POST `/v1/vms/validate`

Direct validation against the agent. Can be used with or without expected VM IDs.

**Request Body**:
```json
{
  "expected_vm_ids": ["vm-id-1", "vm-id-2"],
  "data_root": "/var/lib/chv"
}
```

**Response**:
```json
{
  "running_vms": [...],
  "orphan_vms": [...],
  "missing_vm_ids": [...],
  "valid_vms": [...],
  "summary": {
    "total_running": 2,
    "valid": 1,
    "orphans": 1,
    "missing": 1
  }
}
```

#### GET `/v1/vms/details?pid=<pid>`
#### GET `/v1/vms/details?vm_id=<vm_id>`

Get detailed information about a specific running VM by PID or VM ID.

## CLI Tool

The `chv-validator` tool provides command-line access to the validation functionality.

### Installation

```bash
go build -o chv-validator ./cmd/chv-validator
```

### Usage

#### Controller Mode (default)

Validates VMs through the controller API:

```bash
# Basic validation
./chv-validator

# With custom controller URL
./chv-validator -controller http://chv-controller:8080

# With authentication
./chv-validator -token <your-auth-token>
# or
export CHV_TOKEN=<your-auth-token>
./chv-validator

# Verbose output
./chv-validator -v

# JSON output
./chv-validator -format json
```

#### Direct Agent Mode

Validates VMs directly through the agent API:

```bash
./chv-validator -agent http://localhost:9090
```

### Example Output

```
======================================================================
VM VALIDATION REPORT
======================================================================

SUMMARY
-------
Total Running VMs: 2
Valid (managed):   2
Orphans:           0
Missing:           0

RUNNING VMs
-----------
PID     VM ID                           VCPU  MEM   IP              MANAGED
486799  c92d2159-0635-48d6-81ea-2256...  2     2048M               yes
788804  7531f6c9-cb56-43ec-9cb0-bda0...  1     1024M               yes

VALID VMs (MANAGED)
-------------------
PID     VM ID                           VCPU  MEM   IP
486799  c92d2159-0635-48d6-81ea-2256...  2     2048M
788804  7531f6c9-cb56-43ec-9cb0-bda0...  1     1024M

✅ All VMs are valid!
```

### Handling Issues

When orphan or missing VMs are detected:

```
⚠️  ORPHAN VMs (NOT MANAGED)
------------------------------
PID:      999999
VM ID:    unknown-999999
VCPU:     2
Memory:   2048 MB
Disk:     /some/other/path/disk.qcow2
Socket:   /some/other/path/api.sock

❌ MISSING VMs (EXPECTED BUT NOT RUNNING)
------------------------------------------
  - vm-id-that-should-be-running

⚠️  Validation found issues!
```

The tool exits with code 1 when issues are found, making it suitable for monitoring scripts.

## Data Structures

### RunningVMInfo

```go
type RunningVMInfo struct {
    PID           int    `json:"pid"`
    VMID          string `json:"vm_id"`
    SocketPath    string `json:"socket_path"`
    DiskPath      string `json:"disk_path"`
    SeedISOPath   string `json:"seed_iso_path"`
    VCPU          int    `json:"vcpu"`
    MemoryMB      int    `json:"memory_mb"`
    TAPDevice     string `json:"tap_device"`
    MACAddress    string `json:"mac_address"`
    IPAddress     string `json:"ip_address"`
    KernelPath    string `json:"kernel_path"`
    CommandLine   string `json:"command_line"`
    IsManaged     bool   `json:"is_managed"`
    WorkspacePath string `json:"workspace_path"`
}
```

## Integration with Reconciliation

The validation mechanism complements the existing reconciliation loop by:

1. **On-Demand Validation**: Allows administrators to manually trigger a full validation
2. **External Discovery**: Can discover VMs that were started outside of CHV management
3. **Forensic Analysis**: Provides detailed information about running VMs for troubleshooting

The reconciliation loop handles day-to-day state synchronization, while the validation tool is designed for:
- Manual audits
- CI/CD pipeline validation
- Troubleshooting
- Integration with monitoring systems

## Security Considerations

1. The validation endpoint requires authentication (Bearer token)
2. The agent runs with root privileges to read `/proc` entries
3. No sensitive data (like passwords) is exposed through the validation API
4. Command line arguments are captured but may contain file paths

## Troubleshooting

### No VMs Found

If the validator reports no VMs when you know VMs are running:

1. Verify the agent is running: `curl http://localhost:9090/health`
2. Check agent logs for errors
3. Verify cloud-hypervisor processes are running: `ps -ef | grep cloud-hypervisor`

### Permission Denied

The agent needs to read `/proc/[pid]/cmdline` for all processes. Ensure the agent runs with appropriate privileges.

### Incorrect VM IDs

VM IDs are extracted from socket paths. Ensure your VMs have `--api-socket` configured with a path that includes the VM ID:
```
--api-socket /var/lib/chv/vms/<vm-id>/api.sock
```
