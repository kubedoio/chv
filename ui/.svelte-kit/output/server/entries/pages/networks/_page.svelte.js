import { f as derived } from "../../../chunks/root.js";
import "@sveltejs/kit/internal";
import "../../../chunks/exports.js";
import "../../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../../chunks/client.js";
import { g as getStoredToken, c as createAPIClient, t as toast } from "../../../chunks/client2.js";
import { D as DataTable } from "../../../chunks/DataTable.js";
import { u as useTable, P as Pagination } from "../../../chunks/Pagination.js";
import { F as FilterBar } from "../../../chunks/FilterBar.js";
import { S as StateBadge } from "../../../chunks/StateBadge.js";
import { C as CreateNetworkModal } from "../../../chunks/CreateNetworkModal.js";
import { P as Plus } from "../../../chunks/plus.js";
import { N as Network } from "../../../chunks/network.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    const token = getStoredToken();
    const client = createAPIClient({ token: token ?? void 0 });
    let items = [];
    let loading = true;
    let createModalOpen = false;
    let table = derived(() => useTable({ data: items, pageSize: 10 }));
    const filterOptions = [
      {
        key: "mode",
        label: "Mode",
        type: "select",
        options: [
          { value: "bridge", label: "Bridge" },
          { value: "nat", label: "NAT" },
          { value: "macvtap", label: "MacVTap" }
        ]
      },
      {
        key: "status",
        label: "Status",
        type: "select",
        options: [
          { value: "active", label: "Active" },
          { value: "inactive", label: "Inactive" },
          { value: "error", label: "Error" }
        ]
      }
    ];
    const columns = [
      { key: "name", title: "Name", sortable: true },
      {
        key: "mode",
        title: "Mode",
        sortable: true,
        align: "center",
        width: "100px",
        render: (net) => net.mode || "bridge"
      },
      {
        key: "bridge_name",
        title: "Bridge",
        width: "140px",
        render: (net) => net.bridge_name
      },
      {
        key: "cidr",
        title: "CIDR",
        width: "140px",
        render: (net) => net.cidr || "—"
      },
      {
        key: "gateway_ip",
        title: "Gateway",
        width: "130px",
        render: (net) => net.gateway_ip || "—"
      },
      {
        key: "is_system_managed",
        title: "Managed By",
        width: "120px",
        render: (net) => net.is_system_managed ? "System" : "User"
      }
    ];
    async function loadNetworks() {
      loading = true;
      try {
        items = await client.listNetworks();
      } catch (err) {
        toast.error("Failed to load networks");
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
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      $$renderer3.push(`<section class="table-card"><div class="card-header px-4 py-3 flex items-center justify-between"><div><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Networks</div> <div class="mt-1 text-lg font-semibold">Bridge-backed Networks</div></div> <button class="px-4 py-2 rounded bg-primary text-white font-medium text-sm hover:bg-primary/90 transition-colors flex items-center gap-2">`);
      Plus($$renderer3, { size: 16 });
      $$renderer3.push(`<!----> Create Network</button></div> `);
      FilterBar($$renderer3, {
        filters: filterOptions,
        activeFilters: table().filters,
        onFilterChange: table().setFilter,
        onClearAll: table().clearAllFilters
      });
      $$renderer3.push(`<!----> `);
      {
        let children = function($$renderer4, net) {
          StateBadge($$renderer4, { label: net.status });
        };
        DataTable($$renderer3, {
          data: table().paginatedData,
          columns,
          loading,
          sortColumn: table().sortColumn ?? void 0,
          sortDirection: table().sortDirection,
          emptyIcon: Network,
          emptyTitle: "No networks yet",
          emptyDescription: "Create a network to connect your VMs",
          onSort: handleSort,
          rowId: (net) => net.id,
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
      CreateNetworkModal($$renderer3, {
        onSuccess: loadNetworks,
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
  });
}
export {
  _page as default
};
