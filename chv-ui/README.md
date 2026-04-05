# CHV Web UI

Vue.js 3 frontend for the CHV Cloud Hypervisor Platform.

## Features

- **VM Management**: Create, start, stop, reboot, and delete VMs
- **Node Management**: Register and monitor hypervisor nodes
- **Network Management**: Configure Linux bridge networks
- **Storage Management**: Manage local and NFS storage pools
- **Image Management**: Import and manage cloud images
- **Real-time Dashboard**: Overview of system resources and status

## Tech Stack

- Vue 3 with Composition API
- TypeScript
- Vite
- Pinia (state management)
- PrimeVue (UI components)
- Tailwind CSS
- Axios (HTTP client)

## Getting Started

### Prerequisites

- Node.js 18+
- CHV API running (backend)

### Installation

```bash
cd chv-ui
npm install
```

### Development

```bash
npm run dev
```

The UI will be available at http://localhost:3000

### Build for Production

```bash
npm run build
```

### Docker Build

```bash
docker build -t chv-ui .
```

## Configuration

Set the API URL via environment variable:

```bash
VITE_API_URL=http://localhost:8081 npm run dev
```

Default: `http://localhost:8081`

## Project Structure

```
src/
├── api/           # API client and endpoint modules
├── components/    # Vue components
├── router/        # Vue Router configuration
├── stores/        # Pinia stores
├── views/         # Page components
├── types/         # TypeScript type definitions
└── assets/        # Styles and static assets
```

## Authentication

The UI uses API tokens for authentication. Tokens are stored in localStorage.

To obtain a token:
1. Run the backend
2. Create a token via API: `POST /api/v1/tokens`
3. Use the token in the login page

## Design System

The UI follows VMware/Proxmox enterprise console design principles:
- Light theme with industrial/functional aesthetic
- Data-dense tables with zebra striping
- Status badges with color-coded indicators
- Three-pane layout for resource management
- Monospace font for IDs and technical data

See DESIGN.md in the main project for full design specifications.
