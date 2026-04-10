import { n as head, e as escape_html } from "../../../../chunks/renderer.js";
import { o as onDestroy } from "../../../../chunks/index-server.js";
import "@sveltejs/kit/internal";
import "../../../../chunks/exports.js";
import "../../../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../../../chunks/root.js";
import "../../../../chunks/client.js";
import { g as getStoredToken, c as createAPIClient } from "../../../../chunks/client2.js";
/* empty css                                                          */
import { R as Refresh_cw } from "../../../../chunks/refresh-cw.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    const token = getStoredToken();
    createAPIClient({ token: token ?? void 0 });
    onDestroy(() => {
    });
    head("jrs5ma", $$renderer2, ($$renderer3) => {
      $$renderer3.title(($$renderer4) => {
        $$renderer4.push(`<title>${escape_html("Node")} | Node Details</title>`);
      });
    });
    {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="flex items-center justify-center h-96"><div class="flex items-center gap-3 text-slate-500">`);
      Refresh_cw($$renderer2, { class: "animate-spin", size: 24 });
      $$renderer2.push(`<!----> <span>Loading node information...</span></div></div>`);
    }
    $$renderer2.push(`<!--]-->`);
  });
}
export {
  _page as default
};
