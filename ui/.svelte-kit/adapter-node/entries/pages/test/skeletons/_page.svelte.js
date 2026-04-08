import { e as ensure_array_like, n as attr_style, h as derived, s as stringify, c as escape_html, b as attr } from "../../../../chunks/renderer.js";
import { S as SkeletonRow } from "../../../../chunks/SkeletonRow.js";
function SkeletonCard($$renderer, $$props) {
  let { lines = 3 } = $$props;
  const lineArray = derived(() => Array.from({ length: lines }, (_, i) => i));
  function getLineWidth(index) {
    if (index === 0) return "80%";
    if (index === lines - 1) return "40%";
    if (index % 2 === 0) return "65%";
    return "90%";
  }
  $$renderer.push(`<div class="skeleton-pulse rounded border border-line bg-white p-4 svelte-lx25l7"><div class="flex items-start gap-4 svelte-lx25l7"><div class="h-10 w-10 flex-shrink-0 rounded-full bg-gradient-to-br from-gray-200 via-gray-100 to-gray-200 svelte-lx25l7"></div> <div class="flex-1 space-y-3 py-1 svelte-lx25l7"><!--[-->`);
  const each_array = ensure_array_like(lineArray());
  for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
    let i = each_array[$$index];
    $$renderer.push(`<div class="h-3 rounded bg-gradient-to-r from-gray-200 via-gray-100 to-gray-200 svelte-lx25l7"${attr_style(`width: ${stringify(getLineWidth(i))}`)}></div>`);
  }
  $$renderer.push(`<!--]--></div></div></div>`);
}
function _page($$renderer) {
  let customColumns = 6;
  let customLines = 4;
  $$renderer.push(`<div class="space-y-8 p-6"><h1 class="text-2xl font-semibold text-ink">Skeleton Loading States Test</h1> <section class="table-card"><div class="card-header px-4 py-3"><div class="flex items-center justify-between"><div><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Component Test</div> <div class="mt-1 text-lg font-semibold">SkeletonRow</div></div> <button class="button-primary rounded px-4 py-2 text-sm">${escape_html("Show Data")}</button></div></div> <div class="p-4"><div class="mb-4 flex items-center gap-4"><label class="text-sm text-muted">Columns: <input type="number" min="1" max="10"${attr("value", customColumns)} class="ml-2 h-8 w-16 rounded border border-line px-2 text-sm"/></label></div> <table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Column 1</th><th class="border-b border-line px-4 py-3">Column 2</th><th class="border-b border-line px-4 py-3">Column 3</th><th class="border-b border-line px-4 py-3">Column 4</th><th class="border-b border-line px-4 py-3">Column 5</th><th class="border-b border-line px-4 py-3">Status</th></tr></thead><tbody>`);
  {
    $$renderer.push("<!--[0-->");
    $$renderer.push(`<!--[-->`);
    const each_array = ensure_array_like(Array(5));
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      each_array[$$index];
      SkeletonRow($$renderer, { columns: customColumns });
    }
    $$renderer.push(`<!--]-->`);
  }
  $$renderer.push(`<!--]--></tbody></table></div></section> <section class="table-card"><div class="card-header px-4 py-3"><div class="flex items-center justify-between"><div><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Component Test</div> <div class="mt-1 text-lg font-semibold">SkeletonCard</div></div> <button class="button-primary rounded px-4 py-2 text-sm">${escape_html("Show Data")}</button></div></div> <div class="p-4"><div class="mb-4 flex items-center gap-4"><label class="text-sm text-muted">Lines: <input type="number" min="1" max="8"${attr("value", customLines)} class="ml-2 h-8 w-16 rounded border border-line px-2 text-sm"/></label></div> <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">`);
  {
    $$renderer.push("<!--[0-->");
    $$renderer.push(`<!--[-->`);
    const each_array_2 = ensure_array_like(Array(6));
    for (let $$index_2 = 0, $$length = each_array_2.length; $$index_2 < $$length; $$index_2++) {
      each_array_2[$$index_2];
      SkeletonCard($$renderer, { lines: customLines });
    }
    $$renderer.push(`<!--]-->`);
  }
  $$renderer.push(`<!--]--></div></div></section> <section class="rounded border border-line bg-chrome/50 p-4"><h2 class="mb-4 text-lg font-semibold text-ink">Usage</h2> <pre class="overflow-x-auto rounded bg-white p-4 text-sm mono">
&lt;!-- Table loading state -->
{#if loading}
  {#each Array(5) as _}
    &lt;SkeletonRow columns={6} />
  {/each}
{:else}
  {#each items as item}...
{/if}

&lt;!-- Card loading state -->
{#if loading}
  &lt;SkeletonCard lines={4} />
{:else}
  &lt;Card>...&lt;/Card>
{/if}</pre></section></div>`);
}
export {
  _page as default
};
