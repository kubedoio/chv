# Specification: Security, RBAC and Secrets

## Security principles

1. No raw secrets in YAML.
2. No raw private SSH keys in YAML.
3. Public SSH keys are allowed.
4. Secret references must use `secret_ref`.
5. Deployment requires explicit permissions.
6. Destructive changes require extra confirmation.
7. Platform users and instance OS users must remain separate.
8. Every apply run must be auditable.

## Secret handling

Forbidden:

```yaml
password: my-secret-password
token: abc123
private_key: -----BEGIN PRIVATE KEY-----
```

Allowed:

```yaml
secret_ref: pbs-main-token
```

## MVP secret backend

MVP may use CHV's own encrypted secret table.

Future options:

```text
Vault
Kubernetes Secrets
external secret manager
```

## Permission model

Suggested permissions:

```text
architecture:read
architecture:create
architecture:update
architecture:delete
architecture:validate
architecture:check_fleet
architecture:plan
architecture:apply
architecture:destroy
architecture:export
architecture:import
architecture:read_runs
architecture:read_drift
```

Resource permissions used by apply:

```text
host:read
network:create
network:update
network:delete
datastore:read
image:create
image:read
template:create
template:update
instance:create
instance:update
instance:delete
user:create
role:assign
backup_policy:bind
```

## Apply authorization

A user may create/edit a topology but not apply it.

Recommended roles:

```text
architecture-viewer
architecture-designer
architecture-operator
architecture-admin
```

## Production protection

If `metadata.environment = production`, require stronger confirmation:

```text
- typed architecture name
- warning acknowledgement
- permission architecture:apply:production
```

## Audit events

Emit events for:

```text
architecture_created
architecture_updated
architecture_validated
architecture_plan_created
architecture_plan_discarded
architecture_apply_started
architecture_apply_succeeded
architecture_apply_failed
architecture_drift_detected
architecture_exported
```
