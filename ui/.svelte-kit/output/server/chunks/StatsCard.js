import { s as sanitize_props, a as spread_props, b as slot, c as attr, e as escape_html } from "./renderer.js";
import { I as Icon } from "./Icon.js";
import { M as Minus, T as Trending_down, a as Trending_up } from "./trending-up.js";
import { C as Chevron_right } from "./chevron-right.js";
function Play($$renderer, $$props) {
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
        "d": "M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z"
      }
    ]
  ];
  Icon($$renderer, spread_props([
    { name: "play" },
    $$sanitized_props,
    {
      /**
       * @component @name Play
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNNSA1YTIgMiAwIDAgMSAzLjAwOC0xLjcyOGwxMS45OTcgNi45OThhMiAyIDAgMCAxIC4wMDMgMy40NThsLTEyIDdBMiAyIDAgMCAxIDUgMTl6IiAvPgo8L3N2Zz4K) - https://lucide.dev/icons/play
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
function StatsCard($$renderer, $$props) {
  let { title, value, icon: Icon2, trend, subtitle, href } = $$props;
  const trendConfig = {
    up: { icon: Trending_up, colorClass: "text-success" },
    down: { icon: Trending_down, colorClass: "text-danger" },
    neutral: { icon: Minus, colorClass: "text-light" }
  };
  if (href) {
    $$renderer.push("<!--[0-->");
    $$renderer.push(`<a${attr("href", href)} class="rounded border border-line bg-chrome p-4 block no-underline text-inherit hover:shadow-md transition-shadow"${attr("aria-label", title)}><div class="flex items-start justify-between"><div class="flex-1"><div class="mb-2 text-[11px] uppercase tracking-[0.08em] text-muted">${escape_html(title)}</div> <div class="flex items-center gap-2">`);
    if (Icon2) {
      $$renderer.push("<!--[0-->");
      if (Icon2) {
        $$renderer.push("<!--[-->");
        Icon2($$renderer, { size: 24, class: "text-muted", "aria-hidden": "true" });
        $$renderer.push("<!--]-->");
      } else {
        $$renderer.push("<!--[!-->");
        $$renderer.push("<!--]-->");
      }
    } else {
      $$renderer.push("<!--[-1-->");
    }
    $$renderer.push(`<!--]--> <span class="text-[32px] font-semibold text-ink">${escape_html(value)}</span></div> `);
    if (subtitle) {
      $$renderer.push("<!--[0-->");
      $$renderer.push(`<div class="mt-1 text-sm text-muted">${escape_html(subtitle)}</div>`);
    } else {
      $$renderer.push("<!--[-1-->");
    }
    $$renderer.push(`<!--]--></div> `);
    if (trend) {
      $$renderer.push("<!--[0-->");
      const TrendIcon = trendConfig[trend].icon;
      const trendColor = trendConfig[trend].colorClass;
      $$renderer.push(`<div class="flex-shrink-0">`);
      if (TrendIcon) {
        $$renderer.push("<!--[-->");
        TrendIcon($$renderer, { size: 20, class: trendColor, "aria-hidden": "true" });
        $$renderer.push("<!--]-->");
      } else {
        $$renderer.push("<!--[!-->");
        $$renderer.push("<!--]-->");
      }
      $$renderer.push(`</div>`);
    } else {
      $$renderer.push("<!--[-1-->");
      $$renderer.push(`<div class="flex-shrink-0">`);
      Chevron_right($$renderer, { size: 20, class: "text-muted", "aria-hidden": "true" });
      $$renderer.push(`<!----></div>`);
    }
    $$renderer.push(`<!--]--></div></a>`);
  } else {
    $$renderer.push("<!--[-1-->");
    $$renderer.push(`<div class="rounded border border-line bg-chrome p-4" role="region"${attr("aria-label", title)}><div class="flex items-start justify-between"><div class="flex-1"><div class="mb-2 text-[11px] uppercase tracking-[0.08em] text-muted">${escape_html(title)}</div> <div class="flex items-center gap-2">`);
    if (Icon2) {
      $$renderer.push("<!--[0-->");
      if (Icon2) {
        $$renderer.push("<!--[-->");
        Icon2($$renderer, { size: 24, class: "text-muted", "aria-hidden": "true" });
        $$renderer.push("<!--]-->");
      } else {
        $$renderer.push("<!--[!-->");
        $$renderer.push("<!--]-->");
      }
    } else {
      $$renderer.push("<!--[-1-->");
    }
    $$renderer.push(`<!--]--> <span class="text-[32px] font-semibold text-ink">${escape_html(value)}</span></div> `);
    if (subtitle) {
      $$renderer.push("<!--[0-->");
      $$renderer.push(`<div class="mt-1 text-sm text-muted">${escape_html(subtitle)}</div>`);
    } else {
      $$renderer.push("<!--[-1-->");
    }
    $$renderer.push(`<!--]--></div> `);
    if (trend) {
      $$renderer.push("<!--[0-->");
      const TrendIcon = trendConfig[trend].icon;
      const trendColor = trendConfig[trend].colorClass;
      $$renderer.push(`<div class="flex-shrink-0">`);
      if (TrendIcon) {
        $$renderer.push("<!--[-->");
        TrendIcon($$renderer, { size: 20, class: trendColor, "aria-hidden": "true" });
        $$renderer.push("<!--]-->");
      } else {
        $$renderer.push("<!--[!-->");
        $$renderer.push("<!--]-->");
      }
      $$renderer.push(`</div>`);
    } else {
      $$renderer.push("<!--[-1-->");
    }
    $$renderer.push(`<!--]--></div></div>`);
  }
  $$renderer.push(`<!--]-->`);
}
export {
  Play as P,
  StatsCard as S
};
