import { l as sanitize_props, m as spread_props, j as slot, k as head, b as attr, e as ensure_array_like, c as escape_html, h as derived } from "../../../chunks/renderer.js";
import { o as onDestroy } from "../../../chunks/index-server.js";
import { c as createAPIClient, g as getStoredToken } from "../../../chunks/client2.js";
import { S as StateBadge } from "../../../chunks/StateBadge.js";
import { S as SkeletonRow } from "../../../chunks/SkeletonRow.js";
import { E as EmptyState } from "../../../chunks/EmptyState.js";
import { t as toast } from "../../../chunks/toast.js";
import { R as Refresh_cw, C as Clock } from "../../../chunks/refresh-cw.js";
import { I as Icon } from "../../../chunks/Icon.js";
import { A as Activity } from "../../../chunks/activity.js";
function Funnel($$renderer, $$props) {
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
      "path",
      {
        "d": "M10 20a1 1 0 0 0 .553.895l2 1A1 1 0 0 0 14 21v-7a2 2 0 0 1 .517-1.341L21.74 4.67A1 1 0 0 0 21 3H3a1 1 0 0 0-.742 1.67l7.225 7.989A2 2 0 0 1 10 14z"
      }
    ]
  ];
  Icon($$renderer, spread_props([
    { name: "funnel" },
    $$sanitized_props,
    {
      /**
       * @component @name Funnel
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNMTAgMjBhMSAxIDAgMCAwIC41NTMuODk1bDIgMUExIDEgMCAwIDAgMTQgMjF2LTdhMiAyIDAgMCAxIC41MTctMS4zNDFMMjEuNzQgNC42N0ExIDEgMCAwIDAgMjEgM0gzYTEgMSAwIDAgMC0uNzQyIDEuNjdsNy4yMjUgNy45ODlBMiAyIDAgMCAxIDEwIDE0eiIgLz4KPC9zdmc+Cg==) - https://lucide.dev/icons/funnel
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
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    const client = createAPIClient({ token: getStoredToken() ?? void 0 });
    let events = [];
    let loading = true;
    let autoRefresh = true;
    let sinceAppLoad = true;
    let newEventCount = 0;
    let lastEventCount = 0;
    let filterOperation = "";
    let filterStatus = "";
    let filterResource = "";
    const appStartTime = /* @__PURE__ */ new Date();
    async function loadEvents() {
      try {
        const params = new URLSearchParams();
        if (filterOperation) ;
        if (filterStatus) ;
        if (filterResource) ;
        const query = params.toString();
        let data = await client.listEvents(query ? `?${query}` : "");
        if (sinceAppLoad) {
          data = data.filter((e) => new Date(e.timestamp) >= appStartTime);
        }
        if (events.length > 0 && data.length > lastEventCount) {
          newEventCount = data.length - lastEventCount;
        }
        lastEventCount = data.length;
        events = data;
      } catch {
        toast.error("Failed to load events");
      } finally {
        loading = false;
      }
    }
    onDestroy(() => {
    });
    const operations = derived(() => [...new Set(events.map((e) => e.operation))].sort());
    const resources = derived(() => [...new Set(events.map((e) => e.resource))].sort());
    function formatTime(ts) {
      return new Date(ts).toLocaleString();
    }
    head("13hsgdq", $$renderer2, ($$renderer3) => {
      $$renderer3.title(($$renderer4) => {
        $$renderer4.push(`<title>Events | chv</title>`);
      });
    });
    $$renderer2.push(`<section class="table-card svelte-13hsgdq"><div class="card-header px-4 py-3 svelte-13hsgdq"><div class="flex items-center justify-between"><div><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Operations</div> <div class="mt-1 text-lg font-semibold">Event Log</div></div> <div class="flex items-center gap-3"><label class="flex items-center gap-2 text-sm cursor-pointer"><input type="checkbox"${attr("checked", autoRefresh, true)}/> <span class="text-muted">Auto-refresh (10s)</span> `);
    if (newEventCount > 0) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<span class="bg-red-500 text-white text-xs px-2 py-0.5 rounded-full">${escape_html(newEventCount)} new</span>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></label> <label class="flex items-center gap-2 text-sm cursor-pointer"><input type="checkbox"${attr("checked", sinceAppLoad, true)}/> <span class="text-muted">Since app load</span></label> <button class="p-2 hover:bg-chrome rounded">`);
    Refresh_cw($$renderer2, { size: 16 });
    $$renderer2.push(`<!----></button></div></div> <div class="flex items-center gap-3 mt-4">`);
    Funnel($$renderer2, { size: 16, class: "text-muted" });
    $$renderer2.push(`<!----> `);
    $$renderer2.select(
      {
        value: filterOperation,
        onchange: loadEvents,
        class: "border border-line rounded px-3 py-1.5 text-sm"
      },
      ($$renderer3) => {
        $$renderer3.option({ value: "" }, ($$renderer4) => {
          $$renderer4.push(`All Operations`);
        });
        $$renderer3.push(`<!--[-->`);
        const each_array = ensure_array_like(operations());
        for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
          let op = each_array[$$index];
          $$renderer3.option({ value: op }, ($$renderer4) => {
            $$renderer4.push(`${escape_html(op)}`);
          });
        }
        $$renderer3.push(`<!--]-->`);
      }
    );
    $$renderer2.push(` `);
    $$renderer2.select(
      {
        value: filterStatus,
        onchange: loadEvents,
        class: "border border-line rounded px-3 py-1.5 text-sm"
      },
      ($$renderer3) => {
        $$renderer3.option({ value: "" }, ($$renderer4) => {
          $$renderer4.push(`All Statuses`);
        });
        $$renderer3.option({ value: "success" }, ($$renderer4) => {
          $$renderer4.push(`Success`);
        });
        $$renderer3.option({ value: "failed" }, ($$renderer4) => {
          $$renderer4.push(`Failed`);
        });
        $$renderer3.option({ value: "pending" }, ($$renderer4) => {
          $$renderer4.push(`Pending`);
        });
      }
    );
    $$renderer2.push(` `);
    $$renderer2.select(
      {
        value: filterResource,
        onchange: loadEvents,
        class: "border border-line rounded px-3 py-1.5 text-sm"
      },
      ($$renderer3) => {
        $$renderer3.option({ value: "" }, ($$renderer4) => {
          $$renderer4.push(`All Resources`);
        });
        $$renderer3.push(`<!--[-->`);
        const each_array_1 = ensure_array_like(resources());
        for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
          let res = each_array_1[$$index_1];
          $$renderer3.option({ value: res }, ($$renderer4) => {
            $$renderer4.push(`${escape_html(res)}`);
          });
        }
        $$renderer3.push(`<!--]-->`);
      }
    );
    $$renderer2.push(` `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div></div> `);
    if (loading) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Timestamp</th><th class="border-b border-line px-4 py-3">Operation</th><th class="border-b border-line px-4 py-3">Status</th><th class="border-b border-line px-4 py-3">Resource</th><th class="border-b border-line px-4 py-3">Message</th></tr></thead><tbody><!--[-->`);
      const each_array_2 = ensure_array_like(Array(5));
      for (let $$index_2 = 0, $$length = each_array_2.length; $$index_2 < $$length; $$index_2++) {
        each_array_2[$$index_2];
        SkeletonRow($$renderer2, { columns: 5 });
      }
      $$renderer2.push(`<!--]--></tbody></table>`);
    } else if (events.length === 0) {
      $$renderer2.push("<!--[1-->");
      EmptyState($$renderer2, {
        icon: Activity,
        title: "No events",
        description: "No events since app loaded. Events will appear here as operations complete."
      });
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Timestamp</th><th class="border-b border-line px-4 py-3">Operation</th><th class="border-b border-line px-4 py-3">Status</th><th class="border-b border-line px-4 py-3">Resource</th><th class="border-b border-line px-4 py-3">Message</th></tr></thead><tbody><!--[-->`);
      const each_array_3 = ensure_array_like(events);
      for (let $$index_3 = 0, $$length = each_array_3.length; $$index_3 < $$length; $$index_3++) {
        let event = each_array_3[$$index_3];
        $$renderer2.push(`<tr class="odd:bg-white even:bg-[#f8f8f8]"><td class="border-b border-line px-4 py-3 text-muted whitespace-nowrap"><div class="flex items-center gap-2">`);
        Clock($$renderer2, { size: 14 });
        $$renderer2.push(`<!----> ${escape_html(formatTime(event.timestamp))}</div></td><td class="border-b border-line px-4 py-3 font-medium">${escape_html(event.operation)}</td><td class="border-b border-line px-4 py-3">`);
        StateBadge($$renderer2, { label: event.status });
        $$renderer2.push(`<!----></td><td class="border-b border-line px-4 py-3"><span class="text-xs bg-chrome px-2 py-1 rounded">${escape_html(event.resource)}</span></td><td class="border-b border-line px-4 py-3 text-muted">${escape_html(event.message || "-")}</td></tr>`);
      }
      $$renderer2.push(`<!--]--></tbody></table>`);
    }
    $$renderer2.push(`<!--]--></section>`);
  });
}
export {
  _page as default
};
