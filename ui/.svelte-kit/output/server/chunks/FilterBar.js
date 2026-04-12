import { s as sanitize_props, a as spread_props, b as slot, c as attr, e as escape_html, f as attr_class, g as ensure_array_like, i as derived } from "./root.js";
import { I as Icon } from "./Icon.js";
import { C as Chevron_down } from "./chevron-down.js";
import { X } from "./x.js";
function Sliders_horizontal($$renderer, $$props) {
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
    ["path", { "d": "M10 5H3" }],
    ["path", { "d": "M12 19H3" }],
    ["path", { "d": "M14 3v4" }],
    ["path", { "d": "M16 17v4" }],
    ["path", { "d": "M21 12h-9" }],
    ["path", { "d": "M21 19h-5" }],
    ["path", { "d": "M21 5h-7" }],
    ["path", { "d": "M8 10v4" }],
    ["path", { "d": "M8 12H3" }]
  ];
  Icon($$renderer, spread_props([
    { name: "sliders-horizontal" },
    $$sanitized_props,
    {
      /**
       * @component @name SlidersHorizontal
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNMTAgNUgzIiAvPgogIDxwYXRoIGQ9Ik0xMiAxOUgzIiAvPgogIDxwYXRoIGQ9Ik0xNCAzdjQiIC8+CiAgPHBhdGggZD0iTTE2IDE3djQiIC8+CiAgPHBhdGggZD0iTTIxIDEyaC05IiAvPgogIDxwYXRoIGQ9Ik0yMSAxOWgtNSIgLz4KICA8cGF0aCBkPSJNMjEgNWgtNyIgLz4KICA8cGF0aCBkPSJNOCAxMHY0IiAvPgogIDxwYXRoIGQ9Ik04IDEySDMiIC8+Cjwvc3ZnPgo=) - https://lucide.dev/icons/sliders-horizontal
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
function FilterBar($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { filters, activeFilters, onFilterChange, onClearAll } = $$props;
    let isExpanded = false;
    const activeCount = derived(() => Object.values(activeFilters).filter((v) => v !== void 0 && v !== null && v !== "").length);
    function getFilterDisplayValue(key, value) {
      const filter = filters.find((f) => f.key === key);
      if (!filter) return String(value);
      if (filter.type === "select" && filter.options) {
        const option = filter.options.find((o) => o.value === value);
        return option?.label ?? String(value);
      }
      if (filter.type === "boolean") {
        return value ? "Yes" : "No";
      }
      return String(value);
    }
    const activeFilterEntries = derived(() => Object.entries(activeFilters).filter(([_, v]) => v !== void 0 && v !== null && v !== ""));
    function handleFilterChange(key, e) {
      const target = e.target;
      let value = target.value;
      if (target.type === "checkbox") {
        value = target.checked;
      }
      onFilterChange(key, value);
    }
    $$renderer2.push(`<div class="filter-bar svelte-m9tjun"><button type="button" class="filter-toggle svelte-m9tjun"${attr("aria-expanded", isExpanded)}>`);
    Sliders_horizontal($$renderer2, { size: 16 });
    $$renderer2.push(`<!----> <span>Filters</span> `);
    if (activeCount() > 0) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<span class="filter-badge svelte-m9tjun">${escape_html(activeCount())}</span>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> <span${attr_class("chevron svelte-m9tjun", void 0, { "rotated": isExpanded })}>`);
    Chevron_down($$renderer2, { size: 16 });
    $$renderer2.push(`<!----></span></button> <div${attr_class("filter-content svelte-m9tjun", void 0, { "expanded": isExpanded })}>`);
    if (activeCount() > 0) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="filter-chips svelte-m9tjun"><!--[-->`);
      const each_array = ensure_array_like(activeFilterEntries());
      for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
        let [key, value] = each_array[$$index];
        $$renderer2.push(`<span class="filter-chip svelte-m9tjun"><span class="chip-label svelte-m9tjun">${escape_html(getFilterDisplayValue(key, value))}</span> <button type="button" class="chip-remove svelte-m9tjun"${attr("aria-label", `Remove ${key} filter`)}>`);
        X($$renderer2, { size: 12 });
        $$renderer2.push(`<!----></button></span>`);
      }
      $$renderer2.push(`<!--]--> <button type="button" class="clear-all-btn svelte-m9tjun">Clear all</button></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> <div class="filter-inputs svelte-m9tjun"><!--[-->`);
    const each_array_1 = ensure_array_like(filters);
    for (let $$index_2 = 0, $$length = each_array_1.length; $$index_2 < $$length; $$index_2++) {
      let filter = each_array_1[$$index_2];
      $$renderer2.push(`<div class="filter-field svelte-m9tjun"><label${attr("for", `filter-${filter.key}`)} class="filter-label svelte-m9tjun">${escape_html(filter.label)}</label> `);
      if (filter.type === "select") {
        $$renderer2.push("<!--[0-->");
        $$renderer2.select(
          {
            id: `filter-${filter.key}`,
            class: "filter-select",
            value: activeFilters[filter.key] ?? "",
            onchange: (e) => handleFilterChange(filter.key, e)
          },
          ($$renderer3) => {
            $$renderer3.option({ value: "" }, ($$renderer4) => {
              $$renderer4.push(`All`);
            });
            $$renderer3.push(`<!--[-->`);
            const each_array_2 = ensure_array_like(filter.options ?? []);
            for (let $$index_1 = 0, $$length2 = each_array_2.length; $$index_1 < $$length2; $$index_1++) {
              let option = each_array_2[$$index_1];
              $$renderer3.option({ value: option.value }, ($$renderer4) => {
                $$renderer4.push(`${escape_html(option.label)}`);
              });
            }
            $$renderer3.push(`<!--]-->`);
          },
          "svelte-m9tjun"
        );
      } else if (filter.type === "text") {
        $$renderer2.push("<!--[1-->");
        $$renderer2.push(`<input type="text"${attr("id", `filter-${filter.key}`)} class="filter-input svelte-m9tjun"${attr("placeholder", filter.placeholder ?? `Filter by ${filter.label}`)}${attr("value", activeFilters[filter.key] ?? "")}/>`);
      } else if (filter.type === "date") {
        $$renderer2.push("<!--[2-->");
        $$renderer2.push(`<input type="date"${attr("id", `filter-${filter.key}`)} class="filter-input svelte-m9tjun"${attr("value", activeFilters[filter.key] ?? "")}/>`);
      } else if (filter.type === "boolean") {
        $$renderer2.push("<!--[3-->");
        $$renderer2.push(`<label class="filter-checkbox svelte-m9tjun"><input type="checkbox"${attr("checked", !!activeFilters[filter.key], true)} class="svelte-m9tjun"/> <span>${escape_html(filter.label)}</span></label>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]--></div>`);
    }
    $$renderer2.push(`<!--]--></div></div></div>`);
  });
}
export {
  FilterBar as F
};
