# WebUI State and Tasks Spec

## Backend-aligned state model

### Node states
- discovered
- bootstrapping
- host_ready
- storage_ready
- network_ready
- tenant_ready
- degraded
- draining
- maintenance
- failed

### VM states
- creating
- stopped
- starting
- running
- stopping
- rebooting
- deleting
- failed
- unknown

### Task states
- queued
- running
- succeeded
- failed
- cancelled

## UI behavior rules
- degraded and failed states always show banners or badges
- unknown states never look healthy
- task pages show duration and failure reason where available
- resource pages show latest relevant tasks inline

## Consistency rules
- optimistic UI must be limited
- accepted task != completed action
- UI must show accepted, in-progress, and completed distinctly
