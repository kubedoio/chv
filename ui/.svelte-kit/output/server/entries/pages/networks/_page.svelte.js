import { e as ensure_array_like, c as escape_html } from "../../../chunks/renderer.js";
import { c as createAPIClient, g as getStoredToken } from "../../../chunks/client.js";
import { S as StateBadge } from "../../../chunks/StateBadge.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    createAPIClient({ token: getStoredToken() ?? void 0 });
    let items = [];
    $$renderer2.push(`<section class="table-card"><div class="card-header px-4 py-3"><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Networks</div> <div class="mt-1 text-lg font-semibold">Bridge-backed Networks</div></div> <table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Name</th><th class="border-b border-line px-4 py-3">Bridge</th><th class="border-b border-line px-4 py-3">CIDR</th><th class="border-b border-line px-4 py-3">Gateway</th><th class="border-b border-line px-4 py-3">Managed</th><th class="border-b border-line px-4 py-3">Status</th></tr></thead><tbody><!--[-->`);
    const each_array = ensure_array_like(items);
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      let item = each_array[$$index];
      $$renderer2.push(`<tr class="odd:bg-white even:bg-[#f8f8f8]"><td class="border-b border-line px-4 py-3">${escape_html(item.name)}</td><td class="border-b border-line px-4 py-3 mono">${escape_html(item.bridge_name)}</td><td class="border-b border-line px-4 py-3 mono">${escape_html(item.cidr)}</td><td class="border-b border-line px-4 py-3 mono">${escape_html(item.gateway_ip)}</td><td class="border-b border-line px-4 py-3">${escape_html(item.is_system_managed ? "system" : "manual")}</td><td class="border-b border-line px-4 py-3">`);
      StateBadge($$renderer2, { label: item.status });
      $$renderer2.push(`<!----></td></tr>`);
    }
    $$renderer2.push(`<!--]--></tbody></table></section>`);
  });
}
export {
  _page as default
};
