# Runtime Sequences

## Boot sequence
1. host OS starts
2. installer-provided prerequisites are verified
3. `chv-agent` starts and loads the local durable cache
4. `chv-stord` and `chv-nwd` start in parallel
5. `chv-agent` evaluates service readiness
6. node reports `HostReady`, then `StorageReady`, then `NetworkReady`
7. node enters `TenantReady`
8. control plane may schedule new workloads

## VM create sequence
1. control plane issues `CreateVm`
2. `chv-agent` validates node readiness and desired-state generation
3. `chv-agent` asks `chv-stord` to open/prepare required volumes
4. `chv-agent` asks `chv-nwd` to ensure topology and attach NICs
5. `chv-agent` prepares Cloud Hypervisor config and API socket
6. `chv-agent` launches the Cloud Hypervisor process
7. `chv-agent` boots the VM via the local API socket
8. observed state is reported upstream

## Volume attach sequence
1. desired attach arrives
2. `chv-agent` validates VM and node state
3. `chv-agent` calls `AttachVolumeToVm` on `chv-stord`
4. `chv-stord` returns export metadata
5. `chv-agent` performs the VMM hotplug action
6. result is reported through telemetry

## Network exposure sequence
1. desired exposure arrives
2. `chv-agent` validates policy
3. `chv-agent` calls `ExposeService` on `chv-nwd`
4. `chv-nwd` applies routing/NAT/firewall/LB config
5. health result is reported upstream
