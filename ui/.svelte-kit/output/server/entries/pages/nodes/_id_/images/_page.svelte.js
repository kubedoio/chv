import { n as head, e as escape_html, c as attr, h as stringify, i as derived, d as store_get, u as unsubscribe_stores } from "../../../../../chunks/root.js";
import { p as page } from "../../../../../chunks/stores.js";
import "@sveltejs/kit/internal";
import "../../../../../chunks/exports.js";
import "../../../../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../../../../chunks/client.js";
import { g as getStoredToken, c as createAPIClient, t as toast } from "../../../../../chunks/client2.js";
import { D as DataTable } from "../../../../../chunks/DataTable.js";
import { u as useTable, P as Pagination } from "../../../../../chunks/Pagination.js";
/* empty css                                                             */
import { I as ImportImageModal } from "../../../../../chunks/ImportImageModal.js";
import { g as getDefaultNode } from "../../../../../chunks/nodes.js";
import { A as Arrow_left } from "../../../../../chunks/arrow-left.js";
import { D as Download } from "../../../../../chunks/download.js";
import { I as Image } from "../../../../../chunks/image.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    var $$store_subs;
    const token = getStoredToken();
    const client = createAPIClient({ token: token ?? void 0 });
    const nodeId = derived(() => store_get($$store_subs ??= {}, "$page", page).params.id);
    const node = derived(getDefaultNode);
    let items = [];
    let loading = true;
    let importModalOpen = false;
    let table = useTable({ data: [], pageSize: 10 });
    const columns = [
      {
        key: "name",
        title: "Name",
        sortable: true,
        render: (img) => img.name
      },
      {
        key: "os_family",
        title: "OS Family",
        sortable: true,
        render: (img) => img.os_family
      },
      {
        key: "architecture",
        title: "Architecture",
        sortable: true,
        width: "120px",
        render: (img) => img.architecture
      },
      {
        key: "status",
        title: "Status",
        sortable: true,
        width: "120px",
        render: (img) => img.status
      },
      {
        key: "cloud_init_supported",
        title: "Cloud-Init",
        width: "120px",
        render: (img) => img.cloud_init_supported ? "Supported" : "Not Supported"
      },
      {
        key: "created_at",
        title: "Created",
        sortable: true,
        width: "150px",
        render: (img) => new Date(img.created_at).toLocaleDateString()
      }
    ];
    async function loadData() {
      loading = true;
      try {
        const response = await client.listNodeImages(nodeId());
        items = response.resources;
      } catch (err) {
        toast.error(err instanceof Error ? err.message : "Failed to load images");
      } finally {
        loading = false;
      }
    }
    function handleSort(column, direction) {
      if (direction) {
        table.setSort(column, direction);
      } else {
        table.clearSort();
      }
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      head("i8zqt3", $$renderer3, ($$renderer4) => {
        $$renderer4.title(($$renderer5) => {
          $$renderer5.push(`<title>Images | ${escape_html(node().name)}</title>`);
        });
      });
      $$renderer3.push(`<div class="space-y-6"><div class="flex items-center justify-between"><div class="flex items-center gap-4"><a${attr("href", `/nodes/${stringify(nodeId())}`)} class="p-2 text-slate-500 hover:text-slate-700 hover:bg-slate-100 rounded-lg transition-colors" aria-label="Back to node">`);
      Arrow_left($$renderer3, { size: 20 });
      $$renderer3.push(`<!----></a> <div><h1 class="text-2xl font-bold text-slate-900">Images</h1> <p class="text-sm text-slate-500">Node: ${escape_html(node().name)}</p></div></div> <button class="inline-flex items-center gap-2 px-4 py-2 bg-orange-500 hover:bg-orange-600 text-white text-sm font-medium rounded-lg transition-colors">`);
      Download($$renderer3, { size: 16 });
      $$renderer3.push(`<!----> Import Image</button></div> <div class="grid gap-4 md:grid-cols-4"><div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-purple-50 rounded-lg">`);
      Image($$renderer3, { size: 20, class: "text-purple-600" });
      $$renderer3.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">Total Images</p> <p class="text-xl font-bold text-slate-900">${escape_html(items.length)}</p></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-green-50 rounded-lg">`);
      Image($$renderer3, { size: 20, class: "text-green-600" });
      $$renderer3.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">Ready</p> <p class="text-xl font-bold text-slate-900">${escape_html(items.filter((i) => i.status === "ready").length)}</p></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-blue-50 rounded-lg">`);
      Image($$renderer3, { size: 20, class: "text-blue-600" });
      $$renderer3.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">With Cloud-Init</p> <p class="text-xl font-bold text-slate-900">${escape_html(items.filter((i) => i.cloud_init_supported).length)}</p></div></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200">`);
      DataTable($$renderer3, {
        data: table.paginatedData,
        columns,
        loading,
        sortColumn: table.sortColumn,
        sortDirection: table.sortDirection,
        onSort: handleSort,
        rowId: (img) => img.id
      });
      $$renderer3.push(`<!----> `);
      Pagination($$renderer3, {
        page: table.page,
        pageSize: table.pageSize,
        totalItems: table.totalItems,
        onPageChange: (p) => table.setPage(p),
        onPageSizeChange: (size) => table.setPageSize(size)
      });
      $$renderer3.push(`<!----></div></div> `);
      ImportImageModal($$renderer3, {
        onSuccess: loadData,
        get open() {
          return importModalOpen;
        },
        set open($$value) {
          importModalOpen = $$value;
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
