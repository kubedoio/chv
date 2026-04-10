import { k as bind_props, i as stringify, c as attr, d as ensure_array_like, e as escape_html } from "./renderer.js";
import { M as Modal } from "./Modal.js";
import { F as FormField, I as Input } from "./Input.js";
import { c as createAPIClient, g as getStoredToken } from "./client2.js";
import "./toast.js";
function CreateVMModal($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let {
      open = false,
      onSuccess,
      images = [],
      pools = [],
      networks = []
    } = $$props;
    createAPIClient({ token: getStoredToken() ?? void 0 });
    let step = 1;
    let name = "";
    let imageId = "";
    let poolId = "";
    let networkId = "";
    let vcpu = 2;
    let memoryMb = 2048;
    let submitting = false;
    let nameError = "";
    const nameRegex = /^[a-z0-9-]+$/;
    function validateName() {
      if (!name.trim()) {
        nameError = "Name is required";
        return false;
      }
      if (!nameRegex.test(name)) {
        nameError = "Name must contain only lowercase letters, numbers, and hyphens";
        return false;
      }
      if (name.startsWith("-") || name.endsWith("-")) {
        nameError = "Name cannot start or end with a hyphen";
        return false;
      }
      nameError = "";
      return true;
    }
    function canProceedToStep2() {
      return name.trim() !== "" && nameRegex.test(name) && !name.startsWith("-") && !name.endsWith("-") && imageId !== "" && poolId !== "" && networkId !== "";
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      {
        let footer = function($$renderer4) {
          {
            $$renderer4.push("<!--[0-->");
            $$renderer4.push(`<button type="button"${attr("disabled", submitting, true)} class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50 disabled:cursor-not-allowed">Cancel</button> <button type="button"${attr("disabled", !canProceedToStep2() || submitting, true)} class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors disabled:bg-primary/30 disabled:cursor-not-allowed">Next</button>`);
          }
          $$renderer4.push(`<!--]-->`);
        };
        Modal($$renderer3, {
          title: `Create VM - Step ${stringify(step)} of 3`,
          closeOnBackdrop: !submitting,
          width: "wide",
          get open() {
            return open;
          },
          set open($$value) {
            open = $$value;
            $$settled = false;
          },
          footer,
          children: ($$renderer4) => {
            {
              $$renderer4.push("<!--[0-->");
              $$renderer4.push(`<form id="create-vm-step1" class="space-y-5">`);
              {
                $$renderer4.push("<!--[-1-->");
              }
              $$renderer4.push(`<!--]--> `);
              FormField($$renderer4, {
                label: "Name",
                error: nameError,
                required: true,
                labelFor: "vm-name",
                children: ($$renderer5) => {
                  Input($$renderer5, {
                    id: "vm-name",
                    placeholder: "my-vm",
                    disabled: submitting,
                    onblur: validateName,
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
              FormField($$renderer4, {
                label: "Image",
                required: true,
                labelFor: "vm-image",
                children: ($$renderer5) => {
                  $$renderer5.select(
                    {
                      id: "vm-image",
                      value: imageId,
                      class: "h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm",
                      disabled: submitting
                    },
                    ($$renderer6) => {
                      $$renderer6.option({ value: "" }, ($$renderer7) => {
                        $$renderer7.push(`Select an image...`);
                      });
                      $$renderer6.push(`<!--[-->`);
                      const each_array = ensure_array_like(images);
                      for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
                        let img = each_array[$$index];
                        $$renderer6.option({ value: img.id }, ($$renderer7) => {
                          $$renderer7.push(`${escape_html(img.name)} (${escape_html(img.os_family)})`);
                        });
                      }
                      $$renderer6.push(`<!--]-->`);
                    }
                  );
                }
              });
              $$renderer4.push(`<!----> `);
              FormField($$renderer4, {
                label: "Storage Pool",
                required: true,
                labelFor: "vm-pool",
                children: ($$renderer5) => {
                  $$renderer5.select(
                    {
                      id: "vm-pool",
                      value: poolId,
                      class: "h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm",
                      disabled: submitting
                    },
                    ($$renderer6) => {
                      $$renderer6.option({ value: "" }, ($$renderer7) => {
                        $$renderer7.push(`Select a pool...`);
                      });
                      $$renderer6.push(`<!--[-->`);
                      const each_array_1 = ensure_array_like(pools);
                      for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
                        let pool = each_array_1[$$index_1];
                        $$renderer6.option({ value: pool.id }, ($$renderer7) => {
                          $$renderer7.push(`${escape_html(pool.name)}`);
                        });
                      }
                      $$renderer6.push(`<!--]-->`);
                    }
                  );
                }
              });
              $$renderer4.push(`<!----> `);
              FormField($$renderer4, {
                label: "Network",
                required: true,
                labelFor: "vm-network",
                children: ($$renderer5) => {
                  $$renderer5.select(
                    {
                      id: "vm-network",
                      value: networkId,
                      class: "h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm",
                      disabled: submitting
                    },
                    ($$renderer6) => {
                      $$renderer6.option({ value: "" }, ($$renderer7) => {
                        $$renderer7.push(`Select a network...`);
                      });
                      $$renderer6.push(`<!--[-->`);
                      const each_array_2 = ensure_array_like(networks);
                      for (let $$index_2 = 0, $$length = each_array_2.length; $$index_2 < $$length; $$index_2++) {
                        let net = each_array_2[$$index_2];
                        $$renderer6.option({ value: net.id }, ($$renderer7) => {
                          $$renderer7.push(`${escape_html(net.name)} (${escape_html(net.bridge_name)})`);
                        });
                      }
                      $$renderer6.push(`<!--]-->`);
                    }
                  );
                }
              });
              $$renderer4.push(`<!----> <div class="grid grid-cols-2 gap-4">`);
              FormField($$renderer4, {
                label: "vCPUs",
                labelFor: "vm-vcpu",
                children: ($$renderer5) => {
                  Input($$renderer5, {
                    id: "vm-vcpu",
                    type: "number",
                    min: 1,
                    max: 32,
                    disabled: submitting,
                    get value() {
                      return vcpu;
                    },
                    set value($$value) {
                      vcpu = $$value;
                      $$settled = false;
                    }
                  });
                }
              });
              $$renderer4.push(`<!----> `);
              FormField($$renderer4, {
                label: "Memory (MB)",
                labelFor: "vm-memory",
                children: ($$renderer5) => {
                  Input($$renderer5, {
                    id: "vm-memory",
                    type: "number",
                    min: 512,
                    step: 512,
                    disabled: submitting,
                    get value() {
                      return memoryMb;
                    },
                    set value($$value) {
                      memoryMb = $$value;
                      $$settled = false;
                    }
                  });
                }
              });
              $$renderer4.push(`<!----></div></form>`);
            }
            $$renderer4.push(`<!--]-->`);
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
  CreateVMModal as C
};
