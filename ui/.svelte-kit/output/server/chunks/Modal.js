import { g as attr_class, k as bind_props, c as attr, i as stringify, e as escape_html, f as derived } from "./root.js";
import "clsx";
function FocusTrap($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { children } = $$props;
    $$renderer2.push(`<div class="focus-trap-container svelte-l5ckn">`);
    children($$renderer2);
    $$renderer2.push(`<!----></div>`);
  });
}
function Modal($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let {
      open = false,
      title,
      width = "default",
      closeOnBackdrop = true,
      closeOnEscape = true,
      onClose,
      description,
      header,
      children,
      footer
    } = $$props;
    const widthClasses = { default: "w-[480px]", wide: "w-[640px]" };
    let isVisible = false;
    let modalId = `modal-${Math.random().toString(36).slice(2, 9)}`;
    let titleId = derived(() => title ? `${modalId}-title` : void 0);
    let descId = derived(() => description ? `${modalId}-description` : void 0);
    if (
      // Use setTimeout instead of requestAnimationFrame to avoid race conditions
      open
    ) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div${attr_class("fixed inset-0 z-50 flex items-center justify-center bg-black/50 transition-opacity duration-200 ease-out", void 0, {
        "opacity-0": !isVisible,
        "opacity-100": isVisible
      })} aria-hidden="true">`);
      FocusTrap($$renderer2, {
        children: ($$renderer3) => {
          $$renderer3.push(`<div role="dialog" aria-modal="true"${attr("aria-labelledby", titleId())}${attr("aria-describedby", descId())} tabindex="-1"${attr_class(`${stringify(widthClasses[width])} max-h-[80vh] overflow-hidden rounded-lg bg-white shadow-[0_4px_16px_rgba(0,0,0,0.15)] transition-all duration-200 ease-out mx-4`, void 0, {
            "scale-95": !isVisible,
            "scale-100": isVisible,
            "opacity-0": !isVisible,
            "opacity-100": isVisible
          })}><div class="flex h-14 items-center justify-between border-b border-line px-6">`);
          if (header) {
            $$renderer3.push("<!--[0-->");
            header($$renderer3);
            $$renderer3.push(`<!---->`);
          } else if (title) {
            $$renderer3.push("<!--[1-->");
            $$renderer3.push(`<div><h2${attr("id", titleId())} class="text-base font-semibold text-ink">${escape_html(title)}</h2> `);
            if (description) {
              $$renderer3.push("<!--[0-->");
              $$renderer3.push(`<p${attr("id", descId())} class="text-sm text-muted mt-0.5">${escape_html(description)}</p>`);
            } else {
              $$renderer3.push("<!--[-1-->");
            }
            $$renderer3.push(`<!--]--></div>`);
          } else {
            $$renderer3.push("<!--[-1-->");
            $$renderer3.push(`<div></div>`);
          }
          $$renderer3.push(`<!--]--> <button class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded text-muted transition-colors hover:bg-black/5 hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2" aria-label="Close modal" type="button"><svg xmlns="http://www.w3.org/2000/svg"${attr("width", 20)}${attr("height", 20)} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M18 6 6 18"></path><path d="m6 6 12 12"></path></svg></button></div> `);
          if (children) {
            $$renderer3.push("<!--[0-->");
            $$renderer3.push(`<div class="max-h-[calc(80vh-3.5rem-4.5rem)] overflow-y-auto p-6">`);
            children($$renderer3);
            $$renderer3.push(`<!----></div>`);
          } else {
            $$renderer3.push("<!--[-1-->");
          }
          $$renderer3.push(`<!--]--> `);
          if (footer) {
            $$renderer3.push("<!--[0-->");
            $$renderer3.push(`<div class="flex justify-end gap-2 border-t border-line px-6 py-4">`);
            footer($$renderer3);
            $$renderer3.push(`<!----></div>`);
          } else {
            $$renderer3.push("<!--[-1-->");
          }
          $$renderer3.push(`<!--]--></div>`);
        }
      });
      $$renderer2.push(`<!----></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]-->`);
    bind_props($$props, { open });
  });
}
export {
  Modal as M
};
