import { c as attr, g as attr_class } from "../../../chunks/renderer.js";
import "../../../chunks/client2.js";
import "../../../chunks/toast.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let loading = true;
    $$renderer2.push(`<div class="space-y-6"><div class="flex items-center justify-between"><div><h1 class="text-2xl font-bold text-slate-900">Resource Quotas</h1> <p class="text-sm text-slate-500 mt-1">Monitor your resource usage and limits</p></div> <button${attr("disabled", loading, true)} class="px-4 py-2 bg-white border border-slate-200 text-slate-700 rounded-lg hover:bg-slate-50 transition-colors disabled:opacity-50 flex items-center gap-2"><span${attr_class("animate-spin", void 0, { "hidden": !loading })}>↻</span> Refresh</button></div> `);
    {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="flex items-center justify-center py-12"><div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div> <span class="ml-3 text-slate-500">Loading quota data...</span></div>`);
    }
    $$renderer2.push(`<!--]--></div>`);
  });
}
export {
  _page as default
};
