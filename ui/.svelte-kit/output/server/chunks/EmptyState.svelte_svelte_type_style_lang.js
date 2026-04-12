import { g as ensure_array_like, m as attr_style, i as derived, h as stringify } from "./root.js";
function SkeletonRow($$renderer, $$props) {
  let { columns = 4 } = $$props;
  const columnArray = derived(() => Array.from({ length: columns }, (_, i) => i));
  function getColumnWidth(index) {
    if (index === 0) return "60%";
    if (index === columns - 1) return "40%";
    if (index % 2 === 0) return "70%";
    return "85%";
  }
  $$renderer.push(`<tr class="skeleton-pulse svelte-13zwz4t"><!--[-->`);
  const each_array = ensure_array_like(columnArray());
  for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
    let i = each_array[$$index];
    $$renderer.push(`<td class="border-b border-line px-4 py-3 svelte-13zwz4t"><div class="h-4 rounded bg-gradient-to-r from-gray-200 via-gray-100 to-gray-200 svelte-13zwz4t"${attr_style(`width: ${stringify(getColumnWidth(i))}`)}></div></td>`);
  }
  $$renderer.push(`<!--]--></tr>`);
}
export {
  SkeletonRow as S
};
