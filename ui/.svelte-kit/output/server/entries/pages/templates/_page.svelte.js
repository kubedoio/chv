import { s as sanitize_props, a as spread_props, b as slot, j as bind_props, e as escape_html, c as attr, g as ensure_array_like, i as derived, f as attr_class, h as stringify } from "../../../chunks/root.js";
import "@sveltejs/kit/internal";
import "../../../chunks/exports.js";
import "../../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../../chunks/client.js";
import { c as createAPIClient, g as getStoredToken, t as toast } from "../../../chunks/client2.js";
import { D as DataTable } from "../../../chunks/DataTable.js";
/* empty css                                                       */
import { M as Modal } from "../../../chunks/Modal.js";
import { F as FormField, I as Input } from "../../../chunks/Input.js";
import { C as Copy } from "../../../chunks/copy.js";
import { h as html } from "../../../chunks/html.js";
import { C as ConfirmDialog } from "../../../chunks/ConfirmDialog.js";
import { I as Icon } from "../../../chunks/Icon.js";
import { T as Trash_2 } from "../../../chunks/trash-2.js";
function Box($$renderer, $$props) {
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
        "d": "M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"
      }
    ],
    ["path", { "d": "m3.3 7 8.7 5 8.7-5" }],
    ["path", { "d": "M12 22V12" }]
  ];
  Icon($$renderer, spread_props([
    { name: "box" },
    $$sanitized_props,
    {
      /**
       * @component @name Box
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNMjEgOGEyIDIgMCAwIDAtMS0xLjczbC03LTRhMiAyIDAgMCAwLTIgMGwtNyA0QTIgMiAwIDAgMCAzIDh2OGEyIDIgMCAwIDAgMSAxLjczbDcgNGEyIDIgMCAwIDAgMiAwbDctNEEyIDIgMCAwIDAgMjEgMTZaIiAvPgogIDxwYXRoIGQ9Im0zLjMgNyA4LjcgNSA4LjctNSIgLz4KICA8cGF0aCBkPSJNMTIgMjJWMTIiIC8+Cjwvc3ZnPgo=) - https://lucide.dev/icons/box
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
function File_code($$renderer, $$props) {
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
    ["path", { "d": "M10 12.5 8 15l2 2.5" }],
    ["path", { "d": "m14 12.5 2 2.5-2 2.5" }]
  ];
  Icon($$renderer, spread_props([
    { name: "file-code" },
    $$sanitized_props,
    {
      /**
       * @component @name FileCode
       * @description Lucide SVG icon component, renders SVG Element with children.
       *
       * @preview ![img](data:image/svg+xml;base64,PHN2ZyAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogIHdpZHRoPSIyNCIKICBoZWlnaHQ9IjI0IgogIHZpZXdCb3g9IjAgMCAyNCAyNCIKICBmaWxsPSJub25lIgogIHN0cm9rZT0iIzAwMCIgc3R5bGU9ImJhY2tncm91bmQtY29sb3I6ICNmZmY7IGJvcmRlci1yYWRpdXM6IDJweCIKICBzdHJva2Utd2lkdGg9IjIiCiAgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIgogIHN0cm9rZS1saW5lam9pbj0icm91bmQiCj4KICA8cGF0aCBkPSJNNiAyMmEyIDIgMCAwIDEtMi0yVjRhMiAyIDAgMCAxIDItMmg4YTIuNCAyLjQgMCAwIDEgMS43MDQuNzA2bDMuNTg4IDMuNTg4QTIuNCAyLjQgMCAwIDEgMjAgOHYxMmEyIDIgMCAwIDEtMiAyeiIgLz4KICA8cGF0aCBkPSJNMTQgMnY1YTEgMSAwIDAgMCAxIDFoNSIgLz4KICA8cGF0aCBkPSJNMTAgMTIuNSA4IDE1bDIgMi41IiAvPgogIDxwYXRoIGQ9Im0xNCAxMi41IDIgMi41LTIgMi41IiAvPgo8L3N2Zz4K) - https://lucide.dev/icons/file-code
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
function CreateFromTemplate($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let {
      open = false,
      template = null,
      images = [],
      networks = [],
      pools = [],
      onSuccess
    } = $$props;
    createAPIClient({ token: getStoredToken() ?? void 0 });
    let name = "";
    let cloudInitTemplateId = "";
    let cloudInitVars = {};
    let useCustomUserData = false;
    let cloudInitTemplates = [];
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
    const selectedCloudInitTemplate = derived(() => cloudInitTemplates.find((t) => t.id === cloudInitTemplateId));
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      {
        let footer = function($$renderer4) {
          $$renderer4.push(`<button type="button"${attr("disabled", submitting, true)} class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50 disabled:cursor-not-allowed">Cancel</button> <button type="button"${attr("disabled", !name.trim(), true)} class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors disabled:bg-primary/30 disabled:cursor-not-allowed flex items-center gap-2">`);
          {
            $$renderer4.push("<!--[-1-->");
          }
          $$renderer4.push(`<!--]--> ${escape_html("Create VM")}</button>`);
        };
        Modal($$renderer3, {
          title: "Create VM from Template",
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
            if (template) {
              $$renderer4.push("<!--[0-->");
              $$renderer4.push(`<div class="space-y-5"><div class="bg-chrome rounded-lg p-4"><h3 class="text-sm font-semibold text-ink mb-2">Template: ${escape_html(template.name)}</h3> <div class="grid grid-cols-2 gap-2 text-sm"><div class="text-muted">Resources:</div> <div class="text-ink">${escape_html(template.vcpu)} vCPU, ${escape_html(template.memory_mb)} MB</div> <div class="text-muted">Image:</div> <div class="text-ink">${escape_html(images.find((i) => i.id === template?.image_id)?.name || template.image_id)}</div> <div class="text-muted">Network:</div> <div class="text-ink">${escape_html(networks.find((n) => n.id === template?.network_id)?.name || template.network_id)}</div></div></div> `);
              {
                $$renderer4.push("<!--[-1-->");
              }
              $$renderer4.push(`<!--]--> `);
              FormField($$renderer4, {
                label: "VM Name",
                error: nameError,
                required: true,
                labelFor: "vm-name",
                children: ($$renderer5) => {
                  Input($$renderer5, {
                    id: "vm-name",
                    placeholder: "my-new-vm",
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
              $$renderer4.push(`<!----> <div class="border-t border-line pt-4"><h3 class="text-sm font-semibold text-ink mb-3">Cloud-init Configuration</h3> <div class="flex gap-4 mb-4"><label class="flex items-center gap-2 cursor-pointer"><input type="radio"${attr("checked", useCustomUserData === false, true)}${attr("value", false)}${attr("disabled", submitting, true)} class="text-primary"/> <span class="text-sm text-ink">Use Template</span></label> <label class="flex items-center gap-2 cursor-pointer"><input type="radio"${attr("checked", useCustomUserData === true, true)}${attr("value", true)}${attr("disabled", submitting, true)} class="text-primary"/> <span class="text-sm text-ink">Custom User Data</span></label></div> `);
              {
                $$renderer4.push("<!--[0-->");
                FormField($$renderer4, {
                  label: "Cloud-init Template",
                  labelFor: "cloudinit-template",
                  children: ($$renderer5) => {
                    $$renderer5.select(
                      {
                        id: "cloudinit-template",
                        value: cloudInitTemplateId,
                        class: "h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm",
                        disabled: submitting
                      },
                      ($$renderer6) => {
                        $$renderer6.option({ value: "" }, ($$renderer7) => {
                          $$renderer7.push(`Select a template...`);
                        });
                        $$renderer6.push(`<!--[-->`);
                        const each_array = ensure_array_like(cloudInitTemplates);
                        for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
                          let cit = each_array[$$index];
                          $$renderer6.option({ value: cit.id }, ($$renderer7) => {
                            $$renderer7.push(`${escape_html(cit.name)}`);
                          });
                        }
                        $$renderer6.push(`<!--]-->`);
                      }
                    );
                  }
                });
                $$renderer4.push(`<!----> `);
                if (selectedCloudInitTemplate() && selectedCloudInitTemplate().variables.length > 0) {
                  $$renderer4.push("<!--[0-->");
                  $$renderer4.push(`<div class="mt-4 space-y-3"><h4 class="text-xs font-medium text-muted uppercase tracking-wide">Template Variables</h4> <!--[-->`);
                  const each_array_1 = ensure_array_like(selectedCloudInitTemplate().variables);
                  for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
                    let varName = each_array_1[$$index_1];
                    FormField($$renderer4, {
                      label: varName,
                      labelFor: `var-${varName}`,
                      children: ($$renderer5) => {
                        Input($$renderer5, {
                          id: `var-${varName}`,
                          value: cloudInitVars[varName] || "",
                          oninput: (e) => {
                            cloudInitVars = { ...cloudInitVars, [varName]: e.currentTarget.value };
                          },
                          placeholder: `Enter ${varName}...`,
                          disabled: submitting
                        });
                      }
                    });
                  }
                  $$renderer4.push(`<!--]--></div>`);
                } else {
                  $$renderer4.push("<!--[-1-->");
                }
                $$renderer4.push(`<!--]-->`);
              }
              $$renderer4.push(`<!--]--> <div class="mt-4"><button type="button" class="text-sm text-primary hover:text-primary/80 font-medium">${escape_html("Show Preview")}</button> `);
              {
                $$renderer4.push("<!--[-1-->");
              }
              $$renderer4.push(`<!--]--></div></div></div>`);
            } else {
              $$renderer4.push("<!--[-1-->");
              $$renderer4.push(`<div class="text-center py-8 text-muted">No template selected</div>`);
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
function CloudInitViewer($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { open = false, template = null } = $$props;
    createAPIClient({ token: getStoredToken() ?? void 0 });
    let previewVariables = {};
    function highlightYAML(content) {
      if (!content) return "";
      return content.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/(#.*$)/gm, '<span class="text-neutral-500">$1</span>').replace(/^(\s*)([a-zA-Z_][a-zA-Z0-9_]*)(:)/gm, '$1<span class="text-sky-400">$2</span>$3').replace(/(:\s*)'(.*?)'/g, `$1<span class="text-emerald-400">'$2'</span>`).replace(/(:\s*)(\d+)/g, '$1<span class="text-amber-400">$2</span>').replace(/(\{\{.*?\}\})/g, '<span class="text-pink-400">$1</span>');
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      {
        let footer = function($$renderer4) {
          $$renderer4.push(`<button type="button" class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors">Close</button>`);
        };
        Modal($$renderer3, {
          title: template?.name || "Cloud-init Template",
          closeOnBackdrop: true,
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
            if (template) {
              $$renderer4.push("<!--[0-->");
              $$renderer4.push(`<div class="space-y-4">`);
              if (template.description) {
                $$renderer4.push("<!--[0-->");
                $$renderer4.push(`<p class="text-sm text-muted">${escape_html(template.description)}</p>`);
              } else {
                $$renderer4.push("<!--[-1-->");
              }
              $$renderer4.push(`<!--]--> `);
              if (template.variables.length > 0) {
                $$renderer4.push("<!--[0-->");
                $$renderer4.push(`<div class="bg-chrome rounded-lg p-3"><h4 class="text-xs font-medium text-muted uppercase tracking-wide mb-2">Available Variables</h4> <div class="flex flex-wrap gap-2"><!--[-->`);
                const each_array = ensure_array_like(template.variables);
                for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
                  let varName = each_array[$$index];
                  $$renderer4.push(`<code class="text-xs bg-white px-2 py-1 rounded border border-line text-primary">${escape_html(`{{.${varName}}}`)}</code>`);
                }
                $$renderer4.push(`<!--]--></div></div>`);
              } else {
                $$renderer4.push("<!--[-1-->");
              }
              $$renderer4.push(`<!--]--> `);
              if (template.variables.length > 0) {
                $$renderer4.push("<!--[0-->");
                $$renderer4.push(`<div class="border-t border-line pt-4"><h4 class="text-sm font-semibold text-ink mb-3">Preview with Variables</h4> <div class="grid grid-cols-2 gap-3"><!--[-->`);
                const each_array_1 = ensure_array_like(template.variables);
                for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
                  let varName = each_array_1[$$index_1];
                  $$renderer4.push(`<div><label class="block text-xs text-muted mb-1">${escape_html(varName)}</label> <input type="text"${attr("value", previewVariables[varName] || "")}${attr("placeholder", `Enter ${varName}...`)} class="w-full h-8 rounded border border-[#CCCCCC] bg-white px-2 py-1 text-sm"/></div>`);
                }
                $$renderer4.push(`<!--]--></div> <button type="button" class="mt-3 text-sm text-primary hover:text-primary/80 font-medium">Render Preview</button></div>`);
              } else {
                $$renderer4.push("<!--[-1-->");
              }
              $$renderer4.push(`<!--]--> <div class="border-t border-line pt-4"><div class="flex items-center justify-between mb-2"><h4 class="text-sm font-semibold text-ink">${escape_html("Template Content")}</h4> <div class="flex items-center gap-2">`);
              if (template.variables.length > 0) {
                $$renderer4.push("<!--[0-->");
                $$renderer4.push(`<button type="button" class="text-xs text-muted hover:text-ink">${escape_html("Show Rendered")}</button> <span class="text-line">|</span>`);
              } else {
                $$renderer4.push("<!--[-1-->");
              }
              $$renderer4.push(`<!--]--> <button type="button" class="flex items-center gap-1 text-xs text-muted hover:text-ink">`);
              {
                $$renderer4.push("<!--[-1-->");
                Copy($$renderer4, { size: 12 });
                $$renderer4.push(`<!----> Copy`);
              }
              $$renderer4.push(`<!--]--></button></div></div> <div class="rounded-lg bg-neutral-900 overflow-auto max-h-96"><pre class="p-4 text-sm font-mono whitespace-pre-wrap"><code>${html(highlightYAML(template.content))}</code></pre></div></div></div>`);
            } else {
              $$renderer4.push("<!--[-1-->");
              $$renderer4.push(`<div class="text-center py-8 text-muted">No template selected</div>`);
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
    let vmTemplates = [];
    let cloudInitTemplates = [];
    let images = [];
    let networks = [];
    let pools = [];
    let loading = true;
    let error = "";
    let createFromTemplateOpen = false;
    let selectedTemplate = null;
    let cloudInitViewerOpen = false;
    let selectedCloudInitTemplate = null;
    let confirmDialog = {
      open: false,
      title: "",
      description: "",
      action: async () => {
      }
    };
    const imageMap = derived(() => new Map(images.map((i) => [i.id, i])));
    const vmTemplateColumns = [
      {
        key: "name",
        title: "Name",
        sortable: true,
        render: (t) => t.name
      },
      {
        key: "description",
        title: "Description",
        render: (t) => t.description || "—"
      },
      {
        key: "resources",
        title: "Resources",
        render: (t) => `${t.vcpu} vCPU, ${t.memory_mb} MB`
      },
      {
        key: "image_id",
        title: "Image",
        render: (t) => {
          const img = imageMap().get(t.image_id);
          return img?.name || t.image_id;
        }
      },
      {
        key: "tags",
        title: "Tags",
        render: (t) => {
          if (!t.tags || t.tags.length === 0) return "—";
          return t.tags.slice(0, 3).join(", ") + (t.tags.length > 3 ? ` +${t.tags.length - 3}` : "");
        }
      }
    ];
    async function loadData() {
      loading = true;
      error = "";
      try {
        const [vmTemps, cloudTemps, imgs, nets, ps] = await Promise.all([
          client.listVMTemplates(),
          client.listCloudInitTemplates(),
          client.listImages(),
          client.listNetworks(),
          client.listStoragePools()
        ]);
        vmTemplates = vmTemps;
        cloudInitTemplates = cloudTemps;
        images = imgs;
        networks = nets;
        pools = ps;
      } catch (err) {
        error = err instanceof Error ? err.message : "Failed to load templates";
        toast.error(error);
      } finally {
        loading = false;
      }
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      $$renderer3.push(`<div class="flex justify-between items-center mb-6"><div><h1 class="text-2xl font-bold text-ink">Templates</h1> <p class="text-muted text-sm mt-1">VM templates and cloud-init configurations for rapid provisioning</p></div></div> <div class="border-b border-line mb-6"><div class="flex gap-6"><button${attr_class(`pb-3 text-sm font-medium border-b-2 transition-colors ${stringify(
        "border-primary text-primary"
      )}`)}><span class="flex items-center gap-2">`);
      Box($$renderer3, { size: 16 });
      $$renderer3.push(`<!----> VM Templates <span class="bg-chrome text-muted text-xs px-2 py-0.5 rounded-full">${escape_html(vmTemplates.length)}</span></span></button> <button${attr_class(`pb-3 text-sm font-medium border-b-2 transition-colors ${stringify("border-transparent text-muted hover:text-ink")}`)}><span class="flex items-center gap-2">`);
      File_code($$renderer3, { size: 16 });
      $$renderer3.push(`<!----> Cloud-init Templates <span class="bg-chrome text-muted text-xs px-2 py-0.5 rounded-full">${escape_html(cloudInitTemplates.length)}</span></span></button></div></div> `);
      if (error) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<div class="mb-4 border border-danger bg-red-50 px-4 py-3 text-danger">${escape_html(error)}</div>`);
      } else {
        $$renderer3.push("<!--[-1-->");
      }
      $$renderer3.push(`<!--]--> `);
      {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<section class="table-card">`);
        {
          let children = function($$renderer4, template) {
            $$renderer4.push(`<div class="flex items-center gap-1"><button type="button" class="action-btn start svelte-ubkcxg" title="Clone VM from template">`);
            Copy($$renderer4, { size: 14 });
            $$renderer4.push(`<!----></button> <button type="button" class="action-btn danger svelte-ubkcxg" title="Delete template">`);
            Trash_2($$renderer4, { size: 14 });
            $$renderer4.push(`<!----></button></div>`);
          };
          DataTable($$renderer3, {
            data: vmTemplates,
            columns: vmTemplateColumns,
            loading,
            selectable: false,
            emptyIcon: Box,
            emptyTitle: "No VM templates yet",
            emptyDescription: "Create a VM template from an existing VM to enable rapid provisioning",
            rowId: (t) => t.id,
            children
          });
        }
        $$renderer3.push(`<!----></section>`);
      }
      $$renderer3.push(`<!--]--> `);
      CreateFromTemplate($$renderer3, {
        template: selectedTemplate,
        images,
        networks,
        pools,
        onSuccess: loadData,
        get open() {
          return createFromTemplateOpen;
        },
        set open($$value) {
          createFromTemplateOpen = $$value;
          $$settled = false;
        }
      });
      $$renderer3.push(`<!----> `);
      CloudInitViewer($$renderer3, {
        template: selectedCloudInitTemplate,
        get open() {
          return cloudInitViewerOpen;
        },
        set open($$value) {
          cloudInitViewerOpen = $$value;
          $$settled = false;
        }
      });
      $$renderer3.push(`<!----> `);
      ConfirmDialog($$renderer3, {
        title: confirmDialog.title,
        description: confirmDialog.description,
        confirmText: "Delete",
        variant: "danger",
        onConfirm: () => {
          confirmDialog.action();
          confirmDialog.open = false;
        },
        onCancel: () => confirmDialog.open = false,
        get open() {
          return confirmDialog.open;
        },
        set open($$value) {
          confirmDialog.open = $$value;
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
