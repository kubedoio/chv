import { j as ensure_array_like, l as attr, m as escape_html } from './renderer-Xy7Nl1fv.js';
import { c as createAPIClient, g as getStoredToken } from './client-iYtOZkWx.js';
import { S as StateBadge } from './StateBadge-D-RbwxiK.js';
import './shared-server-BU2DVf8Q.js';

function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    createAPIClient({ token: getStoredToken() ?? void 0 });
    let items = [];
    $$renderer2.push(`<section class="table-card"><div class="card-header px-4 py-3"><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Virtual Machines</div> <div class="mt-1 text-lg font-semibold">Desired vs Actual Runtime State</div></div> <table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Name</th><th class="border-b border-line px-4 py-3">Desired</th><th class="border-b border-line px-4 py-3">Actual</th><th class="border-b border-line px-4 py-3">CPU</th><th class="border-b border-line px-4 py-3">Memory</th><th class="border-b border-line px-4 py-3">IP</th><th class="border-b border-line px-4 py-3">Error</th></tr></thead><tbody><!--[-->`);
    const each_array = ensure_array_like(items);
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      let item = each_array[$$index];
      $$renderer2.push(`<tr class="odd:bg-white even:bg-[#f8f8f8]"><td class="border-b border-line px-4 py-3"><a class="text-primary no-underline"${attr("href", `/vms/${item.id}`)}>${escape_html(item.name)}</a></td><td class="border-b border-line px-4 py-3">`);
      StateBadge($$renderer2, { label: item.desired_state });
      $$renderer2.push(`<!----></td><td class="border-b border-line px-4 py-3">`);
      StateBadge($$renderer2, { label: item.actual_state });
      $$renderer2.push(`<!----></td><td class="border-b border-line px-4 py-3 mono">${escape_html(item.vcpu)}</td><td class="border-b border-line px-4 py-3 mono">${escape_html(item.memory_mb)} MB</td><td class="border-b border-line px-4 py-3 mono">${escape_html(item.ip_address || "pending")}</td><td class="border-b border-line px-4 py-3">${escape_html(item.last_error || "—")}</td></tr>`);
    }
    $$renderer2.push(`<!--]--></tbody></table></section>`);
  });
}

export { _page as default };
//# sourceMappingURL=_page.svelte-D0aWxyZw.js.map
