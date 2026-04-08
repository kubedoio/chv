import { p as public_env } from "./shared-server.js";
import { g as goto } from "./client.js";
import { t as toast } from "./toast.js";
const DEFAULT_BASE_URL = public_env.PUBLIC_CHV_API_BASE_URL || "";
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
class APIError extends Error {
  status;
  code;
  retryable;
  hint;
  constructor(message, status, code, retryable = false, hint) {
    super(message);
    this.name = "APIError";
    this.status = status;
    this.code = code;
    this.retryable = retryable;
    this.hint = hint;
  }
}
function isNetworkError(error) {
  return error instanceof TypeError && (error.message.includes("fetch") || error.message.includes("Network") || error.message.includes("Failed to fetch"));
}
function getUserFriendlyMessage(error) {
  if (error instanceof APIError) {
    return error.message;
  }
  if (error instanceof TypeError && isNetworkError(error)) {
    return "Unable to connect to the server. Please check your network connection and try again.";
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "An unexpected error occurred. Please try again.";
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
    let response;
    try {
      response = await fetch(`${baseUrl}${path}`, {
        ...init,
        headers
      });
    } catch (fetchError) {
      const message = getUserFriendlyMessage(fetchError);
      console.error("API Network Error:", {
        path,
        error: fetchError,
        timestamp: (/* @__PURE__ */ new Date()).toISOString()
      });
      toast.error(message);
      throw new Error(message);
    }
    if (!response.ok) {
      let payload;
      try {
        payload = await response.json();
      } catch {
        payload = void 0;
      }
      if (response.status === 401) {
        clearToken();
        toast.error("Session expired. Please log in again.");
        if (typeof window !== "undefined") {
          try {
            await goto("/login");
          } catch {
            window.location.href = "/login";
          }
        }
        throw new APIError(
          "Session expired. Please log in again.",
          401,
          "UNAUTHORIZED",
          false
        );
      }
      console.error("API Error:", {
        path,
        status: response.status,
        code: payload?.error.code,
        message: payload?.error.message,
        retryable: payload?.error.retryable,
        hint: payload?.error.hint,
        timestamp: (/* @__PURE__ */ new Date()).toISOString()
      });
      const error = new APIError(
        payload?.error.message ?? `Request failed with status ${response.status}`,
        response.status,
        payload?.error.code ?? "UNKNOWN_ERROR",
        payload?.error.retryable ?? false,
        payload?.error.hint
      );
      if (response.status >= 500) {
        toast.error("A server error occurred. Please try again later.");
      }
      throw error;
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
    createNetwork(data) {
      return request("/networks", {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    listStoragePools() {
      return request("/storage-pools");
    },
    createStoragePool(data) {
      return request("/storage-pools", {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    listImages() {
      return request("/images");
    },
    importImage(data) {
      return request("/images/import", {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    listVMs() {
      return request("/vms");
    },
    createVM(data) {
      return request("/vms", {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    listOperations() {
      return request("/operations");
    },
    getVM(id) {
      return request(`/vms/${id}`);
    },
    startVM(id) {
      return request(`/vms/${id}/start`, { method: "POST" });
    },
    stopVM(id) {
      return request(`/vms/${id}/stop`, { method: "POST" });
    },
    restartVM(id) {
      return request(`/vms/${id}/restart`, { method: "POST" });
    },
    deleteVM(id) {
      return request(`/vms/${id}`, { method: "DELETE" });
    },
    listEvents(query = "") {
      return request(`/events${query}`);
    },
    getVMMetrics(id) {
      return request(`/vms/${id}/metrics`);
    },
    getVMConsoleURL(id) {
      return request(`/vms/${id}/console`);
    },
    getVMStatus(id) {
      return request(`/vms/${id}/status`);
    },
    getImageProgress(id) {
      return request(`/images/${id}/progress`);
    },
    login(username, password) {
      return request("/auth/login", {
        method: "POST",
        body: JSON.stringify({ username, password })
      });
    },
    logout() {
      return request("/auth/logout", { method: "POST" });
    },
    getCurrentUser() {
      return request("/auth/me");
    }
  };
}
export {
  createAPIClient as c,
  getStoredToken as g
};
