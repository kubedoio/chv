import { c as store_get, d as slot, f as unsubscribe_stores, h as fallback, j as ensure_array_like, k as attr_class, l as attr, m as escape_html, n as bind_props } from './renderer-Xy7Nl1fv.js';
import { p as page } from './stores-BmPt74IK.js';
import './root-DBsdwk96.js';
import './state.svelte-Bdbc4HWz.js';

function Sidebar($$renderer, $$props) {
  let currentPath = fallback($$props["currentPath"], "/");
  const items = [
    { href: "/", label: "Overview" },
    { href: "/install", label: "Install" },
    { href: "/networks", label: "Networks" },
    { href: "/storage", label: "Storage" },
    { href: "/images", label: "Images" },
    { href: "/vms", label: "Virtual Machines" },
    { href: "/operations", label: "Operations" },
    { href: "/settings", label: "Settings" }
  ];
  $$renderer.push(`<aside class="border-r border-line bg-chrome"><div class="border-b border-line px-5 py-4"><div class="text-[11px] uppercase tracking-[0.2em] text-muted">CHV</div> <div class="mt-2 text-lg font-semibold text-ink">Operator Console</div> <div class="mt-1 text-sm text-muted">Cloud Hypervisor MVP-1</div></div> <nav class="p-3"><!--[-->`);
  const each_array = ensure_array_like(items);
  for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
    let item = each_array[$$index];
    $$renderer.push(`<a${attr_class(`mb-1 block border px-3 py-2 text-sm no-underline transition ${currentPath === item.href ? "border-primary bg-selected text-ink" : "border-transparent text-muted hover:border-line hover:bg-white hover:text-ink"}`)}${attr("href", item.href)}>${escape_html(item.label)}</a>`);
  }
  $$renderer.push(`<!--]--></nav></aside>`);
  bind_props($$props, { currentPath });
}
function _layout($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    var $$store_subs;
    if (store_get($$store_subs ??= {}, "$page", page).url.pathname === "/login") {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<!--[-->`);
      slot($$renderer2, $$props, "default", {});
      $$renderer2.push(`<!--]-->`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<div class="console-shell">`);
      Sidebar($$renderer2, {
        currentPath: store_get($$store_subs ??= {}, "$page", page).url.pathname
      });
      $$renderer2.push(`<!----> <main class="min-w-0"><header class="border-b border-line bg-chrome px-6 py-4"><div class="text-[11px] uppercase tracking-[0.18em] text-muted">CHV</div> <div class="mt-1 text-xl font-semibold text-ink">Cloud Hypervisor Virtualization</div></header> <div class="p-6"><!--[-->`);
      slot($$renderer2, $$props, "default", {});
      $$renderer2.push(`<!--]--></div></main></div>`);
    }
    $$renderer2.push(`<!--]-->`);
    if ($$store_subs) unsubscribe_stores($$store_subs);
  });
}

export { _layout as default };
//# sourceMappingURL=_layout.svelte-D8sx2ua-.js.map
