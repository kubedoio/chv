import { c as attr, e as escape_html, f as stringify } from "./root.js";
import "./EmptyState.svelte_svelte_type_style_lang.js";
function EmptyState($$renderer, $$props) {
  let { icon: Icon, title, description, children, role = "status" } = $$props;
  $$renderer.push(`<div class="empty-state svelte-13862ru"${attr("role", role)} aria-live="polite"${attr("aria-label", `${stringify(title)}: ${stringify(description)}`)}><div class="empty-state-icon svelte-13862ru" aria-hidden="true">`);
  if (Icon) {
    $$renderer.push("<!--[-->");
    Icon($$renderer, { size: 48 });
    $$renderer.push("<!--]-->");
  } else {
    $$renderer.push("<!--[!-->");
    $$renderer.push("<!--]-->");
  }
  $$renderer.push(`</div> <h2 class="empty-state-title svelte-13862ru">${escape_html(title)}</h2> <p class="empty-state-description svelte-13862ru">${escape_html(description)}</p> `);
  if (children) {
    $$renderer.push("<!--[0-->");
    $$renderer.push(`<div class="empty-state-actions svelte-13862ru">`);
    children($$renderer);
    $$renderer.push(`<!----></div>`);
  } else {
    $$renderer.push("<!--[-1-->");
  }
  $$renderer.push(`<!--]--></div>`);
}
export {
  EmptyState as E
};
