import { n as head, c as attr, d as ensure_array_like, e as escape_html, f as derived } from "../../../chunks/root.js";
import { o as onDestroy } from "../../../chunks/index-server.js";
import { c as createAPIClient, g as getStoredToken, t as toast } from "../../../chunks/client2.js";
import { S as StateBadge } from "../../../chunks/StateBadge.js";
import { S as SkeletonRow } from "../../../chunks/EmptyState.svelte_svelte_type_style_lang.js";
import { E as EmptyState } from "../../../chunks/EmptyState.js";
import { R as Refresh_cw } from "../../../chunks/refresh-cw.js";
import { F as Funnel } from "../../../chunks/funnel.js";
import { C as Clock } from "../../../chunks/clock.js";
import { A as Activity } from "../../../chunks/activity.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    const client = createAPIClient({ token: getStoredToken() ?? void 0 });
    let events = [];
    let loading = true;
    let autoRefresh = true;
    let sinceAppLoad = true;
    let newEventCount = 0;
    let lastEventCount = 0;
    let filterOperation = "";
    let filterStatus = "";
    let filterResource = "";
    const appStartTime = /* @__PURE__ */ new Date();
    async function loadEvents() {
      try {
        const params = new URLSearchParams();
        if (filterOperation) ;
        if (filterStatus) ;
        if (filterResource) ;
        const query = params.toString();
        let data = await client.listEvents(query ? `?${query}` : "");
        if (sinceAppLoad) {
          data = data.filter((e) => new Date(e.timestamp) >= appStartTime);
        }
        if (events.length > 0 && data.length > lastEventCount) {
          newEventCount = data.length - lastEventCount;
        }
        lastEventCount = data.length;
        events = data;
      } catch {
        toast.error("Failed to load events");
      } finally {
        loading = false;
      }
    }
    onDestroy(() => {
    });
    const operations = derived(() => [...new Set(events.map((e) => e.operation))].sort());
    const resources = derived(() => [...new Set(events.map((e) => e.resource))].sort());
    function formatTime(ts) {
      return new Date(ts).toLocaleString();
    }
    head("13hsgdq", $$renderer2, ($$renderer3) => {
      $$renderer3.title(($$renderer4) => {
        $$renderer4.push(`<title>Events | chv</title>`);
      });
    });
    $$renderer2.push(`<section class="table-card svelte-13hsgdq"><div class="card-header px-4 py-3 svelte-13hsgdq"><div class="flex items-center justify-between"><div><div class="text-[11px] uppercase tracking-[0.16em] text-muted">Operations</div> <div class="mt-1 text-lg font-semibold">Event Log</div></div> <div class="flex items-center gap-3"><label class="flex items-center gap-2 text-sm cursor-pointer"><input type="checkbox"${attr("checked", autoRefresh, true)}/> <span class="text-muted">Auto-refresh (10s)</span> `);
    if (newEventCount > 0) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<span class="bg-red-500 text-white text-xs px-2 py-0.5 rounded-full">${escape_html(newEventCount)} new</span>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></label> <label class="flex items-center gap-2 text-sm cursor-pointer"><input type="checkbox"${attr("checked", sinceAppLoad, true)}/> <span class="text-muted">Since app load</span></label> <button class="p-2 hover:bg-chrome rounded">`);
    Refresh_cw($$renderer2, { size: 16 });
    $$renderer2.push(`<!----></button></div></div> <div class="flex items-center gap-3 mt-4">`);
    Funnel($$renderer2, { size: 16, class: "text-muted" });
    $$renderer2.push(`<!----> `);
    $$renderer2.select(
      {
        value: filterOperation,
        onchange: loadEvents,
        class: "border border-line rounded px-3 py-1.5 text-sm"
      },
      ($$renderer3) => {
        $$renderer3.option({ value: "" }, ($$renderer4) => {
          $$renderer4.push(`All Operations`);
        });
        $$renderer3.push(`<!--[-->`);
        const each_array = ensure_array_like(operations());
        for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
          let op = each_array[$$index];
          $$renderer3.option({ value: op }, ($$renderer4) => {
            $$renderer4.push(`${escape_html(op)}`);
          });
        }
        $$renderer3.push(`<!--]-->`);
      }
    );
    $$renderer2.push(` `);
    $$renderer2.select(
      {
        value: filterStatus,
        onchange: loadEvents,
        class: "border border-line rounded px-3 py-1.5 text-sm"
      },
      ($$renderer3) => {
        $$renderer3.option({ value: "" }, ($$renderer4) => {
          $$renderer4.push(`All Statuses`);
        });
        $$renderer3.option({ value: "success" }, ($$renderer4) => {
          $$renderer4.push(`Success`);
        });
        $$renderer3.option({ value: "failed" }, ($$renderer4) => {
          $$renderer4.push(`Failed`);
        });
        $$renderer3.option({ value: "pending" }, ($$renderer4) => {
          $$renderer4.push(`Pending`);
        });
      }
    );
    $$renderer2.push(` `);
    $$renderer2.select(
      {
        value: filterResource,
        onchange: loadEvents,
        class: "border border-line rounded px-3 py-1.5 text-sm"
      },
      ($$renderer3) => {
        $$renderer3.option({ value: "" }, ($$renderer4) => {
          $$renderer4.push(`All Resources`);
        });
        $$renderer3.push(`<!--[-->`);
        const each_array_1 = ensure_array_like(resources());
        for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
          let res = each_array_1[$$index_1];
          $$renderer3.option({ value: res }, ($$renderer4) => {
            $$renderer4.push(`${escape_html(res)}`);
          });
        }
        $$renderer3.push(`<!--]-->`);
      }
    );
    $$renderer2.push(` `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div></div> `);
    if (loading) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Timestamp</th><th class="border-b border-line px-4 py-3">Operation</th><th class="border-b border-line px-4 py-3">Status</th><th class="border-b border-line px-4 py-3">Resource</th><th class="border-b border-line px-4 py-3">Message</th></tr></thead><tbody><!--[-->`);
      const each_array_2 = ensure_array_like(Array(5));
      for (let $$index_2 = 0, $$length = each_array_2.length; $$index_2 < $$length; $$index_2++) {
        each_array_2[$$index_2];
        SkeletonRow($$renderer2, { columns: 5 });
      }
      $$renderer2.push(`<!--]--></tbody></table>`);
    } else if (events.length === 0) {
      $$renderer2.push("<!--[1-->");
      EmptyState($$renderer2, {
        icon: Activity,
        title: "No events",
        description: "No events since app loaded. Events will appear here as operations complete."
      });
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<table class="w-full border-collapse text-sm"><thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted"><tr><th class="border-b border-line px-4 py-3">Timestamp</th><th class="border-b border-line px-4 py-3">Operation</th><th class="border-b border-line px-4 py-3">Status</th><th class="border-b border-line px-4 py-3">Resource</th><th class="border-b border-line px-4 py-3">Message</th></tr></thead><tbody><!--[-->`);
      const each_array_3 = ensure_array_like(events);
      for (let $$index_3 = 0, $$length = each_array_3.length; $$index_3 < $$length; $$index_3++) {
        let event = each_array_3[$$index_3];
        $$renderer2.push(`<tr class="odd:bg-white even:bg-[#f8f8f8]"><td class="border-b border-line px-4 py-3 text-muted whitespace-nowrap"><div class="flex items-center gap-2">`);
        Clock($$renderer2, { size: 14 });
        $$renderer2.push(`<!----> ${escape_html(formatTime(event.timestamp))}</div></td><td class="border-b border-line px-4 py-3 font-medium">${escape_html(event.operation)}</td><td class="border-b border-line px-4 py-3">`);
        StateBadge($$renderer2, { label: event.status });
        $$renderer2.push(`<!----></td><td class="border-b border-line px-4 py-3"><span class="text-xs bg-chrome px-2 py-1 rounded">${escape_html(event.resource)}</span></td><td class="border-b border-line px-4 py-3 text-muted">${escape_html(event.message || "-")}</td></tr>`);
      }
      $$renderer2.push(`<!--]--></tbody></table>`);
    }
    $$renderer2.push(`<!--]--></section>`);
  });
}
export {
  _page as default
};
