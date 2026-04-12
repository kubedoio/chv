import "clsx";
import { c as createAPIClient, g as getStoredToken, t as toast } from "../../../chunks/client2.js";
import { o as fallback, e as escape_html, g as ensure_array_like, c as attr, j as bind_props } from "../../../chunks/root.js";
import { S as StateBadge } from "../../../chunks/StateBadge.js";
function InstallStatusPanel($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let status = fallback($$props["status"], null);
    let loading = fallback($$props["loading"], false);
    let actionLoading = fallback($$props["actionLoading"], false);
    let error = fallback($$props["error"], "");
    let lastActionResult = fallback($$props["lastActionResult"], null);
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
      $$renderer2.push(`<!----></dd></dl></div> `);
      if (lastActionResult) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<div class="table-card"><div class="card-header px-4 py-2 text-sm font-medium">Last Action Result</div> <div class="space-y-3 p-4 text-sm"><div class="flex items-center gap-3"><span class="text-muted">State:</span> `);
        StateBadge($$renderer2, { label: lastActionResult.overall_state });
        $$renderer2.push(`<!----></div> `);
        if (lastActionResult.actions_taken.length > 0) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<div><span class="text-muted">Actions taken:</span> <ul class="mt-1 list-disc pl-5"><!--[-->`);
          const each_array = ensure_array_like(lastActionResult.actions_taken);
          for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
            let action = each_array[$$index];
            $$renderer2.push(`<li>${escape_html(action)}</li>`);
          }
          $$renderer2.push(`<!--]--></ul></div>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--> `);
        if (lastActionResult.warnings.length > 0) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<div class="rounded border border-warning bg-yellow-50 px-3 py-2"><span class="text-warning font-medium">Warnings:</span> <ul class="mt-1 list-disc pl-5 text-warning"><!--[-->`);
          const each_array_1 = ensure_array_like(lastActionResult.warnings);
          for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
            let warning = each_array_1[$$index_1];
            $$renderer2.push(`<li>${escape_html(warning)}</li>`);
          }
          $$renderer2.push(`<!--]--></ul></div>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--> `);
        if (lastActionResult.errors.length > 0) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<div class="rounded border border-danger bg-red-50 px-3 py-2"><span class="text-danger font-medium">Errors:</span> <ul class="mt-1 list-disc pl-5 text-danger"><!--[-->`);
          const each_array_2 = ensure_array_like(lastActionResult.errors);
          for (let $$index_2 = 0, $$length = each_array_2.length; $$index_2 < $$length; $$index_2++) {
            let err = each_array_2[$$index_2];
            $$renderer2.push(`<li>${escape_html(err)}</li>`);
          }
          $$renderer2.push(`<!--]--></ul></div>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--></div></div>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]--> <div class="flex flex-wrap gap-3"><button class="button-primary px-4 py-2 text-sm font-medium"${attr("disabled", actionLoading, true)}>${escape_html(actionLoading ? "Running…" : "Bootstrap")}</button> <button class="button-secondary px-4 py-2 text-sm font-medium"${attr("disabled", actionLoading, true)}>Re-run Checks</button> <button class="button-secondary px-4 py-2 text-sm font-medium"${attr("disabled", actionLoading, true)}>Repair Bridge</button> <button class="button-secondary px-4 py-2 text-sm font-medium"${attr("disabled", actionLoading, true)}>Repair Directories</button> <button class="button-secondary px-4 py-2 text-sm font-medium"${attr("disabled", actionLoading, true)}>Repair Localdisk</button></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<div class="border border-line bg-chrome px-4 py-6 text-sm text-muted">No install status available yet.</div>`);
    }
    $$renderer2.push(`<!--]--></div></section>`);
    bind_props($$props, {
      status,
      loading,
      actionLoading,
      error,
      lastActionResult,
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
    let actionLoading = false;
    let error = "";
    let lastActionResult = null;
    async function loadStatus() {
      loading = true;
      error = "";
      try {
        status = await client.getInstallStatus();
      } catch (err) {
        error = err instanceof Error ? err.message : "Could not load install status.";
        toast.error(error);
      } finally {
        loading = false;
      }
    }
    async function bootstrapInstall() {
      actionLoading = true;
      lastActionResult = null;
      try {
        const result = await client.bootstrapInstall();
        lastActionResult = result;
        if (result.errors.length > 0) {
          toast.error(`Bootstrap completed with ${result.errors.length} error(s)`);
        } else if (result.warnings.length > 0) {
          toast.success(`Bootstrap completed with ${result.warnings.length} warning(s)`);
        } else {
          toast.success(`Bootstrap completed successfully: ${result.actions_taken.join(", ")}`);
        }
        await loadStatus();
      } catch (err) {
        const message = err instanceof Error ? err.message : "Bootstrap failed";
        toast.error(`Bootstrap failed: ${message}`);
      } finally {
        actionLoading = false;
      }
    }
    async function repairBridge() {
      await runRepair(
        {
          repair_bridge: true,
          repair_directories: false,
          repair_localdisk: false
        },
        "Bridge"
      );
    }
    async function repairDirectories() {
      await runRepair(
        {
          repair_bridge: false,
          repair_directories: true,
          repair_localdisk: false
        },
        "Directories"
      );
    }
    async function repairLocaldisk() {
      await runRepair(
        {
          repair_bridge: false,
          repair_directories: false,
          repair_localdisk: true
        },
        "Localdisk"
      );
    }
    async function runRepair(body, name) {
      actionLoading = true;
      lastActionResult = null;
      try {
        const result = await client.repairInstall(body);
        lastActionResult = result;
        if (result.errors.length > 0) {
          toast.error(`${name} repair completed with ${result.errors.length} error(s)`);
        } else if (result.warnings.length > 0) {
          toast.success(`${name} repair completed with ${result.warnings.length} warning(s)`);
        } else {
          toast.success(`${name} repair completed: ${result.actions_taken.join(", ")}`);
        }
        await loadStatus();
      } catch (err) {
        const message = err instanceof Error ? err.message : "Repair failed";
        toast.error(`${name} repair failed: ${message}`);
      } finally {
        actionLoading = false;
      }
    }
    InstallStatusPanel($$renderer2, {
      status,
      loading,
      actionLoading,
      error,
      lastActionResult,
      handleBootstrap: bootstrapInstall,
      handleRefresh: loadStatus,
      handleRepairBridge: repairBridge,
      handleRepairDirectories: repairDirectories,
      handleRepairLocaldisk: repairLocaldisk
    });
  });
}
export {
  _page as default
};
