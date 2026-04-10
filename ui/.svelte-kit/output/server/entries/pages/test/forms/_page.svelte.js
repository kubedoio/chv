import { e as escape_html } from "../../../../chunks/renderer.js";
import "clsx";
import { I as Input, F as FormField } from "../../../../chunks/Input.js";
import { S as Select } from "../../../../chunks/Select.js";
function _page($$renderer) {
  let nameValue = "";
  let emailValue = "";
  let typeValue = "";
  let nameError = "";
  let emailError = "Email is required";
  const typeOptions = [
    { value: "bridge", label: "Bridge" },
    { value: "nat", label: "NAT" },
    { value: "host-only", label: "Host Only" },
    {
      value: "disabled-opt",
      label: "Disabled Option",
      disabled: true
    }
  ];
  const categoryOptions = [
    { value: "vm", label: "Virtual Machine" },
    { value: "network", label: "Network" },
    { value: "storage", label: "Storage" }
  ];
  let $$settled = true;
  let $$inner_renderer;
  function $$render_inner($$renderer2) {
    $$renderer2.push(`<div class="p-8 max-w-2xl"><h1 class="text-2xl font-semibold mb-8">Form Components Test</h1> <section class="mb-12"><h2 class="text-lg font-medium mb-4 pb-2 border-b border-line">Input Component</h2> <div class="space-y-6"><div><h3 class="text-sm font-medium text-muted mb-2">Default State</h3> `);
    Input($$renderer2, {
      placeholder: "Enter your name",
      get value() {
        return nameValue;
      },
      set value($$value) {
        nameValue = $$value;
        $$settled = false;
      }
    });
    $$renderer2.push(`<!----></div> <div><h3 class="text-sm font-medium text-muted mb-2">Focus State (click input)</h3> `);
    Input($$renderer2, { placeholder: "Click to focus" });
    $$renderer2.push(`<!----></div> <div><h3 class="text-sm font-medium text-muted mb-2">With Value</h3> `);
    Input($$renderer2, { value: "John Doe" });
    $$renderer2.push(`<!----></div> <div><h3 class="text-sm font-medium text-muted mb-2">Disabled State</h3> `);
    Input($$renderer2, { value: "Disabled value", disabled: true });
    $$renderer2.push(`<!----></div> <div><h3 class="text-sm font-medium text-muted mb-2">Error State</h3> `);
    Input($$renderer2, {
      error: emailError,
      placeholder: "Enter email",
      type: "email",
      get value() {
        return emailValue;
      },
      set value($$value) {
        emailValue = $$value;
        $$settled = false;
      }
    });
    $$renderer2.push(`<!----></div> <div><h3 class="text-sm font-medium text-muted mb-2">Password Type</h3> `);
    Input($$renderer2, {
      type: "password",
      value: "secret123",
      placeholder: "Enter password"
    });
    $$renderer2.push(`<!----></div> <div><h3 class="text-sm font-medium text-muted mb-2">Number Type</h3> `);
    Input($$renderer2, { type: "number", placeholder: "Enter number" });
    $$renderer2.push(`<!----></div></div></section> <section class="mb-12"><h2 class="text-lg font-medium mb-4 pb-2 border-b border-line">Select Component</h2> <div class="space-y-6"><div><h3 class="text-sm font-medium text-muted mb-2">With Placeholder</h3> `);
    Select($$renderer2, {
      options: typeOptions,
      placeholder: "Select network type",
      get value() {
        return typeValue;
      },
      set value($$value) {
        typeValue = $$value;
        $$settled = false;
      }
    });
    $$renderer2.push(`<!----> <p class="text-xs text-muted mt-2">Selected: ${escape_html(typeValue || "(none)")}</p></div> <div><h3 class="text-sm font-medium text-muted mb-2">Pre-selected Value</h3> `);
    Select($$renderer2, { value: "vm", options: categoryOptions });
    $$renderer2.push(`<!----></div> <div><h3 class="text-sm font-medium text-muted mb-2">Disabled State</h3> `);
    Select($$renderer2, { options: categoryOptions, disabled: true });
    $$renderer2.push(`<!----></div> <div><h3 class="text-sm font-medium text-muted mb-2">Error State</h3> `);
    Select($$renderer2, { options: categoryOptions, error: "Please select a category" });
    $$renderer2.push(`<!----></div></div></section> <section class="mb-12"><h2 class="text-lg font-medium mb-4 pb-2 border-b border-line">FormField Component</h2> <div class="space-y-8">`);
    FormField($$renderer2, {
      label: "Full Name",
      children: ($$renderer3) => {
        Input($$renderer3, {
          placeholder: "Enter your full name",
          get value() {
            return nameValue;
          },
          set value($$value) {
            nameValue = $$value;
            $$settled = false;
          }
        });
      }
    });
    $$renderer2.push(`<!----> `);
    FormField($$renderer2, {
      label: "Bridge Name",
      helper: "Name of the bridge interface on the host",
      children: ($$renderer3) => {
        Input($$renderer3, { placeholder: "e.g., chvbr0" });
      }
    });
    $$renderer2.push(`<!----> `);
    FormField($$renderer2, {
      label: "Email Address",
      required: true,
      children: ($$renderer3) => {
        Input($$renderer3, { type: "email", placeholder: "user@example.com" });
      }
    });
    $$renderer2.push(`<!----> `);
    FormField($$renderer2, {
      label: "Username",
      error: nameError,
      required: true,
      children: ($$renderer3) => {
        Input($$renderer3, {
          placeholder: "Choose a username",
          get value() {
            return nameValue;
          },
          set value($$value) {
            nameValue = $$value;
            $$settled = false;
          }
        });
      }
    });
    $$renderer2.push(`<!----> <button class="mt-2 px-3 py-1.5 text-sm border border-line rounded hover:bg-chrome transition-colors">${escape_html("Show Error")}</button> `);
    FormField($$renderer2, {
      label: "Resource Type",
      helper: "Select the type of resource to create",
      children: ($$renderer3) => {
        Select($$renderer3, { options: categoryOptions, placeholder: "Select type" });
      }
    });
    $$renderer2.push(`<!----> `);
    FormField($$renderer2, {
      label: "Instance ID",
      helper: "Auto-generated identifier",
      children: ($$renderer3) => {
        Input($$renderer3, { value: "inst-12345", disabled: true });
      }
    });
    $$renderer2.push(`<!----></div></section> <section class="mb-12"><h2 class="text-lg font-medium mb-4 pb-2 border-b border-line">Create Network Example</h2> <div class="p-6 border border-line rounded-lg bg-white space-y-6">`);
    FormField($$renderer2, {
      label: "Network Name",
      required: true,
      helper: "Unique name for this network",
      children: ($$renderer3) => {
        Input($$renderer3, { placeholder: "e.g., production-vms" });
      }
    });
    $$renderer2.push(`<!----> `);
    FormField($$renderer2, {
      label: "Mode",
      required: true,
      children: ($$renderer3) => {
        Select($$renderer3, {
          options: [{ value: "bridge", label: "Bridge" }],
          value: "bridge",
          disabled: true
        });
      }
    });
    $$renderer2.push(`<!----> `);
    FormField($$renderer2, {
      label: "Bridge Interface",
      required: true,
      helper: "Host bridge interface name",
      children: ($$renderer3) => {
        Input($$renderer3, { placeholder: "e.g., br0" });
      }
    });
    $$renderer2.push(`<!----> `);
    FormField($$renderer2, {
      label: "CIDR",
      required: true,
      helper: "Network CIDR in format x.x.x.x/x",
      children: ($$renderer3) => {
        Input($$renderer3, { placeholder: "e.g., 10.0.0.0/24" });
      }
    });
    $$renderer2.push(`<!----> `);
    FormField($$renderer2, {
      label: "Gateway IP",
      helper: "Default gateway for this network",
      children: ($$renderer3) => {
        Input($$renderer3, { placeholder: "e.g., 10.0.0.1" });
      }
    });
    $$renderer2.push(`<!----> <div class="flex gap-3 pt-4 border-t border-line"><button class="button-primary px-4 py-2 rounded text-sm font-medium">Create Network</button> <button class="button-secondary px-4 py-2 rounded text-sm">Cancel</button></div></div></section></div>`);
  }
  do {
    $$settled = true;
    $$inner_renderer = $$renderer.copy();
    $$render_inner($$inner_renderer);
  } while (!$$settled);
  $$renderer.subsume($$inner_renderer);
}
export {
  _page as default
};
