import { k as bind_props, g as attr_class, c as attr, e as escape_html, i as stringify } from "./renderer.js";
import { c as createAPIClient, g as getStoredToken } from "./client2.js";
import { M as Modal } from "./Modal.js";
import { F as FormField, I as Input } from "./Input.js";
import "./toast.js";
function ImportImageModal($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { open = false, onSuccess } = $$props;
    createAPIClient({ token: getStoredToken() ?? void 0 });
    let name = "";
    let sourceUrl = "";
    let checksum = "";
    let osFamily = "linux";
    let architecture = "x86_64";
    let submitting = false;
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      {
        let footer = function($$renderer4) {
          $$renderer4.push(`<button type="button" class="button-secondary px-4 py-2 rounded text-sm">Cancel</button> <button type="button"${attr("disabled", submitting, true)} class="button-primary px-4 py-2 rounded text-sm">${escape_html("Start Import")}</button>`);
        };
        Modal($$renderer3, {
          title: "Import Image",
          get open() {
            return open;
          },
          set open($$value) {
            open = $$value;
            $$settled = false;
          },
          footer,
          children: ($$renderer4) => {
            $$renderer4.push(`<div class="tabs flex border-b border-line mb-4 svelte-lv1n1t"><button${attr_class(`px-4 py-2 text-sm font-medium ${stringify("border-b-2 border-accent text-accent")}`, "svelte-lv1n1t")}>Remote URL</button> <button${attr_class(`px-4 py-2 text-sm font-medium ${stringify("text-muted")}`, "svelte-lv1n1t")}>Local File</button></div> <form class="space-y-4">`);
            {
              $$renderer4.push("<!--[-1-->");
            }
            $$renderer4.push(`<!--]--> `);
            FormField($$renderer4, {
              label: "Name",
              required: true,
              children: ($$renderer5) => {
                Input($$renderer5, {
                  placeholder: "ubuntu-22.04",
                  get value() {
                    return name;
                  },
                  set value($$value) {
                    name = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            {
              $$renderer4.push("<!--[0-->");
              FormField($$renderer4, {
                label: "Source URL",
                required: true,
                helper: "URL to qcow2 image",
                children: ($$renderer5) => {
                  Input($$renderer5, {
                    placeholder: "https://cloud-images.ubuntu.com/...",
                    get value() {
                      return sourceUrl;
                    },
                    set value($$value) {
                      sourceUrl = $$value;
                      $$settled = false;
                    }
                  });
                }
              });
              $$renderer4.push(`<!----> `);
              FormField($$renderer4, {
                label: "Checksum",
                helper: "sha256:hash (optional)",
                children: ($$renderer5) => {
                  Input($$renderer5, {
                    placeholder: "sha256:abc123...",
                    get value() {
                      return checksum;
                    },
                    set value($$value) {
                      checksum = $$value;
                      $$settled = false;
                    }
                  });
                }
              });
              $$renderer4.push(`<!---->`);
            }
            $$renderer4.push(`<!--]--> <div class="grid grid-cols-2 gap-4">`);
            FormField($$renderer4, {
              label: "OS Family",
              children: ($$renderer5) => {
                $$renderer5.select(
                  {
                    value: osFamily,
                    class: "w-full border border-line rounded px-3 py-2 text-sm"
                  },
                  ($$renderer6) => {
                    $$renderer6.option({ value: "linux" }, ($$renderer7) => {
                      $$renderer7.push(`Linux`);
                    });
                  }
                );
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "Architecture",
              children: ($$renderer5) => {
                $$renderer5.select(
                  {
                    value: architecture,
                    class: "w-full border border-line rounded px-3 py-2 text-sm"
                  },
                  ($$renderer6) => {
                    $$renderer6.option({ value: "x86_64" }, ($$renderer7) => {
                      $$renderer7.push(`x86_64`);
                    });
                    $$renderer6.option({ value: "aarch64" }, ($$renderer7) => {
                      $$renderer7.push(`aarch64`);
                    });
                  }
                );
              }
            });
            $$renderer4.push(`<!----></div> `);
            FormField($$renderer4, {
              label: "Format",
              children: ($$renderer5) => {
                Input($$renderer5, { value: "qcow2", disabled: true });
                $$renderer5.push(`<!----> <p class="text-xs text-muted mt-1">Images are normalized to qcow2 for MVP-1</p>`);
              }
            });
            $$renderer4.push(`<!----></form>`);
          },
          $$slots: { footer: true, default: true }
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
  ImportImageModal as I
};
