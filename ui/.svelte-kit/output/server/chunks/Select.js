import { f as stringify, e as escape_html, g as ensure_array_like, j as bind_props, h as derived } from "./root.js";
function Select($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let {
      value = "",
      options,
      placeholder,
      error,
      disabled,
      id,
      class: className = "",
      $$slots,
      $$events,
      ...rest
    } = $$props;
    let selectRef = null;
    let isFocused = false;
    function focus() {
    }
    const baseClasses = "h-9 w-full appearance-none rounded border bg-white px-3 py-2 pr-10 text-sm font-sans transition-colors duration-150";
    const stateClasses = derived(() => () => {
      if (disabled) {
        return "border-[#CCCCCC] bg-gray-50 text-muted cursor-not-allowed";
      }
      if (error) {
        return "border-danger bg-white text-ink focus:border-danger focus:outline-none focus:ring-2 focus:ring-danger/20";
      }
      if (isFocused) {
        return "border-primary bg-white text-ink focus:outline-none focus:ring-2 focus:ring-primary/20";
      }
      return "border-[#CCCCCC] bg-white text-ink hover:border-muted focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20";
    });
    $$renderer2.push(`<div class="relative">`);
    $$renderer2.select(
      {
        this: selectRef,
        value,
        disabled,
        id,
        class: `${stringify(baseClasses)} ${stringify(stateClasses()())} ${stringify(className)}`,
        onfocus: () => isFocused = true,
        onblur: () => isFocused = false,
        ...rest
      },
      ($$renderer3) => {
        if (placeholder) {
          $$renderer3.push("<!--[0-->");
          $$renderer3.option({ value: "", disabled: true, selected: true }, ($$renderer4) => {
            $$renderer4.push(`${escape_html(placeholder)}`);
          });
        } else {
          $$renderer3.push("<!--[-1-->");
        }
        $$renderer3.push(`<!--]--><!--[-->`);
        const each_array = ensure_array_like(options);
        for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
          let option = each_array[$$index];
          $$renderer3.option({ value: option.value, disabled: option.disabled }, ($$renderer4) => {
            $$renderer4.push(`${escape_html(option.label)}`);
          });
        }
        $$renderer3.push(`<!--]-->`);
      }
    );
    $$renderer2.push(` <div class="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-muted" aria-hidden="true"><svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"></path></svg></div></div>`);
    bind_props($$props, { value, focus });
  });
}
export {
  Select as S
};
