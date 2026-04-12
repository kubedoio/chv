import "clsx";
import { o as onDestroy } from "../../../../chunks/index-server.js";
import { g as goto } from "../../../../chunks/client.js";
import { c as createAPIClient, g as getStoredToken } from "../../../../chunks/client2.js";
/* empty css                                                          */
import { j as bind_props, e as escape_html } from "../../../../chunks/root.js";
import { M as Modal } from "../../../../chunks/Modal.js";
import { T as Triangle_alert } from "../../../../chunks/triangle-alert.js";
/* empty css                                                       */
import "xterm";
import "xterm-addon-attach";
import "xterm-addon-fit";
function DeleteVMModal($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { open = false, vm = null, onSuccess } = $$props;
    createAPIClient({ token: getStoredToken() ?? void 0 });
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      {
        let children = function($$renderer4) {
          $$renderer4.push(`<div>`);
          {
            $$renderer4.push("<!--[0-->");
            $$renderer4.push(`<div class="flex items-start gap-3 text-amber-700">`);
            Triangle_alert($$renderer4, { size: 24 });
            $$renderer4.push(`<!----> <div><p class="font-medium">Are you sure you want to delete <strong>${escape_html(vm?.name)}</strong>?</p> <p class="text-sm text-muted mt-1">This action cannot be undone. The VM and all its data will be permanently removed.</p></div></div>`);
          }
          $$renderer4.push(`<!--]--></div>`);
        }, footer = function($$renderer4) {
          $$renderer4.push(`<button class="button-secondary svelte-vpf3gr">Cancel</button> `);
          {
            $$renderer4.push("<!--[0-->");
            $$renderer4.push(`<button class="button-danger svelte-vpf3gr">Delete VM</button>`);
          }
          $$renderer4.push(`<!--]-->`);
        };
        Modal($$renderer3, {
          title: "Delete VM",
          get open() {
            return open;
          },
          set open($$value) {
            open = $$value;
            $$settled = false;
          },
          children,
          footer,
          $$slots: { default: true, footer: true }
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
    createAPIClient({ token: getStoredToken() ?? void 0 });
    let vm = null;
    let deleteModalOpen = false;
    onDestroy(() => {
    });
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<div class="flex items-center justify-center h-64"><div class="text-muted">Loading...</div></div>`);
      }
      $$renderer3.push(`<!--]--> `);
      DeleteVMModal($$renderer3, {
        vm,
        onSuccess: () => goto(),
        get open() {
          return deleteModalOpen;
        },
        set open($$value) {
          deleteModalOpen = $$value;
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
