import { c as attr } from "../../../chunks/renderer.js";
import "@sveltejs/kit/internal";
import "../../../chunks/exports.js";
import "../../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../../chunks/root.js";
import "../../../chunks/client.js";
import { c as createAPIClient } from "../../../chunks/client2.js";
import "../../../chunks/toast.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let username = "";
    let password = "";
    let loading = false;
    createAPIClient();
    $$renderer2.push(`<div class="flex min-h-screen items-center justify-center bg-chrome p-6"><div class="w-full max-w-md table-card"><div class="card-header px-6 py-4"><div class="text-[11px] uppercase tracking-[0.16em] text-muted">CHV</div> <div class="mt-1 text-xl font-semibold">Sign In</div></div> <div class="space-y-5 p-6"><div class="text-sm text-muted">Default credentials: admin / admin</div> <label class="block"><span class="mb-2 block text-sm text-muted">Username</span> <input${attr("value", username)} class="w-full border border-line px-3 py-2 text-sm" placeholder="Enter username" autocomplete="username"/></label> <label class="block"><span class="mb-2 block text-sm text-muted">Password</span> <input${attr("value", password)} type="password" class="w-full border border-line px-3 py-2 text-sm" placeholder="Enter password" autocomplete="current-password"/></label> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> <button class="button-primary w-full px-4 py-2 text-sm font-medium"${attr("disabled", loading, true)}>`);
    {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`Sign In`);
    }
    $$renderer2.push(`<!--]--></button> <div class="border-t border-line pt-4"><div class="text-xs text-muted"><p><strong>First time?</strong> Use the default credentials above.</p></div></div></div></div></div>`);
  });
}
export {
  _page as default
};
