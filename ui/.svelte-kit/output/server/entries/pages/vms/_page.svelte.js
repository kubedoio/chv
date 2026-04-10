import { e as escape_html, c as attr, f as derived } from "../../../chunks/renderer.js";
import { g as goto } from "../../../chunks/client.js";
import { g as getStoredToken, c as createAPIClient } from "../../../chunks/client2.js";
import { t as toast } from "../../../chunks/toast.js";
import { D as DataTable } from "../../../chunks/DataTable.js";
import { u as useTable, P as Pagination } from "../../../chunks/Pagination.js";
import { F as FilterBar } from "../../../chunks/FilterBar.js";
/* empty css                                                       */
import { S as StatsCard, P as Play } from "../../../chunks/StatsCard.js";
import { C as CreateVMModal } from "../../../chunks/CreateVMModal.js";
import { C as ConfirmDialog } from "../../../chunks/ConfirmDialog.js";
import { S as Server } from "../../../chunks/server.js";
import { S as Square } from "../../../chunks/square.js";
import { C as Circle_alert } from "../../../chunks/circle-alert.js";
import { P as Plus } from "../../../chunks/plus.js";
import { T as Trash_2 } from "../../../chunks/trash-2.js";
import { S as Settings } from "../../../chunks/settings.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    const token = getStoredToken();
    const client = createAPIClient({ token: token ?? void 0 });
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
    let table = derived(() => useTable({ data: items, pageSize: 10 }));
    const imageMap = derived(() => new Map(images.map((i) => [i.id, i])));
    const poolMap = derived(() => new Map(pools.map((p) => [p.id, p])));
    const networkMap = derived(() => new Map(networks.map((n) => [n.id, n])));
    const total = derived(() => items.length);
    const running = derived(() => items.filter((vm) => vm.actual_state === "running").length);
    const stopped = derived(() => items.filter((vm) => vm.actual_state === "stopped").length);
    const other = derived(() => items.filter((vm) => !["running", "stopped"].includes(vm.actual_state)).length);
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
          const img = imageMap().get(vm.image_id);
          return img?.name ?? vm.image_id;
        }
      },
      {
        key: "storage_pool_id",
        title: "Pool",
        render: (vm) => {
          const pool = poolMap().get(vm.storage_pool_id);
          return pool?.name ?? vm.storage_pool_id;
        }
      },
      {
        key: "network_id",
        title: "Network",
        render: (vm) => {
          const net = networkMap().get(vm.network_id);
          return net?.name ?? vm.network_id;
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
    async function loadVMs() {
      loading = true;
      error = "";
      try {
        items = await client.listVMs();
      } catch (err) {
        error = err instanceof Error ? err.message : "Failed to load VMs";
        toast.error(error);
        items = [];
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
    function handleSelect(ids) {
      const newSet = new Set(ids);
      table().selectedIds.forEach((id) => {
        if (!newSet.has(id)) table().deselect(id);
      });
      ids.forEach((id) => {
        if (!table().selectedIds.has(id)) table().select(id);
      });
    }
    function navigateToVM(vm) {
      goto(`/vms/${vm.id}`);
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      $$renderer3.push(`<div class="flex justify-between items-start mb-6"><div class="grid grid-cols-2 lg:grid-cols-4 gap-4 flex-1">`);
      StatsCard($$renderer3, { title: "Total VMs", value: total(), icon: Server });
      $$renderer3.push(`<!----> `);
      StatsCard($$renderer3, { title: "Running", value: running(), icon: Play, trend: "up" });
      $$renderer3.push(`<!----> `);
      StatsCard($$renderer3, { title: "Stopped", value: stopped(), icon: Square });
      $$renderer3.push(`<!----> `);
      StatsCard($$renderer3, { title: "Other", value: other(), icon: Circle_alert });
      $$renderer3.push(`<!----></div> <button class="ml-4 button-primary flex items-center gap-2">`);
      Plus($$renderer3, { size: 16 });
      $$renderer3.push(`<!----> Create VM</button></div> `);
      if (error) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<div class="mb-4 border border-danger bg-red-50 px-4 py-3 text-danger">${escape_html(error)}</div>`);
      } else {
        $$renderer3.push("<!--[-1-->");
      }
      $$renderer3.push(`<!--]--> <section class="table-card">`);
      FilterBar($$renderer3, {
        filters: filterOptions,
        activeFilters: table().filters,
        onFilterChange: table().setFilter,
        onClearAll: table().clearAllFilters
      });
      $$renderer3.push(`<!----> `);
      {
        let children = function($$renderer4, vm) {
          $$renderer4.push(`<div class="flex items-center gap-1">`);
          if (vm.actual_state === "stopped") {
            $$renderer4.push("<!--[0-->");
            $$renderer4.push(`<button type="button" class="action-btn start svelte-1httffh" title="Start VM">`);
            Play($$renderer4, { size: 14 });
            $$renderer4.push(`<!----></button>`);
          } else if (vm.actual_state === "running") {
            $$renderer4.push("<!--[1-->");
            $$renderer4.push(`<button type="button" class="action-btn stop svelte-1httffh" title="Stop VM">`);
            Square($$renderer4, { size: 14 });
            $$renderer4.push(`<!----></button>`);
          } else {
            $$renderer4.push("<!--[-1-->");
          }
          $$renderer4.push(`<!--]--> <a${attr("href", `/vms/${vm.id}`)} class="action-btn svelte-1httffh" title="Settings">`);
          Settings($$renderer4, { size: 14 });
          $$renderer4.push(`<!----></a> <button type="button" class="action-btn danger svelte-1httffh" title="Delete VM">`);
          Trash_2($$renderer4, { size: 14 });
          $$renderer4.push(`<!----></button></div>`);
        };
        DataTable($$renderer3, {
          data: table().paginatedData,
          columns,
          loading,
          selectable: true,
          selectedIds: Array.from(table().selectedIds),
          sortColumn: table().sortColumn ?? void 0,
          sortDirection: table().sortDirection,
          emptyIcon: Server,
          emptyTitle: "No VMs yet",
          emptyDescription: "Create a virtual machine to get started",
          onSort: handleSort,
          onSelect: handleSelect,
          onRowClick: navigateToVM,
          rowId: (vm) => vm.id,
          children
        });
      }
      $$renderer3.push(`<!----> `);
      if (!loading && table().totalItems > 0) {
        $$renderer3.push("<!--[0-->");
        Pagination($$renderer3, {
          page: table().page,
          pageSize: table().pageSize,
          totalItems: table().totalItems,
          onPageChange: table().setPage,
          onPageSizeChange: table().setPageSize
        });
      } else {
        $$renderer3.push("<!--[-1-->");
      }
      $$renderer3.push(`<!--]--></section> `);
      if (table().selectedCount > 0) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<div class="fixed bottom-6 left-1/2 -translate-x-1/2 bg-ink text-white px-6 py-3 rounded-full shadow-2xl flex items-center gap-6 z-50 animate-in fade-in slide-in-from-bottom-4 duration-300 svelte-1httffh"><div class="flex items-center gap-2 border-r border-white/20 pr-6"><span class="bg-primary text-[10px] font-bold px-1.5 py-0.5 rounded uppercase tracking-wider">${escape_html(table().selectedCount)}</span> <span class="text-sm font-medium">Selected</span></div> <div class="flex items-center gap-4"><button class="flex items-center gap-2 text-sm hover:text-primary transition-colors font-medium">`);
        Play($$renderer3, { size: 14, fill: "currentColor" });
        $$renderer3.push(`<!----> Start</button> <button class="flex items-center gap-2 text-sm hover:text-primary transition-colors font-medium">`);
        Square($$renderer3, { size: 14, fill: "currentColor" });
        $$renderer3.push(`<!----> Stop</button> <button class="flex items-center gap-2 text-sm text-danger hover:text-red-400 transition-colors font-medium">`);
        Trash_2($$renderer3, { size: 14 });
        $$renderer3.push(`<!----> Delete</button></div> <button class="ml-2 text-white/50 hover:text-white transition-colors">Cancel</button></div>`);
      } else {
        $$renderer3.push("<!--[-1-->");
      }
      $$renderer3.push(`<!--]--> `);
      CreateVMModal($$renderer3, {
        images,
        pools,
        networks,
        onSuccess: loadVMs,
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
