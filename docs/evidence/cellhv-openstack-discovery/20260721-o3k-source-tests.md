# O3K Source and Test Evidence

**Recorded:** 2026-07-21  
**Repository:** `https://github.com/kubedoio/o3k`  
**Revision:** `53fd2cb36ee79f42da49c8181d6ceed12b41b3aa`  
**Tier:** T0 only

The following command was executed against the pinned checkout:

```bash
go test ./pkg/hypervisor ./internal/tunnel ./internal/nova ./test/contract/nova
```

Results:

- `pkg/hypervisor`: pass;
- `internal/tunnel`: pass;
- `internal/nova`: pass;
- `test/contract/nova`: not executed successfully because no O3K Keystone
  service was listening at `localhost:35357`; requests failed before API
  contract behavior was exercised.

The passing packages prove only O3K's source-level unit behavior. They do not
prove a running O3K deployment, Core API integration, KVM behavior, upstream
Nova behavior, libvirt compatibility, or OpenStack support. No service or VM
was started and the CHV workspace host was not mutated.
