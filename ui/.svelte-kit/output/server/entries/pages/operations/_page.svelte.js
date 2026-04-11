import { d as ensure_array_like } from "../../../chunks/root.js";
import "@sveltejs/kit/internal";
import "../../../chunks/exports.js";
import "../../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../../chunks/client.js";
import { g as getStoredToken, c as createAPIClient } from "../../../chunks/client2.js";
/* empty css                                                       */
import { S as SkeletonRow } from "../../../chunks/EmptyState.svelte_svelte_type_style_lang.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    const token = getStoredToken();
    createAPIClient({ token: token ?? void 0 });
    $$renderer2.push(`<section class="table-card"><div class="card-header px-4 py-3"><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Operations</div> <div class="mt-1 text-lg font-semibold">Auditable Change Log</div></div> `);
    {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Resource</th><th class="border-b border-line px-4 py-3">Operation</th><th class="border-b border-line px-4 py-3">State</th><th class="border-b border-line px-4 py-3">Created</th></tr></thead><tbody><!--[-->`);
      const each_array = ensure_array_like(Array(5));
      for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
        each_array[$$index];
        SkeletonRow($$renderer2, { columns: 4 });
      }
      $$renderer2.push(`<!--]--></tbody></table>`);
    }
    $$renderer2.push(`<!--]--></section>`);
  });
}
export {
  _page as default
};
