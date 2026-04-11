import{c as le,a as u,f as d}from"./Dhysq48Y.js";import{f as ne,p as re,t as p,b as ie,c,e as oe,g as t,d as _,s as ve,r as o,u as R,aF as ce}from"./CD0AeUN1.js";import{d as ue,a as b,s as H}from"./jRuktn7D.js";import{i as V}from"./QUCH3Vci.js";import{e as X,i as Y}from"./BZhe11m3.js";import{I as de,s as pe,t as _e}from"./EcgFWJmQ.js";import{a as y,i as me,d as fe,r as q,e as Z,f as he}from"./Jn9glCRu.js";import{s as $}from"./jLPFnnxY.js";import{s as be}from"./CFvbdoeL.js";import"./Fw9PxynC.js";import{l as ye,s as ge}from"./DhnqYNKl.js";import{C as ke,X as je}from"./0sK4QKWu.js";function Fe(A,n){const m=ye(n,["children","$$slots","$$events","$$legacy"]);/**
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
 */const F=[["path",{d:"M10 5H3"}],["path",{d:"M12 19H3"}],["path",{d:"M14 3v4"}],["path",{d:"M16 17v4"}],["path",{d:"M21 12h-9"}],["path",{d:"M21 19h-5"}],["path",{d:"M21 5h-7"}],["path",{d:"M8 10v4"}],["path",{d:"M8 12H3"}]];de(A,ge({name:"sliders-horizontal"},()=>m,{get iconNode(){return F},children:(E,G)=>{var f=le(),I=ne(f);pe(I,n,"default",{}),u(E,f)},$$slots:{default:!0}}))}var xe=d('<span class="filter-badge svelte-m9tjun"> </span>'),Me=d('<span class="filter-chip svelte-m9tjun"><span class="chip-label svelte-m9tjun"> </span> <button type="button" class="chip-remove svelte-m9tjun"><!></button></span>'),Ce=d('<div class="filter-chips svelte-m9tjun"><!> <button type="button" class="clear-all-btn svelte-m9tjun">Clear all</button></div>'),ze=d("<option> </option>"),Se=d('<select class="filter-select svelte-m9tjun"><option>All</option><!></select>'),He=d('<input type="text" class="filter-input svelte-m9tjun"/>'),Ne=d('<input type="date" class="filter-input svelte-m9tjun"/>'),Ae=d('<label class="filter-checkbox svelte-m9tjun"><input type="checkbox" class="svelte-m9tjun"/> <span> </span></label>'),Ee=d('<div class="filter-field svelte-m9tjun"><label class="filter-label svelte-m9tjun"> </label> <!></div>'),Ie=d('<div class="filter-bar svelte-m9tjun"><button type="button" class="filter-toggle svelte-m9tjun"><!> <span>Filters</span> <!> <span><!></span></button> <div><!> <div class="filter-inputs svelte-m9tjun"></div></div></div>');function Ke(A,n){re(n,!0);let m=ve(!1);const F=R(()=>Object.values(n.activeFilters).filter(s=>s!=null&&s!=="").length);function E(s,e){const l=n.filters.find(v=>v.key===s);if(!l)return String(e);if(l.type==="select"&&l.options){const v=l.options.find(h=>h.value===e);return(v==null?void 0:v.label)??String(e)}return l.type==="boolean"?e?"Yes":"No":String(e)}const G=R(()=>Object.entries(n.activeFilters).filter(([s,e])=>e!=null&&e!==""));function f(s,e){const l=e.target;let v=l.value;l.type==="checkbox"&&(v=l.checked),n.onFilterChange(s,v)}function I(s){n.onFilterChange(s,"")}var O=Ie(),x=c(O),J=c(x);Fe(J,{size:16});var K=_(J,4);{var ee=s=>{var e=xe(),l=c(e,!0);o(e),p(()=>H(l,t(F))),u(s,e)};V(K,s=>{t(F)>0&&s(ee)})}var w=_(K,2);let L;var te=c(w);ke(te,{size:16}),o(w),o(x);var B=_(x,2);let Q;var T=c(B);{var ae=s=>{var e=Ce(),l=c(e);X(l,17,()=>t(G),Y,(h,g)=>{var N=R(()=>ce(t(g),2));let M=()=>t(N)[0],D=()=>t(N)[1];var C=Me(),i=c(C),a=c(i,!0);o(i);var r=_(i,2),k=c(r);je(k,{size:12}),o(r),o(C),p(j=>{H(a,j),y(r,"aria-label",`Remove ${M()} filter`)},[()=>E(M(),D())]),b("click",r,()=>I(M())),u(h,C)});var v=_(l,2);o(e),b("click",v,function(...h){var g;(g=n.onClearAll)==null||g.apply(this,h)}),_e(3,e,()=>be,()=>({duration:150})),u(s,e)};V(T,s=>{t(F)>0&&s(ae)})}var U=_(T,2);X(U,21,()=>n.filters,Y,(s,e)=>{var l=Ee(),v=c(l),h=c(v,!0);o(v);var g=_(v,2);{var N=i=>{var a=Se(),r=c(a);r.value=r.__value="";var k=_(r);X(k,17,()=>t(e).options??[],Y,(z,P)=>{var S=ze(),se=c(S,!0);o(S);var W={};p(()=>{H(se,t(P).label),W!==(W=t(P).value)&&(S.value=(S.__value=t(P).value)??"")}),u(z,S)}),o(a);var j;me(a),p(()=>{y(a,"id",`filter-${t(e).key}`),j!==(j=n.activeFilters[t(e).key]??"")&&(a.value=(a.__value=n.activeFilters[t(e).key]??"")??"",fe(a,n.activeFilters[t(e).key]??""))}),b("change",a,z=>f(t(e).key,z)),u(i,a)},M=i=>{var a=He();q(a),p(()=>{y(a,"id",`filter-${t(e).key}`),y(a,"placeholder",t(e).placeholder??`Filter by ${t(e).label}`),Z(a,n.activeFilters[t(e).key]??"")}),b("input",a,r=>f(t(e).key,r)),u(i,a)},D=i=>{var a=Ne();q(a),p(()=>{y(a,"id",`filter-${t(e).key}`),Z(a,n.activeFilters[t(e).key]??"")}),b("change",a,r=>f(t(e).key,r)),u(i,a)},C=i=>{var a=Ae(),r=c(a);q(r);var k=_(r,2),j=c(k,!0);o(k),o(a),p(()=>{he(r,!!n.activeFilters[t(e).key]),H(j,t(e).label)}),b("change",r,z=>f(t(e).key,z)),u(i,a)};V(g,i=>{t(e).type==="select"?i(N):t(e).type==="text"?i(M,1):t(e).type==="date"?i(D,2):t(e).type==="boolean"&&i(C,3)})}o(l),p(()=>{y(v,"for",`filter-${t(e).key}`),H(h,t(e).label)}),u(s,l)}),o(U),o(B),o(O),p(()=>{y(x,"aria-expanded",t(m)),L=$(w,1,"chevron svelte-m9tjun",null,L,{rotated:t(m)}),Q=$(B,1,"filter-content svelte-m9tjun",null,Q,{expanded:t(m)})}),b("click",x,()=>oe(m,!t(m))),u(A,O),ie()}ue(["click","change","input"]);export{Ke as F};
