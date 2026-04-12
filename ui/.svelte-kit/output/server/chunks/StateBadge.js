import { f as attr_class, e as escape_html, j as bind_props } from "./root.js";
/* empty css                                         */
function StateBadge($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let label = $$props["label"];
    const toneClass = (value) => {
      const val = value.toLowerCase();
      switch (val) {
        case "ready":
        case "running":
        case "active":
        case "succeeded":
          return "bg-success/15 text-success-dark border-success/20 glow-success";
        case "degraded":
        case "warning":
        case "starting":
        case "stopping":
        case "importing":
        case "provisioning":
        case "prepared":
          return "bg-warning/15 text-warning-dark border-warning/20 glow-warning";
        case "error":
        case "failed":
        case "missing_prerequisites":
        case "drift_detected":
          return "bg-danger/15 text-danger-dark border-danger/20 glow-danger";
        default:
          return "bg-slate-100 text-slate-600 border-slate-200";
      }
    };
    const isTransitioning = (value) => {
      return ["starting", "stopping", "importing", "provisioning"].includes(value.toLowerCase());
    };
    $$renderer2.push(`<span${attr_class(`state-badge inline-flex items-center gap-1.5 border px-2.5 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider ${toneClass(label)}`, "svelte-502yak")}><span${attr_class(`status-dot w-1.5 h-1.5 rounded-full ${isTransitioning(label) ? "animate-pulse" : ""}`, "svelte-502yak")} style="background-color: currentColor"></span> ${escape_html(label.replaceAll("_", " "))}</span>`);
    bind_props($$props, { label });
  });
}
export {
  StateBadge as S
};
