# Serial Console Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a live, interactive serial console to the CHV Web UI with agent-native WebSocket service.

**Architecture:** Agent creates a PTY for each VM, exposes a WebSocket server for bidirectional serial I/O, and validates short-lived tokens issued by the BFF. UI uses xterm.js connected directly to the agent.

**Tech Stack:** Rust (nix, axum ws, tokio), SvelteKit (xterm.js, xterm-addon-fit)

---

### Task 1: Agent Config — Add `console_bind` option

**Files:**
- Modify: `crates/chv-config/src/lib.rs`
- Modify: `crates/chv-config/src/agent.rs`
- Test: `crates/chv-config/src/lib.rs` (existing tests)

**Step 1: Add `console_bind` field to `AgentConfig`**

In `crates/chv-config/src/agent.rs`, add:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    // ... existing fields ...
    #[serde(default = "default_console_bind")]
    pub console_bind: String,
}

fn default_console_bind() -> String {
    "127.0.0.1:8444".to_string()
}
```

Update `AgentConfig::default()` to include `console_bind: default_console_bind()`.

**Step 2: Verify config parses correctly**

Run: `cargo test -p chv-config`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/chv-config/
git commit -m "config: add console_bind to AgentConfig"
```

---

### Task 2: Agent PTY Creation in CHV Process Adapter

**Files:**
- Modify: `crates/chv-agent-runtime-ch/src/process.rs`
- Modify: `crates/chv-agent-runtime-ch/src/adapter.rs`
- Test: `crates/chv-agent-runtime-ch/src/process.rs` (existing tests)

**Step 1: Add `nix` dependency with pty feature**

In `crates/chv-agent-runtime-ch/Cargo.toml`, add:

```toml
[dependencies]
nix = { workspace = true, features = ["pty", "ioctl"] }
```

**Step 2: Create PTY before spawning CHV**

In `process.rs` `create_vm`:

```rust
use nix::pty::{openpty, ptsname};
use nix::sys::stat::Mode;
use std::os::fd::IntoRawFd;

// Create PTY
let pty = openpty(None, None)?;
let slave_path = ptsname(&pty.master)?;

// Pass PTY slave to CHV
let serial_arg = format!("tty={}", slave_path);
cmd.arg("--serial").arg(&serial_arg);

// Remove old --serial file=... argument
// (replace the existing cmd.arg("--serial")... line)
```

Store `pty.master` in `VmProcess`:

```rust
struct VmProcess {
    api_socket: std::path::PathBuf,
    child: Child,
    pty_master: std::os::fd::OwnedFd,
}
```

**Step 3: Add method to get PTY master FD by vm_id**

```rust
impl ProcessCloudHypervisorAdapter {
    pub fn pty_master(&self, vm_id: &str) -> Option<std::os::fd::OwnedFd> {
        let map = self.vms.blocking_lock();
        map.get(vm_id).map(|p| p.pty_master.try_clone().ok()).flatten()
    }
}
```

**Step 4: Run existing tests**

Run: `cargo test -p chv-agent-runtime-ch`
Expected: PASS (update mock adapter if needed)

**Step 5: Commit**

```bash
git add crates/chv-agent-runtime-ch/
git commit -m "feat: create PTY for VM serial console"
```

---

### Task 3: Agent Console WebSocket Server

**Files:**
- Create: `crates/chv-agent-core/src/console_server.rs`
- Modify: `crates/chv-agent-core/src/lib.rs`
- Modify: `crates/chv-agent-core/Cargo.toml`
- Test: `crates/chv-agent-core/src/console_server.rs`

**Step 1: Add dependencies**

In `crates/chv-agent-core/Cargo.toml`:

```toml
[dependencies]
axum = { workspace = true }
nix = { workspace = true, features = ["pty", "ioctl"] }
tokio-tungstenite = "0.24"
```

**Step 2: Implement console WebSocket handler**

In `console_server.rs`:

```rust
use axum::{
    extract::{Path, Query, WebSocketUpgrade},
    response::Response,
    routing::get,
    Router,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Clone)]
pub struct ConsoleServer {
    vm_runtime: crate::vm_runtime::VmRuntime,
    token_secret: String,
}

#[derive(serde::Deserialize)]
struct ConsoleParams {
    token: String,
}

impl ConsoleServer {
    pub fn new(vm_runtime: crate::vm_runtime::VmRuntime, token_secret: String) -> Self {
        Self { vm_runtime, token_secret }
    }

    pub async fn run(self, bind: &str) -> Result<(), crate::ChvError> {
        let app = Router::new()
            .route("/vms/:vm_id/console", get(Self::ws_handler));
        let listener = TcpListener::bind(bind).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }

    async fn ws_handler(
        ws: WebSocketUpgrade,
        Path(vm_id): Path<String>,
        Query(params): Query<ConsoleParams>,
    ) -> Response {
        // TODO: validate token
        ws.on_upgrade(move |socket| Self::handle_socket(socket, vm_id))
    }

    async fn handle_socket(mut socket: axum::extract::ws::WebSocket, vm_id: String) {
        // TODO: get PTY master FD from vm_runtime, spawn read/write loops
    }
}
```

**Step 3: Write PTY relay logic**

```rust
async fn relay_pty(
    mut socket: axum::extract::ws::WebSocket,
    mut pty_reader: tokio::fs::File,
    mut pty_writer: tokio::fs::File,
) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // PTY → WebSocket
    let read_task = tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            match pty_reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let _ = ws_tx.send(axum::extract::ws::Message::Binary(buf[..n].to_vec())).await;
                }
                Err(_) => break,
            }
        }
    });

    // WebSocket → PTY
    let write_task = tokio::spawn(async move {
        while let Some(msg) = ws_rx.next().await {
            match msg {
                Ok(axum::extract::ws::Message::Binary(data)) => {
                    let _ = pty_writer.write_all(&data).await;
                }
                Ok(axum::extract::ws::Message::Text(text)) => {
                    // Handle resize JSON messages
                    if let Ok(resize) = serde_json::from_str::<ResizeMsg>(&text) {
                        // TODO: ioctl TIOCSWINSZ
                    } else {
                        let _ = pty_writer.write_all(text.as_bytes()).await;
                    }
                }
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = read_task => {},
        _ = write_task => {},
    }
}
```

**Step 4: Run tests**

Run: `cargo check -p chv-agent-core`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/chv-agent-core/src/console_server.rs crates/chv-agent-core/src/lib.rs crates/chv-agent-core/Cargo.toml
git commit -m "feat: add agent console WebSocket server skeleton"
```

---

### Task 4: Wire Console Server into Agent Startup

**Files:**
- Modify: `cmd/chv-agent/src/main.rs`
- Modify: `crates/chv-agent-core/src/lib.rs`
- Test: manual smoke test

**Step 1: Spawn console server alongside gRPC server**

In `cmd/chv-agent/src/main.rs`, after creating `vm_runtime`:

```rust
let console_bind = config.console_bind.clone();
let console_server = chv_agent_core::console_server::ConsoleServer::new(
    vm_runtime.clone(),
    bootstrap_token.clone(),
);
tokio::spawn(async move {
    if let Err(e) = console_server.run(&console_bind).await {
        tracing::error!(error = %e, "console server failed");
    }
});
```

**Step 2: Commit**

```bash
git add cmd/chv-agent/src/main.rs crates/chv-agent-core/src/lib.rs
git commit -m "feat: wire console server into agent startup"
```

---

### Task 5: BFF Console URL Endpoint

**Files:**
- Modify: `crates/chv-webui-bff/src/handlers/vms.rs`
- Modify: `crates/chv-webui-bff/src/router.rs`
- Test: `crates/chv-webui-bff/src/handlers/vms.rs` (existing tests)

**Step 1: Add `get_vm_console_url` handler**

```rust
pub async fn get_vm_console_url(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::extract::Path(vm_id): axum::extract::Path<String>,
) -> Result<Json<Value>, BffError> {
    // Look up VM node_id
    let row = sqlx::query_as::<_, VmNodeRow>("SELECT node_id FROM vms WHERE vm_id = ?")
        .bind(&vm_id)
        .fetch_optional(&state.pool)
        .await?;

    let node_id = row.ok_or_else(|| BffError::NotFound(format!("vm {} not found", vm_id)))?.node_id;

    // Generate short-lived token
    let token = generate_console_token(&vm_id, &claims.username, &state.jwt_secret)?;

    // TODO: resolve agent console URL from node inventory
    let console_url = format!("ws://{}:8444/vms/{}/console?token={}", node_id, vm_id, token);

    Ok(Json(json!({
        "vm_id": vm_id,
        "url": console_url,
        "expires_at": (chrono::Utc::now() + chrono::Duration::seconds(60)).to_rfc3339(),
    })))
}
```

**Step 2: Register route**

In `router.rs`, add:

```rust
.route("/v1/vms/:vm_id/console-url", get(handlers::vms::get_vm_console_url))
```

**Step 3: Commit**

```bash
git add crates/chv-webui-bff/
git commit -m "feat: add BFF console URL endpoint"
```

---

### Task 6: UI xterm.js Console Component

**Files:**
- Create: `ui/src/lib/components/vms/VmConsole.svelte`
- Modify: `ui/src/routes/vms/[id]/+page.svelte`
- Modify: `ui/package.json`
- Test: manual browser test

**Step 1: Install xterm.js**

```bash
cd ui && npm install xterm @xterm/addon-fit
```

**Step 2: Create `VmConsole.svelte`**

```svelte
<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Terminal } from 'xterm';
	import { FitAddon } from '@xterm/addon-fit';

	let { vmId, consoleUrl }: { vmId: string; consoleUrl: string } = $props();

	let terminalEl: HTMLDivElement;
	let terminal: Terminal;
	let fitAddon: FitAddon;
	let socket: WebSocket;
	let reconnectTimer: ReturnType<typeof setTimeout>;

	function connect() {
		if (socket) socket.close();
		socket = new WebSocket(consoleUrl);
		socket.binaryType = 'arraybuffer';

		socket.onopen = () => {
			terminal.writeln('\r\n\x1b[32m[Connected to serial console]\x1b[0m\r\n');
		};

		socket.onmessage = (event) => {
			if (event.data instanceof ArrayBuffer) {
				const data = new Uint8Array(event.data);
				terminal.write(data);
			}
		};

		socket.onclose = () => {
			terminal.writeln('\r\n\x1b[31m[Disconnected]\x1b[0m');
			reconnectTimer = setTimeout(connect, 3000);
		};

		socket.onerror = () => {
			terminal.writeln('\r\n\x1b[31m[Connection error]\x1b[0m');
		};
	}

	onMount(() => {
		terminal = new Terminal({
			cursorBlink: true,
			fontSize: 14,
			fontFamily: 'monospace',
		});
		fitAddon = new FitAddon();
		terminal.loadAddon(fitAddon);
		terminal.open(terminalEl);
		fitAddon.fit();

		terminal.onData((data) => {
			if (socket?.readyState === WebSocket.OPEN) {
				socket.send(data);
			}
		});

		terminal.onResize(({ cols, rows }) => {
			if (socket?.readyState === WebSocket.OPEN) {
				socket.send(JSON.stringify({ type: 'resize', cols, rows }));
			}
		});

		connect();
	});

	onDestroy(() => {
		clearTimeout(reconnectTimer);
		socket?.close();
		terminal?.dispose();
	});
</script>

<div class="console-container">
	<div bind:this={terminalEl} class="terminal"></div>
</div>

<style>
	.console-container {
		width: 100%;
		height: 500px;
		background: #1a1a1a;
		border-radius: 8px;
		padding: 12px;
	}
	.terminal {
		width: 100%;
		height: 100%;
	}
	:global(.xterm) {
		padding: 8px;
	}
</style>
```

**Step 3: Add Console tab to VM detail page**

In `+page.svelte`, add a new tab:

```svelte
{#if detail.currentTab === 'console'}
	<VmConsole vmId={detail.summary.vm_id} consoleUrl={detail.consoleUrl} />
{/if}
```

Update `+page.ts` to fetch `consoleUrl` from BFF.

**Step 4: Commit**

```bash
git add ui/
git commit -m "feat: add xterm.js serial console component"
```

---

### Task 7: Nginx WebSocket Proxy + Agent Config

**Files:**
- Modify: `scripts/install.sh`
- Modify: `ui/src/lib/bff/endpoints.ts`

**Step 1: Add nginx location**

In `install.sh` nginx config:

```nginx
location /ws/vms/ {
    proxy_pass http://127.0.0.1:8444;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_read_timeout 86400;
}
```

**Step 2: Add `console_bind` to agent.toml generation**

In `install.sh`:

```bash
cat > "$CHV_CONFIG_DIR/agent.toml" <<EOF
...
console_bind = "127.0.0.1:8444"
EOF
```

**Step 3: Commit**

```bash
git add scripts/install.sh
git commit -m "feat: nginx WebSocket proxy and agent console_bind config"
```

---

### Task 8: Integration Test

**Files:**
- Manual test only (no automated e2e yet)

**Steps:**
1. Build release: `./scripts/build-release.sh`
2. Install: `sudo ./scripts/dev-install.sh`
3. Open browser to VM detail page
4. Click "Console" tab
5. Verify xterm.js renders
6. Verify WebSocket connects
7. Type in terminal, verify keystrokes reach VM
8. Verify VM output streams to terminal

**Commit:**

```bash
git commit -m "feat: serial console integration complete" --allow-empty
```

---

## Verification Checklist

- [ ] `cargo test -p chv-agent-core` passes
- [ ] `cargo test -p chv-agent-runtime-ch` passes
- [ ] `cargo test -p chv-webui-bff` passes
- [ ] `npm run check` in `ui/` passes (0 errors)
- [ ] Console tab visible on VM detail page
- [ ] WebSocket connects when VM is running
- [ ] Keyboard input reaches VM serial
- [ ] VM serial output renders in xterm.js
- [ ] Disconnect/reconnect works
