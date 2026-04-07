import { p as public_env } from './shared-server-BU2DVf8Q.js';

const DEFAULT_BASE_URL = public_env.PUBLIC_CHV_API_BASE_URL || "http://localhost:8080/api/v1";
const TOKEN_STORAGE_KEY = "chv-api-token";
function getStoredToken() {
  if (typeof localStorage === "undefined") {
    return null;
  }
  return localStorage.getItem(TOKEN_STORAGE_KEY);
}
function storeToken(token) {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(TOKEN_STORAGE_KEY, token);
  }
}
function clearToken() {
  if (typeof localStorage !== "undefined") {
    localStorage.removeItem(TOKEN_STORAGE_KEY);
  }
}
function createAPIClient(options) {
  const baseUrl = options?.baseUrl ?? DEFAULT_BASE_URL;
  let token = options?.token ?? getStoredToken() ?? "";
  async function request(path, init) {
    const headers = new Headers(init?.headers ?? {});
    headers.set("Content-Type", "application/json");
    if (token) {
      headers.set("Authorization", `Bearer ${token}`);
    }
    const response = await fetch(`${baseUrl}${path}`, {
      ...init,
      headers
    });
    if (!response.ok) {
      let payload;
      try {
        payload = await response.json();
      } catch {
        payload = void 0;
      }
      throw new Error(payload?.error.message ?? `Request failed with status ${response.status}`);
    }
    return await response.json();
  }
  return {
    setToken(next) {
      token = next;
      storeToken(next);
    },
    clearToken() {
      token = "";
      clearToken();
    },
    createToken(name) {
      return request("/tokens", {
        method: "POST",
        body: JSON.stringify({ name })
      });
    },
    validateLogin() {
      return request("/login/validate", { method: "POST" });
    },
    getInstallStatus() {
      return request("/install/status");
    },
    bootstrapInstall() {
      return request("/install/bootstrap", {
        method: "POST",
        body: JSON.stringify({})
      });
    },
    repairInstall(body) {
      return request("/install/repair", {
        method: "POST",
        body: JSON.stringify(body)
      });
    },
    listNetworks() {
      return request("/networks");
    },
    listStoragePools() {
      return request("/storage-pools");
    },
    listImages() {
      return request("/images");
    },
    listVMs() {
      return request("/vms");
    },
    listOperations() {
      return request("/operations");
    }
  };
}

export { createAPIClient as c, getStoredToken as g };
//# sourceMappingURL=client-iYtOZkWx.js.map
