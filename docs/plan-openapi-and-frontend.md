# Plan: OpenAPI Documentation & Vue.js Frontend

## Overview

Create a complete frontend solution for CHV with:
1. OpenAPI/Swagger documentation for the API
2. Vue.js 3 web UI following VMware/Proxmox design principles

## Phase 1: OpenAPI Documentation

### Goals
- Document all API endpoints with OpenAPI 3.0 spec
- Generate Swagger UI for interactive API documentation
- Use swaggo/swag for Go integration

### Tasks

#### 1.1 Install OpenAPI Tools
```bash
# Install swag CLI
go install github.com/swaggo/swag/cmd/swag@latest

# Add to go.mod
go get -u github.com/swaggo/swag
```

#### 1.2 Add Swagger Annotations
Files to modify:
- `internal/api/handler.go` - Add main API annotations
- `internal/api/vms.go` - Add VM endpoint annotations
- `internal/api/nodes.go` - Add node endpoint annotations
- `internal/api/networks.go` - Add network annotations
- `internal/api/storage.go` - Add storage annotations
- `internal/api/images.go` - Add image annotations
- `internal/api/auth.go` - Add auth annotations

#### 1.3 Generate and Serve Swagger UI
- Add swagger endpoint at `/swagger/index.html`
- Serve generated docs statically

#### 1.4 OpenAPI Specification Output
Output: `docs/openapi.yaml`

## Phase 2: Vue.js Frontend

### Goals
- Enterprise virtualization console (VMware/Proxmox style)
- Dark/light theme support
- Real-time data updates
- Responsive design

### Tech Stack
- **Framework**: Vue 3 with Composition API
- **Build Tool**: Vite
- **State Management**: Pinia
- **HTTP Client**: Axios
- **UI Framework**: PrimeVue (enterprise components)
- **Charts**: Chart.js / PrimeVue Charts
- **Icons**: PrimeIcons + Phosphor Icons
- **Styling**: Tailwind CSS + Custom CSS variables

### Design System (VMware/Proxmox Style)

#### Colors
```css
:root {
  --color-primary: #0066CC;
  --color-success: #54B435;
  --color-warning: #F0AB00;
  --color-error: #E60000;
  --color-bg-chrome: #F5F5F5;
  --color-bg-content: #FFFFFF;
  --color-border: #D0D0D0;
  --color-text-primary: #1A1A1A;
  --color-text-secondary: #666666;
}
```

#### Typography
- **UI Font**: Roboto
- **Monospace**: Roboto Mono (for VM IDs, IPs)

#### Layout
- Three-pane layout (sidebar, content, details)
- Data-dense tables
- Status badges with icons

### Project Structure

```
chv-ui/
├── public/
│   └── favicon.ico
├── src/
│   ├── api/
│   │   ├── client.ts          # Axios instance
│   │   ├── vms.ts             # VM API calls
│   │   ├── nodes.ts           # Node API calls
│   │   ├── networks.ts        # Network API calls
│   │   ├── storage.ts         # Storage API calls
│   │   ├── images.ts          # Image API calls
│   │   └── auth.ts            # Auth API calls
│   ├── components/
│   │   ├── layout/
│   │   │   ├── AppSidebar.vue
│   │   │   ├── AppHeader.vue
│   │   │   └── AppLayout.vue
│   │   ├── common/
│   │   │   ├── StatusBadge.vue
│   │   │   ├── ResourceChart.vue
│   │   │   ├── DataTable.vue
│   │   │   └── ConfirmDialog.vue
│   │   ├── vms/
│   │   │   ├── VMList.vue
│   │   │   ├── VMDetails.vue
│   │   │   ├── VMCreateModal.vue
│   │   │   └── VMConsole.vue
│   │   ├── nodes/
│   │   │   ├── NodeList.vue
│   │   │   └── NodeDetails.vue
│   │   ├── networks/
│   │   │   └── NetworkList.vue
│   │   └── storage/
│   │       └── StorageList.vue
│   ├── stores/
│   │   ├── auth.ts
│   │   ├── vms.ts
│   │   ├── nodes.ts
│   │   ├── networks.ts
│   │   ├── storage.ts
│   │   └── images.ts
│   ├── router/
│   │   └── index.ts
│   ├── views/
│   │   ├── LoginView.vue
│   │   ├── DashboardView.vue
│   │   ├── VMsView.vue
│   │   ├── NodesView.vue
│   │   ├── NetworksView.vue
│   │   ├── StorageView.vue
│   │   └── ImagesView.vue
│   ├── types/
│   │   ├── api.ts
│   │   └── index.ts
│   ├── utils/
│   │   └── formatters.ts
│   ├── App.vue
│   └── main.ts
├── index.html
├── package.json
├── tsconfig.json
├── vite.config.ts
└── tailwind.config.js
```

### Views

#### 1. Login View
- Simple login form with token input
- Store token in localStorage

#### 2. Dashboard View
- Stats cards (Total VMs, Running VMs, Nodes, Storage)
- Recent activity list
- Resource utilization charts
- Quick actions

#### 3. VMs View (Main Interface)
- **Left**: VM list with status, name, IP, resources
- **Center**: VM details (tabs: Summary, Console, Settings)
- **Right**: Actions panel (Start, Stop, Reboot, Delete)

#### 4. Nodes View
- Node list with status, resources, VM count
- Node details with health metrics

#### 5. Networks View
- Network list with CIDR, gateway
- Network topology visualization

#### 6. Storage View
- Storage pools list
- Capacity usage charts

### Features

#### Real-time Updates
- Polling every 30 seconds
- WebSocket support (future)
- Toast notifications for state changes

#### Data Tables
- Sortable columns
- Filtering
- Pagination
- Row actions

#### Status Indicators
- Running (green dot)
- Stopped (gray dot)
- Error (red dot)
- Warning (amber dot)

### Implementation Order

1. **Setup**: Initialize Vue project with Vite
2. **API Client**: Create axios instance with auth
3. **Auth Store**: Login/logout functionality
4. **Layout**: Three-pane layout component
5. **Dashboard**: Stats and overview
6. **VMs View**: Main VM management interface
7. **Other Views**: Nodes, networks, storage
8. **Polish**: Themes, animations, error handling

### Build & Deploy

```bash
# Development
npm run dev

# Production build
npm run build

# Docker build for UI
docker build -t chv-ui .
```

## Deliverables

### Phase 1 (OpenAPI)
- [ ] `docs/swagger.yaml` - OpenAPI specification
- [ ] `/swagger/index.html` - Swagger UI endpoint
- [ ] API annotations in all handler files

### Phase 2 (UI)
- [ ] `chv-ui/` - Complete Vue.js project
- [ ] Docker configuration for UI
- [ ] Documentation for frontend setup

## Success Criteria

- API fully documented with OpenAPI
- Swagger UI accessible and functional
- Vue UI connects to CHV API successfully
- UI follows VMware/Proxmox design principles
- Responsive design works on desktop and tablet
