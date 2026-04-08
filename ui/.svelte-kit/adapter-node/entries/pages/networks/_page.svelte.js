import { l as sanitize_props, m as spread_props, j as slot, d as bind_props, b as attr, c as escape_html, e as ensure_array_like } from "../../../chunks/renderer.js";
import { c as createAPIClient, g as getStoredToken } from "../../../chunks/client2.js";
import { S as StateBadge } from "../../../chunks/StateBadge.js";
import { S as SkeletonRow } from "../../../chunks/SkeletonRow.js";
import { E as EmptyState } from "../../../chunks/EmptyState.js";
import { M as Modal } from "../../../chunks/Modal.js";
import { F as FormField, I as Input } from "../../../chunks/Input.js";
import { S as Select } from "../../../chunks/Select.js";
import "../../../chunks/toast.js";
import { I as Icon } from "../../../chunks/Icon.js";
import { P as Plus } from "../../../chunks/plus.js";
function Network($$renderer, $$props) {
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
    [
      "rect",
      { "x": "16", "y": "16", "width": "6", "height": "6", "rx": "1" }
    ],
    [
      "rect",
      { "x": "2", "y": "16", "width": "6", "height": "6", "rx": "1" }
    ],
    [
      "rect",
      { "x": "9", "y": "2", "width": "6", "height": "6", "rx": "1" }
    ],
    ["path", { "d": "M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3" }],
    ["path", { "d": "M12 12V8" }]
  ];
  Icon($$renderer, spread_props([
    { name: "network" },
    $$sanitized_props,
    {
      /**
       * @component @name Network
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cmVjdCB4PSIxNiIgeT0iMTYiIHdpZHRoPSI2IiBoZWlnaHQ9IjYiIHJ4PSIxIiAvPgogIDxyZWN0IHg9IjIiIHk9IjE2IiB3aWR0aD0iNiIgaGVpZ2h0PSI2IiByeD0iMSIgLz4KICA8cmVjdCB4PSI5IiB5PSIyIiB3aWR0aD0iNiIgaGVpZ2h0PSI2IiByeD0iMSIgLz4KICA8cGF0aCBkPSJNNSAxNnYtM2ExIDEgMCAwIDEgMS0xaDEyYTEgMSAwIDAgMSAxIDF2MyIgLz4KICA8cGF0aCBkPSJNMTIgMTJWOCIgLz4KPC9zdmc+Cg==) - https://lucide.dev/icons/network
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
function CreateNetworkModal($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { open = false, onSuccess } = $$props;
    createAPIClient({ token: getStoredToken() ?? void 0 });
    let name = "";
    let mode = "bridge";
    let bridgeName = "chvbr0";
    let cidr = "10.0.0.0/24";
    let gatewayIp = "10.0.0.1";
    let submitting = false;
    let nameError = "";
    let bridgeNameError = "";
    let cidrError = "";
    let gatewayIpError = "";
    const nameRegex = /^[a-z0-9-]+$/;
    const cidrRegex = /^(\d{1,3}\.){3}\d{1,3}\/\d{1,2}$/;
    const ipRegex = /^(\d{1,3}\.){3}\d{1,3}$/;
    const modeOptions = [{ value: "bridge", label: "bridge" }];
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
    function validateBridgeName() {
      if (!bridgeName.trim()) {
        bridgeNameError = "Bridge name is required";
        return false;
      }
      bridgeNameError = "";
      return true;
    }
    function validateCidr() {
      if (!cidr.trim()) {
        cidrError = "CIDR is required";
        return false;
      }
      if (!cidrRegex.test(cidr)) {
        cidrError = "CIDR must be in format x.x.x.x/x (e.g., 10.0.0.0/24)";
        return false;
      }
      const [ip, prefix] = cidr.split("/");
      const octets = ip.split(".").map(Number);
      if (octets.some((o) => o < 0 || o > 255)) {
        cidrError = "IP octets must be between 0 and 255";
        return false;
      }
      const prefixNum = Number(prefix);
      if (prefixNum < 0 || prefixNum > 32) {
        cidrError = "Prefix must be between 0 and 32";
        return false;
      }
      cidrError = "";
      return true;
    }
    function validateGateway() {
      if (!gatewayIp.trim()) {
        gatewayIpError = "Gateway IP is required";
        return false;
      }
      if (!ipRegex.test(gatewayIp)) {
        gatewayIpError = "Gateway must be a valid IP address (e.g., 10.0.0.1)";
        return false;
      }
      const octets = gatewayIp.split(".").map(Number);
      if (octets.some((o) => o < 0 || o > 255)) {
        gatewayIpError = "IP octets must be between 0 and 255";
        return false;
      }
      gatewayIpError = "";
      return true;
    }
    function isValid() {
      if (!name.trim() || !bridgeName.trim() || !cidr.trim() || !gatewayIp.trim()) {
        return false;
      }
      if (!nameRegex.test(name) || name.startsWith("-") || name.endsWith("-")) {
        return false;
      }
      if (!cidrRegex.test(cidr)) return false;
      if (!ipRegex.test(gatewayIp)) return false;
      const [ip] = cidr.split("/");
      if (ip.split(".").map(Number).some((o) => o < 0 || o > 255)) return false;
      if (gatewayIp.split(".").map(Number).some((o) => o < 0 || o > 255)) return false;
      return true;
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      {
        let footer = function($$renderer4) {
          $$renderer4.push(`<button type="button"${attr("disabled", submitting, true)} class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50 disabled:cursor-not-allowed">Cancel</button> <button type="submit" form="create-network-form"${attr("disabled", !isValid() || submitting, true)} class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors disabled:bg-primary/30 disabled:cursor-not-allowed flex items-center gap-2">`);
          {
            $$renderer4.push("<!--[-1-->");
          }
          $$renderer4.push(`<!--]--> ${escape_html("Create Network")}</button>`);
        };
        Modal($$renderer3, {
          title: "Create Network",
          closeOnBackdrop: !submitting,
          get open() {
            return open;
          },
          set open($$value) {
            open = $$value;
            $$settled = false;
          },
          footer,
          children: ($$renderer4) => {
            $$renderer4.push(`<form id="create-network-form" class="space-y-5">`);
            {
              $$renderer4.push("<!--[-1-->");
            }
            $$renderer4.push(`<!--]--> `);
            FormField($$renderer4, {
              label: "Name",
              error: nameError,
              required: true,
              labelFor: "network-name",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "network-name",
                  placeholder: "my-network",
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
              label: "Mode",
              helper: "Only 'bridge' mode is supported in MVP-1",
              labelFor: "network-mode",
              children: ($$renderer5) => {
                Select($$renderer5, {
                  id: "network-mode",
                  options: modeOptions,
                  disabled: true,
                  get value() {
                    return mode;
                  },
                  set value($$value) {
                    mode = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "Bridge Name",
              error: bridgeNameError,
              required: true,
              labelFor: "bridge-name",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "bridge-name",
                  placeholder: "chvbr0",
                  disabled: submitting,
                  onblur: validateBridgeName,
                  get value() {
                    return bridgeName;
                  },
                  set value($$value) {
                    bridgeName = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "CIDR",
              error: cidrError,
              required: true,
              labelFor: "network-cidr",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "network-cidr",
                  placeholder: "10.0.0.0/24",
                  disabled: submitting,
                  onblur: validateCidr,
                  get value() {
                    return cidr;
                  },
                  set value($$value) {
                    cidr = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "Gateway IP",
              error: gatewayIpError,
              required: true,
              labelFor: "gateway-ip",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "gateway-ip",
                  placeholder: "10.0.0.1",
                  disabled: submitting,
                  onblur: validateGateway,
                  get value() {
                    return gatewayIp;
                  },
                  set value($$value) {
                    gatewayIp = $$value;
                    $$settled = false;
                  }
                });
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
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    const client = createAPIClient({ token: getStoredToken() ?? void 0 });
    let items = [];
    let loading = true;
    let createModalOpen = false;
    async function loadNetworks() {
      loading = true;
      try {
        items = await client.listNetworks();
      } catch {
        items = [];
      } finally {
        loading = false;
      }
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      $$renderer3.push(`<section class="table-card"><div class="card-header px-4 py-3 flex items-center justify-between"><div><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Networks</div> <div class="mt-1 text-lg font-semibold">Bridge-backed Networks</div></div> <button class="px-4 py-2 rounded bg-primary text-white font-medium text-sm hover:bg-primary/90 transition-colors flex items-center gap-2"><svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"></path><path d="M12 5v14"></path></svg> Create</button></div> `);
      if (loading) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Name</th><th class="border-b border-line px-4 py-3">Bridge</th><th class="border-b border-line px-4 py-3">CIDR</th><th class="border-b border-line px-4 py-3">Gateway</th><th class="border-b border-line px-4 py-3">Managed</th><th class="border-b border-line px-4 py-3">Status</th></tr></thead><tbody><!--[-->`);
        const each_array = ensure_array_like(Array(5));
        for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
          each_array[$$index];
          SkeletonRow($$renderer3, { columns: 6 });
        }
        $$renderer3.push(`<!--]--></tbody></table>`);
      } else if (items.length === 0) {
        $$renderer3.push("<!--[1-->");
        EmptyState($$renderer3, {
          icon: Network,
          title: "No networks yet",
          description: "Create a network to connect your VMs",
          children: ($$renderer4) => {
            $$renderer4.push(`<button class="px-4 py-2 rounded bg-primary text-white font-medium text-sm hover:bg-primary/90 transition-colors flex items-center gap-2">`);
            Plus($$renderer4, { size: 16 });
            $$renderer4.push(`<!----> Create Network</button>`);
          }
        });
      } else {
        $$renderer3.push("<!--[-1-->");
        $$renderer3.push(`<table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Name</th><th class="border-b border-line px-4 py-3">Bridge</th><th class="border-b border-line px-4 py-3">CIDR</th><th class="border-b border-line px-4 py-3">Gateway</th><th class="border-b border-line px-4 py-3">Managed</th><th class="border-b border-line px-4 py-3">Status</th></tr></thead><tbody><!--[-->`);
        const each_array_1 = ensure_array_like(items);
        for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
          let item = each_array_1[$$index_1];
          $$renderer3.push(`<tr class="odd:bg-white even:bg-[#f8f8f8] hover:bg-hover transition-colors"><td class="border-b border-line px-4 py-3 font-medium">${escape_html(item.name)}</td><td class="border-b border-line px-4 py-3 mono">${escape_html(item.bridge_name)}</td><td class="border-b border-line px-4 py-3 mono">${escape_html(item.cidr)}</td><td class="border-b border-line px-4 py-3 mono">${escape_html(item.gateway_ip)}</td><td class="border-b border-line px-4 py-3">${escape_html(item.is_system_managed ? "system" : "manual")}</td><td class="border-b border-line px-4 py-3">`);
          StateBadge($$renderer3, { label: item.status });
          $$renderer3.push(`<!----></td></tr>`);
        }
        $$renderer3.push(`<!--]--></tbody></table>`);
      }
      $$renderer3.push(`<!--]--></section> `);
      CreateNetworkModal($$renderer3, {
        onSuccess: loadNetworks,
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
