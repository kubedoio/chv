import { d as attr_class, e as escape_html, h as derived } from "./root.js";
/* empty css                                         */
function StateBadge($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { label } = $$props;
    const tone = derived(() => {
      const val = label.toLowerCase();
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
    });
    const transitioning = derived(() => ["starting", "stopping", "importing", "provisioning"].includes(label.toLowerCase()));
    $$renderer2.push(`<span${attr_class(`state-badge inline-flex items-center gap-1.5 border px-2.5 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider ${tone()}`, "svelte-502yak")}><span${attr_class(`status-dot w-1.5 h-1.5 rounded-full ${transitioning() ? "animate-pulse" : ""}`, "svelte-502yak")} style="background-color: currentColor"></span> ${escape_html(label.replaceAll("_", " "))}</span>`);
  });
}
export {
  StateBadge as S
};
