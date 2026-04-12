import { c as attr, e as escape_html, h as derived, l as attributes, f as stringify, j as bind_props } from "./root.js";
import "clsx";
/* empty css                                             */
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
function FormField($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { label, error, helper, required = false, labelFor, children } = $$props;
    let fieldId = derived(() => labelFor || `field-${Math.random().toString(36).slice(2, 9)}`);
    let helperId = derived(() => helper ? `${fieldId()}-helper` : void 0);
    let errorId = derived(() => error ? `${fieldId()}-error` : void 0);
    $$renderer2.push(`<div class="form-field svelte-py80wu"><label${attr("for", fieldId())} class="form-label svelte-py80wu">${escape_html(label)} `);
    if (required) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<span class="required-indicator svelte-py80wu" aria-hidden="true">*</span> `);
      VisuallyHidden($$renderer2, {
        children: ($$renderer3) => {
          $$renderer3.push(`<!---->(required)`);
        }
      });
      $$renderer2.push(`<!---->`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></label>  `);
    children($$renderer2);
    $$renderer2.push(`<!----> `);
    if (helper && !error) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<p${attr("id", helperId())} class="helper-text svelte-py80wu">${escape_html(helper)}</p>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> `);
    if (error) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div${attr("id", errorId())} class="error-container svelte-py80wu" role="alert" aria-live="assertive"><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="error-icon svelte-py80wu" aria-hidden="true"><circle cx="12" cy="12" r="10"></circle><line x1="12" x2="12" y1="8" y2="12"></line><line x1="12" x2="12.01" y1="16" y2="16"></line></svg> <p class="error-text svelte-py80wu">${escape_html(error)}</p></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div>`);
  });
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
