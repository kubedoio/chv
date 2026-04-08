import { l as sanitize_props, m as spread_props, j as slot, d as bind_props, e as ensure_array_like, c as escape_html, h as stringify, b as attr, i as derived } from "../../../chunks/renderer.js";
import "@sveltejs/kit/internal";
import "../../../chunks/exports.js";
import "../../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../../chunks/root.js";
import "../../../chunks/client.js";
import { c as createAPIClient, g as getStoredToken } from "../../../chunks/client2.js";
import { t as toast } from "../../../chunks/toast.js";
import { S as StateBadge } from "../../../chunks/StateBadge.js";
import { S as StatsCard, a as Server, P as Play, b as Square } from "../../../chunks/StatsCard.js";
import { S as SkeletonRow } from "../../../chunks/SkeletonRow.js";
import { E as EmptyState } from "../../../chunks/EmptyState.js";
import { M as Modal } from "../../../chunks/Modal.js";
import { F as FormField, I as Input } from "../../../chunks/Input.js";
import { I as Icon } from "../../../chunks/Icon.js";
import { P as Plus } from "../../../chunks/plus.js";
function Circle_alert($$renderer, $$props) {
  const $$sanitized_props = sanitize_props($$props);
  /**
   * @license lucide-svelte v1.0.1 - ISC
   *
   * ISC License
   *
   * Copyright (c) 2026 Lucide Icons and Contributors
   *
   * Permission to use, copy, modify, and/or distribute this software for any
   * purpose with or without fee is hereby granted, provided that the above
   * copyright notice and this permission notice appear in all copies.
   *
   * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
   * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
   * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
   * ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
   * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
   * ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
   * OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
   *
   * ---
   *
   * The following Lucide icons are derived from the Feather project:
   *
   * airplay, alert-circle, alert-octagon, alert-triangle, aperture, arrow-down-circle, arrow-down-left, arrow-down-right, arrow-down, arrow-left-circle, arrow-left, arrow-right-circle, arrow-right, arrow-up-circle, arrow-up-left, arrow-up-right, arrow-up, at-sign, calendar, cast, check, chevron-down, chevron-left, chevron-right, chevron-up, chevrons-down, chevrons-left, chevrons-right, chevrons-up, circle, clipboard, clock, code, columns, command, compass, corner-down-left, corner-down-right, corner-left-down, corner-left-up, corner-right-down, corner-right-up, corner-up-left, corner-up-right, crosshair, database, divide-circle, divide-square, dollar-sign, download, external-link, feather, frown, hash, headphones, help-circle, info, italic, key, layout, life-buoy, link-2, link, loader, lock, log-in, log-out, maximize, meh, minimize, minimize-2, minus-circle, minus-square, minus, monitor, moon, more-horizontal, more-vertical, move, music, navigation-2, navigation, octagon, pause-circle, percent, plus-circle, plus-square, plus, power, radio, rss, search, server, share, shopping-bag, sidebar, smartphone, smile, square, table-2, tablet, target, terminal, trash-2, trash, triangle, tv, type, upload, x-circle, x-octagon, x-square, x, zoom-in, zoom-out
   *
   * The MIT License (MIT) (for the icons listed above)
   *
   * Copyright (c) 2013-present Cole Bemis
   *
   * Permission is hereby granted, free of charge, to any person obtaining a copy
   * of this software and associated documentation files (the "Software"), to deal
   * in the Software without restriction, including without limitation the rights
   * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
   * copies of the Software, and to permit persons to whom the Software is
   * furnished to do so, subject to the following conditions:
   *
   * The above copyright notice and this permission notice shall be included in all
   * copies or substantial portions of the Software.
   *
   * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
   * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
   * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
   * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
   * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
   * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
   * SOFTWARE.
   *
   */
  const iconNode = [
    ["circle", { "cx": "12", "cy": "12", "r": "10" }],
    ["line", { "x1": "12", "x2": "12", "y1": "8", "y2": "12" }],
    [
      "line",
      { "x1": "12", "x2": "12.01", "y1": "16", "y2": "16" }
    ]
  ];
  Icon($$renderer, spread_props([
    { name: "circle-alert" },
    $$sanitized_props,
    {
      /**
       * @component @name CircleAlert
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8Y2lyY2xlIGN4PSIxMiIgY3k9IjEyIiByPSIxMCIgLz4KICA8bGluZSB4MT0iMTIiIHgyPSIxMiIgeTE9IjgiIHkyPSIxMiIgLz4KICA8bGluZSB4MT0iMTIiIHgyPSIxMi4wMSIgeTE9IjE2IiB5Mj0iMTYiIC8+Cjwvc3ZnPgo=) - https://lucide.dev/icons/circle-alert
       * @see https://lucide.dev/guide/packages/lucide-svelte - Documentation
       *
       * @param {Object} props - Lucide icons props and any valid SVG attribute
       * @returns {FunctionalComponent} Svelte component
       *
       */
      iconNode,
      children: ($$renderer2) => {
        $$renderer2.push(`<!--[-->`);
        slot($$renderer2, $$props, "default", {});
        $$renderer2.push(`<!--]-->`);
      },
      $$slots: { default: true }
    }
  ]));
}
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
      return name.trim() !== "" && nameRegex.test(name) && !name.startsWith("-") && !name.endsWith("-") && imageId !== "";
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      {
        let footer = function($$renderer4) {
          {
            $$renderer4.push("<!--[0-->");
            $$renderer4.push(`<button type="button"${attr("disabled", submitting, true)} class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50 disabled:cursor-not-allowed">Cancel</button> <button type="button"${attr("disabled", !canProceedToStep2(), true)} class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors disabled:bg-primary/30 disabled:cursor-not-allowed">Next</button>`);
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
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    const token = getStoredToken();
    const client = createAPIClient({ token: token ?? void 0 });
    let items = [];
    let loading = true;
    let error = "";
    let createModalOpen = false;
    let images = [];
    let pools = [];
    let networks = [];
    const total = derived(() => items.length);
    const running = derived(() => items.filter((vm) => vm.actual_state === "running").length);
    const stopped = derived(() => items.filter((vm) => vm.actual_state === "stopped").length);
    const other = derived(() => items.filter((vm) => !["running", "stopped"].includes(vm.actual_state)).length);
    async function loadVMs() {
      loading = true;
      error = "";
      try {
        items = await client.listVMs();
      } catch (err) {
        error = err instanceof Error ? err.message : "Failed to load VMs";
        toast.error(error);
        items = [];
      }
      loading = false;
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      $$renderer3.push(`<div class="flex justify-between items-start mb-6"><div class="grid grid-cols-2 lg:grid-cols-4 gap-4 flex-1">`);
      StatsCard($$renderer3, { title: "Total VMs", value: total(), icon: Server });
      $$renderer3.push(`<!----> `);
      StatsCard($$renderer3, { title: "Running", value: running(), icon: Play, trend: "up" });
      $$renderer3.push(`<!----> `);
      StatsCard($$renderer3, { title: "Stopped", value: stopped(), icon: Square });
      $$renderer3.push(`<!----> `);
      StatsCard($$renderer3, { title: "Other", value: other(), icon: Circle_alert });
      $$renderer3.push(`<!----></div> <button class="ml-4 px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors flex items-center gap-2">`);
      Plus($$renderer3, { size: 16 });
      $$renderer3.push(`<!----> Create VM</button></div> `);
      if (error) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<div class="mb-4 border border-danger bg-red-50 px-4 py-3 text-danger">${escape_html(error)}</div>`);
      } else {
        $$renderer3.push("<!--[-1-->");
      }
      $$renderer3.push(`<!--]--> `);
      if (loading) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<section class="table-card"><div class="card-header px-4 py-3"><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Virtual Machines</div> <div class="mt-1 text-lg font-semibold">VM List</div></div> <table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Name</th><th class="border-b border-line px-4 py-3">State</th><th class="border-b border-line px-4 py-3">Image</th><th class="border-b border-line px-4 py-3">Pool</th><th class="border-b border-line px-4 py-3">Network</th><th class="border-b border-line px-4 py-3">vCPU</th><th class="border-b border-line px-4 py-3">Memory</th><th class="border-b border-line px-4 py-3">IP</th><th class="border-b border-line px-4 py-3">Last Error</th></tr></thead><tbody><!--[-->`);
        const each_array = ensure_array_like(Array(5));
        for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
          each_array[$$index];
          SkeletonRow($$renderer3, { columns: 9 });
        }
        $$renderer3.push(`<!--]--></tbody></table></section>`);
      } else if (items.length === 0) {
        $$renderer3.push("<!--[1-->");
        $$renderer3.push(`<section class="table-card"><div class="card-header px-4 py-3"><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Virtual Machines</div> <div class="mt-1 text-lg font-semibold">VM List</div></div> `);
        EmptyState($$renderer3, {
          icon: Server,
          title: "No VMs yet",
          description: "Create a virtual machine to get started",
          children: ($$renderer4) => {
            $$renderer4.push(`<button class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors flex items-center gap-2">`);
            Plus($$renderer4, { size: 16 });
            $$renderer4.push(`<!----> Create VM</button>`);
          }
        });
        $$renderer3.push(`<!----></section>`);
      } else {
        $$renderer3.push("<!--[-1-->");
        $$renderer3.push(`<section class="table-card"><div class="card-header px-4 py-3"><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Virtual Machines</div> <div class="mt-1 text-lg font-semibold">VM List</div></div> <table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Name</th><th class="border-b border-line px-4 py-3">State</th><th class="border-b border-line px-4 py-3">Image</th><th class="border-b border-line px-4 py-3">Pool</th><th class="border-b border-line px-4 py-3">Network</th><th class="border-b border-line px-4 py-3">vCPU</th><th class="border-b border-line px-4 py-3">Memory</th><th class="border-b border-line px-4 py-3">IP</th><th class="border-b border-line px-4 py-3">Last Error</th></tr></thead><tbody><!--[-->`);
        const each_array_1 = ensure_array_like(items);
        for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
          let item = each_array_1[$$index_1];
          $$renderer3.push(`<tr class="odd:bg-white even:bg-[#f8f8f8]"><td class="border-b border-line px-4 py-3"><a class="text-primary no-underline hover:underline"${attr("href", `/vms/${item.id}`)}>${escape_html(item.name)}</a></td><td class="border-b border-line px-4 py-3">`);
          if (item.desired_state === item.actual_state) {
            $$renderer3.push("<!--[0-->");
            StateBadge($$renderer3, { label: item.actual_state });
          } else {
            $$renderer3.push("<!--[-1-->");
            $$renderer3.push(`<div class="flex flex-col gap-1"><span class="text-xs text-muted">desired: ${escape_html(item.desired_state)}</span> `);
            StateBadge($$renderer3, { label: item.actual_state });
            $$renderer3.push(`<!----></div>`);
          }
          $$renderer3.push(`<!--]--></td><td class="border-b border-line px-4 py-3">${escape_html(item.image_id)}</td><td class="border-b border-line px-4 py-3">${escape_html(item.storage_pool_id)}</td><td class="border-b border-line px-4 py-3">${escape_html(item.network_id)}</td><td class="border-b border-line px-4 py-3 mono">${escape_html(item.vcpu)}</td><td class="border-b border-line px-4 py-3 mono">${escape_html(item.memory_mb)} MB</td><td class="border-b border-line px-4 py-3 mono">${escape_html(item.ip_address || "pending")}</td><td class="border-b border-line px-4 py-3 text-danger text-xs max-w-[200px] truncate"${attr("title", item.last_error)}>${escape_html(item.last_error || "—")}</td></tr>`);
        }
        $$renderer3.push(`<!--]--></tbody></table></section>`);
      }
      $$renderer3.push(`<!--]--> `);
      CreateVMModal($$renderer3, {
        images,
        pools,
        networks,
        onSuccess: loadVMs,
        get open() {
          return createModalOpen;
        },
        set open($$value) {
          createModalOpen = $$value;
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
