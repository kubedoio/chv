import{c as K,a as D}from"./CN9aTnHa.js";import"./CnpuDcaw.js";import{f as T,s as p,a as M,e as a,g as r}from"./CxHtshv_.js";import{I as E,s as $}from"./D_Cs22R6.js";import{l as q,s as _}from"./l5guW54h.js";import{g as h}from"./WFL9UeeG.js";function H(t,e){const i=q(e,["children","$$slots","$$events","$$legacy"]);/**
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
 */const n=[["path",{d:"M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"}],["path",{d:"M14 2v5a1 1 0 0 0 1 1h5"}],["path",{d:"M10 9H8"}],["path",{d:"M16 13H8"}],["path",{d:"M16 17H8"}]];E(t,_({name:"file-text"},()=>i,{get iconNode(){return n},children:(o,c)=>{var f=K(),l=T(f);$(l,e,"default",{}),D(o,f)},$$slots:{default:!0}}))}let d=p(M([])),S=p("global"),m=p(!1),u=p(M([])),s=p(null);const k=1e3,y=navigator.platform.toUpperCase().includes("MAC");function U(){return y?"⌘":"Ctrl"}function v(t){if(!t)return!1;const e=t.tagName.toLowerCase();return e==="input"||e==="textarea"||e==="select"||t.getAttribute("contenteditable")==="true"}function I(t,e){if(!e||e.length===0)return!t.ctrlKey&&!t.metaKey&&!t.altKey&&!t.shiftKey;const i=e.includes("ctrl"),n=e.includes("meta"),o=e.includes("alt"),c=e.includes("shift");return t.ctrlKey===i&&t.metaKey===n&&t.altKey===o&&t.shiftKey===c}function x(t){if(t.key==="?"&&!v(document.activeElement)){t.preventDefault(),a(m,!0);return}if(t.key==="Escape"&&r(m)){t.preventDefault(),a(m,!1);return}const e=t.target,i=v(e);if(!i&&r(u).length>0){const o=[...r(u),t.key.toLowerCase()],c=r(d).find(l=>{if(l.modifiers&&l.modifiers.length>0)return!1;const b=l.key.toLowerCase().split("");return o.length===b.length&&o.every((w,C)=>w===b[C])});if(c){t.preventDefault(),a(u,[],!0),r(s)&&(clearTimeout(r(s)),a(s,null)),c.handler(t);return}if(r(d).some(l=>{if(l.modifiers&&l.modifiers.length>0)return!1;const g=l.key.toLowerCase();return g.length>1&&g.startsWith(o.join(""))})){t.preventDefault(),a(u,o,!0),r(s)&&clearTimeout(r(s)),a(s,window.setTimeout(()=>{a(u,[],!0)},k),!0);return}a(u,[],!0),r(s)&&(clearTimeout(r(s)),a(s,null))}const n=r(d).find(o=>{if(o.context&&o.context!=="global"&&o.context!==r(S)||i&&!o.allowInInput||!I(t,o.modifiers))return!1;const c=t.key.toLowerCase(),f=o.key.toLowerCase();return f.length>1?c===f[0]?(a(u,[c],!0),r(s)&&clearTimeout(r(s)),a(s,window.setTimeout(()=>{a(u,[],!0)},k),!0),!0):!1:c===f});if(n){if(n.key.length>1){t.preventDefault();return}n.preventDefault!==!1&&t.preventDefault(),n.handler(t)}}function z(){return document.addEventListener("keydown",x),()=>{document.removeEventListener("keydown",x)}}function F(t){return a(d,[...r(d),...t],!0),()=>{a(d,r(d).filter(e=>!t.some(i=>i.id===e.id)),!0)}}function R(t){a(S,t,!0)}function j(t){a(m,t,!0)}function B(){return r(m)}function P(){const t=new Map;for(const n of r(d)){const o=n.context||"global";t.has(o)||t.set(o,[]),t.get(o).push(n)}const e=["global","navigation","vms","vm-detail"],i=[];for(const n of e)t.has(n)&&(i.push({name:n,shortcuts:t.get(n)}),t.delete(n));for(const[n,o]of t)i.push({name:n,shortcuts:o});return i}function Q(t,e){return[{id:"global-search",key:"k",modifiers:[y?"meta":"ctrl"],context:"global",description:"Open global search",handler:t,preventDefault:!0},{id:"quick-actions",key:"p",modifiers:[y?"meta":"ctrl","shift"],context:"global",description:"Open quick actions",handler:e,preventDefault:!0},{id:"go-dashboard",key:"gd",context:"global",description:"Go to Dashboard",handler:()=>h("/")},{id:"go-vms",key:"gv",context:"global",description:"Go to VMs",handler:()=>h("/vms")},{id:"go-images",key:"gi",context:"global",description:"Go to Images",handler:()=>h("/images")},{id:"go-storage",key:"gs",context:"global",description:"Go to Storage",handler:()=>h("/storage")},{id:"go-networks",key:"gn",context:"global",description:"Go to Networks",handler:()=>h("/networks")}]}function W(t){return[{id:"vmd-edit",key:"e",context:"vm-detail",description:"Edit VM",handler:t.onEdit},{id:"vmd-start",key:"s",context:"vm-detail",description:"Start VM",handler:t.onStart},{id:"vmd-stop",key:"x",context:"vm-detail",description:"Stop VM",handler:t.onStop},{id:"vmd-restart",key:"r",context:"vm-detail",description:"Restart VM",handler:t.onRestart},{id:"vmd-delete",key:"delete",context:"vm-detail",description:"Delete VM",handler:t.onDelete},{id:"vmd-tab-1",key:"1",context:"vm-detail",description:"Overview tab",handler:()=>t.onTabChange(0)},{id:"vmd-tab-2",key:"2",context:"vm-detail",description:"Metrics tab",handler:()=>t.onTabChange(1)},{id:"vmd-tab-3",key:"3",context:"vm-detail",description:"Snapshots tab",handler:()=>t.onTabChange(2)},{id:"vmd-tab-4",key:"4",context:"vm-detail",description:"Console tab",handler:()=>t.onTabChange(3)}]}export{H as F,P as a,z as b,Q as c,W as d,U as g,B as i,F as r,R as s,j as t};
