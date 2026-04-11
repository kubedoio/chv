import { n as head, e as escape_html, c as attr, i as stringify, f as derived, j as store_get, u as unsubscribe_stores } from "../../../../../chunks/root.js";
import { p as page } from "../../../../../chunks/stores.js";
import "@sveltejs/kit/internal";
import "../../../../../chunks/exports.js";
import "../../../../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../../../../chunks/client.js";
import { g as getStoredToken, c as createAPIClient, t as toast } from "../../../../../chunks/client2.js";
import { D as DataTable } from "../../../../../chunks/DataTable.js";
import { P as Pagination, u as useTable } from "../../../../../chunks/Pagination.js";
/* empty css                                                             */
import { C as CreateNetworkModal } from "../../../../../chunks/CreateNetworkModal.js";
import { g as getDefaultNode } from "../../../../../chunks/nodes.js";
import { A as Arrow_left } from "../../../../../chunks/arrow-left.js";
import { P as Plus } from "../../../../../chunks/plus.js";
import { N as Network } from "../../../../../chunks/network.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    var $$store_subs;
    const token = getStoredToken();
    const client = createAPIClient({ token: token ?? void 0 });
    const nodeId = derived(() => store_get($$store_subs ??= {}, "$page", page).params.id);
    const node = derived(getDefaultNode);
    let items = [];
    let loading = true;
    let createModalOpen = false;
    let table = derived(() => useTable({ data: items, pageSize: 10 }));
    const columns = [
      {
        key: "name",
        title: "Name",
        sortable: true,
        render: (net) => net.name
      },
      {
        key: "mode",
        title: "Mode",
        sortable: true,
        width: "100px",
        render: (net) => net.mode
      },
      {
        key: "bridge_name",
        title: "Bridge",
        width: "120px",
        render: (net) => net.bridge_name
      },
      {
        key: "cidr",
        title: "CIDR",
        width: "140px",
        render: (net) => net.cidr
      },
      {
        key: "gateway_ip",
        title: "Gateway",
        width: "130px",
        render: (net) => net.gateway_ip || "—"
      },
      {
        key: "status",
        title: "Status",
        sortable: true,
        width: "100px",
        render: (net) => net.status
      }
    ];
    async function loadData() {
      loading = true;
      try {
        const response = await client.listNodeNetworks(nodeId());
        items = response.resources;
      } catch (err) {
        toast.error(err instanceof Error ? err.message : "Failed to load networks");
      } finally {
        loading = false;
      }
    }
    function handleSort(column, direction) {
      if (direction) {
        table().setSort(column, direction);
      } else {
        table().clearSort();
      }
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      head("10fndha", $$renderer3, ($$renderer4) => {
        $$renderer4.title(($$renderer5) => {
          $$renderer5.push(`<title>Networks | ${escape_html(node().name)}</title>`);
        });
      });
      $$renderer3.push(`<div class="space-y-6"><div class="flex items-center justify-between"><div class="flex items-center gap-4"><a${attr("href", `/nodes/${stringify(nodeId())}`)} class="p-2 text-slate-500 hover:text-slate-700 hover:bg-slate-100 rounded-lg transition-colors" aria-label="Back to node">`);
      Arrow_left($$renderer3, { size: 20 });
      $$renderer3.push(`<!----></a> <div><h1 class="text-2xl font-bold text-slate-900">Networks</h1> <p class="text-sm text-slate-500">Node: ${escape_html(node().name)}</p></div></div> <button class="inline-flex items-center gap-2 px-4 py-2 bg-orange-500 hover:bg-orange-600 text-white text-sm font-medium rounded-lg transition-colors">`);
      Plus($$renderer3, { size: 16 });
      $$renderer3.push(`<!----> Create Network</button></div> <div class="grid gap-4 md:grid-cols-4"><div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-green-50 rounded-lg">`);
      Network($$renderer3, { size: 20, class: "text-green-600" });
      $$renderer3.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">Total Networks</p> <p class="text-xl font-bold text-slate-900">${escape_html(items.length)}</p></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-blue-50 rounded-lg">`);
      Network($$renderer3, { size: 20, class: "text-blue-600" });
      $$renderer3.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">Bridge Mode</p> <p class="text-xl font-bold text-slate-900">${escape_html(items.filter((n) => n.mode === "bridge").length)}</p></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-purple-50 rounded-lg">`);
      Network($$renderer3, { size: 20, class: "text-purple-600" });
      $$renderer3.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">NAT Mode</p> <p class="text-xl font-bold text-slate-900">${escape_html(items.filter((n) => n.mode === "nat").length)}</p></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-green-50 rounded-lg">`);
      Network($$renderer3, { size: 20, class: "text-green-600" });
      $$renderer3.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">Active</p> <p class="text-xl font-bold text-slate-900">${escape_html(items.filter((n) => n.status === "active").length)}</p></div></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200">`);
      DataTable($$renderer3, {
        data: table().paginatedData,
        columns,
        loading,
        sortColumn: table().sortColumn,
        sortDirection: table().sortDirection,
        onSort: handleSort,
        rowId: (net) => net.id
      });
      $$renderer3.push(`<!----> `);
      Pagination($$renderer3, {
        page: table().page,
        pageSize: table().pageSize,
        totalItems: table().totalItems,
        onPageChange: (p) => table().setPage(p),
        onPageSizeChange: (size) => table().setPageSize(size)
      });
      $$renderer3.push(`<!----></div></div> `);
      CreateNetworkModal($$renderer3, {
        onSuccess: loadData,
        get open() {
          return createModalOpen;
        },
        set open($$value) {
          createModalOpen = $$value;
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
    if ($$store_subs) unsubscribe_stores($$store_subs);
  });
}
export {
  _page as default
};
