# Serial Console Design

## Overview

Add a live, interactive serial console to the CHV Web UI. The console streams VM serial output in real time and accepts keyboard input, rendered via xterm.js. The console service runs natively on the agent process so it works for remote hypervisor nodes and agent-only deployments.

## Goals

- Live bidirectional serial console for every VM
- Keyboard interaction (tty semantics)
- Agent-native service (no BFF proxying of stream data)
- Works for remote agents and agent-only nodes

## Non-Goals

- VNC / graphical console
- Multiple concurrent console sessions per VM
- Log persistence or rotation (handled separately)

## Architecture

```
┌─────────────┐      wss://agent-node:8444      ┌─────────────┐
│   Browser   │ ────────────────────────────────→ │    Agent    │
│  (xterm.js) │      /vms/{vm_id}/console         │  WebSocket  │
└─────────────┘                                   │   Server    │
     ↑                                            │      ↓      │
     │                                            │   PTY master │
     │                                            │      ↓      │
     │ GET /v1/vms/{vm_id}/console-url            │     CHV      │
     │                                            │      ↓      │
     │                                            │   PTY slave  │
     │                                            │      ↓      │
     │                                            │      VM      │
     │                                            └─────────────┘
     │
     ↓
┌─────────────┐
│     BFF     │  ← returns WebSocket URL + auth token only
│  (metadata) │
└─────────────┘
```

### Agent

1. **PTY creation**: Before spawning CHV, the agent creates a PTY pair via `nix::pty::openpty()`, resolves the slave path with `ptsname_r()`, and passes `--serial tty=/dev/pts/N` to CHV.
2. **PTY ownership**: The master FD is stored in `VmProcess` alongside the `Child` handle.
3. **WebSocket server**: A lightweight axum server runs on a configurable bind address (`console_bind` in `agent.toml`, default `127.0.0.1:8444`).
4. **Endpoint**: `GET /vms/{vm_id}/console` validates a short-lived token, upgrades to WebSocket, and spawns a bidirectional PTY relay task.
5. **Auth**: Accepts a `token` query parameter. Tokens are HMAC-signed JWTs issued by the control plane / BFF with a short expiry (60s) and a `vm_id` claim.

### BFF

1. **`GET /v1/vms/{vm_id}/console-url`** — looks up the VM’s `node_id`, resolves the agent’s console address (from `vms` table or node inventory), generates a signed console token, and returns:
   ```json
   {
     "url": "wss://agent-host:8444/vms/test-1/console?token=...",
     "expires_at": "2026-04-18T12:00:00Z"
   }
   ```
2. No stream proxying. The BFF only issues URLs and tokens.

### UI

1. **VM detail page** (`/vms/[id]`) gets a new **"Console"** tab.
2. **`VmConsole.svelte`** component:
   - On mount, calls BFF for console URL + token
   - Opens WebSocket to the agent
   - Instantiates `xterm.js` (`Terminal` + `FitAddon`)
   - Forwards WebSocket binary frames to `terminal.write()`
   - Forwards `terminal.onData()` keystrokes to WebSocket
   - Handles `onResize` → sends `{"type":"resize","cols":...,"rows":...}` JSON message
   - Auto-reconnect with exponential backoff on disconnect
   - Shows a "Connect" overlay when the VM is not running

### Nginx (all-in-one / dev)

For the dev install where agent and BFF run on the same host, nginx proxies the browser’s same-origin WebSocket to the agent’s local port:

```nginx
location /ws/vms/ {
    proxy_pass http://127.0.0.1:8444;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_read_timeout 86400;
}
```

For remote agents, the admin exposes the agent’s `console_bind` port directly (with TLS termination at a load balancer or reverse proxy).

## Data Flow

```
User keystroke → xterm.js → WebSocket → Agent → PTY master → CHV → VM
VM output      → CHV → PTY slave → PTY master → Agent → WebSocket → xterm.js
Resize event   → xterm.js → WebSocket JSON → Agent → ioctl(TIOCSWINSZ) → PTY master
```

## PTY Lifecycle

- **Create**: `prepare_vm` creates PTY before `cmd.spawn()`
- **Store**: `VmProcess` holds `(child, api_socket, pty_master_fd)`
- **Read/Write**: Console WebSocket task owns the `pty_master_fd` while connected
- **Close**: When `delete_vm` kills the child, the PTY master FD is closed and the slave is cleaned up by the kernel

## Token Auth

```
Header: { "alg": "HS256", "typ": "JWT" }
Payload: { "sub": "console", "vm_id": "test-1", "exp": 1713432000, "iat": 1713431940 }
```

- Signed with the bootstrap token (shared secret between control plane and agent)
- Agent validates signature, expiry, and `vm_id` claim on every WebSocket upgrade
- One-time use recommended (agent can maintain a small LRU cache of consumed tokens to prevent replay)

## Files to Create / Modify

| File | Change |
|------|--------|
| `crates/chv-agent-runtime-ch/src/process.rs` | PTY creation, `--serial tty=...`, store master FD |
| `crates/chv-agent-core/src/console_server.rs` | WebSocket server, PTY relay task, token validation |
| `crates/chv-agent-core/src/agent_server.rs` | Wire console server into agent startup |
| `crates/chv-config/src/lib.rs` | Add `console_bind` to agent config |
| `crates/chv-webui-bff/src/handlers/vms.rs` | `GET /v1/vms/{vm_id}/console-url` handler |
| `crates/chv-webui-bff/src/router.rs` | Register console URL route |
| `crates/chv-controlplane-service/src/inventory.rs` | Store agent console address per node |
| `ui/src/routes/vms/[id]/+page.svelte` | Add "Console" tab |
| `ui/src/lib/components/vms/VmConsole.svelte` | xterm.js terminal component |
| `scripts/install.sh` | Nginx WebSocket proxy location, `agent.toml` console_bind |
| `docs/plans/2026-04-18-serial-console-design.md` | This document |

## Dependencies

- `nix` (already in workspace) — for `openpty`, `ptsname_r`, `ioctl`
- `xterm` + `xterm-addon-fit` (npm) — terminal emulator in UI
- `axum::extract::ws` (already available via axum 0.7) — WebSocket support

## Open Questions

1. Should the console URL use the agent’s reported external IP, or a configured `console_external_url`?
2. Do we need to support multiple concurrent console sessions, or serialise them?
3. Should the PTY default termios be raw mode, or rely on guest kernel setup?
