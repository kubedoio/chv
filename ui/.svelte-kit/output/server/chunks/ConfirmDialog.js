import { j as bind_props, e as escape_html, d as attr_class, f as stringify } from "./root.js";
import { M as Modal } from "./Modal.js";
import { T as Triangle_alert } from "./triangle-alert.js";
function ConfirmDialog($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let {
      open = false,
      title,
      description,
      confirmText = "Confirm",
      cancelText = "Cancel",
      variant = "danger",
      onConfirm,
      onCancel
    } = $$props;
    function handleCancel() {
      open = false;
      onCancel?.();
    }
    const confirmButtonClasses = {
      danger: "border border-danger text-danger hover:bg-danger/5 focus:ring-danger/30",
      primary: "bg-primary text-white hover:bg-primary/90 focus:ring-primary/30"
    };
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      {
        let header = function($$renderer4) {
          $$renderer4.push(`<div class="flex items-center gap-3">`);
          if (variant === "danger") {
            $$renderer4.push("<!--[0-->");
            Triangle_alert($$renderer4, { class: "h-5 w-5 text-warning", "aria-hidden": "true" });
          } else {
            $$renderer4.push("<!--[-1-->");
          }
          $$renderer4.push(`<!--]--> <h2 id="modal-title" class="text-base font-semibold text-ink">${escape_html(title)}</h2></div>`);
        }, footer = function($$renderer4) {
          $$renderer4.push(`<button type="button" class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors">${escape_html(cancelText)}</button> <button type="button"${attr_class(`px-4 py-2 rounded font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-offset-1 ${stringify(confirmButtonClasses[variant])}`)}>${escape_html(confirmText)}</button>`);
        };
        Modal($$renderer3, {
          closeOnBackdrop: true,
          onClose: handleCancel,
          get open() {
            return open;
          },
          set open($$value) {
            open = $$value;
            $$settled = false;
          },
          header,
          footer,
          children: ($$renderer4) => {
            $$renderer4.push(`<p class="text-sm text-muted">${escape_html(description)}</p>`);
          },
          $$slots: { header: true, footer: true, default: true }
        });
      }
    }
    do {
      $$settled = true;
      $$inner_renderer = $$renderer2.copy();
      $$render_inner($$inner_renderer);
    } while (!$$settled);
    $$renderer2.subsume($$inner_renderer);
    bind_props($$props, { open });
  });
}
export {
  ConfirmDialog as C
};
