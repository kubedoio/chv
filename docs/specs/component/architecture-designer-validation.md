# Specification: Validation and Consistency Checks

## Validation layers

CHV Architecture Designer must implement three validation layers.

## Layer 1: Static validation

Does not require live fleet access.

Checks:

```text
required fields exist
schema valid
name uniqueness per section
reference existence
CIDR validity
CIDR overlap detection
duplicate static IP detection
IP belongs to selected network
gateway belongs to network
DHCP range belongs to network
DHCP range start <= range end
static IP outside DHCP range warning
valid role permissions
valid datastore/image/template references
valid backup policy references
no raw passwords/tokens/private keys
platform users separate from instance users
```

## Layer 2: Fleet consistency check

Requires current CHV inventory.

Checks:

```text
target host exists
target host healthy
target host schedulable
target host has enough CPU
target host has enough memory
target network/bridge exists or can be created
VLAN available
IP address free
datastore exists or can be created
datastore has enough capacity
image exists or source reachable
backup target reachable
secret_ref exists
user has permission to deploy
```

## Layer 3: Deployment safety check

Checks plan risk before apply.

Checks:

```text
will delete instance
will remove disk
will reduce disk size
will change network attachment
will expose public network
will remove user/role
will deploy without backup policy
will deploy to degraded host
will exceed capacity threshold
will consume last datastore free capacity
```

## Finding codes

Suggested static codes:

```text
SCHEMA_INVALID
DUPLICATE_NAME
MISSING_REFERENCE
INVALID_CIDR
NETWORK_CIDR_OVERLAP
DUPLICATE_IP
IP_OUTSIDE_NETWORK
GATEWAY_OUTSIDE_NETWORK
DHCP_RANGE_INVALID
RAW_SECRET_FORBIDDEN
INVALID_PERMISSION
INVALID_EDGE
```

Suggested fleet codes:

```text
HOST_NOT_FOUND
HOST_NOT_HEALTHY
HOST_NOT_SCHEDULABLE
INSUFFICIENT_CPU
INSUFFICIENT_MEMORY
DATASTORE_NOT_FOUND
DATASTORE_CAPACITY_LOW
NETWORK_BRIDGE_NOT_FOUND
VLAN_NOT_AVAILABLE
IP_ALREADY_USED
IMAGE_NOT_FOUND
BACKUP_TARGET_UNREACHABLE
SECRET_REF_NOT_FOUND
PERMISSION_DENIED
```

Suggested safety codes:

```text
DESTRUCTIVE_DELETE
DISK_REMOVAL
DISK_SIZE_REDUCTION
PUBLIC_EXPOSURE_CHANGE
ROLE_REMOVAL
MISSING_BACKUP_POLICY
DEPLOY_TO_DEGRADED_HOST
SOFT_CAPACITY_EXCEEDED
```

## Blocking policy

Errors block deployment.
Warnings require explicit acknowledgement.
Info is non-blocking.

## Validation output must be stable

Findings must include stable `code` values so UI tests and future automation can depend on them.
