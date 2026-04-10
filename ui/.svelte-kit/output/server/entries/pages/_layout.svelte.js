import { s as sanitize_props, a as spread_props, b as slot, c as attr, e as escape_html, d as ensure_array_like, f as derived, g as attr_class, h as attr_style, i as stringify, j as store_get, u as unsubscribe_stores, k as bind_props } from "../../chunks/renderer.js";
import "@sveltejs/kit/internal";
import "../../chunks/exports.js";
import "../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../chunks/root.js";
import { g as goto } from "../../chunks/client.js";
import { p as page } from "../../chunks/stores.js";
import { c as createAPIClient } from "../../chunks/client2.js";
import { t as toast } from "../../chunks/toast.js";
import { V as VisuallyHidden } from "../../chunks/VisuallyHidden.js";
import { D as Database } from "../../chunks/database.js";
import { C as Chevron_down } from "../../chunks/chevron-down.js";
import { C as Chevron_right } from "../../chunks/chevron-right.js";
import { C as Circle } from "../../chunks/circle.js";
import { I as Icon } from "../../chunks/Icon.js";
import { S as Settings } from "../../chunks/settings.js";
import { A as Activity } from "../../chunks/activity.js";
import { N as Network } from "../../chunks/network.js";
import { H as Hard_drive } from "../../chunks/hard-drive.js";
import { I as Image } from "../../chunks/image.js";
import { S as Server } from "../../chunks/server.js";
import "clsx";
import { X } from "../../chunks/x.js";
import { C as Clock } from "../../chunks/clock.js";
import { h as html } from "../../chunks/html.js";
import Fuse from "fuse.js";
import { P as Plus } from "../../chunks/plus.js";
import { D as Download } from "../../chunks/download.js";
import { R as Refresh_cw } from "../../chunks/refresh-cw.js";
import { g as getDefaultNode } from "../../chunks/nodes.js";
function Chart_column($$renderer, $$props) {
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
    ["path", { "d": "M3 3v16a2 2 0 0 0 2 2h16" }],
    ["path", { "d": "M18 17V9" }],
    ["path", { "d": "M13 17V5" }],
    ["path", { "d": "M8 17v-3" }]
  ];
  Icon($$renderer, spread_props([
    { name: "chart-column" },
    $$sanitized_props,
    {
      /**
       * @component @name ChartColumn
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNMyAzdjE2YTIgMiAwIDAgMCAyIDJoMTYiIC8+CiAgPHBhdGggZD0iTTE4IDE3VjkiIC8+CiAgPHBhdGggZD0iTTEzIDE3VjUiIC8+CiAgPHBhdGggZD0iTTggMTd2LTMiIC8+Cjwvc3ZnPgo=) - https://lucide.dev/icons/chart-column
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
function Command($$renderer, $$props) {
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
        "d": "M15 6v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3"
      }
    ]
  ];
  Icon($$renderer, spread_props([
    { name: "command" },
    $$sanitized_props,
    {
      /**
       * @component @name Command
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNMTUgNnYxMmEzIDMgMCAxIDAgMy0zSDZhMyAzIDAgMSAwIDMgM1Y2YTMgMyAwIDEgMC0zIDNoMTJhMyAzIDAgMSAwLTMtMyIgLz4KPC9zdmc+Cg==) - https://lucide.dev/icons/command
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
function File_text($$renderer, $$props) {
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
        "d": "M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"
      }
    ],
    ["path", { "d": "M14 2v5a1 1 0 0 0 1 1h5" }],
    ["path", { "d": "M10 9H8" }],
    ["path", { "d": "M16 13H8" }],
    ["path", { "d": "M16 17H8" }]
  ];
  Icon($$renderer, spread_props([
    { name: "file-text" },
    $$sanitized_props,
    {
      /**
       * @component @name FileText
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNNiAyMmEyIDIgMCAwIDEtMi0yVjRhMiAyIDAgMCAxIDItMmg4YTIuNCAyLjQgMCAwIDEgMS43MDQuNzA2bDMuNTg4IDMuNTg4QTIuNCAyLjQgMCAwIDEgMjAgOHYxMmEyIDIgMCAwIDEtMiAyeiIgLz4KICA8cGF0aCBkPSJNMTQgMnY1YTEgMSAwIDAgMCAxIDFoNSIgLz4KICA8cGF0aCBkPSJNMTAgOUg4IiAvPgogIDxwYXRoIGQ9Ik0xNiAxM0g4IiAvPgogIDxwYXRoIGQ9Ik0xNiAxN0g4IiAvPgo8L3N2Zz4K) - https://lucide.dev/icons/file-text
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
function Folder_tree($$renderer, $$props) {
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
        "d": "M20 10a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1h-2.5a1 1 0 0 1-.8-.4l-.9-1.2A1 1 0 0 0 15 3h-2a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1Z"
      }
    ],
    [
      "path",
      {
        "d": "M20 21a1 1 0 0 0 1-1v-3a1 1 0 0 0-1-1h-2.9a1 1 0 0 1-.88-.55l-.42-.85a1 1 0 0 0-.92-.6H13a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1Z"
      }
    ],
    ["path", { "d": "M3 5a2 2 0 0 0 2 2h3" }],
    ["path", { "d": "M3 3v13a2 2 0 0 0 2 2h3" }]
  ];
  Icon($$renderer, spread_props([
    { name: "folder-tree" },
    $$sanitized_props,
    {
      /**
       * @component @name FolderTree
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNMjAgMTBhMSAxIDAgMCAwIDEtMVY2YTEgMSAwIDAgMC0xLTFoLTIuNWExIDEgMCAwIDEtLjgtLjRsLS45LTEuMkExIDEgMCAwIDAgMTUgM2gtMmExIDEgMCAwIDAtMSAxdjVhMSAxIDAgMCAwIDEgMVoiIC8+CiAgPHBhdGggZD0iTTIwIDIxYTEgMSAwIDAgMCAxLTF2LTNhMSAxIDAgMCAwLTEtMWgtMi45YTEgMSAwIDAgMS0uODgtLjU1bC0uNDItLjg1YTEgMSAwIDAgMC0uOTItLjZIMTNhMSAxIDAgMCAwLTEgMXY1YTEgMSAwIDAgMCAxIDFaIiAvPgogIDxwYXRoIGQ9Ik0zIDVhMiAyIDAgMCAwIDIgMmgzIiAvPgogIDxwYXRoIGQ9Ik0zIDN2MTNhMiAyIDAgMCAwIDIgMmgzIiAvPgo8L3N2Zz4K) - https://lucide.dev/icons/folder-tree
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
function House($$renderer, $$props) {
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
      { "d": "M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8" }
    ],
    [
      "path",
      {
        "d": "M3 10a2 2 0 0 1 .709-1.528l7-6a2 2 0 0 1 2.582 0l7 6A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"
      }
    ]
  ];
  Icon($$renderer, spread_props([
    { name: "house" },
    $$sanitized_props,
    {
      /**
       * @component @name House
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNMTUgMjF2LThhMSAxIDAgMCAwLTEtMWgtNGExIDEgMCAwIDAtMSAxdjgiIC8+CiAgPHBhdGggZD0iTTMgMTBhMiAyIDAgMCAxIC43MDktMS41MjhsNy02YTIgMiAwIDAgMSAyLjU4MiAwbDcgNkEyIDIgMCAwIDEgMjEgMTB2OWEyIDIgMCAwIDEtMiAySDVhMiAyIDAgMCAxLTItMnoiIC8+Cjwvc3ZnPgo=) - https://lucide.dev/icons/house
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
function Layout_grid($$renderer, $$props) {
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
      { "width": "7", "height": "7", "x": "3", "y": "3", "rx": "1" }
    ],
    [
      "rect",
      { "width": "7", "height": "7", "x": "14", "y": "3", "rx": "1" }
    ],
    [
      "rect",
      { "width": "7", "height": "7", "x": "14", "y": "14", "rx": "1" }
    ],
    [
      "rect",
      { "width": "7", "height": "7", "x": "3", "y": "14", "rx": "1" }
    ]
  ];
  Icon($$renderer, spread_props([
    { name: "layout-grid" },
    $$sanitized_props,
    {
      /**
       * @component @name LayoutGrid
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cmVjdCB3aWR0aD0iNyIgaGVpZ2h0PSI3IiB4PSIzIiB5PSIzIiByeD0iMSIgLz4KICA8cmVjdCB3aWR0aD0iNyIgaGVpZ2h0PSI3IiB4PSIxNCIgeT0iMyIgcng9IjEiIC8+CiAgPHJlY3Qgd2lkdGg9IjciIGhlaWdodD0iNyIgeD0iMTQiIHk9IjE0IiByeD0iMSIgLz4KICA8cmVjdCB3aWR0aD0iNyIgaGVpZ2h0PSI3IiB4PSIzIiB5PSIxNCIgcng9IjEiIC8+Cjwvc3ZnPgo=) - https://lucide.dev/icons/layout-grid
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
function Search($$renderer, $$props) {
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
    ["path", { "d": "m21 21-4.34-4.34" }],
    ["circle", { "cx": "11", "cy": "11", "r": "8" }]
  ];
  Icon($$renderer, spread_props([
    { name: "search" },
    $$sanitized_props,
    {
      /**
       * @component @name Search
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJtMjEgMjEtNC4zNC00LjM0IiAvPgogIDxjaXJjbGUgY3g9IjExIiBjeT0iMTEiIHI9IjgiIC8+Cjwvc3ZnPgo=) - https://lucide.dev/icons/search
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
function Wifi($$renderer, $$props) {
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
    ["path", { "d": "M12 20h.01" }],
    ["path", { "d": "M2 8.82a15 15 0 0 1 20 0" }],
    ["path", { "d": "M5 12.859a10 10 0 0 1 14 0" }],
    ["path", { "d": "M8.5 16.429a5 5 0 0 1 7 0" }]
  ];
  Icon($$renderer, spread_props([
    { name: "wifi" },
    $$sanitized_props,
    {
      /**
       * @component @name Wifi
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNMTIgMjBoLjAxIiAvPgogIDxwYXRoIGQ9Ik0yIDguODJhMTUgMTUgMCAwIDEgMjAgMCIgLz4KICA8cGF0aCBkPSJNNSAxMi44NTlhMTAgMTAgMCAwIDEgMTQgMCIgLz4KICA8cGF0aCBkPSJNOC41IDE2LjQyOWE1IDUgMCAwIDEgNyAwIiAvPgo8L3N2Zz4K) - https://lucide.dev/icons/wifi
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
function Zap($$renderer, $$props) {
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
        "d": "M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z"
      }
    ]
  ];
  Icon($$renderer, spread_props([
    { name: "zap" },
    $$sanitized_props,
    {
      /**
       * @component @name Zap
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNNCAxNGExIDEgMCAwIDEtLjc4LTEuNjNsOS45LTEwLjJhLjUuNSAwIDAgMSAuODYuNDZsLTEuOTIgNi4wMkExIDEgMCAwIDAgMTMgMTBoN2ExIDEgMCAwIDEgLjc4IDEuNjNsLTkuOSAxMC4yYS41LjUgMCAwIDEtLjg2LS40NmwxLjkyLTYuMDJBMSAxIDAgMCAwIDExIDE0eiIgLz4KPC9zdmc+Cg==) - https://lucide.dev/icons/zap
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
function UserMenu($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { userName = "Administrator", userEmail = "admin@chv.local" } = $$props;
    let isOpen = false;
    $$renderer2.push(`<div class="relative"><button type="button" class="w-full flex items-center gap-3 px-2 py-2 rounded hover:bg-slate-800/50 cursor-pointer transition-colors duration-150 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[#e57035] focus-visible:outline-offset-2" aria-haspopup="true"${attr("aria-expanded", isOpen)} aria-controls="user-menu"><div class="w-8 h-8 rounded-full bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center text-white text-xs font-semibold shrink-0">${escape_html(userName.charAt(0).toUpperCase())}</div> <div class="flex-1 min-w-0 text-left"><div class="text-sm font-medium text-white truncate">${escape_html(userName)}</div> <div class="text-[10px] text-slate-500 truncate">${escape_html(userEmail)}</div></div></button> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div>`);
  });
}
function TreeNavigation($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    var $$store_subs;
    let { nodes = [] } = $$props;
    function generateTree(nodes2) {
      const nodeChildren = nodes2.map((node) => ({
        id: node.id,
        type: "node",
        label: node.name,
        status: node.status,
        expanded: true,
        href: `/nodes/${node.id}`,
        children: [
          {
            id: `${node.id}-vms`,
            type: "resource",
            label: "Virtual Machines",
            icon: "server",
            href: `/nodes/${node.id}/vms`,
            badge: node.resources?.vms ?? 0
          },
          {
            id: `${node.id}-images`,
            type: "resource",
            label: "Images",
            icon: "image",
            href: `/nodes/${node.id}/images`,
            badge: node.resources?.images ?? 0
          },
          {
            id: `${node.id}-storage`,
            type: "resource",
            label: "Storage",
            icon: "hardDrive",
            href: `/nodes/${node.id}/storage`,
            badge: node.resources?.storagePools ?? 0
          },
          {
            id: `${node.id}-networks`,
            type: "resource",
            label: "Networks",
            icon: "network",
            href: `/nodes/${node.id}/networks`,
            badge: node.resources?.networks ?? 0
          }
        ]
      }));
      return [
        {
          id: "datacenter",
          type: "datacenter",
          label: "Datacenter",
          expanded: true,
          icon: "datacenter",
          href: "/",
          children: [
            {
              id: "overview",
              type: "resource",
              label: "Overview",
              icon: "layout",
              href: "/"
            },
            ...nodeChildren.length > 0 ? nodeChildren : [],
            {
              id: "global-images",
              type: "resource",
              label: "Images",
              icon: "image",
              href: "/images"
            },
            {
              id: "global-storage",
              type: "resource",
              label: "Storage",
              icon: "hardDrive",
              href: "/storage"
            },
            {
              id: "global-networks",
              type: "resource",
              label: "Networks",
              icon: "network",
              href: "/networks"
            },
            {
              id: "global-metrics",
              type: "resource",
              label: "Metrics",
              icon: "metrics",
              href: "/metrics"
            }
          ]
        }
      ];
    }
    let treeNodes = derived(() => generateTree(nodes));
    let expandedNodes = /* @__PURE__ */ new Set(["datacenter"]);
    let currentPath = derived(() => store_get($$store_subs ??= {}, "$page", page).url.pathname);
    let focusedNodeId = null;
    createAPIClient();
    function isExpanded(node) {
      return node.expanded || expandedNodes.has(node.id);
    }
    function isActive(href) {
      if (!href) return false;
      if (href === "/") return currentPath() === "/";
      return currentPath().startsWith(href);
    }
    function isNodeActive(node) {
      if (!node.href) return false;
      return isActive(node.href);
    }
    function getIcon(iconName) {
      switch (iconName) {
        case "server":
          return Server;
        case "image":
          return Image;
        case "hardDrive":
          return Hard_drive;
        case "network":
          return Network;
        case "activity":
          return Activity;
        case "settings":
          return Settings;
        case "datacenter":
          return Database;
        case "folder":
          return Folder_tree;
        case "layout":
          return Layout_grid;
        case "metrics":
          return Chart_column;
        default:
          return Circle;
      }
    }
    function getStatusColor(status) {
      switch (status) {
        case "online":
          return "text-green-500";
        case "offline":
          return "text-red-500";
        case "warning":
          return "text-yellow-500";
        case "maintenance":
          return "text-orange-500";
        default:
          return "text-slate-400";
      }
    }
    function getStatusLabel(status) {
      switch (status) {
        case "online":
          return "Online";
        case "offline":
          return "Offline";
        case "warning":
          return "Warning";
        case "maintenance":
          return "Maintenance";
        default:
          return "Unknown";
      }
    }
    function treeNodeItem($$renderer3, node, level) {
      const expanded = isExpanded(node);
      const active = isNodeActive(node);
      const hasChildren = node.children && node.children.length > 0;
      const IconComponent = getIcon(node.icon);
      const paddingLeft = `${0.5 + level * 0.75}rem`;
      $$renderer3.push(`<li class="select-none" role="none"><div${attr_class(`group flex items-center relative mx-2 rounded-md transition-all duration-150 ${stringify(active ? "bg-[#e57035]/15 text-[#ff9a65]" : "hover:bg-white/5 hover:text-slate-100")}`)}${attr_style(`margin-left: ${stringify(paddingLeft)}; margin-right: 0.5rem;`)}>`);
      if (active) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<div class="absolute left-0 top-1/2 -translate-y-1/2 w-0.5 h-6 bg-[#e57035] rounded-full" aria-hidden="true"></div>`);
      } else {
        $$renderer3.push("<!--[-1-->");
      }
      $$renderer3.push(`<!--]--> <a${attr("data-tree-node", node.id)}${attr("href", node.href || "#")} class="flex items-center gap-2 flex-1 px-3 py-2 text-sm focus-visible:outline-none svelte-2ilt8a"${attr_style(`padding-left: calc(0.5rem + ${stringify(active ? "2px" : "0")});`)} role="treeitem"${attr("aria-expanded", hasChildren ? expanded : void 0)}${attr("aria-selected", active)}${attr("aria-current", active ? "page" : void 0)}${attr("tabindex", focusedNodeId === node.id || active ? 0 : -1)}>`);
      if (hasChildren) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<button type="button" class="w-5 h-5 flex items-center justify-center rounded hover:bg-white/10 transition-colors duration-150 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[#e57035] focus-visible:outline-offset-0" tabindex="-1"${attr("aria-label", expanded ? `Collapse ${node.label}` : `Expand ${node.label}`)}${attr("aria-expanded", expanded)}>`);
        if (expanded) {
          $$renderer3.push("<!--[0-->");
          Chevron_down($$renderer3, {
            size: 14,
            class: "text-slate-400 transition-transform duration-150",
            "aria-hidden": "true"
          });
        } else {
          $$renderer3.push("<!--[-1-->");
          Chevron_right($$renderer3, {
            size: 14,
            class: "text-slate-400 transition-transform duration-150",
            "aria-hidden": "true"
          });
        }
        $$renderer3.push(`<!--]--></button>`);
      } else {
        $$renderer3.push("<!--[-1-->");
        $$renderer3.push(`<span class="w-5" aria-hidden="true"></span>`);
      }
      $$renderer3.push(`<!--]--> <span class="flex items-center justify-center w-5 shrink-0">`);
      if (node.type === "node") {
        $$renderer3.push("<!--[0-->");
        Circle($$renderer3, {
          size: 10,
          class: `${stringify(getStatusColor(node.status))} ${stringify(active ? "animate-pulse" : "")}`,
          fill: "currentColor",
          "aria-hidden": "true"
        });
        $$renderer3.push(`<!----> `);
        VisuallyHidden($$renderer3, {
          children: ($$renderer4) => {
            $$renderer4.push(`<!---->${escape_html(getStatusLabel(node.status))}`);
          }
        });
        $$renderer3.push(`<!---->`);
      } else {
        $$renderer3.push("<!--[-1-->");
        if (IconComponent) {
          $$renderer3.push("<!--[-->");
          IconComponent($$renderer3, {
            size: 16,
            class: active ? "text-orange-400" : "text-slate-400",
            "aria-hidden": "true"
          });
          $$renderer3.push("<!--]-->");
        } else {
          $$renderer3.push("<!--[!-->");
          $$renderer3.push("<!--]-->");
        }
      }
      $$renderer3.push(`<!--]--></span> <span class="flex-1 truncate font-medium">${escape_html(node.label)}</span> `);
      if (node.badge !== void 0 && node.badge > 0) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<span class="bg-[#e57035] text-white text-[10px] px-1.5 py-0.5 rounded min-w-[1.25rem] text-center font-semibold shadow-sm ml-2 shrink-0"${attr("aria-label", `${stringify(node.badge)} items`)}>${escape_html(node.badge)}</span>`);
      } else {
        $$renderer3.push("<!--[-1-->");
      }
      $$renderer3.push(`<!--]--></a></div> `);
      if (hasChildren && expanded) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<ul class="mt-0.5 overflow-hidden" role="group"${attr("aria-label", `${stringify(node.label)} children`)}><!--[-->`);
        const each_array = ensure_array_like(node.children);
        for (let $$index_1 = 0, $$length = each_array.length; $$index_1 < $$length; $$index_1++) {
          let child = each_array[$$index_1];
          treeNodeItem($$renderer3, child, level + 1);
        }
        $$renderer3.push(`<!--]--></ul>`);
      } else {
        $$renderer3.push("<!--[-1-->");
      }
      $$renderer3.push(`<!--]--></li>`);
    }
    $$renderer2.push(`<aside class="h-screen flex flex-col bg-[#252532] text-slate-300 w-64 border-r border-[#1e1e28] svelte-2ilt8a" role="navigation" aria-label="Main navigation"><header class="h-14 flex items-center px-4 border-b border-[#1e1e28] bg-[#1e1e28]"><div class="flex items-center gap-3"><div class="w-8 h-8 rounded bg-gradient-to-br from-[#e57035] to-[#d14a28] flex items-center justify-center shadow-lg shadow-orange-900/20">`);
    Database($$renderer2, { class: "text-white", size: 18, "aria-hidden": "true" });
    $$renderer2.push(`<!----></div> <div><div class="text-sm font-semibold text-white">CHV Manager</div> <div class="text-[10px] text-slate-500">Virtualization Platform</div></div></div></header> <nav class="flex-1 overflow-y-auto py-2 svelte-2ilt8a" aria-label="Resource tree"><ul role="tree" aria-label="Navigation tree"><!--[-->`);
    const each_array_1 = ensure_array_like(treeNodes());
    for (let $$index = 0, $$length = each_array_1.length; $$index < $$length; $$index++) {
      let node = each_array_1[$$index];
      treeNodeItem($$renderer2, node, 0);
    }
    $$renderer2.push(`<!--]--></ul></nav> <footer class="border-t border-[#1e1e28] p-3 bg-[#1e1e28]">`);
    UserMenu($$renderer2, { userName: "Administrator", userEmail: "admin@chv.local" });
    $$renderer2.push(`<!----></footer></aside>`);
    if ($$store_subs) unsubscribe_stores($$store_subs);
  });
}
function Breadcrumbs($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { items } = $$props;
    $$renderer2.push(`<nav aria-label="Breadcrumb" class="flex items-center min-w-0"><ol class="flex items-center gap-1.5 min-w-0"><!--[-->`);
    const each_array = ensure_array_like(items);
    for (let index = 0, $$length = each_array.length; index < $$length; index++) {
      let item = each_array[index];
      $$renderer2.push(`<li${attr_class(`flex items-center gap-1.5 ${stringify(index === items.length - 1 ? "min-w-0" : "shrink-0")}`)}>`);
      if (index === 0) {
        $$renderer2.push("<!--[0-->");
        if (item.href) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<a${attr("href", item.href)} class="p-1 rounded hover:bg-white/10 transition-colors duration-150 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[#e57035] focus-visible:outline-offset-2" aria-label="Home">`);
          House($$renderer2, { size: 16, class: "text-slate-400" });
          $$renderer2.push(`<!----></a>`);
        } else {
          $$renderer2.push("<!--[-1-->");
          $$renderer2.push(`<span class="p-1">`);
          House($$renderer2, { size: 16, class: "text-slate-400" });
          $$renderer2.push(`<!----></span>`);
        }
        $$renderer2.push(`<!--]-->`);
      } else {
        $$renderer2.push("<!--[-1-->");
        Chevron_right($$renderer2, {
          size: 14,
          class: "text-slate-600 shrink-0",
          "aria-hidden": "true"
        });
        $$renderer2.push(`<!----> `);
        if (index === items.length - 1) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<span class="text-[#e57035] font-medium truncate text-sm" aria-current="page">${escape_html(item.label)}</span>`);
        } else {
          $$renderer2.push("<!--[-1-->");
          $$renderer2.push(`<a${attr("href", item.href || "#")} class="text-slate-400 hover:text-slate-200 transition-colors duration-150 text-sm whitespace-nowrap focus-visible:outline focus-visible:outline-2 focus-visible:outline-[#e57035] focus-visible:outline-offset-2">${escape_html(item.label)}</a>`);
        }
        $$renderer2.push(`<!--]-->`);
      }
      $$renderer2.push(`<!--]--></li>`);
    }
    $$renderer2.push(`<!--]--></ol></nav>`);
  });
}
function Toast($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { type, message } = $$props;
    const styles = {
      success: {
        bg: "bg-[#F0F9F0]",
        border: "border-l-[#54B435]",
        iconColor: "text-[#54B435]",
        label: "Success"
      },
      error: {
        bg: "bg-[#FFF0F0]",
        border: "border-l-[#E60000]",
        iconColor: "text-[#E60000]",
        label: "Error"
      },
      info: {
        bg: "bg-[#E8F4FC]",
        border: "border-l-[#0066CC]",
        iconColor: "text-[#0066CC]",
        label: "Information"
      }
    };
    let style = derived(() => styles[type]);
    $$renderer2.push(`<div${attr_class(`w-[320px] max-w-full rounded shadow-[0_4px_12px_rgba(0,0,0,0.15)] border-l-4 flex items-start gap-3 p-4 ${stringify(style().bg)} ${stringify(style().border)}`, "svelte-1cpok13")} role="alert"${attr("aria-live", type === "error" ? "assertive" : "polite")} aria-atomic="true"><div${attr_class(`flex-shrink-0 ${stringify(style().iconColor)}`, "svelte-1cpok13")} aria-hidden="true">`);
    if (type === "success") {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"></path></svg>`);
    } else if (type === "error") {
      $$renderer2.push("<!--[1-->");
      $$renderer2.push(`<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 16h.01"></path><path d="M12 8v4"></path><path d="M15.312 2a2 2 0 0 1 1.414.586l4.688 4.688A2 2 0 0 1 22 8.688v6.624a2 2 0 0 1-.586 1.414l-4.688 4.688a2 2 0 0 1-1.414.586H8.688a2 2 0 0 1-1.414-.586l-4.688-4.688A2 2 0 0 1 2 15.312V8.688a2 2 0 0 1 .586-1.414l4.688-4.688A2 2 0 0 1 8.688 2z"></path></svg>`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><path d="M12 16v-4"></path><path d="M12 8h.01"></path></svg>`);
    }
    $$renderer2.push(`<!--]--></div> <div class="flex-1 text-sm text-ink leading-5"><span class="sr-only svelte-1cpok13">${escape_html(style().label)}:</span> ${escape_html(message)}</div> <button class="flex-shrink-0 p-1 rounded hover:bg-black/5 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-1"${attr("aria-label", `Dismiss ${stringify(style().label.toLowerCase())} notification`)} type="button"><svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-muted" aria-hidden="true"><path d="M18 6 6 18"></path><path d="m6 6 12 12"></path></svg></button></div>`);
  });
}
function ToastContainer($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    var $$store_subs;
    $$renderer2.push(`<div class="toast-container svelte-cqwvc2" role="region" aria-label="Notifications" aria-live="polite" aria-atomic="false"><!--[-->`);
    const each_array = ensure_array_like(
      // Announce toast changes for screen readers
      store_get($$store_subs ??= {}, "$toast", toast).toasts
    );
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      let t = each_array[$$index];
      $$renderer2.push(`<div class="toast-wrapper svelte-cqwvc2">`);
      Toast($$renderer2, { id: t.id, type: t.type, message: t.message });
      $$renderer2.push(`<!----></div>`);
    }
    $$renderer2.push(`<!--]--></div>`);
    if ($$store_subs) unsubscribe_stores($$store_subs);
  });
}
let recentSearches = [];
let searchQuery = "";
function getGroupedResults(results) {
  const grouped = /* @__PURE__ */ new Map();
  for (const result of results) {
    const type = result.item.type;
    if (!grouped.has(type)) {
      grouped.set(type, []);
    }
    grouped.get(type).push(result);
  }
  return grouped;
}
const typeLabels = {
  vm: "VMs",
  image: "Images",
  network: "Networks",
  storage: "Storage",
  page: "Pages"
};
function getSearchQuery() {
  return searchQuery;
}
function getRecentSearches() {
  return recentSearches;
}
function highlightMatches(text, matches, key) {
  if (!matches) return text;
  const match = matches.find((m) => m.key === key);
  if (!match) return text;
  let result = "";
  let lastIndex = 0;
  const indices = [...match.indices].sort((a, b) => a[0] - b[0]);
  for (const [start, end] of indices) {
    result += text.slice(lastIndex, start);
    result += `<mark class="search-highlight">${text.slice(start, end + 1)}</mark>`;
    lastIndex = end + 1;
  }
  result += text.slice(lastIndex);
  return result;
}
function SearchModal($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { open = false } = $$props;
    let results = [];
    let selectedIndex = 0;
    let isVisible = false;
    let query = derived(getSearchQuery);
    let recent = derived(getRecentSearches);
    let hasQuery = derived(() => query().trim().length > 0);
    let groupedResults = derived(() => getGroupedResults(results));
    let allItems = derived(getAllItems);
    function getAllItems() {
      if (hasQuery()) {
        return results;
      }
      return recent().map((r) => ({ item: r, isRecent: true, score: 0 }));
    }
    let groupedEntries = derived(() => Array.from(groupedResults().entries()));
    function getGlobalIndex(type, indexInGroup) {
      let count = 0;
      for (const [t, typeResults] of groupedEntries()) {
        if (t === type) {
          return count + indexInGroup;
        }
        count += typeResults.length;
      }
      return 0;
    }
    function getIconComponent(type) {
      switch (type) {
        case "vm":
          return Server;
        case "image":
          return Image;
        case "network":
          return Network;
        case "storage":
          return Hard_drive;
        case "page":
          return File_text;
        default:
          return File_text;
      }
    }
    function getIconColor(type) {
      switch (type) {
        case "vm":
          return "text-blue-500";
        case "image":
          return "text-purple-500";
        case "network":
          return "text-green-500";
        case "storage":
          return "text-orange-500";
        case "page":
          return "text-gray-500";
        default:
          return "text-gray-500";
      }
    }
    if (open) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div${attr_class("fixed inset-0 z-50 bg-black/50 flex items-start justify-center pt-[15vh] transition-opacity duration-150", void 0, {
        "opacity-0": !isVisible,
        "opacity-100": isVisible
      })} aria-hidden="true"><div role="dialog" aria-modal="true" tabindex="-1" aria-label="Global search"${attr_class("w-full max-w-2xl mx-4 bg-white rounded-lg shadow-2xl overflow-hidden transition-all duration-150", void 0, {
        "scale-95": !isVisible,
        "scale-100": isVisible,
        "opacity-0": !isVisible,
        "opacity-100": isVisible
      })}><div class="flex items-center gap-3 px-4 py-3 border-b border-gray-200">`);
      Search($$renderer2, { size: 20, class: "text-gray-400 flex-shrink-0" });
      $$renderer2.push(`<!----> <input type="text"${attr("value", query())} placeholder="Search VMs, images, networks..." class="flex-1 bg-transparent text-base outline-none placeholder:text-gray-400" aria-label="Search" aria-autocomplete="list" aria-controls="search-results"${attr("aria-activedescendant", allItems().length > 0 ? `search-item-${selectedIndex}` : void 0)}/> `);
      if (query()) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<button class="p-1 rounded hover:bg-gray-100 text-gray-400 hover:text-gray-600" aria-label="Clear search">`);
        X($$renderer2, { size: 16 });
        $$renderer2.push(`<!----></button>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]--> <kbd class="hidden sm:inline-flex items-center gap-1 px-2 py-1 text-xs font-mono bg-gray-100 text-gray-500 rounded border border-gray-200">ESC</kbd></div> <div id="search-results" class="max-h-[60vh] overflow-y-auto" role="listbox" aria-label="Search results">`);
      if (hasQuery()) {
        $$renderer2.push("<!--[0-->");
        if (results.length === 0) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<div class="px-4 py-8 text-center text-gray-500"><p>No results found for "${escape_html(query())}"</p> <p class="text-sm mt-1 text-gray-400">Try a different search term</p></div>`);
        } else {
          $$renderer2.push("<!--[-1-->");
          $$renderer2.push(`<!--[-->`);
          const each_array = ensure_array_like(groupedEntries());
          for (let $$index_1 = 0, $$length = each_array.length; $$index_1 < $$length; $$index_1++) {
            let [type, typeResults] = each_array[$$index_1];
            $$renderer2.push(`<div class="py-2"><div class="px-4 py-1.5 text-xs font-semibold text-gray-500 uppercase tracking-wider bg-gray-50">${escape_html(typeLabels[type])}</div> <!--[-->`);
            const each_array_1 = ensure_array_like(typeResults);
            for (let i = 0, $$length2 = each_array_1.length; i < $$length2; i++) {
              let result = each_array_1[i];
              const globalIndex = getGlobalIndex(type, i);
              const Icon2 = getIconComponent(type);
              $$renderer2.push(`<button${attr("id", `search-item-${stringify(globalIndex)}`)}${attr("data-index", globalIndex)} role="option"${attr("aria-selected", selectedIndex === globalIndex)}${attr_class("w-full px-4 py-2.5 flex items-center gap-3 text-left hover:bg-gray-100 transition-colors", void 0, { "bg-blue-50": selectedIndex === globalIndex })}>`);
              if (Icon2) {
                $$renderer2.push("<!--[-->");
                Icon2($$renderer2, { size: 18, class: getIconColor(type) });
                $$renderer2.push("<!--]-->");
              } else {
                $$renderer2.push("<!--[!-->");
                $$renderer2.push("<!--]-->");
              }
              $$renderer2.push(` <div class="flex-1 min-w-0"><div class="text-sm font-medium text-gray-900 truncate">${html(highlightMatches(result.item.name, result.matches, "name"))}</div> `);
              if (result.item.description) {
                $$renderer2.push("<!--[0-->");
                $$renderer2.push(`<div class="text-xs text-gray-500 truncate">${html(highlightMatches(result.item.description, result.matches, "description"))}</div>`);
              } else {
                $$renderer2.push("<!--[-1-->");
              }
              $$renderer2.push(`<!--]--></div> `);
              if (result.item.route) {
                $$renderer2.push("<!--[0-->");
                $$renderer2.push(`<span class="text-xs text-gray-400 hidden sm:block">→</span>`);
              } else {
                $$renderer2.push("<!--[-1-->");
              }
              $$renderer2.push(`<!--]--></button>`);
            }
            $$renderer2.push(`<!--]--></div>`);
          }
          $$renderer2.push(`<!--]-->`);
        }
        $$renderer2.push(`<!--]-->`);
      } else if (recent().length > 0) {
        $$renderer2.push("<!--[1-->");
        $$renderer2.push(`<div class="py-2"><div class="px-4 py-1.5 flex items-center justify-between"><span class="text-xs font-semibold text-gray-500 uppercase tracking-wider">Recent</span> <button class="text-xs text-gray-400 hover:text-gray-600">Clear</button></div> <!--[-->`);
        const each_array_2 = ensure_array_like(recent());
        for (let i = 0, $$length = each_array_2.length; i < $$length; i++) {
          let item = each_array_2[i];
          const Icon2 = getIconComponent(item.type);
          $$renderer2.push(`<button${attr("id", `search-item-${stringify(i)}`)}${attr("data-index", i)} role="option"${attr("aria-selected", selectedIndex === i)}${attr_class("w-full px-4 py-2.5 flex items-center gap-3 text-left hover:bg-gray-100 transition-colors", void 0, { "bg-blue-50": selectedIndex === i })}>`);
          Clock($$renderer2, { size: 18, class: "text-gray-400" });
          $$renderer2.push(`<!----> `);
          if (Icon2) {
            $$renderer2.push("<!--[-->");
            Icon2($$renderer2, { size: 18, class: getIconColor(item.type) });
            $$renderer2.push("<!--]-->");
          } else {
            $$renderer2.push("<!--[!-->");
            $$renderer2.push("<!--]-->");
          }
          $$renderer2.push(` <div class="flex-1 min-w-0"><div class="text-sm font-medium text-gray-900 truncate">${escape_html(item.name)}</div> `);
          if (item.description) {
            $$renderer2.push("<!--[0-->");
            $$renderer2.push(`<div class="text-xs text-gray-500 truncate">${escape_html(item.description)}</div>`);
          } else {
            $$renderer2.push("<!--[-1-->");
          }
          $$renderer2.push(`<!--]--></div> <span class="text-xs text-gray-400 hidden sm:block">${escape_html(typeLabels[item.type])}</span></button>`);
        }
        $$renderer2.push(`<!--]--></div>`);
      } else {
        $$renderer2.push("<!--[-1-->");
        $$renderer2.push(`<div class="px-4 py-6"><p class="text-sm text-gray-500 mb-3">Type to search or try:</p> <div class="flex flex-wrap gap-2"><kbd class="px-2 py-1 text-xs bg-gray-100 text-gray-600 rounded border border-gray-200">vm-</kbd> <kbd class="px-2 py-1 text-xs bg-gray-100 text-gray-600 rounded border border-gray-200">image-</kbd> <kbd class="px-2 py-1 text-xs bg-gray-100 text-gray-600 rounded border border-gray-200">network-</kbd></div></div>`);
      }
      $$renderer2.push(`<!--]--></div> <div class="px-4 py-2 bg-gray-50 border-t border-gray-200 flex items-center justify-between text-xs text-gray-500"><div class="flex items-center gap-3"><span class="flex items-center gap-1"><kbd class="px-1.5 py-0.5 bg-white rounded border border-gray-200">↑↓</kbd> <span>Navigate</span></span> <span class="flex items-center gap-1"><kbd class="px-1.5 py-0.5 bg-white rounded border border-gray-200">↵</kbd> <span>Select</span></span></div> <div><span>${escape_html(allItems().length)} item${escape_html(allItems().length !== 1 ? "s" : "")}</span></div></div></div></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]-->`);
    bind_props($$props, { open });
  });
}
function getModifierKey() {
  return "Ctrl";
}
function KeyboardShortcutsHelp($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]-->`);
  });
}
function QuickActions($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { open = false, onClose } = $$props;
    let query = "";
    let selectedIndex = 0;
    let isVisible = false;
    let isClosing = false;
    let recentlyUsed = [];
    const allActions = [
      {
        id: "create-vm",
        title: "Create Virtual Machine",
        description: "Launch a new VM wizard",
        icon: Plus,
        shortcut: ["c"],
        keywords: ["vm", "create", "new", "virtual machine", "launch"],
        section: "VMs",
        action: () => goto()
      },
      {
        id: "go-dashboard",
        title: "Go to Dashboard",
        description: "View system overview",
        icon: House,
        shortcut: ["g", "d"],
        keywords: ["dashboard", "home", "overview", "main"],
        section: "Navigation",
        action: () => goto()
      },
      {
        id: "go-vms",
        title: "Go to Virtual Machines",
        description: "View all VMs",
        icon: Server,
        shortcut: ["g", "v"],
        keywords: ["vms", "virtual machines", "instances"],
        section: "Navigation",
        action: () => goto()
      },
      {
        id: "go-images",
        title: "Go to Images",
        description: "Manage OS images",
        icon: Image,
        shortcut: ["g", "i"],
        keywords: ["images", "os", "templates", "iso"],
        section: "Navigation",
        action: () => goto()
      },
      {
        id: "go-storage",
        title: "Go to Storage",
        description: "Manage storage pools",
        icon: Hard_drive,
        shortcut: ["g", "s"],
        keywords: ["storage", "pools", "disks", "volumes"],
        section: "Navigation",
        action: () => goto()
      },
      {
        id: "go-networks",
        title: "Go to Networks",
        description: "Manage network configuration",
        icon: Network,
        shortcut: ["g", "n"],
        keywords: ["networks", "bridges", "interfaces", "vlan"],
        section: "Navigation",
        action: () => goto()
      },
      {
        id: "import-image",
        title: "Import Image",
        description: "Download an OS image from URL",
        icon: Download,
        keywords: ["import", "download", "image", "os"],
        section: "Images",
        action: () => goto()
      },
      {
        id: "create-network",
        title: "Create Network",
        description: "Add a new network bridge",
        icon: Network,
        keywords: ["network", "create", "bridge", "vlan"],
        section: "Networks",
        action: () => goto()
      },
      {
        id: "create-storage",
        title: "Create Storage Pool",
        description: "Add a new storage pool",
        icon: Hard_drive,
        keywords: ["storage", "pool", "create", "disk"],
        section: "Storage",
        action: () => goto()
      },
      {
        id: "refresh-data",
        title: "Refresh All Data",
        description: "Reload current page data",
        icon: Refresh_cw,
        shortcut: ["r"],
        keywords: ["refresh", "reload", "update", "sync"],
        section: "System",
        action: () => window.location.reload()
      },
      {
        id: "open-settings",
        title: "Open Settings",
        description: "System configuration",
        icon: Settings,
        keywords: ["settings", "config", "preferences"],
        section: "System",
        action: () => goto()
      },
      {
        id: "open-help",
        title: "Keyboard Shortcuts Help",
        description: "View all available shortcuts",
        icon: Command,
        shortcut: ["?"],
        keywords: ["help", "shortcuts", "keyboard", "hotkeys"],
        section: "System",
        action: () => {
          close();
        }
      }
    ];
    const fuse = new Fuse(allActions, {
      keys: [
        { name: "title", weight: 0.5 },
        { name: "description", weight: 0.3 },
        { name: "keywords", weight: 0.2 }
      ],
      threshold: 0.4
    });
    let filteredActions = derived(getFilteredActions);
    function getFilteredActions() {
      if (!query.trim()) {
        const recent = recentlyUsed.map((id) => allActions.find((a) => a.id === id)).filter(Boolean);
        const others = allActions.filter((a) => !recentlyUsed.includes(a.id));
        return [...recent, ...others];
      }
      const results = fuse.search(query);
      return results.map((r) => r.item);
    }
    let groupedActions = derived(() => getGroupedActions(filteredActions()));
    function getGroupedActions(actions) {
      const grouped = /* @__PURE__ */ new Map();
      for (const action of actions) {
        if (!grouped.has(action.section)) {
          grouped.set(action.section, []);
        }
        grouped.get(action.section).push(action);
      }
      return grouped;
    }
    let flatActions = derived(() => getFlatActions(groupedActions()));
    function getFlatActions(grouped) {
      const flat = [];
      for (const actions of grouped.values()) {
        flat.push(...actions);
      }
      return flat;
    }
    function close() {
      if (isClosing) return;
      isClosing = true;
      setTimeout(
        () => {
          isVisible = false;
          open = false;
          query = "";
          selectedIndex = 0;
          isClosing = false;
          onClose?.();
        },
        150
      );
    }
    function getGlobalIndex(sectionIndex, actionIndex) {
      let count = 0;
      const sections = Array.from(groupedActions().entries());
      for (let i = 0; i < sectionIndex; i++) {
        count += sections[i][1].length;
      }
      return count + actionIndex;
    }
    if (open) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div${attr_class("fixed inset-0 z-50 bg-black/50 flex items-start justify-center pt-[15vh] transition-opacity duration-150", void 0, {
        "opacity-0": !isVisible || isClosing,
        "opacity-100": isVisible && !isClosing
      })} aria-hidden="true"><div role="dialog" aria-modal="true" tabindex="-1" aria-label="Quick actions"${attr_class("w-full max-w-xl mx-4 bg-white rounded-lg shadow-2xl overflow-hidden transition-all duration-150", void 0, {
        "scale-95": !isVisible || isClosing,
        "scale-100": isVisible && !isClosing,
        "opacity-0": !isVisible || isClosing,
        "opacity-100": isVisible && !isClosing
      })}><div class="flex items-center gap-3 px-4 py-3 border-b border-gray-200">`);
      Zap($$renderer2, { size: 20, class: "text-amber-500" });
      $$renderer2.push(`<!----> <input type="text"${attr("value", query)} placeholder="What would you like to do?" class="flex-1 bg-transparent text-base outline-none placeholder:text-gray-400" aria-label="Quick action search"/> `);
      if (query) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<button class="p-1 rounded hover:bg-gray-100 text-gray-400 hover:text-gray-600" aria-label="Clear search">`);
        X($$renderer2, { size: 16 });
        $$renderer2.push(`<!----></button>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]--> <kbd class="hidden sm:inline-flex items-center gap-1 px-2 py-1 text-xs font-mono bg-gray-100 text-gray-500 rounded border border-gray-200">ESC</kbd></div> <div class="max-h-[50vh] overflow-y-auto" role="listbox">`);
      if (flatActions().length === 0) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<div class="px-4 py-8 text-center text-gray-500"><p>No actions found for "${escape_html(query)}"</p> <p class="text-sm mt-1 text-gray-400">Try a different search term</p></div>`);
      } else {
        $$renderer2.push("<!--[-1-->");
        const sections = Array.from(groupedActions().entries());
        $$renderer2.push(`<!--[-->`);
        const each_array = ensure_array_like(sections);
        for (let sectionIndex = 0, $$length = each_array.length; sectionIndex < $$length; sectionIndex++) {
          let [section, actions] = each_array[sectionIndex];
          $$renderer2.push(`<div class="py-1"><div class="px-4 py-1.5 text-xs font-semibold text-gray-500 uppercase tracking-wider bg-gray-50">${escape_html(section)}</div> <!--[-->`);
          const each_array_1 = ensure_array_like(actions);
          for (let actionIndex = 0, $$length2 = each_array_1.length; actionIndex < $$length2; actionIndex++) {
            let action = each_array_1[actionIndex];
            const globalIndex = getGlobalIndex(sectionIndex, actionIndex);
            const Icon2 = action.icon;
            $$renderer2.push(`<button${attr("data-index", globalIndex)} role="option"${attr("aria-selected", selectedIndex === globalIndex)}${attr_class("w-full px-4 py-2.5 flex items-center gap-3 text-left hover:bg-gray-100 transition-colors", void 0, { "bg-blue-50": selectedIndex === globalIndex })}>`);
            if (Icon2) {
              $$renderer2.push("<!--[-->");
              Icon2($$renderer2, { size: 18, class: "text-gray-500 flex-shrink-0" });
              $$renderer2.push("<!--]-->");
            } else {
              $$renderer2.push("<!--[!-->");
              $$renderer2.push("<!--]-->");
            }
            $$renderer2.push(` <div class="flex-1 min-w-0"><div class="text-sm font-medium text-gray-900">${escape_html(action.title)}</div> <div class="text-xs text-gray-500">${escape_html(action.description)}</div></div> `);
            if (action.shortcut) {
              $$renderer2.push("<!--[0-->");
              $$renderer2.push(`<div class="hidden sm:flex items-center gap-1"><!--[-->`);
              const each_array_2 = ensure_array_like(action.shortcut);
              for (let $$index = 0, $$length3 = each_array_2.length; $$index < $$length3; $$index++) {
                let key = each_array_2[$$index];
                $$renderer2.push(`<kbd class="px-1.5 py-0.5 text-xs font-mono bg-gray-100 text-gray-600 rounded border border-gray-200">${escape_html(key.toUpperCase())}</kbd>`);
              }
              $$renderer2.push(`<!--]--></div>`);
            } else {
              $$renderer2.push("<!--[-1-->");
            }
            $$renderer2.push(`<!--]--></button>`);
          }
          $$renderer2.push(`<!--]--></div>`);
        }
        $$renderer2.push(`<!--]-->`);
      }
      $$renderer2.push(`<!--]--></div> <div class="px-4 py-2 bg-gray-50 border-t border-gray-200 flex items-center justify-between text-xs text-gray-500"><div class="flex items-center gap-3"><span class="flex items-center gap-1"><kbd class="px-1.5 py-0.5 bg-white rounded border border-gray-200">↑↓</kbd> <span>Navigate</span></span> <span class="flex items-center gap-1"><kbd class="px-1.5 py-0.5 bg-white rounded border border-gray-200">↵</kbd> <span>Select</span></span></div> <div class="flex items-center gap-2"><span>${escape_html(getModifierKey())} + Shift + P</span> <span>to open</span></div></div></div></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]-->`);
    bind_props($$props, { open });
  });
}
function _layout($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    var $$store_subs;
    let nodes = [getDefaultNode()];
    let searchOpen = false;
    let quickActionsOpen = false;
    function generateBreadcrumbs(path) {
      const items = [{ label: "Datacenter", href: "/" }];
      if (path === "/") {
        items.push({ label: "Overview" });
        return items;
      }
      const nodeMatch = path.match(/\/nodes\/([^\/]+)(?:\/(.+))?/);
      if (nodeMatch) {
        const nodeId = nodeMatch[1];
        const subPath = nodeMatch[2];
        const node = nodes.find((n) => n.id === nodeId);
        items.push({ label: node?.name || nodeId, href: `/nodes/${nodeId}` });
        if (subPath) {
          const resourceMap2 = {
            "vms": "Virtual Machines",
            "images": "Images",
            "storage": "Storage",
            "networks": "Networks"
          };
          items.push({ label: resourceMap2[subPath] || subPath });
        }
        return items;
      }
      const resourceMap = {
        "/images": "Images",
        "/storage": "Storage",
        "/networks": "Networks",
        "/settings": "Settings",
        "/profile": "Profile"
      };
      if (resourceMap[path]) {
        items.push({ label: resourceMap[path] });
      } else {
        const segments = path.split("/").filter(Boolean);
        if (segments.length > 0) {
          items.push({
            label: segments[segments.length - 1].charAt(0).toUpperCase() + segments[segments.length - 1].slice(1)
          });
        }
      }
      return items;
    }
    let breadcrumbs = derived(() => generateBreadcrumbs(store_get($$store_subs ??= {}, "$page", page).url.pathname));
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      ToastContainer($$renderer3);
      $$renderer3.push(`<!----> `);
      SearchModal($$renderer3, {
        get open() {
          return searchOpen;
        },
        set open($$value) {
          searchOpen = $$value;
          $$settled = false;
        }
      });
      $$renderer3.push(`<!----> `);
      KeyboardShortcutsHelp($$renderer3);
      $$renderer3.push(`<!----> `);
      QuickActions($$renderer3, {
        get open() {
          return quickActionsOpen;
        },
        set open($$value) {
          quickActionsOpen = $$value;
          $$settled = false;
        }
      });
      $$renderer3.push(`<!----> `);
      if (store_get($$store_subs ??= {}, "$page", page).url.pathname === "/login") {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<!--[-->`);
        slot($$renderer3, $$props, "default", {});
        $$renderer3.push(`<!--]-->`);
      } else {
        $$renderer3.push("<!--[-1-->");
        $$renderer3.push(`<div class="proxmox-layout svelte-12qhfyh">`);
        TreeNavigation($$renderer3, { nodes });
        $$renderer3.push(`<!----> <main class="flex-1 flex flex-col min-w-0 bg-[#f5f5f5] overflow-hidden"><header class="h-12 bg-[#2d2d3a] border-b border-[#3a3a4a] flex items-center px-4 justify-between shrink-0 shadow-sm"><div class="flex items-center gap-4 min-w-0 flex-1">`);
        Breadcrumbs($$renderer3, { items: breadcrumbs() });
        $$renderer3.push(`<!----></div> <div class="flex items-center gap-3 shrink-0"><button class="hidden sm:flex items-center gap-2 px-3 py-1.5 text-xs text-slate-300 hover:text-white hover:bg-[#3a3a4a] rounded-md transition-colors" title="Search (Ctrl+K or Cmd+K)"><span class="hidden lg:inline">Search</span> <kbd class="px-1.5 py-0.5 bg-[#1e1e28] rounded text-[10px] text-slate-400 border border-[#3a3a4a]">Ctrl K</kbd></button> <div${attr_class(`flex items-center gap-2 px-3 py-1.5 rounded-md text-xs border transition-colors duration-200 ${stringify(
          "bg-green-500/10 border-green-500/30 text-green-400"
        )}`)}${attr("title", "Connected to server")}>`);
        {
          $$renderer3.push("<!--[0-->");
          Wifi($$renderer3, { size: 14, class: "animate-pulse" });
          $$renderer3.push(`<!----> <span class="hidden sm:inline">Connected</span>`);
        }
        $$renderer3.push(`<!--]--></div> <div class="flex items-center gap-2 px-3 py-1.5 bg-[#1e1e28] rounded-md text-xs border border-[#3a3a4a]"><span class="w-2 h-2 rounded-full bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.5)]"></span> <span class="text-slate-300">${escape_html(nodes.length)} node${escape_html(nodes.length !== 1 ? "s" : "")}</span></div> <span class="text-[10px] text-slate-400 px-2 py-1 bg-[#1e1e28] rounded-md border border-[#3a3a4a]">v0.1.0-alpha</span></div></header> <div class="flex-1 overflow-auto p-6"><!---->`);
        {
          $$renderer3.push(`<div class="max-w-[1600px] mx-auto"><!--[-->`);
          slot($$renderer3, $$props, "default", {});
          $$renderer3.push(`<!--]--></div>`);
        }
        $$renderer3.push(`<!----></div></main></div>`);
      }
      $$renderer3.push(`<!--]-->`);
    }
    do {
      $$settled = true;
      $$inner_renderer = $$renderer2.copy();
      $$render_inner($$inner_renderer);
    } while (!$$settled);
    $$renderer2.subsume($$inner_renderer);
    if ($$store_subs) unsubscribe_stores($$store_subs);
  });
}
export {
  _layout as default
};
