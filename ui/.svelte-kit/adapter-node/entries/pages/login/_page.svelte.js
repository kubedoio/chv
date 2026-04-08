import { b as attr, c as escape_html } from "../../../chunks/renderer.js";
import "@sveltejs/kit/internal";
import "../../../chunks/exports.js";
import "../../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../../chunks/root.js";
import "../../../chunks/state.svelte.js";
import { c as createAPIClient } from "../../../chunks/client2.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let token = "";
    let tokenName = "admin";
    createAPIClient();
    $$renderer2.push(`<div class="flex min-h-screen items-center justify-center bg-chrome p-6"><div class="w-full max-w-lg table-card"><div class="card-header px-6 py-4"><div class="text-[11px] uppercase tracking-[0.16em] text-muted">CHV Login</div> <div class="mt-1 text-xl font-semibold">Bearer Token Access</div></div> <div class="space-y-5 p-6"><label class="block"><span class="mb-2 block text-sm text-muted">Token name</span> <input${attr("value", tokenName)} class="w-full border border-line px-3 py-2 text-sm"/></label> <label class="block"><span class="mb-2 block text-sm text-muted">Existing token</span> <textarea rows="5" class="mono w-full border border-line px-3 py-2 text-sm">`);
    const $$body = escape_html(token);
    if ($$body) {
      $$renderer2.push(`${$$body}`);
    }
    $$renderer2.push(`</textarea></label> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> <div class="flex flex-wrap gap-3"><button class="button-primary px-4 py-2 text-sm font-medium">Use Token</button> <button class="button-secondary px-4 py-2 text-sm font-medium">Create Token</button></div></div></div></div>`);
  });
}
export {
  _page as default
};
