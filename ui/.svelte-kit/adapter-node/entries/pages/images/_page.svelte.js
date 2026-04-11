import { l as sanitize_props, m as spread_props, j as slot, a as attr_class, s as stringify, c as escape_html, h as derived, d as bind_props, b as attr, e as ensure_array_like } from "../../../chunks/renderer.js";
import { o as onDestroy } from "../../../chunks/index-server.js";
import { c as createAPIClient, g as getStoredToken } from "../../../chunks/client2.js";
import { S as StateBadge } from "../../../chunks/StateBadge.js";
import { A as Activity } from "../../../chunks/activity.js";
import { C as Circle_alert } from "../../../chunks/circle-alert.js";
import { C as Clock, R as Refresh_cw } from "../../../chunks/refresh-cw.js";
import { I as Icon } from "../../../chunks/Icon.js";
import { S as SkeletonRow } from "../../../chunks/SkeletonRow.js";
import { E as EmptyState } from "../../../chunks/EmptyState.js";
import { M as Modal } from "../../../chunks/Modal.js";
import { F as FormField, I as Input } from "../../../chunks/Input.js";
import "../../../chunks/toast.js";
import { P as Plus } from "../../../chunks/plus.js";
function Circle_check_big($$renderer, $$props) {
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
    ["path", { "d": "M21.801 10A10 10 0 1 1 17 3.335" }],
    ["path", { "d": "m9 11 3 3L22 4" }]
  ];
  Icon($$renderer, spread_props([
    { name: "circle-check-big" },
    $$sanitized_props,
    {
      /**
       * @component @name CircleCheckBig
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNMjEuODAxIDEwQTEwIDEwIDAgMSAxIDE3IDMuMzM1IiAvPgogIDxwYXRoIGQ9Im05IDExIDMgM0wyMiA0IiAvPgo8L3N2Zz4K) - https://lucide.dev/icons/circle-check-big
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
function Image($$renderer, $$props) {
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
      {
        "width": "18",
        "height": "18",
        "x": "3",
        "y": "3",
        "rx": "2",
        "ry": "2"
      }
    ],
    ["circle", { "cx": "9", "cy": "9", "r": "2" }],
    ["path", { "d": "m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21" }]
  ];
  Icon($$renderer, spread_props([
    { name: "image" },
    $$sanitized_props,
    {
      /**
       * @component @name Image
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cmVjdCB3aWR0aD0iMTgiIGhlaWdodD0iMTgiIHg9IjMiIHk9IjMiIHJ4PSIyIiByeT0iMiIgLz4KICA8Y2lyY2xlIGN4PSI5IiBjeT0iOSIgcj0iMiIgLz4KICA8cGF0aCBkPSJtMjEgMTUtMy4wODYtMy4wODZhMiAyIDAgMCAwLTIuODI4IDBMNiAyMSIgLz4KPC9zdmc+Cg==) - https://lucide.dev/icons/image
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
function Loader_circle($$renderer, $$props) {
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
  const iconNode = [["path", { "d": "M21 12a9 9 0 1 1-6.219-8.56" }]];
  Icon($$renderer, spread_props([
    { name: "loader-circle" },
    $$sanitized_props,
    {
      /**
       * @component @name LoaderCircle
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNMjEgMTJhOSA5IDAgMSAxLTYuMjE5LTguNTYiIC8+Cjwvc3ZnPgo=) - https://lucide.dev/icons/loader-circle
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
function StatusIndicator($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { status, size = "md", showLabel = true } = $$props;
    const sizeClasses = { sm: "w-2 h-2", md: "w-3 h-3", lg: "w-4 h-4" };
    const config = derived(() => {
      switch (status.toLowerCase()) {
        case "running":
        case "ready":
        case "active":
        case "completed":
        case "success":
          return {
            icon: Circle_check_big,
            color: "text-green-600",
            bg: "bg-green-100",
            pulse: false,
            label: status
          };
        case "starting":
        case "stopping":
        case "provisioning":
        case "downloading":
        case "importing":
        case "pending":
          return {
            icon: Loader_circle,
            color: "text-blue-600",
            bg: "bg-blue-100",
            pulse: true,
            label: status
          };
        case "stopped":
        case "inactive":
          return {
            icon: Clock,
            color: "text-gray-500",
            bg: "bg-gray-100",
            pulse: false,
            label: status
          };
        case "error":
        case "failed":
          return {
            icon: Circle_alert,
            color: "text-red-600",
            bg: "bg-red-100",
            pulse: false,
            label: status
          };
        default:
          return {
            icon: Activity,
            color: "text-gray-500",
            bg: "bg-gray-100",
            pulse: false,
            label: status
          };
      }
    });
    $$renderer2.push(`<div class="flex items-center gap-2"><div class="relative"><div${attr_class(`${stringify(sizeClasses[size])} rounded-full ${stringify(config().bg)} flex items-center justify-center`)}>`);
    if (config().icon) {
      $$renderer2.push("<!--[-->");
      config().icon($$renderer2, {
        class: `${stringify(sizeClasses[size])} ${stringify(config().color)} ${stringify(config().pulse ? "animate-spin" : "")}`
      });
      $$renderer2.push("<!--]-->");
    } else {
      $$renderer2.push("<!--[!-->");
      $$renderer2.push("<!--]-->");
    }
    $$renderer2.push(`</div> `);
    if (config().pulse) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div${attr_class(`absolute inset-0 rounded-full ${stringify(config().bg)} animate-ping opacity-75`)}></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div> `);
    if (showLabel) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<span${attr_class(`text-sm font-medium capitalize ${stringify(config().color)}`)}>${escape_html(config().label)}</span>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div>`);
  });
}
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
            $$renderer4.push(`<form class="space-y-4">`);
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
            $$renderer4.push(`<!----> `);
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
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "Format",
              children: ($$renderer5) => {
                Input($$renderer5, { value: "qcow2", disabled: true });
                $$renderer5.push(`<!----> <p class="text-xs text-muted mt-1">Only qcow2 is supported in MVP-1</p>`);
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
    let importModalOpen = false;
    const hasImportingImages = derived(() => () => {
      return items.some((img) => img.status === "importing");
    });
    async function loadImages() {
      loading = true;
      try {
        items = await client.listImages();
      } catch {
        items = [];
      } finally {
        loading = false;
      }
    }
    onDestroy(() => {
    });
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      $$renderer3.push(`<section class="table-card"><div class="card-header px-4 py-3 flex justify-between items-center"><div><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Images</div> <div class="mt-1 text-lg font-semibold">Cloud Images</div></div> <div class="flex items-center gap-2"><button class="p-2 hover:bg-chrome rounded" title="Refresh">`);
      Refresh_cw($$renderer3, {
        size: 16,
        class: hasImportingImages()() ? "animate-spin" : ""
      });
      $$renderer3.push(`<!----></button> <button class="button-primary flex items-center gap-2 px-4 py-2 rounded text-sm">`);
      Plus($$renderer3, { size: 16 });
      $$renderer3.push(`<!----> Import</button></div></div> `);
      if (loading) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Name</th><th class="border-b border-line px-4 py-3">Format</th><th class="border-b border-line px-4 py-3">Source</th><th class="border-b border-line px-4 py-3">Cloud-init</th><th class="border-b border-line px-4 py-3">Path</th><th class="border-b border-line px-4 py-3">Status</th></tr></thead><tbody><!--[-->`);
        const each_array = ensure_array_like(Array(5));
        for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
          each_array[$$index];
          SkeletonRow($$renderer3, { columns: 6 });
        }
        $$renderer3.push(`<!--]--></tbody></table>`);
      } else if (items.length === 0) {
        $$renderer3.push("<!--[1-->");
        EmptyState($$renderer3, {
          icon: Image,
          title: "No images yet",
          description: "Import cloud images to create VMs from",
          children: ($$renderer4) => {
            $$renderer4.push(`<button class="button-primary px-4 py-2 rounded text-sm">Import Image</button>`);
          }
        });
      } else {
        $$renderer3.push("<!--[-1-->");
        $$renderer3.push(`<table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Name</th><th class="border-b border-line px-4 py-3">Format</th><th class="border-b border-line px-4 py-3">Source</th><th class="border-b border-line px-4 py-3">Cloud-init</th><th class="border-b border-line px-4 py-3">Path</th><th class="border-b border-line px-4 py-3">Status</th></tr></thead><tbody><!--[-->`);
        const each_array_1 = ensure_array_like(items);
        for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
          let item = each_array_1[$$index_1];
          $$renderer3.push(`<tr class="odd:bg-white even:bg-[#f8f8f8]"><td class="border-b border-line px-4 py-3 font-medium">${escape_html(item.name)}</td><td class="border-b border-line px-4 py-3">${escape_html(item.format)}</td><td class="border-b border-line px-4 py-3 mono truncate max-w-[200px]"${attr("title", item.source_url)}>${escape_html(item.source_url)}</td><td class="border-b border-line px-4 py-3">${escape_html(item.cloud_init_supported ? "yes" : "no")}</td><td class="border-b border-line px-4 py-3 mono truncate max-w-[200px]"${attr("title", item.local_path)}>${escape_html(item.local_path)}</td><td class="border-b border-line px-4 py-3">`);
          if (item.status === "importing") {
            $$renderer3.push("<!--[0-->");
            StatusIndicator($$renderer3, { status: item.status, size: "sm" });
          } else {
            $$renderer3.push("<!--[-1-->");
            StateBadge($$renderer3, { label: item.status });
          }
          $$renderer3.push(`<!--]--></td></tr>`);
        }
        $$renderer3.push(`<!--]--></tbody></table>`);
      }
      $$renderer3.push(`<!--]--></section> `);
      ImportImageModal($$renderer3, {
        onSuccess: loadImages,
        get open() {
          return importModalOpen;
        },
        set open($$value) {
          importModalOpen = $$value;
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
