import { k as bind_props, c as attr, e as escape_html } from "./renderer.js";
import { M as Modal } from "./Modal.js";
import { F as FormField, I as Input } from "./Input.js";
import { S as Select } from "./Select.js";
import { c as createAPIClient, g as getStoredToken } from "./client2.js";
import "./toast.js";
function CreateNetworkModal($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { open = false, onSuccess } = $$props;
    createAPIClient({ token: getStoredToken() ?? void 0 });
    let name = "";
    let mode = "bridge";
    let bridgeName = "chvbr0";
    let cidr = "10.0.0.0/24";
    let gatewayIp = "10.0.0.1";
    let submitting = false;
    let nameError = "";
    let bridgeNameError = "";
    let cidrError = "";
    let gatewayIpError = "";
    const nameRegex = /^[a-z0-9-]+$/;
    const cidrRegex = /^(\d{1,3}\.){3}\d{1,3}\/\d{1,2}$/;
    const ipRegex = /^(\d{1,3}\.){3}\d{1,3}$/;
    const modeOptions = [{ value: "bridge", label: "bridge" }];
    function validateName() {
      if (!name.trim()) {
        nameError = "Name is required";
        return false;
      }
      if (!nameRegex.test(name)) {
        nameError = "Name must contain only lowercase letters, numbers, and hyphens";
        return false;
      }
      if (name.startsWith("-") || name.endsWith("-")) {
        nameError = "Name cannot start or end with a hyphen";
        return false;
      }
      nameError = "";
      return true;
    }
    function validateBridgeName() {
      if (!bridgeName.trim()) {
        bridgeNameError = "Bridge name is required";
        return false;
      }
      bridgeNameError = "";
      return true;
    }
    function validateCidr() {
      if (!cidr.trim()) {
        cidrError = "CIDR is required";
        return false;
      }
      if (!cidrRegex.test(cidr)) {
        cidrError = "CIDR must be in format x.x.x.x/x (e.g., 10.0.0.0/24)";
        return false;
      }
      const [ip, prefix] = cidr.split("/");
      const octets = ip.split(".").map(Number);
      if (octets.some((o) => o < 0 || o > 255)) {
        cidrError = "IP octets must be between 0 and 255";
        return false;
      }
      const prefixNum = Number(prefix);
      if (prefixNum < 0 || prefixNum > 32) {
        cidrError = "Prefix must be between 0 and 32";
        return false;
      }
      cidrError = "";
      return true;
    }
    function validateGateway() {
      if (!gatewayIp.trim()) {
        gatewayIpError = "Gateway IP is required";
        return false;
      }
      if (!ipRegex.test(gatewayIp)) {
        gatewayIpError = "Gateway must be a valid IP address (e.g., 10.0.0.1)";
        return false;
      }
      const octets = gatewayIp.split(".").map(Number);
      if (octets.some((o) => o < 0 || o > 255)) {
        gatewayIpError = "IP octets must be between 0 and 255";
        return false;
      }
      gatewayIpError = "";
      return true;
    }
    function isValid() {
      if (!name.trim() || !bridgeName.trim() || !cidr.trim() || !gatewayIp.trim()) {
        return false;
      }
      if (!nameRegex.test(name) || name.startsWith("-") || name.endsWith("-")) {
        return false;
      }
      if (!cidrRegex.test(cidr)) return false;
      if (!ipRegex.test(gatewayIp)) return false;
      const [ip] = cidr.split("/");
      if (ip.split(".").map(Number).some((o) => o < 0 || o > 255)) return false;
      if (gatewayIp.split(".").map(Number).some((o) => o < 0 || o > 255)) return false;
      return true;
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      {
        let footer = function($$renderer4) {
          $$renderer4.push(`<button type="button"${attr("disabled", submitting, true)} class="px-4 py-2 rounded border border-line text-ink bg-white hover:bg-chrome transition-colors disabled:opacity-50 disabled:cursor-not-allowed">Cancel</button> <button type="submit" form="create-network-form"${attr("disabled", !isValid() || submitting, true)} class="px-4 py-2 rounded bg-primary text-white font-medium hover:bg-primary/90 transition-colors disabled:bg-primary/30 disabled:cursor-not-allowed flex items-center gap-2">`);
          {
            $$renderer4.push("<!--[-1-->");
          }
          $$renderer4.push(`<!--]--> ${escape_html("Create Network")}</button>`);
        };
        Modal($$renderer3, {
          title: "Create Network",
          closeOnBackdrop: !submitting,
          get open() {
            return open;
          },
          set open($$value) {
            open = $$value;
            $$settled = false;
          },
          footer,
          children: ($$renderer4) => {
            $$renderer4.push(`<form id="create-network-form" class="space-y-5">`);
            {
              $$renderer4.push("<!--[-1-->");
            }
            $$renderer4.push(`<!--]--> `);
            FormField($$renderer4, {
              label: "Name",
              error: nameError,
              required: true,
              labelFor: "network-name",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "network-name",
                  placeholder: "my-network",
                  disabled: submitting,
                  onblur: validateName,
                  get value() {
                    return name;
                  },
                  set value($$value) {
                    name = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "Mode",
              helper: "Only 'bridge' mode is supported in MVP-1",
              labelFor: "network-mode",
              children: ($$renderer5) => {
                Select($$renderer5, {
                  id: "network-mode",
                  options: modeOptions,
                  disabled: true,
                  get value() {
                    return mode;
                  },
                  set value($$value) {
                    mode = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "Bridge Name",
              error: bridgeNameError,
              required: true,
              labelFor: "bridge-name",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "bridge-name",
                  placeholder: "chvbr0",
                  disabled: submitting,
                  onblur: validateBridgeName,
                  get value() {
                    return bridgeName;
                  },
                  set value($$value) {
                    bridgeName = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "CIDR",
              error: cidrError,
              required: true,
              labelFor: "network-cidr",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "network-cidr",
                  placeholder: "10.0.0.0/24",
                  disabled: submitting,
                  onblur: validateCidr,
                  get value() {
                    return cidr;
                  },
                  set value($$value) {
                    cidr = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----> `);
            FormField($$renderer4, {
              label: "Gateway IP",
              error: gatewayIpError,
              required: true,
              labelFor: "gateway-ip",
              children: ($$renderer5) => {
                Input($$renderer5, {
                  id: "gateway-ip",
                  placeholder: "10.0.0.1",
                  disabled: submitting,
                  onblur: validateGateway,
                  get value() {
                    return gatewayIp;
                  },
                  set value($$value) {
                    gatewayIp = $$value;
                    $$settled = false;
                  }
                });
              }
            });
            $$renderer4.push(`<!----></form>`);
          },
          $$slots: { footer: true, default: true }
        });
      }
    }
    do {
      $$settled = true;
      $$inner_renderer = $$renderer2.copy();
      $$render_inner($$inner_renderer);
    } while (!$$settled);
    $$renderer2.subsume($$inner_renderer);
    bind_props($$props, { open });
  });
}
export {
  CreateNetworkModal as C
};
