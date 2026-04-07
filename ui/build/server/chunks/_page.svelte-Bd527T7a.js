import { c as createAPIClient, g as getStoredToken } from './client-iYtOZkWx.js';
import { h as fallback, m as escape_html, n as bind_props } from './renderer-Xy7Nl1fv.js';
import { S as StateBadge } from './StateBadge-D-RbwxiK.js';
import './shared-server-BU2DVf8Q.js';

function InstallStatusPanel($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let status = fallback($$props["status"], null);
    let loading = fallback($$props["loading"], false);
    let error = fallback($$props["error"], "");
    let handleBootstrap = fallback($$props["handleBootstrap"], () => {
    });
    let handleRefresh = fallback($$props["handleRefresh"], () => {
    });
    let handleRepairBridge = fallback($$props["handleRepairBridge"], () => {
    });
    let handleRepairDirectories = fallback($$props["handleRepairDirectories"], () => {
    });
    let handleRepairLocaldisk = fallback($$props["handleRepairLocaldisk"], () => {
    });
    $$renderer2.push(`<section class="table-card"><div class="card-header flex items-center justify-between px-4 py-3"><div><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Install Status</div> <div class="mt-1 text-base font-semibold">Bootstrap and Host Readiness</div></div> `);
    if (status) {
      $$renderer2.push("<!--[0-->");
      StateBadge($$renderer2, { label: status.overall_state });
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div> <div class="space-y-6 p-4">`);
    if (loading) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="border border-line bg-chrome px-4 py-6 text-sm text-muted">Loading install status…</div>`);
    } else if (error) {
      $$renderer2.push("<!--[1-->");
      $$renderer2.push(`<div class="border border-danger bg-red-50 px-4 py-4 text-sm text-danger">${escape_html(error)}</div>`);
    } else if (status) {
      $$renderer2.push("<!--[2-->");
      $$renderer2.push(`<div class="grid gap-6 lg:grid-cols-2"><div class="table-card"><div class="card-header px-4 py-2 text-sm font-medium">Platform</div> <dl class="grid grid-cols-[180px_minmax(0,1fr)] gap-x-4 gap-y-3 p-4 text-sm"><dt class="text-muted">Data root</dt> <dd class="mono">${escape_html(status.data_root)}</dd> <dt class="text-muted">SQLite path</dt> <dd class="mono">${escape_html(status.database_path)}</dd> <dt class="text-muted">Cloud Hypervisor</dt> <dd class="flex items-center gap-3"><span class="mono">${escape_html(status.cloud_hypervisor.path || "not found")}</span> `);
      StateBadge($$renderer2, {
        label: status.cloud_hypervisor.found ? "ready" : "missing_prerequisites"
      });
      $$renderer2.push(`<!----></dd> <dt class="text-muted">Cloud-init ISO support</dt> <dd>`);
      StateBadge($$renderer2, {
        label: status.cloudinit.supported ? "ready" : "missing_prerequisites"
      });
      $$renderer2.push(`<!----></dd></dl></div> <div class="table-card"><div class="card-header px-4 py-2 text-sm font-medium">Host Network</div> <dl class="grid grid-cols-[180px_minmax(0,1fr)] gap-x-4 gap-y-3 p-4 text-sm"><dt class="text-muted">Bridge</dt> <dd class="mono">${escape_html(status.bridge.name)}</dd> <dt class="text-muted">Exists</dt> <dd>`);
      StateBadge($$renderer2, { label: status.bridge.exists ? "ready" : "bootstrap_required" });
      $$renderer2.push(`<!----></dd> <dt class="text-muted">Expected IP</dt> <dd class="mono">${escape_html(status.bridge.expected_ip)}</dd> <dt class="text-muted">Actual IP</dt> <dd class="mono">${escape_html(status.bridge.actual_ip || "missing")}</dd> <dt class="text-muted">Link state</dt> <dd>`);
      StateBadge($$renderer2, { label: status.bridge.up ? "active" : "degraded" });
      $$renderer2.push(`<!----></dd></dl></div></div> <div class="table-card"><div class="card-header px-4 py-2 text-sm font-medium">Storage</div> <dl class="grid grid-cols-[180px_minmax(0,1fr)] gap-x-4 gap-y-3 p-4 text-sm"><dt class="text-muted">Default pool</dt> <dd class="mono">${escape_html(status.localdisk.path)}</dd> <dt class="text-muted">Pool state</dt> <dd>`);
      StateBadge($$renderer2, {
        label: status.localdisk.ready ? "ready" : "bootstrap_required"
      });
      $$renderer2.push(`<!----></dd></dl></div> <div class="flex flex-wrap gap-3"><button class="button-primary px-4 py-2 text-sm font-medium">Bootstrap</button> <button class="button-secondary px-4 py-2 text-sm font-medium">Re-run Checks</button> <button class="button-secondary px-4 py-2 text-sm font-medium">Repair Bridge</button> <button class="button-secondary px-4 py-2 text-sm font-medium">Repair Directories</button> <button class="button-secondary px-4 py-2 text-sm font-medium">Repair Localdisk</button></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<div class="border border-line bg-chrome px-4 py-6 text-sm text-muted">No install status available yet.</div>`);
    }
    $$renderer2.push(`<!--]--></div></section>`);
    bind_props($$props, {
      status,
      loading,
      error,
      handleBootstrap,
      handleRefresh,
      handleRepairBridge,
      handleRepairDirectories,
      handleRepairLocaldisk
    });
  });
}
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    const client = createAPIClient({ token: getStoredToken() ?? void 0 });
    let status = null;
    let loading = true;
    let error = "";
    async function loadStatus() {
      loading = true;
      error = "";
      try {
        status = await client.getInstallStatus();
      } catch (err) {
        error = err instanceof Error ? err.message : "Could not load install status.";
      } finally {
        loading = false;
      }
    }
    async function bootstrapInstall() {
      await client.bootstrapInstall();
      await loadStatus();
    }
    async function repairBridge() {
      await client.repairInstall({
        repair_bridge: true,
        repair_directories: false,
        repair_localdisk: false
      });
      await loadStatus();
    }
    async function repairDirectories() {
      await client.repairInstall({
        repair_bridge: false,
        repair_directories: true,
        repair_localdisk: false
      });
      await loadStatus();
    }
    async function repairLocaldisk() {
      await client.repairInstall({
        repair_bridge: false,
        repair_directories: false,
        repair_localdisk: true
      });
      await loadStatus();
    }
    InstallStatusPanel($$renderer2, {
      status,
      loading,
      error,
      handleBootstrap: bootstrapInstall,
      handleRefresh: loadStatus,
      handleRepairBridge: repairBridge,
      handleRepairDirectories: repairDirectories,
      handleRepairLocaldisk: repairLocaldisk
    });
  });
}

export { _page as default };
//# sourceMappingURL=_page.svelte-Bd527T7a.js.map
