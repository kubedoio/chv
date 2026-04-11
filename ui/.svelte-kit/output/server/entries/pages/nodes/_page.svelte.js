import { s as sanitize_props, a as spread_props, b as slot, k as bind_props, c as attr, e as escape_html, n as head, d as ensure_array_like, i as stringify, g as attr_class } from "../../../chunks/root.js";
import "@sveltejs/kit/internal";
import "../../../chunks/exports.js";
import "../../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../../chunks/client.js";
import { g as getStoredToken, c as createAPIClient, t as toast } from "../../../chunks/client2.js";
import { M as Modal } from "../../../chunks/Modal.js";
import { F as FormField, I as Input } from "../../../chunks/Input.js";
import { P as Plus } from "../../../chunks/plus.js";
import { S as Server } from "../../../chunks/server.js";
import { I as Icon } from "../../../chunks/Icon.js";
import { C as Copy } from "../../../chunks/copy.js";
import { D as DataTable } from "../../../chunks/DataTable.js";
import { C as Circle } from "../../../chunks/circle.js";
import { a as Cpu, C as Circle_check_big } from "../../../chunks/cpu.js";
import { H as Hard_drive } from "../../../chunks/hard-drive.js";
import { N as Network } from "../../../chunks/network.js";
import { A as Activity } from "../../../chunks/activity.js";
import { S as Settings } from "../../../chunks/settings.js";
import { T as Trash_2 } from "../../../chunks/trash-2.js";
function Arrow_right($$renderer, $$props) {
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
    ["path", { "d": "M5 12h14" }],
    ["path", { "d": "m12 5 7 7-7 7" }]
  ];
  Icon($$renderer, spread_props([
    { name: "arrow-right" },
    $$sanitized_props,
    {
      /**
       * @component @name ArrowRight
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNNSAxMmgxNCIgLz4KICA8cGF0aCBkPSJtMTIgNSA3IDctNyA3IiAvPgo8L3N2Zz4K) - https://lucide.dev/icons/arrow-right
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
function Check($$renderer, $$props) {
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
  const iconNode = [["path", { "d": "M20 6 9 17l-5-5" }]];
  Icon($$renderer, spread_props([
    { name: "check" },
    $$sanitized_props,
    {
      /**
       * @component @name Check
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNMjAgNiA5IDE3bC01LTUiIC8+Cjwvc3ZnPgo=) - https://lucide.dev/icons/check
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
function External_link($$renderer, $$props) {
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
    ["path", { "d": "M15 3h6v6" }],
    ["path", { "d": "M10 14 21 3" }],
    [
      "path",
      {
        "d": "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"
      }
    ]
  ];
  Icon($$renderer, spread_props([
    { name: "external-link" },
    $$sanitized_props,
    {
      /**
       * @component @name ExternalLink
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNMTUgM2g2djYiIC8+CiAgPHBhdGggZD0iTTEwIDE0IDIxIDMiIC8+CiAgPHBhdGggZD0iTTE4IDEzdjZhMiAyIDAgMCAxLTIgMkg1YTIgMiAwIDAgMS0yLTJWOGEyIDIgMCAwIDEgMi0yaDYiIC8+Cjwvc3ZnPgo=) - https://lucide.dev/icons/external-link
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
function AddNodeModal($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { open = false, onClose, onSubmit } = $$props;
    let name = "";
    let hostname = "";
    let ipAddress = "";
    let agentUrl = "";
    let loading = false;
    let error = null;
    let result = null;
    let copied = false;
    function resetForm() {
      name = "";
      hostname = "";
      ipAddress = "";
      agentUrl = "";
      error = null;
      result = null;
      copied = false;
    }
    function handleClose() {
      resetForm();
      onClose();
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      Modal($$renderer3, {
        open,
        onClose: handleClose,
        title: result ? "Node Created Successfully" : "Add New Node",
        description: result ? "Save the agent token - it will only be shown once" : "Register a new remote hypervisor node",
        width: "wide",
        children: ($$renderer4) => {
          if (result) {
            $$renderer4.push("<!--[0-->");
            $$renderer4.push(`<div class="space-y-6"><div class="bg-green-50 border border-green-200 rounded-lg p-4"><div class="flex items-start gap-3"><div class="w-10 h-10 rounded-full bg-green-100 flex items-center justify-center flex-shrink-0">`);
            Server($$renderer4, { class: "text-green-600", size: 20 });
            $$renderer4.push(`<!----></div> <div><h4 class="font-medium text-green-900">${escape_html(result.name)}</h4> <p class="text-sm text-green-700 mt-1">${escape_html(result.hostname)} (${escape_html(result.ip_address)})</p></div></div></div> <div class="space-y-2"><label class="block text-sm font-medium text-slate-700">Agent Token <span class="text-red-500">*</span> <span class="text-xs font-normal text-slate-500 ml-2">Copy and save this token - it will not be shown again</span></label> <div class="flex gap-2"><code class="flex-1 bg-slate-900 text-green-400 px-4 py-3 rounded-lg text-sm font-mono break-all">${escape_html(result.agent_token)}</code> <button type="button" class="px-4 py-2 bg-white border border-slate-200 rounded-lg hover:bg-slate-50 transition-colors flex items-center gap-2" aria-label="Copy token to clipboard">`);
            if (copied) {
              $$renderer4.push("<!--[0-->");
              Check($$renderer4, { size: 18, class: "text-green-600" });
              $$renderer4.push(`<!----> <span class="text-sm text-green-600">Copied!</span>`);
            } else {
              $$renderer4.push("<!--[-1-->");
              Copy($$renderer4, { size: 18, class: "text-slate-600" });
              $$renderer4.push(`<!----> <span class="text-sm text-slate-600">Copy</span>`);
            }
            $$renderer4.push(`<!--]--></button></div></div> `);
            if (result.agent_url) {
              $$renderer4.push("<!--[0-->");
              $$renderer4.push(`<div class="space-y-2"><label class="block text-sm font-medium text-slate-700">Agent URL</label> <div class="flex gap-2"><code class="flex-1 bg-slate-100 text-slate-700 px-4 py-3 rounded-lg text-sm font-mono">${escape_html(result.agent_url)}</code></div></div>`);
            } else {
              $$renderer4.push("<!--[-1-->");
            }
            $$renderer4.push(`<!--]--> <div class="bg-amber-50 border border-amber-200 rounded-lg p-4"><h5 class="font-medium text-amber-900 mb-2 flex items-center gap-2">`);
            External_link($$renderer4, { size: 16 });
            $$renderer4.push(`<!----> Next Steps</h5> <ol class="text-sm text-amber-800 space-y-2 ml-4 list-decimal"><li>Copy and save the agent token above securely</li> <li>Install the CHV agent on <strong>${escape_html(result.hostname)}</strong></li> <li>Configure the agent with the node ID and token</li> <li>The agent will automatically register and connect</li></ol></div> <div class="flex justify-end gap-3 pt-4 border-t border-slate-200"><button type="button" class="px-4 py-2 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium">Done</button></div></div>`);
          } else {
            $$renderer4.push("<!--[-1-->");
            $$renderer4.push(`<form class="space-y-5">`);
            if (error) {
              $$renderer4.push("<!--[0-->");
              $$renderer4.push(`<div class="bg-red-50 border border-red-200 rounded-lg p-4"><p class="text-sm text-red-700">${escape_html(error)}</p></div>`);
            } else {
              $$renderer4.push("<!--[-1-->");
            }
            $$renderer4.push(`<!--]--> `);
            FormField($$renderer4, {
              label: "Node Name",
              required: true,
              error: error && !name ? "Name is required" : void 0,
              children: ($$renderer5) => {
                Input($$renderer5, {
                  type: "text",
                  placeholder: "e.g., hypervisor-02",
                  disabled: loading,
                  autocomplete: "off",
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
              label: "Hostname",
              required: true,
              error: error && !hostname ? "Hostname is required" : void 0,
              children: ($$renderer5) => {
                Input($$renderer5, {
                  type: "text",
                  placeholder: "e.g., hv02.example.com",
                  disabled: loading,
                  autocomplete: "off",
                  get value() {
                    return hostname;
                  },
                  set value($$value) {
                    hostname = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "IP Address",
              required: true,
              error: error && !ipAddress ? "IP address is required" : void 0,
              children: ($$renderer5) => {
                Input($$renderer5, {
                  type: "text",
                  placeholder: "e.g., 10.0.1.10",
                  disabled: loading,
                  autocomplete: "off",
                  get value() {
                    return ipAddress;
                  },
                  set value($$value) {
                    ipAddress = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "Agent URL",
              helper: "Optional URL where the agent will be accessible",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  type: "url",
                  placeholder: "e.g., http://10.0.1.10:9090",
                  disabled: loading,
                  autocomplete: "off",
                  get value() {
                    return agentUrl;
                  },
                  set value($$value) {
                    agentUrl = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> <div class="flex justify-end gap-3 pt-4 border-t border-slate-200"><button type="button" class="px-4 py-2 text-slate-700 hover:bg-slate-100 rounded-lg transition-colors font-medium"${attr("disabled", loading, true)}>Cancel</button> <button type="submit" class="px-4 py-2 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium flex items-center gap-2 disabled:opacity-50"${attr("disabled", loading, true)}>`);
            {
              $$renderer4.push("<!--[-1-->");
              Plus($$renderer4, { size: 18 });
              $$renderer4.push(`<!----> <span>Add Node</span>`);
            }
            $$renderer4.push(`<!--]--></button></div></form>`);
          }
          $$renderer4.push(`<!--]-->`);
        },
        $$slots: { default: true }
      });
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
    let nodes = [];
    let loading = true;
    let showAddModal = false;
    async function loadNodes() {
      loading = true;
      try {
        const data = await client.listNodes();
        nodes = data;
      } catch (e) {
        console.error("Failed to load nodes:", e);
        toast.error("Failed to load nodes");
      } finally {
        loading = false;
      }
    }
    async function handleCreateNode(data) {
      const result = await client.createNode(data);
      await loadNodes();
      return result;
    }
    function getStatusColor(status) {
      switch (status) {
        case "online":
          return "text-green-500";
        case "offline":
          return "text-red-500";
        case "maintenance":
          return "text-orange-500";
        case "error":
          return "text-red-600";
        default:
          return "text-slate-400";
      }
    }
    function getStatusBg(status) {
      switch (status) {
        case "online":
          return "bg-green-100";
        case "offline":
          return "bg-red-100";
        case "maintenance":
          return "bg-orange-100";
        case "error":
          return "bg-red-100";
        default:
          return "bg-slate-100";
      }
    }
    const columns = [
      { key: "name", title: "Name", sortable: true },
      { key: "hostname", title: "Hostname", sortable: true },
      { key: "ip_address", title: "IP Address", sortable: true },
      { key: "status", title: "Status", sortable: true },
      { key: "resources", title: "Resources" },
      { key: "last_seen", title: "Last Seen" }
    ];
    function formatLastSeen(lastSeenAt) {
      if (!lastSeenAt) return "Never";
      const date = new Date(lastSeenAt);
      const now = /* @__PURE__ */ new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffSec = Math.floor(diffMs / 1e3);
      const diffMin = Math.floor(diffSec / 60);
      const diffHour = Math.floor(diffMin / 60);
      const diffDay = Math.floor(diffHour / 24);
      if (diffSec < 60) return "Just now";
      if (diffMin < 60) return `${diffMin}m ago`;
      if (diffHour < 24) return `${diffHour}h ago`;
      if (diffDay < 7) return `${diffDay}d ago`;
      return date.toLocaleDateString();
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      head("1urd2h6", $$renderer3, ($$renderer4) => {
        $$renderer4.title(($$renderer5) => {
          $$renderer5.push(`<title>Nodes | CHV</title>`);
        });
      });
      $$renderer3.push(`<div class="space-y-6"><div class="flex items-center justify-between"><div><h1 class="text-2xl font-bold text-slate-900">Nodes</h1> <p class="text-sm text-slate-500 mt-1">Manage compute nodes in your datacenter</p></div> <button type="button" class="inline-flex items-center gap-2 px-4 py-2 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium">`);
      Plus($$renderer3, { size: 18 });
      $$renderer3.push(`<!----> Add Node</button></div> `);
      if (loading) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<div class="flex items-center justify-center h-64"><div class="flex items-center gap-3 text-slate-500"><div class="w-5 h-5 border-2 border-slate-300 border-t-orange-500 rounded-full animate-spin"></div> <span>Loading nodes...</span></div></div>`);
      } else if (nodes.length === 0) {
        $$renderer3.push("<!--[1-->");
        $$renderer3.push(`<div class="text-center py-16 bg-white rounded-lg border border-slate-200">`);
        Server($$renderer3, { size: 48, class: "mx-auto mb-4 text-slate-300" });
        $$renderer3.push(`<!----> <h3 class="text-lg font-medium text-slate-900">No nodes configured</h3> <p class="text-sm text-slate-500 mt-1 mb-6">Add your first compute node to get started</p> <button type="button" class="inline-flex items-center gap-2 px-4 py-2 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium">`);
        Plus($$renderer3, { size: 18 });
        $$renderer3.push(`<!----> Add Node</button></div>`);
      } else {
        $$renderer3.push("<!--[-1-->");
        $$renderer3.push(`<div class="grid gap-4 lg:grid-cols-2 xl:grid-cols-3"><!--[-->`);
        const each_array = ensure_array_like(nodes);
        for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
          let node = each_array[$$index];
          $$renderer3.push(`<div class="group bg-white rounded-lg shadow-sm border border-slate-200 hover:shadow-md hover:border-orange-200 transition-all"><a${attr("href", `/nodes/${stringify(node.id)}`)} class="block p-5"><div class="flex items-start justify-between"><div class="flex items-center gap-3"><div class="w-10 h-10 rounded-lg bg-gradient-to-br from-orange-500 to-red-600 flex items-center justify-center">`);
          Server($$renderer3, { class: "text-white", size: 20 });
          $$renderer3.push(`<!----></div> <div><h3 class="font-semibold text-slate-900 group-hover:text-orange-600 transition-colors">${escape_html(node.name)}</h3> <div class="flex items-center gap-2 mt-0.5"><span${attr_class(`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium ${stringify(getStatusBg(node.status))}`)}>`);
          Circle($$renderer3, {
            size: 6,
            class: getStatusColor(node.status),
            fill: "currentColor"
          });
          $$renderer3.push(`<!----> <span class="capitalize text-slate-700">${escape_html(node.status)}</span></span> `);
          if (node.is_local) {
            $$renderer3.push("<!--[0-->");
            $$renderer3.push(`<span class="text-xs px-1.5 py-0.5 bg-slate-100 text-slate-600 rounded">Local</span>`);
          } else {
            $$renderer3.push("<!--[-1-->");
          }
          $$renderer3.push(`<!--]--></div></div></div> `);
          Arrow_right($$renderer3, {
            size: 18,
            class: "text-slate-400 group-hover:text-orange-500 transition-colors"
          });
          $$renderer3.push(`<!----></div> <div class="mt-4 pt-4 border-t border-slate-100"><div class="grid grid-cols-2 gap-4"><div class="flex items-center gap-2">`);
          Cpu($$renderer3, { size: 14, class: "text-slate-400" });
          $$renderer3.push(`<!----> <span class="text-sm text-slate-600">${escape_html(node.resources?.vms ?? 0)} VMs</span></div> <div class="flex items-center gap-2">`);
          Circle_check_big($$renderer3, { size: 14, class: "text-slate-400" });
          $$renderer3.push(`<!----> <span class="text-sm text-slate-600">${escape_html(node.resources?.images ?? 0)} Images</span></div> <div class="flex items-center gap-2">`);
          Hard_drive($$renderer3, { size: 14, class: "text-slate-400" });
          $$renderer3.push(`<!----> <span class="text-sm text-slate-600">${escape_html(node.resources?.storage_pools ?? 0)} Pools</span></div> <div class="flex items-center gap-2">`);
          Network($$renderer3, { size: 14, class: "text-slate-400" });
          $$renderer3.push(`<!----> <span class="text-sm text-slate-600">${escape_html(node.resources?.networks ?? 0)} Networks</span></div></div></div> <div class="mt-4 flex items-center justify-between text-xs text-slate-500"><span>${escape_html(node.hostname)}</span> <span>${escape_html(node.ip_address)}</span></div> `);
          if (!node.is_local) {
            $$renderer3.push("<!--[0-->");
            $$renderer3.push(`<div class="mt-3 pt-3 border-t border-slate-100 flex items-center justify-between text-xs"><span class="text-slate-500 flex items-center gap-1">`);
            Activity($$renderer3, { size: 12 });
            $$renderer3.push(`<!----> Last seen: ${escape_html(formatLastSeen(node.last_seen_at))}</span></div>`);
          } else {
            $$renderer3.push("<!--[-1-->");
          }
          $$renderer3.push(`<!--]--></a> `);
          if (!node.is_local) {
            $$renderer3.push("<!--[0-->");
            $$renderer3.push(`<div class="px-5 pb-4 flex items-center gap-2"><button type="button" class="flex-1 inline-flex items-center justify-center gap-1.5 px-3 py-1.5 text-sm font-medium text-slate-700 bg-slate-50 border border-slate-200 rounded-md hover:bg-slate-100 transition-colors"${attr("title", node.status === "maintenance" ? "Bring node online" : "Set to maintenance mode")}>`);
            Settings($$renderer3, { size: 14 });
            $$renderer3.push(`<!----> ${escape_html(node.status === "maintenance" ? "Online" : "Maintenance")}</button> <button type="button" class="inline-flex items-center justify-center gap-1.5 px-3 py-1.5 text-sm font-medium text-red-600 bg-red-50 border border-red-200 rounded-md hover:bg-red-100 transition-colors" title="Delete node">`);
            Trash_2($$renderer3, { size: 14 });
            $$renderer3.push(`<!----></button></div>`);
          } else {
            $$renderer3.push("<!--[-1-->");
          }
          $$renderer3.push(`<!--]--></div>`);
        }
        $$renderer3.push(`<!--]--></div> <div class="hidden">`);
        {
          let children = function($$renderer4, row) {
            $$renderer4.push(`<div class="flex items-center gap-2"><a${attr("href", `/nodes/${stringify(row.id)}`)} class="text-orange-600 hover:text-orange-700 font-medium text-sm">View</a></div>`);
          };
          DataTable($$renderer3, {
            data: nodes,
            columns,
            rowId: (node) => node.id,
            loading,
            emptyTitle: "No nodes found",
            emptyDescription: "Add a new node to get started",
            children
          });
        }
        $$renderer3.push(`<!----></div>`);
      }
      $$renderer3.push(`<!--]--></div> `);
      AddNodeModal($$renderer3, {
        onClose: () => showAddModal = false,
        onSubmit: handleCreateNode,
        get open() {
          return showAddModal;
        },
        set open($$value) {
          showAddModal = $$value;
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
