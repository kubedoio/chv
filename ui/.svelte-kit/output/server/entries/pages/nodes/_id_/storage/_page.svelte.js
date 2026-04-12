import { n as head, c as attr, h as stringify, e as escape_html, i as derived, d as store_get, u as unsubscribe_stores } from "../../../../../chunks/root.js";
import { p as page } from "../../../../../chunks/stores.js";
import "@sveltejs/kit/internal";
import "../../../../../chunks/exports.js";
import "../../../../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../../../../chunks/client.js";
import { g as getStoredToken, c as createAPIClient } from "../../../../../chunks/client2.js";
import { D as DataTable } from "../../../../../chunks/DataTable.js";
import { u as useTable, f as formatBytes, P as Pagination } from "../../../../../chunks/Pagination.js";
/* empty css                                                             */
import { g as getDefaultNode } from "../../../../../chunks/nodes.js";
import { A as Arrow_left } from "../../../../../chunks/arrow-left.js";
import { H as Hard_drive } from "../../../../../chunks/hard-drive.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    var $$store_subs;
    const token = getStoredToken();
    createAPIClient({ token: token ?? void 0 });
    const nodeId = derived(() => store_get($$store_subs ??= {}, "$page", page).params.id);
    const node = derived(getDefaultNode);
    let items = [];
    let loading = true;
    let table = useTable({ data: [], pageSize: 10 });
    const columns = [
      {
        key: "name",
        title: "Name",
        sortable: true,
        render: (pool) => pool.name
      },
      {
        key: "pool_type",
        title: "Type",
        sortable: true,
        width: "100px",
        render: (pool) => pool.pool_type.toUpperCase()
      },
      { key: "path", title: "Path", render: (pool) => pool.path },
      {
        key: "capacity_bytes",
        title: "Capacity",
        sortable: true,
        width: "120px",
        render: (pool) => pool.capacity_bytes ? formatBytes(pool.capacity_bytes) : "—"
      },
      {
        key: "allocatable_bytes",
        title: "Available",
        sortable: true,
        width: "120px",
        render: (pool) => pool.allocatable_bytes ? formatBytes(pool.allocatable_bytes) : "—"
      },
      {
        key: "status",
        title: "Status",
        sortable: true,
        width: "100px",
        render: (pool) => pool.status
      }
    ];
    function handleSort(column, direction) {
      if (direction) {
        table.setSort(column, direction);
      } else {
        table.clearSort();
      }
    }
    const totalCapacity = derived(() => items.reduce((acc, p) => acc + (p.capacity_bytes || 0), 0));
    const totalAvailable = derived(() => items.reduce((acc, p) => acc + (p.allocatable_bytes || 0), 0));
    head("yh7k5c", $$renderer2, ($$renderer3) => {
      $$renderer3.title(($$renderer4) => {
        $$renderer4.push(`<title>Storage | ${escape_html(node().name)}</title>`);
      });
    });
    $$renderer2.push(`<div class="space-y-6"><div class="flex items-center justify-between"><div class="flex items-center gap-4"><a${attr("href", `/nodes/${stringify(nodeId())}`)} class="p-2 text-slate-500 hover:text-slate-700 hover:bg-slate-100 rounded-lg transition-colors" aria-label="Back to node">`);
    Arrow_left($$renderer2, { size: 20 });
    $$renderer2.push(`<!----></a> <div><h1 class="text-2xl font-bold text-slate-900">Storage Pools</h1> <p class="text-sm text-slate-500">Node: ${escape_html(node().name)}</p></div></div></div> <div class="grid gap-4 md:grid-cols-4"><div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-amber-50 rounded-lg">`);
    Hard_drive($$renderer2, { size: 20, class: "text-amber-600" });
    $$renderer2.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">Total Pools</p> <p class="text-xl font-bold text-slate-900">${escape_html(items.length)}</p></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-blue-50 rounded-lg">`);
    Hard_drive($$renderer2, { size: 20, class: "text-blue-600" });
    $$renderer2.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">Total Capacity</p> <p class="text-xl font-bold text-slate-900">${escape_html(formatBytes(totalCapacity()))}</p></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-green-50 rounded-lg">`);
    Hard_drive($$renderer2, { size: 20, class: "text-green-600" });
    $$renderer2.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">Available</p> <p class="text-xl font-bold text-slate-900">${escape_html(formatBytes(totalAvailable()))}</p></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-purple-50 rounded-lg">`);
    Hard_drive($$renderer2, { size: 20, class: "text-purple-600" });
    $$renderer2.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">Used</p> <p class="text-xl font-bold text-slate-900">${escape_html(totalCapacity() > 0 ? ((totalCapacity() - totalAvailable()) / totalCapacity() * 100).toFixed(1) : 0)}%</p></div></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200">`);
    DataTable($$renderer2, {
      data: table.paginatedData,
      columns,
      loading,
      sortColumn: table.sortColumn,
      sortDirection: table.sortDirection,
      onSort: handleSort,
      rowId: (pool) => pool.id
    });
    $$renderer2.push(`<!----> `);
    Pagination($$renderer2, {
      page: table.page,
      pageSize: table.pageSize,
      totalItems: table.totalItems,
      onPageChange: (p) => table.setPage(p),
      onPageSizeChange: (size) => table.setPageSize(size)
    });
    $$renderer2.push(`<!----></div></div>`);
    if ($$store_subs) unsubscribe_stores($$store_subs);
  });
}
export {
  _page as default
};
