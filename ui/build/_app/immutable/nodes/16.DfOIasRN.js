import{c as B,a as n,f as u}from"../chunks/Dhysq48Y.js";import{o as F}from"../chunks/gz8L2jlR.js";import{f as H,p as T,s as O,a as j,b as q,d as l,c as r,g as a,r as e,e as f,t as D}from"../chunks/CD0AeUN1.js";import{s as $}from"../chunks/jRuktn7D.js";import{i as G}from"../chunks/QUCH3Vci.js";import{e as C,i as M}from"../chunks/BZhe11m3.js";import{g as J}from"../chunks/D0iddTRM.js";import{g as K,c as Q,t as U}from"../chunks/jLPFnnxY.js";import{S as V}from"../chunks/DcNjj1U7.js";import{S as W,E as X}from"../chunks/D2i2LMlj.js";import"../chunks/Fw9PxynC.js";import{I as Y,s as Z}from"../chunks/EcgFWJmQ.js";import{l as tt,s as et}from"../chunks/DhnqYNKl.js";function rt(v,c){const b=tt(c,["children","$$slots","$$events","$$legacy"]);/**
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
 */const y=[["path",{d:"M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"}],["path",{d:"M3 3v5h5"}],["path",{d:"M12 7v5l4 2"}]];Y(v,et({name:"history"},()=>b,{get iconNode(){return y},children:(s,m)=>{var h=B(),i=H(h);Z(i,c,"default",{}),n(s,h)},$$slots:{default:!0}}))}var at=u('<table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Resource</th><th class="border-b border-line px-4 py-3">Operation</th><th class="border-b border-line px-4 py-3">State</th><th class="border-b border-line px-4 py-3">Created</th></tr></thead><tbody></tbody></table>'),ot=u('<tr class="odd:bg-white even:bg-[#f8f8f8]"><td class="border-b border-line px-4 py-3"> </td><td class="border-b border-line px-4 py-3"> </td><td class="border-b border-line px-4 py-3"><!></td><td class="border-b border-line px-4 py-3 mono"> </td></tr>'),st=u('<table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Resource</th><th class="border-b border-line px-4 py-3">Operation</th><th class="border-b border-line px-4 py-3">State</th><th class="border-b border-line px-4 py-3">Created</th></tr></thead><tbody></tbody></table>'),dt=u('<section class="table-card"><div class="card-header px-4 py-3"><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Operations</div> <div class="mt-1 text-lg font-semibold">Auditable Change Log</div></div> <!></section>');function gt(v,c){T(c,!0);const b=K(),y=Q({token:b??void 0});let s=O(j([])),m=O(!0);async function h(){f(m,!0);try{f(s,await y.listOperations(),!0)}catch{U.error("Failed to load operations"),f(s,[],!0)}finally{f(m,!1)}}F(()=>{if(!b){J("/login");return}h()});var i=dt(),R=l(r(i),2);{var A=t=>{var o=at(),p=l(r(o));C(p,20,()=>Array(5),M,(x,d)=>{W(x,{columns:4})}),e(p),e(o),n(t,o)},I=t=>{X(t,{get icon(){return rt},title:"No operations yet",description:"Recent operations will appear here"})},N=t=>{var o=st(),p=l(r(o));C(p,21,()=>a(s),M,(x,d)=>{var g=ot(),_=r(g),E=r(_);e(_);var k=l(_),L=r(k,!0);e(k);var S=l(k),P=r(S);V(P,{get label(){return a(d).state}}),e(S);var w=l(S),z=r(w,!0);e(w),e(g),D(()=>{$(E,`${a(d).resource_type??""}:${a(d).resource_id??""}`),$(L,a(d).operation_type),$(z,a(d).created_at)}),n(x,g)}),e(p),e(o),n(t,o)};G(R,t=>{a(m)?t(A):a(s).length===0?t(I,1):t(N,-1)})}e(i),n(v,i),q()}export{gt as component};
