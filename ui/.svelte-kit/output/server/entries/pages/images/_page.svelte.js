import { e as escape_html, d as attr_class, f as stringify, m as attr_style, c as attr, h as derived } from "../../../chunks/root.js";
import { o as onDestroy } from "../../../chunks/index-server.js";
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
import { I as ImportImageModal } from "../../../chunks/ImportImageModal.js";
import { R as Refresh_cw } from "../../../chunks/refresh-cw.js";
import { P as Plus } from "../../../chunks/plus.js";
import { I as Image } from "../../../chunks/image.js";
function ProgressBar($$renderer, $$props) {
  let {
    value,
    max = 100,
    label = "",
    showValue = true,
    size = "md",
    color = "blue"
  } = $$props;
  const percentage = derived(() => Math.min(100, Math.max(0, value / max * 100)));
  const sizeClasses = { sm: "h-1.5", md: "h-2", lg: "h-4" };
  const colorClasses = {
    blue: "bg-blue-600",
    green: "bg-green-600",
    yellow: "bg-yellow-500",
    red: "bg-red-600"
  };
  $$renderer.push(`<div class="w-full">`);
  if (label) {
    $$renderer.push("<!--[0-->");
    $$renderer.push(`<div class="flex justify-between items-center mb-1"><span class="text-sm text-gray-600">${escape_html(label)}</span> `);
    if (showValue) {
      $$renderer.push("<!--[0-->");
      $$renderer.push(`<span class="text-sm font-medium text-gray-900">${escape_html(percentage().toFixed(0))}%</span>`);
    } else {
      $$renderer.push("<!--[-1-->");
    }
    $$renderer.push(`<!--]--></div>`);
  } else {
    $$renderer.push("<!--[-1-->");
  }
  $$renderer.push(`<!--]--> <div${attr_class(`w-full bg-gray-200 rounded-full ${stringify(sizeClasses[size])}`)}><div${attr_class(`${stringify(colorClasses[color])} rounded-full transition-all duration-300 ease-out ${stringify(sizeClasses[size])}`)}${attr_style(`width: ${stringify(percentage())}%`)} role="progressbar"${attr("aria-valuenow", value)}${attr("aria-valuemin", 0)}${attr("aria-valuemax", max)}></div></div></div>`);
}
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    const token = getStoredToken();
    const client = createAPIClient({ token: token ?? void 0 });
    let items = [];
    let loading = true;
    let importModalOpen = false;
    let progressMap = /* @__PURE__ */ new Map();
    const hasImportingImages = derived(() => items.some((img) => img.status === "importing"));
    let table = useTable({ data: [], pageSize: 10 });
    const filterOptions = [
      {
        key: "status",
        label: "Status",
        type: "select",
        options: [
          { value: "ready", label: "Ready" },
          { value: "importing", label: "Importing" },
          { value: "pending", label: "Pending" },
          { value: "failed", label: "Failed" }
        ]
      },
      {
        key: "os_family",
        label: "OS Family",
        type: "select",
        options: [
          { value: "linux", label: "Linux" },
          { value: "windows", label: "Windows" },
          { value: "bsd", label: "BSD" }
        ]
      }
    ];
    const columns = [
      { key: "name", title: "Name", sortable: true },
      {
        key: "os_family",
        title: "OS Family",
        render: (img) => img.os_family || "Unknown"
      },
      {
        key: "architecture",
        title: "Architecture",
        align: "center",
        width: "110px",
        render: (img) => img.architecture || "—"
      },
      {
        key: "status",
        title: "Status",
        sortable: true,
        width: "200px",
        render: (img) => {
          const progress = progressMap.get(img.id);
          if (img.status === "importing" && progress) {
            return "importing";
          }
          return img.status;
        }
      },
      {
        key: "cloud_init_supported",
        title: "Cloud-Init",
        align: "center",
        width: "100px",
        render: (img) => img.cloud_init_supported ? "Yes" : "No"
      },
      {
        key: "format",
        title: "Format",
        align: "center",
        width: "80px"
      },
      {
        key: "local_path",
        title: "Path",
        render: (img) => {
          const parts = img.local_path.split("/");
          return parts[parts.length - 1];
        }
      }
    ];
    async function loadImages() {
      loading = true;
      try {
        items = await client.listImages() ?? [];
      } catch {
        toast.error("Failed to load images");
        items = [];
      } finally {
        loading = false;
      }
    }
    onDestroy(() => {
    });
    function handleSort(column, direction) {
      if (direction) {
        table.setSort(column, direction);
      } else {
        table.clearSort();
      }
    }
    function getProgressForImage(imageId) {
      return progressMap.get(imageId);
    }
    function getProgressColor(status) {
      switch (status) {
        case "ready":
          return "green";
        case "failed":
          return "red";
        case "validating":
          return "yellow";
        default:
          return "blue";
      }
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      $$renderer3.push(`<section class="table-card"><div class="card-header px-4 py-3 flex justify-between items-center"><div><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Images</div> <div class="mt-1 text-lg font-semibold">Cloud Images</div></div> <div class="flex items-center gap-2"><button class="p-2 hover:bg-chrome rounded" title="Refresh">`);
      Refresh_cw($$renderer3, { size: 16, class: hasImportingImages() ? "animate-spin" : "" });
      $$renderer3.push(`<!----></button> <button class="button-primary flex items-center gap-2 px-4 py-2 rounded text-sm">`);
      Plus($$renderer3, { size: 16 });
      $$renderer3.push(`<!----> Import</button></div></div> `);
      FilterBar($$renderer3, {
        filters: filterOptions,
        activeFilters: table.filters,
        onFilterChange: table.setFilter,
        onClearAll: table.clearAllFilters
      });
      $$renderer3.push(`<!----> `);
      {
        let children = function($$renderer4, img) {
          const progress = getProgressForImage(img.id);
          if (img.status === "importing" && progress) {
            $$renderer4.push("<!--[0-->");
            $$renderer4.push(`<div class="w-32">`);
            ProgressBar($$renderer4, {
              value: progress.progress_percent,
              size: "sm",
              color: getProgressColor(progress.status)
            });
            $$renderer4.push(`<!----> `);
            if (progress.speed && progress.speed !== "0 B/s") {
              $$renderer4.push("<!--[0-->");
              $$renderer4.push(`<div class="text-[10px] text-muted mt-1">${escape_html(progress.speed)}</div>`);
            } else {
              $$renderer4.push("<!--[-1-->");
            }
            $$renderer4.push(`<!--]--></div>`);
          } else {
            $$renderer4.push("<!--[-1-->");
            StateBadge($$renderer4, { label: img.status });
          }
          $$renderer4.push(`<!--]-->`);
        };
        DataTable($$renderer3, {
          data: table.paginatedData,
          columns,
          loading,
          sortColumn: table.sortColumn ?? void 0,
          sortDirection: table.sortDirection,
          emptyIcon: Image,
          emptyTitle: "No images yet",
          emptyDescription: "Import cloud images to create VMs from",
          onSort: handleSort,
          rowId: (img) => img.id,
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
      ImportImageModal($$renderer3, {
        onSuccess: loadImages,
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
  });
}
export {
  _page as default
};
