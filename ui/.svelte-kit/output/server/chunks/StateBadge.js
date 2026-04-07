import { a as attr_class, c as escape_html, d as bind_props } from "./renderer.js";
function StateBadge($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let label = $$props["label"];
    const toneClass = (value) => {
      switch (value) {
        case "ready":
        case "running":
        case "active":
        case "succeeded":
          return "border-success text-success";
        case "degraded":
        case "warning":
        case "starting":
        case "stopping":
        case "importing":
          return "border-warning text-warning";
        case "error":
        case "failed":
        case "missing_prerequisites":
        case "drift_detected":
          return "border-danger text-danger";
        default:
          return "border-line text-muted";
      }
    };
    $$renderer2.push(`<span${attr_class(`inline-flex items-center gap-2 border px-2 py-1 text-[12px] font-medium uppercase tracking-[0.12em] ${toneClass(label)}`)}><span class="text-[10px]">●</span> ${escape_html(label.replaceAll("_", " "))}</span>`);
    bind_props($$props, { label });
  });
}
export {
  StateBadge as S
};
