import { s as sanitize_props, a as spread_props, b as slot, l as attributes, i as derived, h as stringify, c as attr, e as escape_html, f as attr_class, g as ensure_array_like, m as attr_style, n as head } from "../../chunks/root.js";
import { o as onDestroy } from "../../chunks/index-server.js";
import { g as goto } from "../../chunks/client.js";
import { g as getStoredToken, c as createAPIClient, t as toast } from "../../chunks/client2.js";
import { g as getDefaultNode } from "../../chunks/nodes.js";
import { I as Icon } from "../../chunks/Icon.js";
import { M as Minus, T as Trending_down, a as Trending_up, C as Circle_alert } from "../../chunks/trending-up.js";
import { R as Refresh_cw } from "../../chunks/refresh-cw.js";
import { C as Chevron_up } from "../../chunks/chevron-up.js";
import { C as Circle_check_big, a as Cpu } from "../../chunks/cpu.js";
import { C as Chevron_down } from "../../chunks/chevron-down.js";
import { S as StateBadge } from "../../chunks/StateBadge.js";
import { F as Funnel } from "../../chunks/funnel.js";
import { T as Trash_2 } from "../../chunks/trash-2.js";
import { A as Activity } from "../../chunks/activity.js";
import { H as Hard_drive } from "../../chunks/hard-drive.js";
import { N as Network } from "../../chunks/network.js";
import { I as Image } from "../../chunks/image.js";
import { S as Server } from "../../chunks/server.js";
import { X } from "../../chunks/x.js";
import { C as CreateVMModal } from "../../chunks/CreateVMModal.js";
import { I as ImportImageModal } from "../../chunks/ImportImageModal.js";
import { C as CreateNetworkModal } from "../../chunks/CreateNetworkModal.js";
import { P as Plus } from "../../chunks/plus.js";
import { D as Download } from "../../chunks/download.js";
import { S as Settings } from "../../chunks/settings.js";
import { D as Database } from "../../chunks/database.js";
function Check_check($$renderer, $$props) {
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
    ["path", { "d": "M18 6 7 17l-5-5" }],
    ["path", { "d": "m22 10-7.5 7.5L13 16" }]
  ];
  Icon($$renderer, spread_props([
    { name: "check-check" },
    $$sanitized_props,
    {
      /**
       * @component @name CheckCheck
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNMTggNiA3IDE3bC01LTUiIC8+CiAgPHBhdGggZD0ibTIyIDEwLTcuNSA3LjVMMTMgMTYiIC8+Cjwvc3ZnPgo=) - https://lucide.dev/icons/check-check
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
function Circle_x($$renderer, $$props) {
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
    ["path", { "d": "m15 9-6 6" }],
    ["path", { "d": "m9 9 6 6" }]
  ];
  Icon($$renderer, spread_props([
    { name: "circle-x" },
    $$sanitized_props,
    {
      /**
       * @component @name CircleX
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8Y2lyY2xlIGN4PSIxMiIgY3k9IjEyIiByPSIxMCIgLz4KICA8cGF0aCBkPSJtMTUgOS02IDYiIC8+CiAgPHBhdGggZD0ibTkgOSA2IDYiIC8+Cjwvc3ZnPgo=) - https://lucide.dev/icons/circle-x
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
function Button($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let {
      variant = "primary",
      size = "md",
      loading = false,
      disabled = false,
      ariaLabel,
      title,
      children,
      $$slots,
      $$events,
      ...rest
    } = $$props;
    const isDisabled = derived(() => disabled || loading);
    const variantClasses = {
      primary: "btn-primary",
      secondary: "btn-secondary",
      ghost: "btn-ghost",
      danger: "btn-danger"
    };
    const sizeClasses = { sm: "btn-sm", md: "btn-md", lg: "btn-lg" };
    let isIconOnly = false;
    $$renderer2.push(`<button${attributes(
      {
        type: "button",
        class: `btn ${stringify(
          // If no children slot is provided, it's likely an icon-only button
          variantClasses[variant]
        )} ${stringify(sizeClasses[size])}`,
        disabled: isDisabled(),
        "aria-disabled": isDisabled(),
        "aria-busy": loading,
        "aria-label": ariaLabel,
        title,
        ...rest
      },
      "svelte-8a1c4v",
      { "icon-only": isIconOnly }
    )}>`);
    if (loading) {
      $$renderer2.push("<!--[0-->");
      Loader_circle($$renderer2, {
        size: size === "lg" ? 20 : 16,
        "aria-hidden": "true",
        class: "animate-spin"
      });
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> `);
    children?.($$renderer2);
    $$renderer2.push(`<!----></button>`);
  });
}
function ResourceCard($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let {
      title,
      value,
      icon: Icon2,
      iconColor = "slate",
      trend,
      trendValue,
      subtitle,
      loading = false,
      progress,
      sparklineData,
      href
    } = $$props;
    const iconColorClasses = {
      blue: "bg-blue-50 text-blue-600",
      green: "bg-green-50 text-green-600",
      amber: "bg-amber-50 text-amber-600",
      purple: "bg-purple-50 text-purple-600",
      red: "bg-red-50 text-red-600",
      slate: "bg-slate-100 text-slate-600"
    };
    const trendConfig = {
      up: {
        icon: Trending_up,
        colorClass: "text-green-600",
        bgClass: "bg-green-50"
      },
      down: {
        icon: Trending_down,
        colorClass: "text-red-600",
        bgClass: "bg-red-50"
      },
      neutral: {
        icon: Minus,
        colorClass: "text-slate-500",
        bgClass: "bg-slate-100"
      }
    };
    const percentage = derived(() => progress ? Math.min(100, Math.max(0, progress.value / progress.max * 100)) : 0);
    const sparklinePoints = derived(() => () => {
      if (!sparklineData || sparklineData.length < 2) return [];
      const min = Math.min(...sparklineData);
      const max = Math.max(...sparklineData);
      const range = max - min || 1;
      const width = 60;
      const height = 24;
      return sparklineData.map((point, i) => ({
        x: i / (sparklineData.length - 1) * width,
        y: height - (point - min) / range * 20 - 2,
        hasPrev: i > 0
      }));
    });
    if (href) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<a${attr("href", href)} class="resource-card card card-interactive block no-underline text-inherit svelte-svj27m"${attr("aria-label", title)}><div class="p-5">`);
      if (loading) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<div class="animate-pulse"><div class="flex items-start justify-between"><div class="flex-1"><div class="h-3 w-20 bg-slate-200 rounded mb-3"></div> <div class="h-8 w-16 bg-slate-200 rounded"></div></div> <div class="h-10 w-10 bg-slate-200 rounded-lg"></div></div> <div class="mt-4 pt-4 border-t border-slate-100"><div class="h-2 w-full bg-slate-200 rounded-full"></div></div></div>`);
      } else {
        $$renderer2.push("<!--[-1-->");
        $$renderer2.push(`<div class="flex items-start justify-between"><div class="flex-1 min-w-0"><div class="flex items-center gap-2"><span class="text-xs font-medium text-slate-500 uppercase tracking-wider">${escape_html(title)}</span> `);
        if (trend && trendValue) {
          $$renderer2.push("<!--[0-->");
          const config = trendConfig[trend];
          $$renderer2.push(`<span${attr_class(`inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium ${stringify(config.bgClass)} ${stringify(config.colorClass)}`, "svelte-svj27m")}>`);
          if (config.icon) {
            $$renderer2.push("<!--[-->");
            config.icon($$renderer2, { size: 10 });
            $$renderer2.push("<!--]-->");
          } else {
            $$renderer2.push("<!--[!-->");
            $$renderer2.push("<!--]-->");
          }
          $$renderer2.push(` ${escape_html(trendValue)}</span>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--></div> <div class="flex items-center gap-3 mt-1"><span class="text-2xl font-bold text-slate-900">${escape_html(value)}</span> `);
        if (sparklineData && sparklineData.length > 1) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<svg width="60" height="24" viewBox="0 0 60 24" class="sparkline-mini svelte-svj27m"><!--[-->`);
          const each_array = ensure_array_like(sparklinePoints()());
          for (let i = 0, $$length = each_array.length; i < $$length; i++) {
            let point = each_array[i];
            $$renderer2.push(`<circle${attr("cx", point.x)}${attr("cy", point.y)} r="1.5" fill="var(--color-primary)"></circle>`);
            if (point.hasPrev) {
              $$renderer2.push("<!--[0-->");
              const prev = sparklinePoints()()[i - 1];
              $$renderer2.push(`<line${attr("x1", prev.x)}${attr("y1", prev.y)}${attr("x2", point.x)}${attr("y2", point.y)} stroke="var(--color-primary)" stroke-width="1.5" stroke-linecap="round"></line>`);
            } else {
              $$renderer2.push("<!--[-1-->");
            }
            $$renderer2.push(`<!--]-->`);
          }
          $$renderer2.push(`<!--]--></svg>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--></div> `);
        if (subtitle) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<p class="text-xs text-slate-500 mt-1">${escape_html(subtitle)}</p>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--></div> `);
        if (Icon2) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<div${attr_class(`p-2.5 rounded-lg ${stringify(iconColorClasses[iconColor])} transition-colors`, "svelte-svj27m")}>`);
          if (Icon2) {
            $$renderer2.push("<!--[-->");
            Icon2($$renderer2, { size: 20 });
            $$renderer2.push("<!--]-->");
          } else {
            $$renderer2.push("<!--[!-->");
            $$renderer2.push("<!--]-->");
          }
          $$renderer2.push(`</div>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--></div> `);
        if (progress) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<div class="mt-4 pt-4 border-t border-slate-100"><div class="flex items-center justify-between text-sm mb-1.5"><span class="text-slate-500">${escape_html(progress.label || "Usage")}</span> <span class="font-medium text-slate-700">${escape_html(percentage().toFixed(0))}%</span></div> <div class="w-full bg-slate-100 rounded-full h-2 overflow-hidden"><div class="h-2 rounded-full transition-all duration-500 ease-out progress-bar svelte-svj27m"${attr_style(`width: ${stringify(percentage())}%`)}></div></div> <p class="text-xs text-slate-500 mt-1.5">${escape_html(progress.value.toLocaleString())} of ${escape_html(progress.max.toLocaleString())}</p></div>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]-->`);
      }
      $$renderer2.push(`<!--]--></div></a>`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<div class="resource-card card svelte-svj27m" role="region"${attr("aria-label", title)}><div class="p-5">`);
      if (loading) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<div class="animate-pulse"><div class="flex items-start justify-between"><div class="flex-1"><div class="h-3 w-20 bg-slate-200 rounded mb-3"></div> <div class="h-8 w-16 bg-slate-200 rounded"></div></div> <div class="h-10 w-10 bg-slate-200 rounded-lg"></div></div> <div class="mt-4 pt-4 border-t border-slate-100"><div class="h-2 w-full bg-slate-200 rounded-full"></div></div></div>`);
      } else {
        $$renderer2.push("<!--[-1-->");
        $$renderer2.push(`<div class="flex items-start justify-between"><div class="flex-1 min-w-0"><div class="flex items-center gap-2"><span class="text-xs font-medium text-slate-500 uppercase tracking-wider">${escape_html(title)}</span> `);
        if (trend && trendValue) {
          $$renderer2.push("<!--[0-->");
          const config = trendConfig[trend];
          $$renderer2.push(`<span${attr_class(`inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium ${stringify(config.bgClass)} ${stringify(config.colorClass)}`, "svelte-svj27m")}>`);
          if (config.icon) {
            $$renderer2.push("<!--[-->");
            config.icon($$renderer2, { size: 10 });
            $$renderer2.push("<!--]-->");
          } else {
            $$renderer2.push("<!--[!-->");
            $$renderer2.push("<!--]-->");
          }
          $$renderer2.push(` ${escape_html(trendValue)}</span>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--></div> <div class="flex items-center gap-3 mt-1"><span class="text-2xl font-bold text-slate-900">${escape_html(value)}</span> `);
        if (sparklineData && sparklineData.length > 1) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<svg width="60" height="24" viewBox="0 0 60 24" class="sparkline-mini svelte-svj27m"><!--[-->`);
          const each_array_1 = ensure_array_like(sparklinePoints()());
          for (let i = 0, $$length = each_array_1.length; i < $$length; i++) {
            let point = each_array_1[i];
            $$renderer2.push(`<circle${attr("cx", point.x)}${attr("cy", point.y)} r="1.5" fill="var(--color-primary)"></circle>`);
            if (point.hasPrev) {
              $$renderer2.push("<!--[0-->");
              const prev = sparklinePoints()()[i - 1];
              $$renderer2.push(`<line${attr("x1", prev.x)}${attr("y1", prev.y)}${attr("x2", point.x)}${attr("y2", point.y)} stroke="var(--color-primary)" stroke-width="1.5" stroke-linecap="round"></line>`);
            } else {
              $$renderer2.push("<!--[-1-->");
            }
            $$renderer2.push(`<!--]-->`);
          }
          $$renderer2.push(`<!--]--></svg>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--></div> `);
        if (subtitle) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<p class="text-xs text-slate-500 mt-1">${escape_html(subtitle)}</p>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--></div> `);
        if (Icon2) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<div${attr_class(`p-2.5 rounded-lg ${stringify(iconColorClasses[iconColor])} transition-colors`, "svelte-svj27m")}>`);
          if (Icon2) {
            $$renderer2.push("<!--[-->");
            Icon2($$renderer2, { size: 20 });
            $$renderer2.push("<!--]-->");
          } else {
            $$renderer2.push("<!--[!-->");
            $$renderer2.push("<!--]-->");
          }
          $$renderer2.push(`</div>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--></div> `);
        if (progress) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<div class="mt-4 pt-4 border-t border-slate-100"><div class="flex items-center justify-between text-sm mb-1.5"><span class="text-slate-500">${escape_html(progress.label || "Usage")}</span> <span class="font-medium text-slate-700">${escape_html(percentage().toFixed(0))}%</span></div> <div class="w-full bg-slate-100 rounded-full h-2 overflow-hidden"><div class="h-2 rounded-full transition-all duration-500 ease-out progress-bar svelte-svj27m"${attr_style(`width: ${stringify(percentage())}%`)}></div></div> <p class="text-xs text-slate-500 mt-1.5">${escape_html(progress.value.toLocaleString())} of ${escape_html(progress.max.toLocaleString())}</p></div>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]-->`);
      }
      $$renderer2.push(`<!--]--></div></div>`);
    }
    $$renderer2.push(`<!--]-->`);
  });
}
function HealthStatus($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { checks, loading = false, onRefresh, lastUpdated = /* @__PURE__ */ new Date() } = $$props;
    let expandedChecks = /* @__PURE__ */ new Set();
    const statusConfig = {
      healthy: {
        icon: Circle_check_big,
        colorClass: "text-green-600",
        bgClass: "bg-green-50",
        borderClass: "border-green-200",
        label: "Healthy"
      },
      warning: {
        icon: Circle_alert,
        colorClass: "text-amber-600",
        bgClass: "bg-amber-50",
        borderClass: "border-amber-200",
        label: "Warning"
      },
      error: {
        icon: Circle_x,
        colorClass: "text-red-600",
        bgClass: "bg-red-50",
        borderClass: "border-red-200",
        label: "Error"
      },
      pending: {
        icon: Loader_circle,
        colorClass: "text-blue-600",
        bgClass: "bg-blue-50",
        borderClass: "border-blue-200",
        label: "Checking"
      }
    };
    const overallStatus = derived(() => () => {
      if (checks.length === 0) return "pending";
      if (checks.some((c) => c.status === "error")) return "error";
      if (checks.some((c) => c.status === "warning")) return "warning";
      return "healthy";
    });
    const overallConfig = derived(() => statusConfig[overallStatus()()]);
    function formatTime(date) {
      const now = /* @__PURE__ */ new Date();
      const diff = Math.floor((now.getTime() - date.getTime()) / 1e3);
      if (diff < 60) return "just now";
      if (diff < 3600) return `${Math.floor(diff / 60)} min ago`;
      return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    }
    $$renderer2.push(`<div class="health-status card svelte-za99o8" role="region" aria-label="System Health Status"><div class="px-5 py-4 border-b border-slate-100"><div class="flex items-center justify-between"><div class="flex items-center gap-3"><div${attr_class(`p-2 rounded-lg ${stringify(overallConfig().bgClass)}`, "svelte-za99o8")}>`);
    if (overallConfig.icon) {
      $$renderer2.push("<!--[-->");
      overallConfig.icon($$renderer2, { size: 20, class: overallConfig().colorClass });
      $$renderer2.push("<!--]-->");
    } else {
      $$renderer2.push("<!--[!-->");
      $$renderer2.push("<!--]-->");
    }
    $$renderer2.push(`</div> <div><h3 class="font-semibold text-slate-900">System Health</h3> <p class="text-xs text-slate-500">`);
    if (loading) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`Checking status...`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`Updated ${escape_html(formatTime(lastUpdated))}`);
    }
    $$renderer2.push(`<!--]--></p></div></div> <div class="flex items-center gap-2">`);
    if (onRefresh) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<button class="p-2 text-slate-400 hover:text-slate-600 hover:bg-slate-100 rounded-lg transition-colors" aria-label="Refresh health status"${attr("disabled", loading, true)}>`);
      Refresh_cw($$renderer2, { size: 16, class: loading ? "animate-spin" : "" });
      $$renderer2.push(`<!----></button>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> <button class="p-2 text-slate-400 hover:text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"${attr("aria-label", "Collapse")}>`);
    {
      $$renderer2.push("<!--[-1-->");
      Chevron_up($$renderer2, { size: 16 });
    }
    $$renderer2.push(`<!--]--></button></div></div></div> `);
    {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="divide-y divide-slate-100"><!--[-->`);
      const each_array = ensure_array_like(checks);
      for (let $$index_1 = 0, $$length = each_array.length; $$index_1 < $$length; $$index_1++) {
        let check = each_array[$$index_1];
        const config = statusConfig[check.status];
        $$renderer2.push(`<div class="health-check svelte-za99o8"><button class="w-full px-5 py-3 flex items-center justify-between hover:bg-slate-50 transition-colors text-left"${attr("aria-expanded", expandedChecks.has(check.id))}><div class="flex items-center gap-3"><div${attr_class(`p-1.5 rounded-md ${stringify(config.bgClass)}`, "svelte-za99o8")}>`);
        if (config.icon) {
          $$renderer2.push("<!--[-->");
          config.icon($$renderer2, { size: 14, class: config.colorClass });
          $$renderer2.push("<!--]-->");
        } else {
          $$renderer2.push("<!--[!-->");
          $$renderer2.push("<!--]-->");
        }
        $$renderer2.push(`</div> <div><span class="text-sm font-medium text-slate-700">${escape_html(check.name)}</span> `);
        if (check.message) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<p class="text-xs text-slate-500">${escape_html(check.message)}</p>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--></div></div> <div class="flex items-center gap-2"><span${attr_class(`text-xs font-medium ${stringify(config.colorClass)}`, "svelte-za99o8")}>${escape_html(config.label)}</span> `);
        if (check.details && check.details.length > 0) {
          $$renderer2.push("<!--[0-->");
          Chevron_down($$renderer2, {
            size: 14,
            class: `text-slate-400 transition-transform ${stringify(expandedChecks.has(check.id) ? "rotate-180" : "")}`
          });
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--></div></button> `);
        if (expandedChecks.has(check.id) && check.details && check.details.length > 0) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<div class="px-5 pb-3 pl-14"><ul class="space-y-1"><!--[-->`);
          const each_array_1 = ensure_array_like(check.details);
          for (let $$index = 0, $$length2 = each_array_1.length; $$index < $$length2; $$index++) {
            let detail = each_array_1[$$index];
            $$renderer2.push(`<li class="text-xs text-slate-500 flex items-start gap-2"><span class="w-1 h-1 rounded-full bg-slate-400 mt-1.5"></span> ${escape_html(detail)}</li>`);
          }
          $$renderer2.push(`<!--]--></ul></div>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--></div>`);
      }
      $$renderer2.push(`<!--]--> `);
      if (checks.length === 0) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<div class="px-5 py-8 text-center">`);
        Loader_circle($$renderer2, { size: 24, class: "mx-auto mb-2 text-slate-400 animate-spin" });
        $$renderer2.push(`<!----> <p class="text-sm text-slate-500">Loading health checks...</p></div>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]--></div>`);
    }
    $$renderer2.push(`<!--]--></div>`);
  });
}
function EventList($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { events, loading = false, onFilter, onClear, onMarkAllRead } = $$props;
    let expandedEvents = /* @__PURE__ */ new Set();
    let currentFilter = null;
    let currentPage = 1;
    const itemsPerPage = 10;
    const resourceIcons = {
      vm: Server,
      image: Image,
      network: Network,
      storage: Hard_drive,
      default: Activity
    };
    const resourceFilters = [
      { id: "vm", label: "VM", icon: Server },
      { id: "image", label: "Image", icon: Image },
      { id: "network", label: "Network", icon: Network },
      { id: "storage", label: "Storage", icon: Hard_drive }
    ];
    const filteredEvents = derived(() => () => {
      if (!currentFilter) return events;
      return events.filter((e) => e.resource.toLowerCase().includes(currentFilter.toLowerCase()));
    });
    const paginatedEvents = derived(() => () => {
      const filtered = filteredEvents()();
      const start = (currentPage - 1) * itemsPerPage;
      return filtered.slice(start, start + itemsPerPage);
    });
    const totalPages = derived(() => () => {
      return Math.ceil(filteredEvents()().length / itemsPerPage);
    });
    function setFilter(filter) {
      currentFilter = filter;
      currentPage = 1;
      onFilter?.(filter);
    }
    function formatRelativeTime(timestamp) {
      const date = new Date(timestamp);
      const now = /* @__PURE__ */ new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffSec = Math.floor(diffMs / 1e3);
      const diffMin = Math.floor(diffSec / 60);
      const diffHour = Math.floor(diffMin / 60);
      const diffDay = Math.floor(diffHour / 24);
      if (diffSec < 60) return "just now";
      if (diffMin < 60) return `${diffMin}m ago`;
      if (diffHour < 24) return `${diffHour}h ago`;
      if (diffDay < 7) return `${diffDay}d ago`;
      return date.toLocaleDateString();
    }
    function formatFullTime(timestamp) {
      return new Date(timestamp).toLocaleString();
    }
    function getResourceIcon(resource) {
      const key = resource.toLowerCase();
      return resourceIcons[key] || resourceIcons.default;
    }
    $$renderer2.push(`<div class="event-list card svelte-1gdibj8" role="region" aria-label="Events List"><div class="px-5 py-4 border-b border-slate-100 bg-slate-50/50"><div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4"><div><h3 class="font-semibold text-slate-900">Recent Events</h3> <p class="text-sm text-slate-500 mt-0.5">`);
    if (loading) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`Loading events...`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`${escape_html(filteredEvents()().length)} events `);
      if (currentFilter) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<span class="text-slate-400">(filtered)</span>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]-->`);
    }
    $$renderer2.push(`<!--]--></p></div> <div class="flex items-center gap-2"><div class="relative">`);
    $$renderer2.select(
      {
        value: currentFilter || "",
        onchange: (e) => setFilter(e.target.value || null),
        class: "appearance-none bg-white border border-slate-200 text-slate-700 text-sm rounded-lg px-3 py-2 pr-8 focus:outline-none focus:ring-2 focus:ring-orange-500/20 focus:border-orange-500",
        "aria-label": "Filter by resource type"
      },
      ($$renderer3) => {
        $$renderer3.option({ value: "" }, ($$renderer4) => {
          $$renderer4.push(`All Types`);
        });
        $$renderer3.push(`<!--[-->`);
        const each_array = ensure_array_like(resourceFilters);
        for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
          let filter = each_array[$$index];
          $$renderer3.option({ value: filter.id }, ($$renderer4) => {
            $$renderer4.push(`${escape_html(filter.label)}`);
          });
        }
        $$renderer3.push(`<!--]-->`);
      }
    );
    $$renderer2.push(` `);
    Funnel($$renderer2, {
      size: 14,
      class: "absolute right-2.5 top-1/2 -translate-y-1/2 text-slate-400 pointer-events-none"
    });
    $$renderer2.push(`<!----></div> `);
    if (onMarkAllRead) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<button class="p-2 text-slate-500 hover:text-slate-700 hover:bg-slate-100 rounded-lg transition-colors" title="Mark all as read" aria-label="Mark all as read">`);
      Check_check($$renderer2, { size: 16 });
      $$renderer2.push(`<!----></button>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> `);
    if (onClear && events.length > 0) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<button class="p-2 text-slate-500 hover:text-red-600 hover:bg-red-50 rounded-lg transition-colors" title="Clear all" aria-label="Clear all events">`);
      Trash_2($$renderer2, { size: 16 });
      $$renderer2.push(`<!----></button>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div></div> `);
    if (currentFilter) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="flex items-center gap-2 mt-3"><span class="inline-flex items-center gap-1.5 px-2.5 py-1 bg-orange-50 text-orange-700 text-xs font-medium rounded-full border border-orange-200">${escape_html(resourceFilters.find((f) => f.id === currentFilter)?.label || currentFilter)} <button class="hover:text-orange-900" aria-label="Clear filter">`);
      X($$renderer2, { size: 12 });
      $$renderer2.push(`<!----></button></span></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div> <div class="divide-y divide-slate-100">`);
    if (loading) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="p-8 text-center">`);
      Loader_circle($$renderer2, { size: 24, class: "mx-auto mb-2 text-slate-400 animate-spin" });
      $$renderer2.push(`<!----> <p class="text-sm text-slate-500">Loading events...</p></div>`);
    } else if (paginatedEvents()().length === 0) {
      $$renderer2.push("<!--[1-->");
      $$renderer2.push(`<div class="p-8 text-center">`);
      Activity($$renderer2, { size: 32, class: "mx-auto mb-3 opacity-40 text-slate-400" });
      $$renderer2.push(`<!----> <p class="text-sm text-slate-500">${escape_html(currentFilter ? "No events match the filter" : "No recent events")}</p></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<!--[-->`);
      const each_array_1 = ensure_array_like(paginatedEvents()());
      for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
        let event = each_array_1[$$index_1];
        const ResourceIcon = getResourceIcon(event.resource);
        const isExpanded = expandedEvents.has(event.id);
        $$renderer2.push(`<div class="event-item svelte-1gdibj8"><button class="w-full px-5 py-3 flex items-center justify-between hover:bg-slate-50 transition-colors text-left"${attr("aria-expanded", isExpanded)}><div class="flex items-center gap-3 min-w-0 flex-1"><div class="p-1.5 bg-slate-100 rounded-md flex-shrink-0">`);
        if (ResourceIcon) {
          $$renderer2.push("<!--[-->");
          ResourceIcon($$renderer2, { size: 14, class: "text-slate-500" });
          $$renderer2.push("<!--]-->");
        } else {
          $$renderer2.push("<!--[!-->");
          $$renderer2.push("<!--]-->");
        }
        $$renderer2.push(`</div> `);
        StateBadge($$renderer2, { label: event.status });
        $$renderer2.push(`<!----> <span class="text-sm font-medium text-slate-700 capitalize truncate">${escape_html(event.operation)}</span> <span class="text-xs text-slate-400 hidden sm:inline">${escape_html(event.resource)} `);
        if (event.resource_id) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<span class="font-mono">(${escape_html(event.resource_id.slice(0, 8))}...)</span>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--></span></div> <div class="flex items-center gap-2 flex-shrink-0"><span class="text-xs text-slate-500 whitespace-nowrap"${attr("title", formatFullTime(event.timestamp))}>${escape_html(formatRelativeTime(event.timestamp))}</span> `);
        Chevron_down($$renderer2, {
          size: 14,
          class: `text-slate-400 transition-transform ${stringify(isExpanded ? "rotate-180" : "")}`
        });
        $$renderer2.push(`<!----></div></button> `);
        if (isExpanded) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<div class="px-5 pb-3 pl-14 bg-slate-50/50"><div class="space-y-2">`);
          if (event.message) {
            $$renderer2.push("<!--[0-->");
            $$renderer2.push(`<p class="text-sm text-slate-600">${escape_html(event.message)}</p>`);
          } else {
            $$renderer2.push("<!--[-1-->");
          }
          $$renderer2.push(`<!--]--> <div class="grid grid-cols-2 gap-4 text-xs"><div><span class="text-slate-400">Resource:</span> <span class="text-slate-600 ml-1 capitalize">${escape_html(event.resource)}</span></div> `);
          if (event.resource_id) {
            $$renderer2.push("<!--[0-->");
            $$renderer2.push(`<div><span class="text-slate-400">ID:</span> <span class="text-slate-600 ml-1 font-mono">${escape_html(event.resource_id)}</span></div>`);
          } else {
            $$renderer2.push("<!--[-1-->");
          }
          $$renderer2.push(`<!--]--> <div><span class="text-slate-400">Time:</span> <span class="text-slate-600 ml-1">${escape_html(formatFullTime(event.timestamp))}</span></div> `);
          if (event.details) {
            $$renderer2.push("<!--[0-->");
            $$renderer2.push(`<div class="col-span-2"><span class="text-slate-400">Details:</span> <pre class="mt-1 p-2 bg-slate-100 rounded text-slate-600 overflow-x-auto">${escape_html(JSON.stringify(event.details, null, 2))}</pre></div>`);
          } else {
            $$renderer2.push("<!--[-1-->");
          }
          $$renderer2.push(`<!--]--></div></div></div>`);
        } else {
          $$renderer2.push("<!--[-1-->");
        }
        $$renderer2.push(`<!--]--></div>`);
      }
      $$renderer2.push(`<!--]-->`);
    }
    $$renderer2.push(`<!--]--></div> `);
    if (totalPages()() > 1) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="px-5 py-3 border-t border-slate-100 flex items-center justify-between"><button${attr("disabled", currentPage === 1, true)} class="text-sm text-slate-600 hover:text-slate-900 disabled:opacity-50 disabled:cursor-not-allowed transition-colors">Previous</button> <span class="text-sm text-slate-500">Page ${escape_html(currentPage)} of ${escape_html(totalPages()())}</span> <button${attr("disabled", currentPage === totalPages()(), true)} class="text-sm text-slate-600 hover:text-slate-900 disabled:opacity-50 disabled:cursor-not-allowed transition-colors">Next</button></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div>`);
  });
}
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    const token = getStoredToken();
    const client = createAPIClient({ token: token ?? void 0 });
    let vms = [];
    let images = [];
    let pools = [];
    let networks = [];
    let events = [];
    let installState = "unknown";
    let loading = true;
    let lastUpdated = /* @__PURE__ */ new Date();
    let showCreateVM = false;
    let showImportImage = false;
    let showCreateNetwork = false;
    const runningVMs = derived(() => vms.filter((v) => v.actual_state === "running").length);
    const stoppedVMs = derived(() => vms.filter((v) => v.actual_state === "stopped").length);
    const totalVcpus = derived(() => vms.reduce((acc, v) => acc + (v.vcpu || 0), 0));
    const totalMemoryGB = derived(() => vms.reduce((acc, v) => acc + (v.memory_mb || 0) / 1024, 0));
    const totalStorageGB = derived(() => pools.reduce((acc, p) => acc + (p.capacity_bytes || 0), 0) / 1024 ** 3);
    const usedStorageGB = derived(() => pools.reduce((acc, p) => acc + ((p.capacity_bytes || 0) - (p.allocatable_bytes || 0)), 0) / 1024 ** 3);
    const vmHistory = derived(() => [
      vms.length * 0.8,
      vms.length * 0.85,
      vms.length * 0.9,
      vms.length * 0.88,
      vms.length * 0.92,
      vms.length * 0.95,
      vms.length
    ]);
    const storageHistory = derived(() => [
      usedStorageGB() * 0.7,
      usedStorageGB() * 0.75,
      usedStorageGB() * 0.8,
      usedStorageGB() * 0.78,
      usedStorageGB() * 0.85,
      usedStorageGB() * 0.9,
      usedStorageGB()
    ]);
    const currentNode = derived(getDefaultNode);
    const healthChecks = derived(() => [
      {
        id: "api",
        name: "API Status",
        status: loading ? "pending" : "healthy",
        message: loading ? "Checking..." : "Responding normally",
        lastChecked: lastUpdated.toISOString()
      },
      {
        id: "node",
        name: "Node Status",
        status: currentNode()?.status === "online" ? "healthy" : "warning",
        message: currentNode()?.status === "online" ? `${currentNode().name} online` : "Node unavailable",
        lastChecked: lastUpdated.toISOString()
      },
      {
        id: "storage",
        name: "Storage Health",
        status: totalStorageGB() > 0 ? "healthy" : "warning",
        message: pools.length > 0 ? `${pools.length} pools active` : "No storage pools",
        details: pools.map((p) => `${p.name}: ${((p.capacity_bytes || 0) / 1024 ** 3).toFixed(1)} GB`),
        lastChecked: lastUpdated.toISOString()
      },
      {
        id: "platform",
        name: "Platform",
        status: installState === "ready" ? "healthy" : installState === "bootstrap_required" ? "warning" : "pending",
        message: installState.replace("_", " "),
        lastChecked: lastUpdated.toISOString()
      }
    ]);
    async function loadData() {
      try {
        const [
          vmsData,
          imagesData,
          poolsData,
          networksData,
          eventsData,
          installData
        ] = await Promise.all([
          client.listVMs(),
          client.listImages(),
          client.listStoragePools(),
          client.listNetworks(),
          client.listEvents(),
          client.getInstallStatus()
        ]);
        vms = vmsData;
        images = imagesData;
        pools = poolsData;
        networks = networksData;
        events = eventsData;
        installState = installData.overall_state;
        lastUpdated = /* @__PURE__ */ new Date();
      } catch (e) {
        console.error("Failed to load dashboard data:", e);
        toast.error("Failed to load dashboard data");
      } finally {
        loading = false;
      }
    }
    function handleRefresh() {
      loading = true;
      loadData();
    }
    function handleFilterChange(filter) {
      console.log("Filter changed:", filter);
    }
    function handleClearEvents() {
      toast.info("Clear events functionality would be implemented here");
    }
    function handleMarkAllRead() {
      toast.info("Mark all as read functionality would be implemented here");
    }
    function handleVMCreated() {
      showCreateVM = false;
      loadData();
      toast.success("VM created successfully");
    }
    function handleImageImported() {
      showImportImage = false;
      loadData();
      toast.success("Image import started");
    }
    function handleNetworkCreated() {
      showCreateNetwork = false;
      loadData();
      toast.success("Network created successfully");
    }
    onDestroy(() => {
    });
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      head("1uha8ag", $$renderer3, ($$renderer4) => {
        $$renderer4.title(($$renderer5) => {
          $$renderer5.push(`<title>Dashboard | CHV</title>`);
        });
      });
      $$renderer3.push(`<div class="space-y-6"><div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4"><div><h1 class="text-2xl font-bold text-slate-900">Dashboard</h1> <p class="text-sm text-slate-500 mt-1">${escape_html(currentNode()?.name || "Datacenter")} overview and system status</p></div> <div class="flex items-center gap-2"><span class="text-xs text-slate-500">Last updated: ${escape_html(lastUpdated.toLocaleTimeString())}</span> <button class="p-2 text-slate-400 hover:text-slate-600 hover:bg-slate-100 rounded-lg transition-colors" aria-label="Refresh data"${attr("disabled", loading, true)}>`);
      Refresh_cw($$renderer3, { size: 16, class: loading ? "animate-spin" : "" });
      $$renderer3.push(`<!----></button></div></div> `);
      if (loading && vms.length === 0) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<div class="flex items-center justify-center h-96"><div class="flex items-center gap-3 text-slate-500">`);
        Loader_circle($$renderer3, { class: "animate-spin", size: 24 });
        $$renderer3.push(`<!----> <span>Loading dashboard...</span></div></div>`);
      } else {
        $$renderer3.push("<!--[-1-->");
        $$renderer3.push(`<section class="grid gap-4 grid-cols-1 sm:grid-cols-2 lg:grid-cols-4">`);
        ResourceCard($$renderer3, {
          title: "Node",
          value: currentNode()?.name || "Unknown",
          subtitle: currentNode()?.hostname || "",
          icon: Server,
          iconColor: "slate",
          loading: loading && vms.length === 0,
          href: "/nodes"
        });
        $$renderer3.push(`<!----> `);
        ResourceCard($$renderer3, {
          title: "Virtual Machines",
          value: vms.length,
          subtitle: runningVMs() + " running, " + stoppedVMs() + " stopped",
          icon: Cpu,
          iconColor: "blue",
          trend: vms.length > 0 ? "up" : "neutral",
          trendValue: vms.length > 0 ? "+1 this week" : void 0,
          sparklineData: vmHistory(),
          loading: loading && vms.length === 0,
          href: "/vms"
        });
        $$renderer3.push(`<!----> `);
        ResourceCard($$renderer3, {
          title: "Storage",
          value: pools.length,
          subtitle: usedStorageGB().toFixed(1) + " GB of " + totalStorageGB().toFixed(1) + " GB used",
          icon: Database,
          iconColor: "amber",
          progress: totalStorageGB() > 0 ? {
            value: usedStorageGB(),
            max: totalStorageGB(),
            label: "Usage"
          } : void 0,
          sparklineData: storageHistory(),
          loading: loading && vms.length === 0,
          href: "/storage"
        });
        $$renderer3.push(`<!----> `);
        ResourceCard($$renderer3, {
          title: "Resources",
          value: images.length + networks.length,
          subtitle: images.length + " images, " + networks.length + " networks",
          icon: Activity,
          iconColor: "purple",
          loading: loading && vms.length === 0,
          href: "/resources"
        });
        $$renderer3.push(`<!----></section> <section class="bg-white rounded-lg border border-slate-200 p-4"><h3 class="text-sm font-medium text-slate-700 mb-3">Quick Actions</h3> <div class="flex flex-wrap gap-2">`);
        Button($$renderer3, {
          variant: "primary",
          size: "sm",
          onclick: () => showCreateVM = true,
          disabled: false,
          children: ($$renderer4) => {
            Plus($$renderer4, { size: 16 });
            $$renderer4.push(`<!----> Create VM`);
          },
          $$slots: { default: true }
        });
        $$renderer3.push(`<!----> `);
        Button($$renderer3, {
          variant: "secondary",
          size: "sm",
          onclick: () => showImportImage = true,
          disabled: false,
          children: ($$renderer4) => {
            Download($$renderer4, { size: 16 });
            $$renderer4.push(`<!----> Import Image`);
          },
          $$slots: { default: true }
        });
        $$renderer3.push(`<!----> `);
        Button($$renderer3, {
          variant: "secondary",
          size: "sm",
          onclick: () => showCreateNetwork = true,
          disabled: false,
          children: ($$renderer4) => {
            Network($$renderer4, { size: 16 });
            $$renderer4.push(`<!----> Add Network`);
          },
          $$slots: { default: true }
        });
        $$renderer3.push(`<!----> `);
        Button($$renderer3, {
          variant: "secondary",
          size: "sm",
          onclick: () => goto(),
          disabled: false,
          children: ($$renderer4) => {
            Hard_drive($$renderer4, { size: 16 });
            $$renderer4.push(`<!----> Add Storage`);
          },
          $$slots: { default: true }
        });
        $$renderer3.push(`<!----> `);
        Button($$renderer3, {
          variant: "ghost",
          size: "sm",
          onclick: () => goto(),
          children: ($$renderer4) => {
            Settings($$renderer4, { size: 16 });
            $$renderer4.push(`<!----> Platform Settings`);
          },
          $$slots: { default: true }
        });
        $$renderer3.push(`<!----></div></section> <div class="grid gap-6 lg:grid-cols-3"><div class="lg:col-span-2">`);
        EventList($$renderer3, {
          events,
          loading,
          onFilter: handleFilterChange,
          onClear: handleClearEvents,
          onMarkAllRead: handleMarkAllRead
        });
        $$renderer3.push(`<!----></div> <div class="space-y-6">`);
        HealthStatus($$renderer3, {
          checks: healthChecks(),
          loading,
          onRefresh: handleRefresh,
          lastUpdated
        });
        $$renderer3.push(`<!----> <div class="card"><div class="px-5 py-4 border-b border-slate-100"><h3 class="font-semibold text-slate-900">Resource Usage</h3> <p class="text-xs text-slate-500 mt-0.5">Across all VMs</p></div> <div class="p-5 space-y-4"><div><div class="flex items-center justify-between text-sm mb-1.5"><span class="text-slate-600 flex items-center gap-2">`);
        Cpu($$renderer3, { size: 14, class: "text-slate-400" });
        $$renderer3.push(`<!----> CPU Cores</span> <span class="font-medium text-slate-700">${escape_html(totalVcpus())}</span></div> <div class="w-full bg-slate-100 rounded-full h-2"><div class="h-2 rounded-full bg-gradient-to-r from-blue-500 to-blue-600 transition-all duration-500"${attr_style(`width: ${stringify(Math.min(100, totalVcpus() / 32 * 100))}%`)}></div></div> <p class="text-xs text-slate-400 mt-1">${escape_html(totalVcpus())} of 32 vCPUs allocated</p></div> <div><div class="flex items-center justify-between text-sm mb-1.5"><span class="text-slate-600 flex items-center gap-2">`);
        Activity($$renderer3, { size: 14, class: "text-slate-400" });
        $$renderer3.push(`<!----> Memory</span> <span class="font-medium text-slate-700">${escape_html(totalMemoryGB().toFixed(1))} GB</span></div> <div class="w-full bg-slate-100 rounded-full h-2"><div class="h-2 rounded-full bg-gradient-to-r from-purple-500 to-purple-600 transition-all duration-500"${attr_style(`width: ${stringify(Math.min(100, totalMemoryGB() / 64 * 100))}%`)}></div></div> <p class="text-xs text-slate-400 mt-1">${escape_html(totalMemoryGB().toFixed(1))} of 64 GB allocated</p></div> `);
        if (pools.length > 0) {
          $$renderer3.push("<!--[0-->");
          $$renderer3.push(`<div class="pt-2 border-t border-slate-100"><span class="text-xs font-medium text-slate-500 uppercase tracking-wider">Storage by Pool</span> <div class="mt-2 space-y-2"><!--[-->`);
          const each_array = ensure_array_like(pools.slice(0, 3));
          for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
            let pool = each_array[$$index];
            const used = ((pool.capacity_bytes || 0) - (pool.allocatable_bytes || 0)) / 1024 ** 3;
            const total = (pool.capacity_bytes || 0) / 1024 ** 3;
            const pct = total > 0 ? used / total * 100 : 0;
            $$renderer3.push(`<div><div class="flex items-center justify-between text-xs mb-1"><span class="text-slate-600 truncate max-w-[120px]">${escape_html(pool.name)}</span> <span class="text-slate-500">${escape_html(pct.toFixed(0))}%</span></div> <div class="w-full bg-slate-100 rounded-full h-1.5"><div class="h-1.5 rounded-full bg-gradient-to-r from-amber-500 to-orange-500 transition-all duration-500"${attr_style(`width: ${stringify(pct)}%`)}></div></div></div>`);
          }
          $$renderer3.push(`<!--]--></div></div>`);
        } else {
          $$renderer3.push("<!--[-1-->");
        }
        $$renderer3.push(`<!--]--></div></div></div></div>`);
      }
      $$renderer3.push(`<!--]--></div> `);
      CreateVMModal($$renderer3, {
        onSuccess: handleVMCreated,
        images,
        pools,
        networks,
        get open() {
          return showCreateVM;
        },
        set open($$value) {
          showCreateVM = $$value;
          $$settled = false;
        }
      });
      $$renderer3.push(`<!----> `);
      ImportImageModal($$renderer3, {
        onSuccess: handleImageImported,
        get open() {
          return showImportImage;
        },
        set open($$value) {
          showImportImage = $$value;
          $$settled = false;
        }
      });
      $$renderer3.push(`<!----> `);
      CreateNetworkModal($$renderer3, {
        onSuccess: handleNetworkCreated,
        get open() {
          return showCreateNetwork;
        },
        set open($$value) {
          showCreateNetwork = $$value;
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
