import { c as escape_html } from "./renderer.js";
import "clsx";
function EmptyState($$renderer, $$props) {
  let { icon: Icon, title, description, children } = $$props;
  $$renderer.push(`<div class="flex flex-col items-center justify-center py-12 text-center" role="status" aria-live="polite"><div class="mb-4 text-line">`);
  if (Icon) {
    $$renderer.push("<!--[-->");
    Icon($$renderer, { size: 48 });
    $$renderer.push("<!--]-->");
  } else {
    $$renderer.push("<!--[!-->");
    $$renderer.push("<!--]-->");
  }
  $$renderer.push(`</div> <h3 class="mb-2 text-base font-medium text-muted">${escape_html(title)}</h3> <p class="mb-6 max-w-sm text-sm text-light">${escape_html(description)}</p> `);
  if (children) {
    $$renderer.push("<!--[0-->");
    $$renderer.push(`<div>`);
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
