import { d as bind_props, c as escape_html, a as attr_class, h as stringify } from "../../../../chunks/renderer.js";
import "clsx";
import { M as Modal } from "../../../../chunks/Modal.js";
import { T as Triangle_alert } from "../../../../chunks/triangle-alert.js";
import { t as toast } from "../../../../chunks/toast.js";
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
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let showDangerDialog = false;
    let showPrimaryDialog = false;
    let lastAction = null;
    function handleDangerConfirm() {
      lastAction = "Danger confirmed - Item deleted!";
      toast.success("Item deleted successfully");
    }
    function handleDangerCancel() {
      lastAction = "Danger cancelled";
    }
    function handlePrimaryConfirm() {
      lastAction = "Primary confirmed - Action completed!";
      toast.success("Action completed");
    }
    function handlePrimaryCancel() {
      lastAction = "Primary cancelled";
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      $$renderer3.push(`<div class="container mx-auto max-w-4xl p-8"><h1 class="mb-8 text-2xl font-bold text-ink">ConfirmDialog Component Test</h1> <div class="space-y-8"><section class="rounded-lg border border-line bg-white p-6"><h2 class="mb-4 text-lg font-semibold text-ink">Danger Variant (Default)</h2> <p class="mb-4 text-sm text-muted">Used for destructive actions like delete, remove, or irreversible operations.</p> <button class="rounded border border-danger px-4 py-2 text-danger transition-colors hover:bg-danger/5">Open Danger Dialog</button></section> <section class="rounded-lg border border-line bg-white p-6"><h2 class="mb-4 text-lg font-semibold text-ink">Primary Variant</h2> <p class="mb-4 text-sm text-muted">Used for non-destructive confirmations that still need user approval.</p> <button class="rounded bg-primary px-4 py-2 text-white transition-colors hover:bg-primary/90">Open Primary Dialog</button></section> <section class="rounded-lg border border-line bg-chrome p-6"><h2 class="mb-4 text-lg font-semibold text-ink">Action Log</h2> `);
      if (lastAction) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<p class="text-sm text-ink" data-testid="action-log">${escape_html(lastAction)}</p>`);
      } else {
        $$renderer3.push("<!--[-1-->");
        $$renderer3.push(`<p class="text-sm text-muted">No actions yet. Click the buttons above to test.</p>`);
      }
      $$renderer3.push(`<!--]--></section> <section class="rounded-lg border border-line bg-white p-6"><h2 class="mb-4 text-lg font-semibold text-ink">Verification Checklist</h2> <ul class="space-y-2 text-sm text-muted"><li class="flex items-center gap-2"><span class="text-success">✓</span> <span>Title prop displays correctly</span></li> <li class="flex items-center gap-2"><span class="text-success">✓</span> <span>Description text shows consequences</span></li> <li class="flex items-center gap-2"><span class="text-success">✓</span> <span>Warning icon appears for danger variant</span></li> <li class="flex items-center gap-2"><span class="text-success">✓</span> <span>Danger button has red border and text</span></li> <li class="flex items-center gap-2"><span class="text-success">✓</span> <span>Cancel button uses secondary style</span></li> <li class="flex items-center gap-2"><span class="text-success">✓</span> <span>Buttons are right-aligned with 8px gap</span></li> <li class="flex items-center gap-2"><span class="text-success">✓</span> <span>Confirm button is focused when opened</span></li> <li class="flex items-center gap-2"><span class="text-success">✓</span> <span>onConfirm callback fires when confirmed</span></li> <li class="flex items-center gap-2"><span class="text-success">✓</span> <span>onCancel callback fires when cancelled</span></li></ul></section></div></div> `);
      ConfirmDialog($$renderer3, {
        title: "Delete VM?",
        description: "This action cannot be undone. The VM data will be permanently removed.",
        confirmText: "Delete",
        variant: "danger",
        onConfirm: handleDangerConfirm,
        onCancel: handleDangerCancel,
        get open() {
          return showDangerDialog;
        },
        set open($$value) {
          showDangerDialog = $$value;
          $$settled = false;
        }
      });
      $$renderer3.push(`<!----> `);
      ConfirmDialog($$renderer3, {
        title: "Apply Changes?",
        description: "This will update the configuration and restart the service. The service will be unavailable for a few seconds.",
        confirmText: "Apply",
        variant: "primary",
        onConfirm: handlePrimaryConfirm,
        onCancel: handlePrimaryCancel,
        get open() {
          return showPrimaryDialog;
        },
        set open($$value) {
          showPrimaryDialog = $$value;
          $$settled = false;
        }
      });
      $$renderer3.push(`<!---->`);
    }
    do {
      $$settled = true;
      $$inner_renderer = $$renderer2.copy();
      $$render_inner($$inner_renderer);
    } while (!$$settled);
    $$renderer2.subsume($$inner_renderer);
  });
}
export {
  _page as default
};
