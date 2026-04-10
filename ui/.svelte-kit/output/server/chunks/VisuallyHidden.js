import "clsx";
function VisuallyHidden($$renderer, $$props) {
  let { children, as = "span" } = $$props;
  if (as === "div") {
    $$renderer.push("<!--[0-->");
    $$renderer.push(`<div class="sr-only svelte-1764d75">`);
    children($$renderer);
    $$renderer.push(`<!----></div>`);
  } else if (as === "p") {
    $$renderer.push("<!--[1-->");
    $$renderer.push(`<p class="sr-only svelte-1764d75">`);
    children($$renderer);
    $$renderer.push(`<!----></p>`);
  } else {
    $$renderer.push("<!--[-1-->");
    $$renderer.push(`<span class="sr-only svelte-1764d75">`);
    children($$renderer);
    $$renderer.push(`<!----></span>`);
  }
  $$renderer.push(`<!--]-->`);
}
export {
  VisuallyHidden as V
};
