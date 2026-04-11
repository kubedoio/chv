import { e as escape_html, d as ensure_array_like } from "../../../chunks/root.js";
import { c as createAPIClient } from "../../../chunks/client2.js";
import { C as Clock } from "../../../chunks/clock.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    createAPIClient();
    $$renderer2.push(`<div class="space-y-6"><div class="flex items-center justify-between"><div><h1 class="text-2xl font-semibold text-slate-900">Metrics Dashboard</h1> <p class="text-sm text-slate-500 mt-1">Real-time monitoring and performance metrics</p></div> <div class="flex items-center gap-2 text-sm text-slate-500">`);
    Clock($$renderer2, { size: 16 });
    $$renderer2.push(`<!----> <span>Updated: ${escape_html((/* @__PURE__ */ new Date()).toLocaleTimeString())}</span></div></div> `);
    {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4"><!--[-->`);
      const each_array = ensure_array_like(Array(4));
      for (let i = 0, $$length = each_array.length; i < $$length; i++) {
        each_array[i];
        $$renderer2.push(`<div class="bg-white rounded-lg border border-slate-200 p-6 animate-pulse"><div class="h-4 bg-slate-200 rounded w-24 mb-4"></div> <div class="h-8 bg-slate-200 rounded w-16"></div></div>`);
      }
      $$renderer2.push(`<!--]--></div>`);
    }
    $$renderer2.push(`<!--]--></div>`);
  });
}
export {
  _page as default
};
