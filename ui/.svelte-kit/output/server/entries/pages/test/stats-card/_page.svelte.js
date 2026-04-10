import "clsx";
import { S as StatsCard, P as Play } from "../../../../chunks/StatsCard.js";
import { S as Server } from "../../../../chunks/server.js";
import { S as Square } from "../../../../chunks/square.js";
import { A as Activity } from "../../../../chunks/activity.js";
function _page($$renderer) {
  $$renderer.push(`<div class="p-6"><h1 class="mb-6 text-xl font-semibold text-ink">Stats Card Test</h1> <div class="mb-8 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">`);
  StatsCard($$renderer, { title: "Total VMs", value: 12, icon: Server });
  $$renderer.push(`<!----> `);
  StatsCard($$renderer, { title: "Running", value: 8, icon: Play, trend: "up" });
  $$renderer.push(`<!----> `);
  StatsCard($$renderer, { title: "Stopped", value: 4, icon: Square, trend: "neutral" });
  $$renderer.push(`<!----> `);
  StatsCard($$renderer, {
    title: "CPU Usage",
    value: "64%",
    icon: Activity,
    trend: "down"
  });
  $$renderer.push(`<!----></div> <h2 class="mb-4 text-lg font-medium text-muted">Without Icons</h2> <div class="mb-8 grid grid-cols-1 gap-4 sm:grid-cols-3">`);
  StatsCard($$renderer, { title: "Memory Used", value: "8.4 GB", trend: "up" });
  $$renderer.push(`<!----> `);
  StatsCard($$renderer, { title: "Disk I/O", value: "124 MB/s" });
  $$renderer.push(`<!----> `);
  StatsCard($$renderer, { title: "Network", value: "2.1 Gbps", trend: "down" });
  $$renderer.push(`<!----></div> <h2 class="mb-4 text-lg font-medium text-muted">Single Cards</h2> <div class="space-y-4">`);
  StatsCard($$renderer, { title: "Just a value", value: 42 });
  $$renderer.push(`<!----> `);
  StatsCard($$renderer, { title: "With icon only", value: 99, icon: Server });
  $$renderer.push(`<!----> `);
  StatsCard($$renderer, { title: "With trend only", value: 50, trend: "up" });
  $$renderer.push(`<!----></div></div>`);
}
export {
  _page as default
};
