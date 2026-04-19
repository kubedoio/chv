# Serial Console — Post-Implementation TODO

## Agent
- [ ] **JWT token validation**: `ConsoleServer` currently only checks `token` query param is non-empty. Implement full JWT verification using `jsonwebtoken` with `jwt_secret`.
- [ ] **Token expiry enforcement**: Validate `exp` claim against current time.
- [ ] **Rate limiting**: Add per-VM connection rate limit to prevent token replay abuse.

## BFF
- [ ] **Multi-node hostname resolution**: `get_vm_console_url` currently returns a relative path (`/ws/vms/...`). For multi-node deployments, the BFF must resolve each node's actual agent address and return the appropriate proxy path.
- [ ] **Console URL refresh**: If the token expires while the user is on the Console tab, auto-refresh the URL before WebSocket reconnect.

## UI
- [ ] **Remove `BootLogViewer.svelte`**: The old log-file-based console is superseded by the WebSocket serial console. Remove component and `getBootLogs` API once the serial console is validated in production.
- [ ] **Copy-to-clipboard / download session**: Add toolbar buttons to copy terminal contents or download the session log.
- [ ] **Reconnect UI**: Show explicit "Reconnect" button instead of relying solely on auto-reconnect.

## Nginx / Ops
- [ ] **Multi-node WebSocket routing**: Current nginx config proxies `/ws/vms/` to `127.0.0.1:8444`. For multi-node, use a dynamic upstream (e.g., based on `node_id` path prefix or query param).
- [ ] **TLS for agent console**: `console_bind` currently uses plain `ws://`. Add TLS certificate support so agents can expose `wss://`.

## Testing
- [ ] **Automated e2e test**: Spin up a mock CHV process, connect via WebSocket, verify echo.
- [ ] **Integration test with real VM**: Boot a VM with serial output, verify xterm.js renders kernel boot messages.
- [ ] **Resize integration test**: Verify `TIOCSWINSZ` propagates to the VM (e.g., `stty size` inside guest reflects new dimensions).
