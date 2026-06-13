# Contract: CHVArchitecture YAML v1alpha1

## Identity

```yaml
apiVersion: chv.kubedo.io/v1alpha1
kind: CHVArchitecture
```

## Top-level sections

```yaml
metadata: {}
servers: []
networks: []
datastores: []
backup_targets: []
backup_policies: []
images: []
templates: []
instances: []
ssh_keys: []
instance_users: []
roles: []
users: []
projects: []
```

## Rules

1. `apiVersion` and `kind` are required.
2. `metadata.name` is required and must be unique within CHV.
3. All list item `name` fields must be unique within their section.
4. Cross-references must point to existing objects.
5. Raw passwords, tokens and private keys are forbidden.
6. Secrets must be referenced via `secret_ref`.
7. Platform users and instance OS users must remain separate.
8. Every deployable resource must be plan-able before apply.

## Metadata

```yaml
metadata:
  name: customer-a-production
  display_name: Customer A Production
  description: Production topology for Customer A
  environment: production
  owner: senol@example.com
  labels:
    customer: customer-a
    criticality: high
```

## Servers

Servers are CHV hosts/hypervisors. MVP should register or reference hosts, not perform bare-metal provisioning.

```yaml
servers:
  - name: chv-node-01
    management_ip: 10.10.0.11
    role: compute
    labels:
      zone: rack-a
      storage: nvme
    resources:
      cpu_cores: 32
      memory_gb: 256
    networks:
      interfaces:
        - name: eno1
          purpose: management
        - name: eno2
          purpose: vm_trunk
```

Allowed roles:

```text
compute
storage
network
management
mixed
```

MVP roles:

```text
compute
mixed
```

## Networks

```yaml
networks:
  - name: tenant-prod
    type: vlan
    bridge: chv-br0
    vlan_id: 120
    cidr: 10.120.0.0/24
    gateway: 10.120.0.1
    dns:
      - 1.1.1.1
    dhcp:
      enabled: true
      range_start: 10.120.0.100
      range_end: 10.120.0.200
```

Allowed types:

```text
bridge
vlan
nat
isolated
routed
```

MVP types:

```text
bridge
vlan
nat
```

## Datastores

```yaml
datastores:
  - name: local-nvme
    type: qcow2-dir
    path: /var/lib/chv/datastores/local-nvme
    capabilities:
      snapshots: true
      thin_provisioning: true
      online_resize: true
```

Allowed types:

```text
qcow2-dir
ceph-rbd
nfs
lvm
zfs
```

Backup targets are separate and must not be modeled as normal datastores.

## Backup targets

```yaml
backup_targets:
  - name: pbs-main
    type: proxmox-backup-server
    endpoint: https://pbs.example.com:8007
    datastore: chv-backups
    user: chv-backup@pbs
    secret_ref: pbs-main-token
```

## Images

```yaml
images:
  - name: ubuntu-24.04
    source: local://images/ubuntu-24.04.qcow2
    format: qcow2
    datastore: local-nvme
```

## Templates

```yaml
templates:
  - name: small-linux
    image: ubuntu-24.04
    cpu: 2
    memory_mb: 4096
    disk_gb: 40
    datastore: local-nvme
    network: tenant-prod
```

## Instances

```yaml
instances:
  - name: app-01
    template: small-linux
    placement:
      server: chv-node-01
    resources:
      cpu: 4
      memory_mb: 8192
    disks:
      - name: root
        size_gb: 60
        datastore: local-nvme
    networks:
      - name: tenant-prod
        ip: 10.120.0.21
    cloud_init:
      hostname: app-01
      users:
        - ref: admin
    backup:
      enabled: true
      policy: daily-retain-14
```

## Instance users

```yaml
instance_users:
  - name: admin
    sudo: true
    shell: /bin/bash
    ssh_authorized_keys:
      - ref: senol-main-key
```

## Platform users

```yaml
users:
  - name: operator-a
    display_name: Operator A
    email: operator@example.com
    auth:
      type: oidc
      subject: oidc-subject-id
    roles:
      - project-operator
```

## Roles

```yaml
roles:
  - name: project-operator
    permissions:
      - architecture:read
      - instance:read
      - instance:start
      - instance:stop
      - console:access
```
