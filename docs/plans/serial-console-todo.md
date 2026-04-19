# Serial Console — Post-Implementation TODO

## Agent
- [x] **JWT token validation**: HS256 JWT decode via `jsonwebtoken` crate. `validate_console_token()` in `console_server.rs`. Config guards reject insecure defaults. _(Sprint 2)_
- [x] **Token expiry enforcement**: `exp` claim validated by `jsonwebtoken` library (default leeway 60s). _(Sprint 2)_
- [ ] **Rate limiting**: Add per-VM connection rate limit to prevent token replay abuse.

## BFF
- [ ] **Multi-node hostname resolution**: `get_vm_console_url` currently returns a relative path (`/ws/vms/...`). For multi-node deployments, the BFF must resolve each node's actual agent address and return the appropriate proxy path.
- [ ] **Console URL refresh**: If the token expires while the user is on the Console tab, auto-refresh the URL before WebSocket reconnect.

## UI
- [x] **Remove `BootLogViewer.svelte`**: Removed component, `getBootLogs` API, and `getVmConsole` BFF function. _(Sprint 3)_
- [ ] **Copy-to-clipboard / download session**: Add toolbar buttons to copy terminal contents or download the session log.
- [ ] **Reconnect UI**: Show explicit "Reconnect" button instead of relying solely on auto-reconnect.

## Nginx / Ops
- [ ] **Multi-node WebSocket routing**: Current nginx config proxies `/ws/vms/` to `127.0.0.1:8444`. For multi-node, use a dynamic upstream (e.g., based on `node_id` path prefix or query param).
- [ ] **TLS for agent console**: `console_bind` currently uses plain `ws://`. Add TLS certificate support so agents can expose `wss://`.

## Testing
- [ ] **Automated e2e test**: Spin up a mock CHV process, connect via WebSocket, verify echo.
- [ ] **Integration test with real VM**: Boot a VM with serial output, verify xterm.js renders kernel boot messages.
- [ ] **Resize integration test**: Verify `TIOCSWINSZ` propagates to the VM (e.g., `stty size` inside guest reflects new dimensions).
