import { b as attr, c as escape_html, o as attributes, h as stringify, d as bind_props, i as derived } from "./renderer.js";
function FormField($$renderer, $$props) {
  let { label, error, helper, required = false, labelFor, children } = $$props;
  $$renderer.push(`<div class="flex flex-col gap-1"><label${attr("for", labelFor)} class="text-xs font-medium text-muted">${escape_html(label)} `);
  if (required) {
    $$renderer.push("<!--[0-->");
    $$renderer.push(`<span class="text-danger">*</span>`);
  } else {
    $$renderer.push("<!--[-1-->");
  }
  $$renderer.push(`<!--]--></label> `);
  children($$renderer);
  $$renderer.push(`<!----> `);
  if (helper && !error) {
    $$renderer.push("<!--[0-->");
    $$renderer.push(`<p class="text-xs text-muted mt-1">${escape_html(helper)}</p>`);
  } else {
    $$renderer.push("<!--[-1-->");
  }
  $$renderer.push(`<!--]--> `);
  if (error) {
    $$renderer.push("<!--[0-->");
    $$renderer.push(`<div class="flex items-center gap-1.5 mt-1" role="alert"><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-danger flex-shrink-0" aria-hidden="true"><circle cx="12" cy="12" r="10"></circle><line x1="12" x2="12" y1="8" y2="12"></line><line x1="12" x2="12.01" y1="16" y2="16"></line></svg> <p class="text-xs text-danger">${escape_html(error)}</p></div>`);
  } else {
    $$renderer.push("<!--[-1-->");
  }
  $$renderer.push(`<!--]--></div>`);
}
function Input($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let {
      value = "",
      error,
      type = "text",
      placeholder,
      disabled,
      id,
      class: className = "",
      $$slots,
      $$events,
      ...rest
    } = $$props;
    function focus() {
    }
    const baseClasses = "h-9 w-full rounded border px-3 py-2 text-sm font-sans transition-colors duration-150";
    const stateClasses = derived(() => () => {
      if (disabled) {
        return "border-[#CCCCCC] bg-gray-50 text-muted cursor-not-allowed";
      }
      if (error) {
        return "border-danger bg-white text-ink focus:border-danger focus:outline-none focus:ring-2 focus:ring-danger/20";
      }
      return "border-[#CCCCCC] bg-white text-ink hover:border-muted focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20";
    });
    $$renderer2.push(`<input${attributes(
      {
        value,
        type,
        placeholder,
        disabled,
        id,
        class: `${stringify(baseClasses)} ${stringify(stateClasses()())} ${stringify(className)}`,
        ...rest
      },
      void 0,
      void 0,
      void 0,
      4
    )}/>`);
    bind_props($$props, { value, focus });
  });
}
export {
  FormField as F,
  Input as I
};
