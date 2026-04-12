import { c as attr, e as escape_html, h as derived } from "../../../chunks/root.js";
import { g as goto } from "../../../chunks/client.js";
import { g as getStoredToken, c as createAPIClient, t as toast } from "../../../chunks/client2.js";
import { D as DataTable, S as Square } from "../../../chunks/DataTable.js";
import { u as useTable, P as Pagination } from "../../../chunks/Pagination.js";
import { F as FilterBar } from "../../../chunks/FilterBar.js";
/* empty css                                                       */
import { M as Minus, T as Trending_down, a as Trending_up, C as Circle_alert } from "../../../chunks/trending-up.js";
import { C as Chevron_right } from "../../../chunks/chevron-right.js";
import { C as CreateVMModal } from "../../../chunks/CreateVMModal.js";
import { C as ConfirmDialog } from "../../../chunks/ConfirmDialog.js";
import { P as Plus } from "../../../chunks/plus.js";
import { S as Server } from "../../../chunks/server.js";
import { P as Play } from "../../../chunks/play.js";
import { T as Trash_2 } from "../../../chunks/trash-2.js";
import { S as Settings } from "../../../chunks/settings.js";
function StatsCard($$renderer, $$props) {
  let { title, value, icon: Icon, trend, subtitle, href } = $$props;
  const trendConfig = {
    up: { icon: Trending_up, colorClass: "text-success" },
    down: { icon: Trending_down, colorClass: "text-danger" },
    neutral: { icon: Minus, colorClass: "text-light" }
  };
  if (href) {
    $$renderer.push("<!--[0-->");
    $$renderer.push(`<a${attr("href", href)} class="rounded border border-line bg-chrome p-4 block no-underline text-inherit hover:shadow-md transition-shadow"${attr("aria-label", title)}><div class="flex items-start justify-between"><div class="flex-1"><div class="mb-2 text-[11px] uppercase tracking-[0.08em] text-muted">${escape_html(title)}</div> <div class="flex items-center gap-2">`);
    if (Icon) {
      $$renderer.push("<!--[0-->");
      if (Icon) {
        $$renderer.push("<!--[-->");
        Icon($$renderer, { size: 24, class: "text-muted", "aria-hidden": "true" });
        $$renderer.push("<!--]-->");
      } else {
        $$renderer.push("<!--[!-->");
        $$renderer.push("<!--]-->");
      }
    } else {
      $$renderer.push("<!--[-1-->");
    }
    $$renderer.push(`<!--]--> <span class="text-[32px] font-semibold text-ink">${escape_html(value)}</span></div> `);
    if (subtitle) {
      $$renderer.push("<!--[0-->");
      $$renderer.push(`<div class="mt-1 text-sm text-muted">${escape_html(subtitle)}</div>`);
    } else {
      $$renderer.push("<!--[-1-->");
    }
    $$renderer.push(`<!--]--></div> `);
    if (trend) {
      $$renderer.push("<!--[0-->");
      const TrendIcon = trendConfig[trend].icon;
      const trendColor = trendConfig[trend].colorClass;
      $$renderer.push(`<div class="flex-shrink-0">`);
      if (TrendIcon) {
        $$renderer.push("<!--[-->");
        TrendIcon($$renderer, { size: 20, class: trendColor, "aria-hidden": "true" });
        $$renderer.push("<!--]-->");
      } else {
        $$renderer.push("<!--[!-->");
        $$renderer.push("<!--]-->");
      }
      $$renderer.push(`</div>`);
    } else {
      $$renderer.push("<!--[-1-->");
      $$renderer.push(`<div class="flex-shrink-0">`);
      Chevron_right($$renderer, { size: 20, class: "text-muted", "aria-hidden": "true" });
      $$renderer.push(`<!----></div>`);
    }
    $$renderer.push(`<!--]--></div></a>`);
  } else {
    $$renderer.push("<!--[-1-->");
    $$renderer.push(`<div class="rounded border border-line bg-chrome p-4" role="region"${attr("aria-label", title)}><div class="flex items-start justify-between"><div class="flex-1"><div class="mb-2 text-[11px] uppercase tracking-[0.08em] text-muted">${escape_html(title)}</div> <div class="flex items-center gap-2">`);
    if (Icon) {
      $$renderer.push("<!--[0-->");
      if (Icon) {
        $$renderer.push("<!--[-->");
        Icon($$renderer, { size: 24, class: "text-muted", "aria-hidden": "true" });
        $$renderer.push("<!--]-->");
      } else {
        $$renderer.push("<!--[!-->");
        $$renderer.push("<!--]-->");
      }
    } else {
      $$renderer.push("<!--[-1-->");
    }
    $$renderer.push(`<!--]--> <span class="text-[32px] font-semibold text-ink">${escape_html(value)}</span></div> `);
    if (subtitle) {
      $$renderer.push("<!--[0-->");
      $$renderer.push(`<div class="mt-1 text-sm text-muted">${escape_html(subtitle)}</div>`);
    } else {
      $$renderer.push("<!--[-1-->");
    }
    $$renderer.push(`<!--]--></div> `);
    if (trend) {
      $$renderer.push("<!--[0-->");
      const TrendIcon = trendConfig[trend].icon;
      const trendColor = trendConfig[trend].colorClass;
      $$renderer.push(`<div class="flex-shrink-0">`);
      if (TrendIcon) {
        $$renderer.push("<!--[-->");
        TrendIcon($$renderer, { size: 20, class: trendColor, "aria-hidden": "true" });
        $$renderer.push("<!--]-->");
      } else {
        $$renderer.push("<!--[!-->");
        $$renderer.push("<!--]-->");
      }
      $$renderer.push(`</div>`);
    } else {
      $$renderer.push("<!--[-1-->");
    }
    $$renderer.push(`<!--]--></div></div>`);
  }
  $$renderer.push(`<!--]-->`);
}
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
    let table = useTable({ data: [], pageSize: 10 });
    const totalCount = derived(() => items.length);
    function getSelectedIdsArray() {
      return Array.from(table.selectedIds);
    }
    function getImage(id) {
      return images.find((i) => i.id === id);
    }
    function getPool(id) {
      return pools.find((p) => p.id === id);
    }
    function getNetwork(id) {
      return networks.find((n) => n.id === id);
    }
    const runningCount = derived(() => {
      let count = 0;
      for (const vm of items) {
        if (vm.actual_state === "running") count++;
      }
      return count;
    });
    const stoppedCount = derived(() => {
      let count = 0;
      for (const vm of items) {
        if (vm.actual_state === "stopped") count++;
      }
      return count;
    });
    const otherCount = derived(() => {
      let count = 0;
      for (const vm of items) {
        if (vm.actual_state !== "running" && vm.actual_state !== "stopped") count++;
      }
      return count;
    });
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
        key: "network_id",
        title: "Network",
        render: (vm) => {
          const net = getNetwork(vm.network_id);
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
        const vms = await client.listVMs();
        items = vms ?? [];
        table.data = items;
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
        table.setSort(column, direction);
      } else {
        table.clearSort();
      }
    }
    function handleSelect(ids) {
      const newSet = new Set(ids);
      table.selectedIds.forEach((id) => {
        if (!newSet.has(id)) table.deselect(id);
      });
      ids.forEach((id) => {
        if (!table.selectedIds.has(id)) table.select(id);
      });
    }
    function navigateToVM(vm) {
      goto(`/vms/${vm.id}`);
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      $$renderer3.push(`<div class="flex justify-between items-start mb-6"><div class="grid grid-cols-2 lg:grid-cols-4 gap-4 flex-1">`);
      StatsCard($$renderer3, { title: "Total VMs", value: totalCount(), icon: Server });
      $$renderer3.push(`<!----> `);
      StatsCard($$renderer3, {
        title: "Running",
        value: runningCount(),
        icon: Play,
        trend: "up"
      });
      $$renderer3.push(`<!----> `);
      StatsCard($$renderer3, { title: "Stopped", value: stoppedCount(), icon: Square });
      $$renderer3.push(`<!----> `);
      StatsCard($$renderer3, { title: "Other", value: otherCount(), icon: Circle_alert });
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
        activeFilters: table.filters,
        onFilterChange: table.setFilter,
        onClearAll: table.clearAllFilters
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
          data: table.paginatedData,
          columns,
          loading,
          selectable: true,
          selectedIds: getSelectedIdsArray(),
          sortColumn: table.sortColumn ?? void 0,
          sortDirection: table.sortDirection,
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
      if (!loading && table.totalItems > 0) {
        $$renderer3.push("<!--[0-->");
        Pagination($$renderer3, {
          page: table.page,
          pageSize: table.pageSize,
          totalItems: table.totalItems,
          onPageChange: table.setPage,
          onPageSizeChange: table.setPageSize
        });
      } else {
        $$renderer3.push("<!--[-1-->");
      }
      $$renderer3.push(`<!--]--></section> `);
      if (table.selectedCount > 0) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<div class="fixed bottom-6 left-1/2 -translate-x-1/2 bg-ink text-white px-6 py-3 rounded-full shadow-2xl flex items-center gap-6 z-50 animate-in fade-in slide-in-from-bottom-4 duration-300 svelte-1httffh"><div class="flex items-center gap-2 border-r border-white/20 pr-6"><span class="bg-primary text-[10px] font-bold px-1.5 py-0.5 rounded uppercase tracking-wider">${escape_html(table.selectedCount)}</span> <span class="text-sm font-medium">Selected</span></div> <div class="flex items-center gap-4"><button class="flex items-center gap-2 text-sm hover:text-primary transition-colors font-medium">`);
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
