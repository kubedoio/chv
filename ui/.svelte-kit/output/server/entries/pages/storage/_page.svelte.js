import { k as bind_props, c as attr, e as escape_html, f as derived } from "../../../chunks/root.js";
import "@sveltejs/kit/internal";
import "../../../chunks/exports.js";
import "../../../chunks/utils.js";
import "@sveltejs/kit/internal/server";
import "../../../chunks/client.js";
import { c as createAPIClient, g as getStoredToken, t as toast } from "../../../chunks/client2.js";
import { D as DataTable } from "../../../chunks/DataTable.js";
import { f as formatBytes, u as useTable, P as Pagination } from "../../../chunks/Pagination.js";
import { F as FilterBar } from "../../../chunks/FilterBar.js";
import { S as StateBadge } from "../../../chunks/StateBadge.js";
import { M as Modal } from "../../../chunks/Modal.js";
import { F as FormField, I as Input } from "../../../chunks/Input.js";
import { S as Select } from "../../../chunks/Select.js";
import { P as Plus } from "../../../chunks/plus.js";
import { H as Hard_drive } from "../../../chunks/hard-drive.js";
function CreateStoragePoolModal($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { open = false, onSuccess, existingNames = [] } = $$props;
    createAPIClient({ token: getStoredToken() ?? void 0 });
    let name = "";
    let poolType = "localdisk";
    let path = "";
    let capacity = "";
    let submitting = false;
    let nameError = "";
    let pathError = "";
    const nameRegex = /^[a-z0-9-]+$/;
    const typeOptions = [{ value: "localdisk", label: "localdisk" }];
    function validateName() {
      if (!name.trim()) {
        nameError = "Name is required";
        return false;
      }
      if (!nameRegex.test(name)) {
        nameError = "Name must contain only lowercase letters, numbers, and hyphens";
        return false;
      }
      if (name.startsWith("-") || name.endsWith("-")) {
        nameError = "Name cannot start or end with a hyphen";
        return false;
      }
      if (existingNames.includes(name.trim())) {
        nameError = "A storage pool with this name already exists";
        return false;
      }
      nameError = "";
      return true;
    }
    function validatePath() {
      if (!path.trim()) {
        pathError = "Path is required";
        return false;
      }
      if (!path.startsWith("/")) {
        pathError = 'Path must be an absolute path (start with "/")';
        return false;
      }
      pathError = "";
      return true;
    }
    function isValid() {
      if (!name.trim() || !path.trim()) {
        return false;
      }
      if (!nameRegex.test(name) || name.startsWith("-") || name.endsWith("-")) {
        return false;
      }
      if (existingNames.includes(name.trim())) {
        return false;
      }
      if (!path.startsWith("/")) {
        return false;
      }
      return true;
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      {
        let footer = function($$renderer4) {
          $$renderer4.push(`<button type="button"${attr("disabled", submitting, true)} class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50 disabled:cursor-not-allowed">Cancel</button> <button type="submit" form="create-storage-pool-form"${attr("disabled", !isValid() || submitting, true)} class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors disabled:bg-primary/30 disabled:cursor-not-allowed flex items-center gap-2">`);
          {
            $$renderer4.push("<!--[-1-->");
          }
          $$renderer4.push(`<!--]--> ${escape_html("Create Pool")}</button>`);
        };
        Modal($$renderer3, {
          title: "Create Storage Pool",
          closeOnBackdrop: !submitting,
          get open() {
            return open;
          },
          set open($$value) {
            open = $$value;
            $$settled = false;
          },
          footer,
          children: ($$renderer4) => {
            $$renderer4.push(`<form id="create-storage-pool-form" class="space-y-5">`);
            {
              $$renderer4.push("<!--[-1-->");
            }
            $$renderer4.push(`<!--]--> `);
            FormField($$renderer4, {
              label: "Name",
              error: nameError,
              required: true,
              labelFor: "pool-name",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "pool-name",
                  placeholder: "my-pool",
                  disabled: submitting,
                  onblur: validateName,
                  get value() {
                    return name;
                  },
                  set value($$value) {
                    name = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "Type",
              helper: "Only 'localdisk' type is supported in MVP-1",
              labelFor: "pool-type",
              children: ($$renderer5) => {
                Select($$renderer5, {
                  id: "pool-type",
                  options: typeOptions,
                  disabled: true,
                  get value() {
                    return poolType;
                  },
                  set value($$value) {
                    poolType = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "Path",
              error: pathError,
              required: true,
              helper: "Absolute path on host filesystem",
              labelFor: "pool-path",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "pool-path",
                  placeholder: "/var/lib/chv/storage/my-pool",
                  disabled: submitting,
                  onblur: validatePath,
                  get value() {
                    return path;
                  },
                  set value($$value) {
                    path = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "Capacity",
              helper: "Optional - Storage capacity in bytes (for display only)",
              labelFor: "pool-capacity",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "pool-capacity",
                  type: "number",
                  placeholder: "10737418240",
                  disabled: submitting,
                  min: "0",
                  get value() {
                    return capacity;
                  },
                  set value($$value) {
                    capacity = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----></form>`);
          },
          $$slots: { footer: true, default: true }
        });
      }
    }
    do {
      $$settled = true;
      $$inner_renderer = $$renderer2.copy();
      $$render_inner($$inner_renderer);
    } while (!$$settled);
    $$renderer2.subsume($$inner_renderer);
    bind_props($$props, { open });
  });
}
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
        key: "pool_type",
        label: "Type",
        type: "select",
        options: [{ value: "localdisk", label: "Local Disk" }]
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
        key: "pool_type",
        title: "Type",
        align: "center",
        width: "100px",
        render: (pool) => pool.pool_type === "localdisk" ? "Local" : pool.pool_type
      },
      { key: "path", title: "Path", render: (pool) => pool.path },
      {
        key: "capacity_bytes",
        title: "Capacity",
        sortable: true,
        align: "right",
        width: "120px",
        render: (pool) => {
          if (!pool.capacity_bytes) return "—";
          return formatBytes(pool.capacity_bytes);
        }
      },
      {
        key: "allocatable_bytes",
        title: "Available",
        align: "right",
        width: "120px",
        render: (pool) => {
          if (!pool.allocatable_bytes) return "—";
          return formatBytes(pool.allocatable_bytes);
        }
      },
      {
        key: "used",
        title: "Used",
        align: "right",
        width: "120px",
        render: (pool) => {
          if (!pool.capacity_bytes || !pool.allocatable_bytes) return "—";
          const used = pool.capacity_bytes - pool.allocatable_bytes;
          return formatBytes(used);
        }
      },
      {
        key: "is_default",
        title: "Default",
        align: "center",
        width: "80px",
        render: (pool) => pool.is_default ? "Yes" : "No"
      }
    ];
    async function loadStoragePools() {
      loading = true;
      try {
        items = await client.listStoragePools();
      } catch (err) {
        toast.error("Failed to load storage pools");
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
      $$renderer3.push(`<section class="table-card"><div class="card-header px-4 py-3 flex items-center justify-between"><div><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Storage</div> <div class="mt-1 text-lg font-semibold">Storage Pools</div></div> <button class="px-4 py-2 rounded bg-primary text-white font-medium text-sm hover:bg-primary/90 transition-colors flex items-center gap-2">`);
      Plus($$renderer3, { size: 16 });
      $$renderer3.push(`<!----> Create Pool</button></div> `);
      FilterBar($$renderer3, {
        filters: filterOptions,
        activeFilters: table().filters,
        onFilterChange: table().setFilter,
        onClearAll: table().clearAllFilters
      });
      $$renderer3.push(`<!----> `);
      {
        let children = function($$renderer4, pool) {
          StateBadge($$renderer4, { label: pool.status });
        };
        DataTable($$renderer3, {
          data: table().paginatedData,
          columns,
          loading,
          sortColumn: table().sortColumn ?? void 0,
          sortDirection: table().sortDirection,
          emptyIcon: Hard_drive,
          emptyTitle: "No storage pools yet",
          emptyDescription: "Create a storage pool to store VM disks",
          onSort: handleSort,
          rowId: (pool) => pool.id,
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
      CreateStoragePoolModal($$renderer3, {
        onSuccess: loadStoragePools,
        existingNames: items.map((i) => i.name),
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
