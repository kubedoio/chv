# WebUI Implementation Spec

## Stack
- SvelteKit
- TypeScript
- server-side data loading for core pages
- form actions or safe mutation handlers
- design system implemented as reusable component library
- BFF/API integration through server routes, not direct browser access to internal services

## Route groups
- `/`
- `/clusters`
- `/nodes`
- `/nodes/[nodeId]`
- `/vms`
- `/vms/[vmId]`
- `/volumes`
- `/volumes/[volumeId]`
- `/networks`
- `/networks/[networkId]`
- `/tasks`
- `/events`
- `/maintenance`
- `/settings`

## MVP-1 implementation priorities
1. app shell and nav
2. overview
3. node list/detail
4. VM list/detail
5. tasks center
6. mutation flows
7. volumes/networks
8. maintenance and settings

## Data-handling rules
- server routes own backend integration
- browser components receive shaped view models
- loading, empty, degraded, and failed states must be designed, not implied
- task creation responses must route users to visible task context
