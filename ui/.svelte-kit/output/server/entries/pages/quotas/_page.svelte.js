import { j as bind_props, e as escape_html, c as attr, h as derived, g as ensure_array_like, d as attr_class, f as stringify, m as attr_style } from "../../../chunks/root.js";
import { c as createAPIClient, A as APIError, t as toast } from "../../../chunks/client2.js";
import { M as Modal } from "../../../chunks/Modal.js";
import { F as FormField, I as Input } from "../../../chunks/Input.js";
import { S as Settings } from "../../../chunks/settings.js";
import { S as Server } from "../../../chunks/server.js";
import { C as Cpu } from "../../../chunks/cpu.js";
import { H as Hard_drive } from "../../../chunks/hard-drive.js";
import { N as Network } from "../../../chunks/network.js";
import { T as Triangle_alert } from "../../../chunks/triangle-alert.js";
function QuotaSettingsModal($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { open = false, quota = null, users = [], onSuccess } = $$props;
    createAPIClient();
    const isEditing = derived(() => quota !== null);
    let userId = "";
    let maxVms = 10;
    let maxCpu = 20;
    let maxMemoryGb = 64;
    let maxStorageGb = 500;
    let maxNetworks = 5;
    let submitting = false;
    let errors = {};
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      {
        let footer = function($$renderer4) {
          $$renderer4.push(`<button type="button"${attr("disabled", submitting, true)} class="px-4 py-2 rounded border border-slate-200 text-slate-700 bg-white hover:bg-slate-50 transition-colors disabled:opacity-50 disabled:cursor-not-allowed">Cancel</button> <button type="submit" form="quota-form"${attr("disabled", submitting, true)} class="px-4 py-2 rounded bg-blue-600 text-white font-medium hover:bg-blue-700 transition-colors disabled:bg-blue-400 disabled:cursor-not-allowed flex items-center gap-2">`);
          {
            $$renderer4.push("<!--[-1-->");
          }
          $$renderer4.push(`<!--]--> ${escape_html(isEditing() ? "Update Quota" : "Create Quota")}</button>`);
        };
        Modal($$renderer3, {
          title: isEditing() ? "Edit Quota" : "Create Quota",
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
            $$renderer4.push(`<form id="quota-form" class="space-y-5">`);
            {
              $$renderer4.push("<!--[-1-->");
            }
            $$renderer4.push(`<!--]--> `);
            if (!isEditing()) {
              $$renderer4.push("<!--[0-->");
              FormField($$renderer4, {
                label: "User",
                error: errors.userId,
                required: true,
                labelFor: "quota-user",
                children: ($$renderer5) => {
                  $$renderer5.select(
                    {
                      id: "quota-user",
                      value: userId,
                      class: "h-9 w-full rounded border border-[#CCCCCC] bg-white px-3 py-2 text-sm disabled:opacity-50",
                      disabled: submitting
                    },
                    ($$renderer6) => {
                      $$renderer6.option({ value: "" }, ($$renderer7) => {
                        $$renderer7.push(`Select a user...`);
                      });
                      $$renderer6.push(`<!--[-->`);
                      const each_array = ensure_array_like(users);
                      for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
                        let user = each_array[$$index];
                        $$renderer6.option({ value: user.id }, ($$renderer7) => {
                          $$renderer7.push(`${escape_html(user.username)} (${escape_html(user.role)})`);
                        });
                      }
                      $$renderer6.push(`<!--]-->`);
                    }
                  );
                }
              });
            } else {
              $$renderer4.push("<!--[-1-->");
              $$renderer4.push(`<div class="rounded bg-slate-50 px-3 py-2 text-sm text-slate-600">Editing quota for: <span class="font-medium">${escape_html(quota?.user_id)}</span></div>`);
            }
            $$renderer4.push(`<!--]--> <div class="grid grid-cols-2 gap-4">`);
            FormField($$renderer4, {
              label: "Max VMs",
              error: errors.maxVms,
              required: true,
              labelFor: "quota-vms",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "quota-vms",
                  type: "number",
                  min: 0,
                  disabled: submitting,
                  get value() {
                    return maxVms;
                  },
                  set value($$value) {
                    maxVms = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "Max CPU Cores",
              error: errors.maxCpu,
              required: true,
              labelFor: "quota-cpu",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "quota-cpu",
                  type: "number",
                  min: 0,
                  disabled: submitting,
                  get value() {
                    return maxCpu;
                  },
                  set value($$value) {
                    maxCpu = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----></div> <div class="grid grid-cols-2 gap-4">`);
            FormField($$renderer4, {
              label: "Max Memory (GB)",
              error: errors.maxMemoryGb,
              required: true,
              labelFor: "quota-memory",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "quota-memory",
                  type: "number",
                  min: 0,
                  step: 1,
                  disabled: submitting,
                  get value() {
                    return maxMemoryGb;
                  },
                  set value($$value) {
                    maxMemoryGb = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "Max Storage (GB)",
              error: errors.maxStorageGb,
              required: true,
              labelFor: "quota-storage",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "quota-storage",
                  type: "number",
                  min: 0,
                  step: 10,
                  disabled: submitting,
                  get value() {
                    return maxStorageGb;
                  },
                  set value($$value) {
                    maxStorageGb = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----></div> `);
            FormField($$renderer4, {
              label: "Max Networks",
              error: errors.maxNetworks,
              required: true,
              labelFor: "quota-networks",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "quota-networks",
                  type: "number",
                  min: 0,
                  disabled: submitting,
                  get value() {
                    return maxNetworks;
                  },
                  set value($$value) {
                    maxNetworks = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> <div class="rounded bg-amber-50 border border-amber-200 px-3 py-2 text-sm text-amber-700"><p class="font-medium mb-1">Default Values</p> <p class="text-xs">VMs: 10, CPU: 20, Memory: 64GB, Storage: 500GB, Networks: 5</p></div></form>`);
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
    let usageData = null;
    let allQuotas = [];
    let users = [];
    let loading = true;
    let error = null;
    let showSettingsModal = false;
    let editingQuota = null;
    const client = createAPIClient();
    const resources = [
      {
        key: "vms",
        label: "Virtual Machines",
        icon: Server,
        unit: "",
        color: "bg-blue-500"
      },
      {
        key: "cpus",
        label: "CPU Cores",
        icon: Cpu,
        unit: "",
        color: "bg-green-500"
      },
      {
        key: "memory_gb",
        label: "Memory",
        icon: Hard_drive,
        unit: "GB",
        color: "bg-purple-500"
      },
      {
        key: "storage_gb",
        label: "Storage",
        icon: Hard_drive,
        unit: "GB",
        color: "bg-orange-500"
      },
      {
        key: "networks",
        label: "Networks",
        icon: Network,
        unit: "",
        color: "bg-pink-500"
      }
    ];
    async function loadQuotaData() {
      loading = true;
      error = null;
      try {
        usageData = await client.getUsage();
      } catch (err) {
        console.error("Failed to load quota data:", err);
        if (err instanceof APIError) {
          error = err.message;
        } else {
          error = "Failed to load quota data. Please try again.";
        }
        toast.error(error);
      } finally {
        loading = false;
      }
    }
    async function loadAllQuotas() {
      try {
        allQuotas = await client.listQuotas();
      } catch (err) {
        console.error("Failed to load quotas:", err);
      }
    }
    function handleModalSuccess() {
      loadQuotaData();
      loadAllQuotas();
    }
    function getUsageValue(key) {
      if (!usageData) return 0;
      switch (key) {
        case "vms":
          return usageData.usage.vms;
        case "cpus":
          return usageData.usage.cpus;
        case "memory_gb":
          return usageData.usage.memory_gb;
        case "storage_gb":
          return usageData.usage.storage_gb;
        case "networks":
          return usageData.usage.networks;
        default:
          return 0;
      }
    }
    function getQuotaValue(key) {
      if (!usageData) return 0;
      switch (key) {
        case "vms":
          return usageData.quota.max_vms;
        case "cpus":
          return usageData.quota.max_cpu;
        case "memory_gb":
          return usageData.quota.max_memory_gb;
        case "storage_gb":
          return usageData.quota.max_storage_gb;
        case "networks":
          return usageData.quota.max_networks;
        default:
          return 0;
      }
    }
    function getPercentage(key) {
      const used = getUsageValue(key);
      const max = getQuotaValue(key);
      if (max === 0) return 0;
      return Math.min(100, Math.round(used / max * 100));
    }
    function formatValue(key, value) {
      const unit = resources.find((r) => r.key === key)?.unit || "";
      return `${value}${unit ? " " + unit : ""}`;
    }
    function getStatusColor(percentage) {
      if (percentage >= 90) return "text-red-500";
      if (percentage >= 75) return "text-amber-500";
      return "text-green-500";
    }
    function getProgressColor(percentage) {
      if (percentage >= 90) return "bg-red-500";
      if (percentage >= 75) return "bg-amber-500";
      return "bg-green-500";
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      $$renderer3.push(`<div class="space-y-6"><div class="flex items-center justify-between"><div><h1 class="text-2xl font-bold text-slate-900">Resource Quotas</h1> <p class="text-sm text-slate-500 mt-1">Monitor your resource usage and limits</p></div> <div class="flex items-center gap-2"><button class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors flex items-center gap-2">`);
      Settings($$renderer3, { size: 18 });
      $$renderer3.push(`<!----> Adjust Quota</button> <button${attr("disabled", loading, true)} class="px-4 py-2 bg-white border border-slate-200 text-slate-700 rounded-lg hover:bg-slate-50 transition-colors disabled:opacity-50 flex items-center gap-2"><span${attr_class("animate-spin", void 0, { "hidden": !loading })}>↻</span> Refresh</button></div></div> `);
      if (loading) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<div class="flex items-center justify-center py-12"><div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div> <span class="ml-3 text-slate-500">Loading quota data...</span></div>`);
      } else if (error) {
        $$renderer3.push("<!--[1-->");
        $$renderer3.push(`<div class="bg-red-50 border border-red-200 rounded-lg p-6 text-center"><div class="text-red-500 text-lg font-medium mb-2">Failed to Load</div> <p class="text-red-600 text-sm mb-4">${escape_html(error)}</p> <button class="px-4 py-2 bg-red-500 text-white rounded-lg hover:bg-red-600 transition-colors">Try Again</button></div>`);
      } else if (usageData) {
        $$renderer3.push("<!--[2-->");
        const criticalResources = resources.filter((r) => getPercentage(r.key) >= 95);
        const warningResources = resources.filter((r) => {
          const p = getPercentage(r.key);
          return p >= 80 && p < 95;
        });
        const highUsageResources = resources.filter((r) => getPercentage(r.key) >= 90);
        const warningResources2 = resources.filter((r) => {
          const p = getPercentage(r.key);
          return p >= 75 && p < 90;
        });
        if (criticalResources.length > 0) {
          $$renderer3.push("<!--[0-->");
          $$renderer3.push(`<div class="bg-red-50 border border-red-200 rounded-lg p-4 flex items-start gap-3">`);
          Triangle_alert($$renderer3, { class: "text-red-500 mt-0.5", size: 20 });
          $$renderer3.push(`<!----> <div><h3 class="text-sm font-medium text-red-800">Critical Usage Alert</h3> <p class="text-sm text-red-700 mt-1">The following resources are at critical usage levels (≥95%):
            ${escape_html(criticalResources.map((r) => `${r.label} (${getPercentage(r.key)}%)`).join(", "))}</p></div></div>`);
        } else if (warningResources.length > 0) {
          $$renderer3.push("<!--[1-->");
          $$renderer3.push(`<div class="bg-amber-50 border border-amber-200 rounded-lg p-4 flex items-start gap-3">`);
          Triangle_alert($$renderer3, { class: "text-amber-500 mt-0.5", size: 20 });
          $$renderer3.push(`<!----> <div><h3 class="text-sm font-medium text-amber-800">Usage Warning</h3> <p class="text-sm text-amber-700 mt-1">The following resources are approaching limits (≥80%):
            ${escape_html(warningResources.map((r) => `${r.label} (${getPercentage(r.key)}%)`).join(", "))}</p></div></div>`);
        } else {
          $$renderer3.push("<!--[-1-->");
        }
        $$renderer3.push(`<!--]--> <div class="grid grid-cols-1 md:grid-cols-3 gap-4"><!--[-->`);
        const each_array = ensure_array_like(resources);
        for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
          let resource = each_array[$$index];
          const Icon = resource.icon;
          const used = getUsageValue(resource.key);
          const max = getQuotaValue(resource.key);
          const percentage = getPercentage(resource.key);
          $$renderer3.push(`<div class="bg-white rounded-lg border border-slate-200 p-5 hover:shadow-md transition-shadow"><div class="flex items-start justify-between"><div class="flex items-center gap-3"><div${attr_class(`p-2 rounded-lg ${stringify(resource.color)} bg-opacity-10`)}>`);
          if (Icon) {
            $$renderer3.push("<!--[-->");
            Icon($$renderer3, { size: 20, class: resource.color.replace("bg-", "text-") });
            $$renderer3.push("<!--]-->");
          } else {
            $$renderer3.push("<!--[!-->");
            $$renderer3.push("<!--]-->");
          }
          $$renderer3.push(`</div> <div><h3 class="text-sm font-medium text-slate-600">${escape_html(resource.label)}</h3> <div class="flex items-baseline gap-1 mt-1"><span class="text-2xl font-bold text-slate-900">${escape_html(used)}</span> <span class="text-sm text-slate-500">/ ${escape_html(formatValue(resource.key, max))}</span></div></div></div> <div class="text-right"><span${attr_class(`text-sm font-medium ${stringify(getStatusColor(percentage))}`)}>${escape_html(percentage)}%</span></div></div> <div class="mt-4"><div class="h-2 bg-slate-100 rounded-full overflow-hidden"><div${attr_class(`h-full ${stringify(getProgressColor(percentage))} transition-all duration-500`)}${attr_style(`width: ${stringify(percentage)}%`)}></div></div> <p class="text-xs text-slate-500 mt-2">${escape_html(formatValue(resource.key, max - used))} available</p></div></div>`);
        }
        $$renderer3.push(`<!--]--></div> <div class="bg-white rounded-lg border border-slate-200 p-6"><h2 class="text-lg font-semibold text-slate-900 mb-4">Quota Summary</h2> <div class="grid grid-cols-1 md:grid-cols-2 gap-6"><div><h3 class="text-sm font-medium text-slate-600 mb-3">Current Limits</h3> <dl class="space-y-2"><!--[-->`);
        const each_array_1 = ensure_array_like(resources);
        for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
          let resource = each_array_1[$$index_1];
          $$renderer3.push(`<div class="flex justify-between text-sm"><dt class="text-slate-500">${escape_html(resource.label)}</dt> <dd class="font-medium text-slate-900">${escape_html(formatValue(resource.key, getQuotaValue(resource.key)))}</dd></div>`);
        }
        $$renderer3.push(`<!--]--></dl></div> <div><h3 class="text-sm font-medium text-slate-600 mb-3">Current Usage</h3> <dl class="space-y-2"><!--[-->`);
        const each_array_2 = ensure_array_like(resources);
        for (let $$index_2 = 0, $$length = each_array_2.length; $$index_2 < $$length; $$index_2++) {
          let resource = each_array_2[$$index_2];
          const percentage = getPercentage(resource.key);
          $$renderer3.push(`<div class="flex justify-between text-sm"><dt class="text-slate-500">${escape_html(resource.label)}</dt> <dd${attr_class(`font-medium ${stringify(getStatusColor(percentage))}`)}>${escape_html(formatValue(resource.key, getUsageValue(resource.key)))}
                  (${escape_html(percentage)}%)</dd></div>`);
        }
        $$renderer3.push(`<!--]--></dl></div></div></div>  `);
        if (highUsageResources.length > 0) {
          $$renderer3.push("<!--[0-->");
          $$renderer3.push(`<div class="bg-red-50 border border-red-200 rounded-lg p-4"><h3 class="text-sm font-medium text-red-800 mb-2">⚠️ High Usage Alerts</h3> <ul class="space-y-1"><!--[-->`);
          const each_array_3 = ensure_array_like(highUsageResources);
          for (let $$index_3 = 0, $$length = each_array_3.length; $$index_3 < $$length; $$index_3++) {
            let resource = each_array_3[$$index_3];
            $$renderer3.push(`<li class="text-sm text-red-700">${escape_html(resource.label)} is at ${escape_html(getPercentage(resource.key))}% capacity</li>`);
          }
          $$renderer3.push(`<!--]--></ul></div>`);
        } else {
          $$renderer3.push("<!--[-1-->");
        }
        $$renderer3.push(`<!--]--> `);
        if (warningResources2.length > 0) {
          $$renderer3.push("<!--[0-->");
          $$renderer3.push(`<div class="bg-amber-50 border border-amber-200 rounded-lg p-4"><h3 class="text-sm font-medium text-amber-800 mb-2">⚡ Usage Warnings</h3> <ul class="space-y-1"><!--[-->`);
          const each_array_4 = ensure_array_like(warningResources2);
          for (let $$index_4 = 0, $$length = each_array_4.length; $$index_4 < $$length; $$index_4++) {
            let resource = each_array_4[$$index_4];
            $$renderer3.push(`<li class="text-sm text-amber-700">${escape_html(resource.label)} is at ${escape_html(getPercentage(resource.key))}% capacity</li>`);
          }
          $$renderer3.push(`<!--]--></ul></div>`);
        } else {
          $$renderer3.push("<!--[-1-->");
        }
        $$renderer3.push(`<!--]-->`);
      } else {
        $$renderer3.push("<!--[-1-->");
      }
      $$renderer3.push(`<!--]--></div> `);
      QuotaSettingsModal($$renderer3, {
        quota: editingQuota,
        users,
        onSuccess: handleModalSuccess,
        get open() {
          return showSettingsModal;
        },
        set open($$value) {
          showSettingsModal = $$value;
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
