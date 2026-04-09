import { a as attr_class, b as attr, c as escape_html, d as bind_props, h as stringify } from "./renderer.js";
function Modal($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let {
      open = false,
      title,
      width = "default",
      closeOnBackdrop = true,
      closeOnEscape = true,
      onClose,
      header,
      children,
      footer
    } = $$props;
    const widthClasses = { default: "w-[480px]", wide: "w-[640px]" };
    let isVisible = false;
    if (
      // Use setTimeout instead of requestAnimationFrame to avoid race conditions
      open
    ) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div${attr_class("fixed inset-0 z-50 flex items-center justify-center bg-black/50 transition-opacity duration-200 ease-out", void 0, {
        "opacity-0": !isVisible,
        "opacity-100": isVisible
      })} aria-hidden="true"><div role="dialog" aria-modal="true"${attr("aria-labelledby", title ? "modal-title" : void 0)} tabindex="-1"${attr_class(`${stringify(widthClasses[width])} max-h-[80vh] overflow-hidden rounded-lg bg-white shadow-[0_4px_16px_rgba(0,0,0,0.15)] transition-all duration-200 ease-out`, void 0, {
        "scale-95": !isVisible,
        "scale-100": isVisible,
        "opacity-0": !isVisible,
        "opacity-100": isVisible
      })}><div class="flex h-14 items-center justify-between border-b border-line px-6">`);
      if (header) {
        $$renderer2.push("<!--[0-->");
        header($$renderer2);
        $$renderer2.push(`<!---->`);
      } else if (title) {
        $$renderer2.push("<!--[1-->");
        $$renderer2.push(`<h2 id="modal-title" class="text-base font-semibold text-ink">${escape_html(title)}</h2>`);
      } else {
        $$renderer2.push("<!--[-1-->");
        $$renderer2.push(`<div></div>`);
      }
      $$renderer2.push(`<!--]--> <button class="flex h-6 w-6 flex-shrink-0 items-center justify-center rounded text-muted transition-colors hover:bg-black/5 hover:text-ink" aria-label="Close modal" type="button"><svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M18 6 6 18"></path><path d="m6 6 12 12"></path></svg></button></div> `);
      if (children) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<div class="max-h-[calc(80vh-3.5rem-4.5rem)] overflow-y-auto p-6">`);
        children($$renderer2);
        $$renderer2.push(`<!----></div>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]--> `);
      if (footer) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<div class="flex justify-end gap-2 border-t border-line px-6 py-4">`);
        footer($$renderer2);
        $$renderer2.push(`<!----></div>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]--></div></div>`);
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
