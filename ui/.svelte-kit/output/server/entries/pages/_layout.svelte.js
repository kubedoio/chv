import { g as getContext, f as fallback, s as store_get, e as ensure_array_like, a as attr_class, b as attr, c as escape_html, u as unsubscribe_stores, d as bind_props, h as stringify, i as derived, j as slot } from "../../chunks/renderer.js";
import "@sveltejs/kit/internal";
import "../../chunks/exports.js";
import "../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../chunks/root.js";
import "../../chunks/client.js";
import "clsx";
import { o as onDestroy } from "../../chunks/index-server.js";
import { c as createAPIClient } from "../../chunks/client2.js";
import { t as toast } from "../../chunks/toast.js";
const getStores = () => {
  const stores$1 = getContext("__svelte__");
  return {
    /** @type {typeof page} */
    page: {
      subscribe: stores$1.page.subscribe
    },
    /** @type {typeof navigating} */
    navigating: {
      subscribe: stores$1.navigating.subscribe
    },
    /** @type {typeof updated} */
    updated: stores$1.updated
  };
};
const page = {
  subscribe(fn) {
    const store = getStores().page;
    return store.subscribe(fn);
  }
};
function Sidebar($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    var $$store_subs;
    let currentPath = fallback($$props["currentPath"], "/");
    const items = [
      { href: "/", label: "Overview" },
      { href: "/install", label: "Install" },
      { href: "/networks", label: "Networks" },
      { href: "/storage", label: "Storage" },
      { href: "/images", label: "Images" },
      { href: "/vms", label: "Virtual Machines" },
      { href: "/operations", label: "Operations" },
      { href: "/events", label: "Events" },
      { href: "/settings", label: "Settings" }
    ];
    createAPIClient();
    let newEvents = 0;
    onDestroy(() => {
    });
    function clearBadge() {
      newEvents = 0;
    }
    if (store_get($$store_subs ??= {}, "$page", page)?.url?.pathname === "/events") {
      clearBadge();
    }
    $$renderer2.push(`<aside class="border-r border-line bg-chrome"><div class="border-b border-line px-5 py-4"><div class="text-[11px] uppercase tracking-[0.2em] text-muted">CHV</div> <div class="mt-2 text-lg font-semibold text-ink">Operator Console</div> <div class="mt-1 text-sm text-muted">Cloud Hypervisor MVP-1</div></div> <nav class="p-3"><!--[-->`);
    const each_array = ensure_array_like(items);
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      let item = each_array[$$index];
      $$renderer2.push(`<a${attr_class(
        `mb-1 flex items-center border px-3 py-2 text-sm no-underline transition ${currentPath === item.href ? "border-primary bg-selected text-ink" : "border-transparent text-muted hover:border-line hover:bg-white hover:text-ink"}`,
        "svelte-129hoe0"
      )}${attr("href", item.href)}><span>${escape_html(item.label)}</span> `);
      if (item.href === "/events" && newEvents > 0) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<span class="badge svelte-129hoe0">${escape_html(newEvents)}</span>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]--></a>`);
    }
    $$renderer2.push(`<!--]--></nav></aside>`);
    if ($$store_subs) unsubscribe_stores($$store_subs);
    bind_props($$props, { currentPath });
  });
}
function Toast($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { type, message } = $$props;
    const styles = {
      success: {
        bg: "bg-[#F0F9F0]",
        border: "border-l-[#54B435]",
        iconColor: "text-[#54B435]"
      },
      error: {
        bg: "bg-[#FFF0F0]",
        border: "border-l-[#E60000]",
        iconColor: "text-[#E60000]"
      },
      info: {
        bg: "bg-[#E8F4FC]",
        border: "border-l-[#0066CC]",
        iconColor: "text-[#0066CC]"
      }
    };
    let style = derived(() => styles[type]);
    $$renderer2.push(`<div${attr_class(`w-[320px] rounded shadow-[0_4px_12px_rgba(0,0,0,0.15)] border-l-4 flex items-start gap-3 p-4 ${stringify(style().bg)} ${stringify(style().border)}`)} role="alert" aria-live="polite"><div${attr_class(`flex-shrink-0 ${stringify(style().iconColor)}`)}>`);
    if (type === "success") {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg>`);
    } else if (type === "error") {
      $$renderer2.push("<!--[1-->");
      $$renderer2.push(`<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 16h.01"></path><path d="M12 8v4"></path><path d="M15.312 2a2 2 0 0 1 1.414.586l4.688 4.688A2 2 0 0 1 22 8.688v6.624a2 2 0 0 1-.586 1.414l-4.688 4.688a2 2 0 0 1-1.414.586H8.688a2 2 0 0 1-1.414-.586l-4.688-4.688A2 2 0 0 1 2 15.312V8.688a2 2 0 0 1 .586-1.414l4.688-4.688A2 2 0 0 1 8.688 2z"></path></svg>`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="10"></circle><path d="M12 16v-4"></path><path d="M12 8h.01"></path></svg>`);
    }
    $$renderer2.push(`<!--]--></div> <div class="flex-1 text-sm text-ink leading-5">${escape_html(message)}</div> <button class="flex-shrink-0 p-1 rounded hover:bg-black/5 transition-colors" aria-label="Dismiss notification" type="button"><svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-muted" aria-hidden="true"><path d="M18 6 6 18"></path><path d="m6 6 12 12"></path></svg></button></div>`);
  });
}
function ToastContainer($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    var $$store_subs;
    if (store_get($$store_subs ??= {}, "$toast", toast).toasts.length > 0) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="fixed top-4 right-4 z-50 flex flex-col gap-2" role="region" aria-label="Notifications"><!--[-->`);
      const each_array = ensure_array_like(store_get($$store_subs ??= {}, "$toast", toast).toasts);
      for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
        let toastItem = each_array[$$index];
        Toast($$renderer2, {
          id: toastItem.id,
          type: toastItem.type,
          message: toastItem.message
        });
      }
      $$renderer2.push(`<!--]--></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]-->`);
    if ($$store_subs) unsubscribe_stores($$store_subs);
  });
}
function _layout($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    var $$store_subs;
    ToastContainer($$renderer2);
    $$renderer2.push(`<!----> `);
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
export {
  _layout as default
};
