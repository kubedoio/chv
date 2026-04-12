import { n as head, e as escape_html, c as attr, f as stringify, h as derived, i as store_get, u as unsubscribe_stores } from "../../../../../chunks/root.js";
import { p as page } from "../../../../../chunks/stores.js";
import "@sveltejs/kit/internal";
import "../../../../../chunks/exports.js";
import "../../../../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../../../../chunks/client.js";
import { g as getStoredToken, c as createAPIClient, t as toast } from "../../../../../chunks/client2.js";
import { D as DataTable } from "../../../../../chunks/DataTable.js";
import { u as useTable, P as Pagination } from "../../../../../chunks/Pagination.js";
import { F as FilterBar } from "../../../../../chunks/FilterBar.js";
/* empty css                                                             */
import { C as CreateVMModal } from "../../../../../chunks/CreateVMModal.js";
import { C as ConfirmDialog } from "../../../../../chunks/ConfirmDialog.js";
import { A as Arrow_left } from "../../../../../chunks/arrow-left.js";
import { P as Plus } from "../../../../../chunks/plus.js";
import { S as Server } from "../../../../../chunks/server.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    var $$store_subs;
    const token = getStoredToken();
    const client = createAPIClient({ token: token ?? void 0 });
    const nodeId = derived(() => store_get($$store_subs ??= {}, "$page", page).params.id);
    let node = null;
    let items = [];
    let loading = true;
    let error = "";
    let createModalOpen = false;
    let images = [];
    let pools = [];
    let networks = [];
    let confirmDialog = {
      open: false,
      title: "",
      description: "",
      action: async () => {
      }
    };
    let table = useTable({ data: [], pageSize: 10 });
    function getImage(id) {
      return images.find((i) => i.id === id);
    }
    function getPool(id) {
      return pools.find((p) => p.id === id);
    }
    const filterOptions = [
      {
        key: "actual_state",
        label: "State",
        type: "select",
        options: [
          { value: "running", label: "Running" },
          { value: "stopped", label: "Stopped" },
          { value: "creating", label: "Creating" },
          { value: "error", label: "Error" }
        ]
      }
    ];
    const columns = [
      {
        key: "name",
        title: "Name",
        sortable: true,
        render: (vm) => vm.name
      },
      {
        key: "actual_state",
        title: "State",
        sortable: true,
        width: "140px",
        render: (vm) => {
          if (vm.desired_state === vm.actual_state) {
            return vm.actual_state;
          }
          return `${vm.actual_state} → ${vm.desired_state}`;
        }
      },
      {
        key: "image_id",
        title: "Image",
        render: (vm) => {
          const img = getImage(vm.image_id);
          return img?.name ?? vm.image_id;
        }
      },
      {
        key: "storage_pool_id",
        title: "Pool",
        render: (vm) => {
          const pool = getPool(vm.storage_pool_id);
          return pool?.name ?? vm.storage_pool_id;
        }
      },
      {
        key: "vcpu",
        title: "vCPU",
        sortable: true,
        align: "center",
        width: "80px"
      },
      {
        key: "memory_mb",
        title: "Memory",
        sortable: true,
        align: "right",
        width: "100px",
        render: (vm) => `${vm.memory_mb} MB`
      },
      {
        key: "ip_address",
        title: "IP Address",
        width: "130px",
        render: (vm) => vm.ip_address || "—"
      }
    ];
    async function loadData() {
      loading = true;
      error = "";
      try {
        const [nodeData, vmsResponse, imgs, ps, nets] = await Promise.all([
          client.getNode(nodeId()),
          client.listNodeVMs(nodeId()),
          client.listImages(),
          client.listStoragePools(),
          client.listNetworks()
        ]);
        node = nodeData;
        items = vmsResponse.resources;
        images = imgs;
        pools = ps;
        networks = nets;
      } catch (err) {
        error = err instanceof Error ? err.message : "Failed to load VMs";
        toast.error(error);
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
      head("aylhsx", $$renderer3, ($$renderer4) => {
        $$renderer4.title(($$renderer5) => {
          $$renderer5.push(`<title>Virtual Machines | ${escape_html(node?.name ?? "Node")}</title>`);
        });
      });
      $$renderer3.push(`<div class="space-y-6"><div class="flex items-center justify-between"><div class="flex items-center gap-4"><a${attr("href", `/nodes/${stringify(nodeId())}`)} class="p-2 text-slate-500 hover:text-slate-700 hover:bg-slate-100 rounded-lg transition-colors" aria-label="Back to node">`);
      Arrow_left($$renderer3, { size: 20 });
      $$renderer3.push(`<!----></a> <div><h1 class="text-2xl font-bold text-slate-900">Virtual Machines</h1> <p class="text-sm text-slate-500">Node: ${escape_html(node?.name ?? "Loading...")}</p></div></div> <button class="inline-flex items-center gap-2 px-4 py-2 bg-orange-500 hover:bg-orange-600 text-white text-sm font-medium rounded-lg transition-colors">`);
      Plus($$renderer3, { size: 16 });
      $$renderer3.push(`<!----> Create VM</button></div> <div class="grid gap-4 md:grid-cols-4"><div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-blue-50 rounded-lg">`);
      Server($$renderer3, { size: 20, class: "text-blue-600" });
      $$renderer3.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">Total VMs</p> <p class="text-xl font-bold text-slate-900">${escape_html(items.length)}</p></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-green-50 rounded-lg">`);
      Server($$renderer3, { size: 20, class: "text-green-600" });
      $$renderer3.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">Running</p> <p class="text-xl font-bold text-slate-900">${escape_html(items.filter((v) => v.actual_state === "running").length)}</p></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-slate-100 rounded-lg">`);
      Server($$renderer3, { size: 20, class: "text-slate-600" });
      $$renderer3.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">Stopped</p> <p class="text-xl font-bold text-slate-900">${escape_html(items.filter((v) => v.actual_state === "stopped").length)}</p></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200 p-4"><div class="flex items-center gap-3"><div class="p-2 bg-red-50 rounded-lg">`);
      Server($$renderer3, { size: 20, class: "text-red-600" });
      $$renderer3.push(`<!----></div> <div><p class="text-xs text-slate-500 uppercase">Error</p> <p class="text-xl font-bold text-slate-900">${escape_html(items.filter((v) => v.actual_state === "error").length)}</p></div></div></div></div> <div class="bg-white rounded-lg shadow-sm border border-slate-200">`);
      FilterBar($$renderer3, {
        filters: filterOptions,
        activeFilters: { actual_state: table.filters.actual_state },
        onFilterChange: (key, value) => table.setFilter(key, value),
        onClearAll: table.clearAllFilters
      });
      $$renderer3.push(`<!----> `);
      DataTable($$renderer3, {
        data: table.paginatedData,
        columns,
        loading,
        selectable: true,
        selectedIds: Array.from(table.selectedIds),
        sortColumn: table.sortColumn,
        sortDirection: table.sortDirection,
        onSort: handleSort,
        onSelect: (ids) => {
          table.selectNone();
          ids.forEach((id) => table.select(id));
        },
        rowId: (vm) => vm.id
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
      CreateVMModal($$renderer3, {
        images,
        pools,
        networks,
        onSuccess: loadData,
        get open() {
          return createModalOpen;
        },
        set open($$value) {
          createModalOpen = $$value;
          $$settled = false;
        }
      });
      $$renderer3.push(`<!----> `);
      ConfirmDialog($$renderer3, {
        title: confirmDialog.title,
        description: confirmDialog.description,
        confirmLabel: "Delete",
        onConfirm: confirmDialog.action,
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
    if ($$store_subs) unsubscribe_stores($$store_subs);
  });
}
export {
  _page as default
};
