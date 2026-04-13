import { p as public_env } from "./shared-server.js";
import { g as goto } from "./client.js";
import { w as writable } from "./exports.js";
function generateId() {
  return `${Date.now().toString(36)}-${Math.random().toString(36).substring(2, 9)}`;
}
function createToastStore() {
  const { subscribe, update } = writable({ toasts: [] });
  const timeouts = /* @__PURE__ */ new Map();
  function showToast(message, type, duration) {
    const id = generateId();
    const toast2 = { id, type, message, duration };
    update((state) => ({
      toasts: [...state.toasts, toast2]
    }));
    if (duration !== void 0 && duration > 0) {
      const timeout = setTimeout(() => {
        dismiss(id);
      }, duration);
      timeouts.set(id, timeout);
    }
  }
  function dismiss(id) {
    const timeout = timeouts.get(id);
    if (timeout) {
      clearTimeout(timeout);
      timeouts.delete(id);
    }
    update((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id)
    }));
  }
  function success(message) {
    showToast(message, "success", 5e3);
  }
  function error(message) {
    showToast(message, "error");
  }
  function info(message) {
    showToast(message, "info", 5e3);
  }
  return {
    subscribe,
    showToast,
    success,
    error,
    info,
    dismiss
  };
}
const toast = createToastStore();
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
  async function upload(path, formData) {
    const headers = new Headers();
    if (token) {
      headers.set("Authorization", `Bearer ${token}`);
    }
    let response;
    try {
      response = await fetch(`${baseUrl}${path}`, {
        method: "POST",
        headers,
        body: formData
      });
    } catch (fetchError) {
      const message = getUserFriendlyMessage(fetchError);
      toast.error(message);
      throw new Error(message);
    }
    if (!response.ok) {
      throw new Error(`Upload failed with status ${response.status}`);
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
      return request("/api/v1/tokens", {
        method: "POST",
        body: JSON.stringify({ name })
      });
    },
    validateLogin() {
      return request("/api/v1/login/validate", { method: "POST" });
    },
    getInstallStatus() {
      return request("/api/v1/install/status");
    },
    bootstrapInstall() {
      return request("/api/v1/install/bootstrap", {
        method: "POST",
        body: JSON.stringify({})
      });
    },
    repairInstall(body) {
      return request("/api/v1/install/repair", {
        method: "POST",
        body: JSON.stringify(body)
      });
    },
    listNetworks() {
      return request("/api/v1/networks");
    },
    createNetwork(data) {
      return request("/api/v1/networks", {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    getNetwork(networkId) {
      return request(`/api/v1/networks/${networkId}`);
    },
    updateNetwork(networkId, data) {
      return request(`/api/v1/networks/${networkId}`, {
        method: "PATCH",
        body: JSON.stringify(data)
      });
    },
    deleteNetwork(networkId) {
      return request(`/api/v1/networks/${networkId}`, { method: "DELETE" });
    },
    listStoragePools() {
      return request("/api/v1/storage-pools");
    },
    createStoragePool(data) {
      return request("/api/v1/storage-pools", {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    listImages() {
      return request("/api/v1/images");
    },
    importImage(data) {
      return request("/api/v1/images/import", {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    uploadImage(formData) {
      return upload("/api/v1/images/upload", formData);
    },
    listVMs() {
      return request("/api/v1/vms");
    },
    createVM(data) {
      return request("/api/v1/vms", {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    listOperations() {
      return request("/api/v1/operations");
    },
    getVM(id) {
      return request(`/api/v1/vms/${id}`);
    },
    startVM(id) {
      return request(`/api/v1/vms/${id}/start`, { method: "POST" });
    },
    stopVM(id) {
      return request(`/api/v1/vms/${id}/stop`, { method: "POST" });
    },
    restartVM(id) {
      return request(`/api/v1/vms/${id}/restart`, { method: "POST" });
    },
    deleteVM(id) {
      return request(`/api/v1/vms/${id}`, { method: "DELETE" });
    },
    listEvents(query = "") {
      return request(`/api/v1/events${query}`);
    },
    getVMMetrics(id) {
      return request(`/api/v1/vms/${id}/metrics`);
    },
    bulkStartVMs(ids) {
      return request("/api/v1/vms/bulk/start", {
        method: "POST",
        body: JSON.stringify({ ids })
      });
    },
    bulkStopVMs(ids) {
      return request("/api/v1/vms/bulk/stop", {
        method: "POST",
        body: JSON.stringify({ ids })
      });
    },
    bulkDeleteVMs(ids) {
      return request("/api/v1/vms/bulk/delete", {
        method: "POST",
        body: JSON.stringify({ ids })
      });
    },
    getVMConsoleURL(id) {
      return request(`/api/v1/vms/${id}/console`);
    },
    getVMStatus(id) {
      return request(`/api/v1/vms/${id}/status`);
    },
    getImageProgress(id) {
      return request(`/api/v1/images/${id}/progress`);
    },
    login(username, password) {
      return request("/api/v1/auth/login", {
        method: "POST",
        body: JSON.stringify({ username, password })
      });
    },
    logout() {
      return request("/api/v1/auth/logout", { method: "POST" });
    },
    getCurrentUser() {
      return request("/api/v1/auth/me");
    },
    listVMSnapshots(id) {
      return request(`/api/v1/vms/${id}/snapshots`);
    },
    createVMSnapshot(id) {
      return request(`/api/v1/vms/${id}/snapshots`, { method: "POST" });
    },
    restoreVMSnapshot(vmId, snapId) {
      return request(`/api/v1/vms/${vmId}/snapshots/${snapId}/restore`, { method: "POST" });
    },
    deleteVMSnapshot(vmId, snapId) {
      return request(`/api/v1/vms/${vmId}/snapshots/${snapId}`, { method: "DELETE" });
    },
    // Node management endpoints
    listNodes() {
      return request("/api/v1/nodes");
    },
    createNode(data) {
      return request("/api/v1/nodes", {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    getNode(nodeId) {
      return request(`/api/v1/nodes/${nodeId}`);
    },
    updateNode(nodeId, data) {
      return request(`/api/v1/nodes/${nodeId}`, {
        method: "PATCH",
        body: JSON.stringify(data)
      });
    },
    deleteNode(nodeId) {
      return request(`/api/v1/nodes/${nodeId}`, { method: "DELETE" });
    },
    setNodeMaintenance(nodeId, enabled) {
      return request(`/api/v1/nodes/${nodeId}/maintenance`, {
        method: "POST",
        body: JSON.stringify({ enabled })
      });
    },
    discoverNode(nodeId) {
      return request(`/api/v1/nodes/${nodeId}/discover`, { method: "POST" });
    },
    // Node-scoped resource endpoints
    listNodeVMs(nodeId) {
      return request(`/api/v1/nodes/${nodeId}/vms`);
    },
    listNodeImages(nodeId) {
      return request(`/api/v1/nodes/${nodeId}/images`);
    },
    listNodeStoragePools(nodeId) {
      return request(`/api/v1/nodes/${nodeId}/storage`);
    },
    listNodeNetworks(nodeId) {
      return request(`/api/v1/nodes/${nodeId}/networks`);
    },
    // VM Power Actions
    shutdownVM(id, timeout) {
      const query = timeout ? `?timeout=${timeout}` : "";
      return request(`/api/v1/vms/${id}/shutdown${query}`, { method: "POST" });
    },
    forceStopVM(id) {
      return request(`/api/v1/vms/${id}/force-stop`, { method: "POST" });
    },
    resetVM(id) {
      return request(`/api/v1/vms/${id}/reset`, { method: "POST" });
    },
    restartVMWithOptions(id, graceful, timeout) {
      const params = new URLSearchParams();
      if (graceful !== void 0) params.append("graceful", String(graceful));
      if (timeout !== void 0) params.append("timeout", String(timeout));
      const query = params.toString() ? `?${params.toString()}` : "";
      return request(`/api/v1/vms/${id}/restart${query}`, { method: "POST" });
    },
    getBootLogs(id, lines) {
      const query = lines ? `?lines=${lines}` : "";
      return request(`/api/v1/vms/${id}/boot-logs${query}`);
    },
    // VM Templates
    listVMTemplates() {
      return request("/api/v1/vm-templates");
    },
    createVMTemplate(data) {
      return request("/api/v1/vm-templates", {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    getVMTemplate(id) {
      return request(`/api/v1/vm-templates/${id}`);
    },
    deleteVMTemplate(id) {
      return request(`/api/v1/vm-templates/${id}`, { method: "DELETE" });
    },
    cloneFromTemplate(templateId, data) {
      return request(`/api/v1/vm-templates/${templateId}/clone`, {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    previewVMTemplate(id) {
      return request(`/api/v1/vm-templates/${id}/preview`);
    },
    // Cloud-init Templates
    listCloudInitTemplates() {
      return request("/api/v1/cloud-init-templates");
    },
    getCloudInitTemplate(id) {
      return request(`/api/v1/cloud-init-templates/${id}`);
    },
    createCloudInitTemplate(data) {
      return request("/api/v1/cloud-init-templates", {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    deleteCloudInitTemplate(id) {
      return request(`/api/v1/cloud-init-templates/${id}`, { method: "DELETE" });
    },
    renderCloudInitTemplate(templateId, data) {
      return request(`/api/v1/cloud-init-templates/${templateId}/render`, {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    applyCloudInit(vmId, data) {
      return request(`/api/v1/vms/${vmId}/cloud-init/apply`, {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    // VLAN endpoints
    listVLANs(networkId) {
      return request(`/api/v1/networks/${networkId}/vlans`);
    },
    createVLAN(networkId, data) {
      return request(`/api/v1/networks/${networkId}/vlans`, {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    deleteVLAN(networkId, vlanId) {
      return request(`/api/v1/networks/${networkId}/vlans/${vlanId}`, {
        method: "DELETE"
      });
    },
    // DHCP endpoints
    getDHCPStatus(networkId) {
      return request(`/api/v1/networks/${networkId}/dhcp`);
    },
    configureDHCP(networkId, data) {
      return request(`/api/v1/networks/${networkId}/dhcp`, {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    startDHCPServer(networkId) {
      return request(`/api/v1/networks/${networkId}/dhcp/start`, {
        method: "POST"
      });
    },
    stopDHCPServer(networkId) {
      return request(`/api/v1/networks/${networkId}/dhcp/stop`, {
        method: "POST"
      });
    },
    getDHCPLeases(networkId) {
      return request(`/api/v1/networks/${networkId}/dhcp/leases`);
    },
    // Firewall endpoints
    listFirewallRules(vmId) {
      return request(`/api/v1/vms/${vmId}/firewall/rules`);
    },
    createFirewallRule(vmId, data) {
      return request(`/api/v1/vms/${vmId}/firewall/rules`, {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    deleteFirewallRule(vmId, ruleId) {
      return request(`/api/v1/vms/${vmId}/firewall/rules/${ruleId}`, {
        method: "DELETE"
      });
    },
    // Backup Jobs
    listBackupJobs() {
      return request("/api/v1/backup-jobs");
    },
    createBackupJob(data) {
      return request("/api/v1/backup-jobs", {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    deleteBackupJob(id) {
      return request(`/api/v1/backup-jobs/${id}`, { method: "DELETE" });
    },
    runBackupJob(id) {
      return request(`/api/v1/backup-jobs/${id}/run`, { method: "POST" });
    },
    toggleBackupJob(id) {
      return request(`/api/v1/backup-jobs/${id}/toggle`, { method: "POST" });
    },
    // VM Backups
    listVMBackups(vmId) {
      return request(`/api/v1/vms/${vmId}/backups`);
    },
    listBackupHistory() {
      return request("/api/v1/backup-history");
    },
    // Export/Import
    exportVM(vmId) {
      return request(`/api/v1/vms/${vmId}/export`, { method: "POST" });
    },
    downloadExport(exportId) {
      return fetch(`${baseUrl}/api/v1/exports/${exportId}/download`, {
        headers: token ? { "Authorization": `Bearer ${token}` } : {}
      });
    },
    importVM(file, name) {
      const formData = new FormData();
      formData.append("file", file);
      formData.append("name", name);
      return upload("/api/v1/vms/import", formData);
    },
    // Quota endpoints
    listQuotas() {
      return request("/api/v1/quotas");
    },
    createQuota(data) {
      return request("/api/v1/quotas", {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    getQuota(userId) {
      return request(`/api/v1/quotas/${userId}`);
    },
    getMyQuota() {
      return request("/api/v1/quotas/me");
    },
    updateQuota(userId, data) {
      return request(`/api/v1/quotas/${userId}`, {
        method: "PATCH",
        body: JSON.stringify(data)
      });
    },
    getUsage() {
      return request("/api/v1/usage");
    },
    getUserUsage(userId) {
      return request(`/api/v1/quotas/${userId}/usage`);
    },
    checkQuota(data) {
      return request("/api/v1/quotas/check", {
        method: "POST",
        body: JSON.stringify(data)
      });
    },
    deleteQuota(userId) {
      return request(`/api/v1/quotas/${userId}`, {
        method: "DELETE"
      });
    }
  };
}
export {
  APIError as A,
  createAPIClient as c,
  getStoredToken as g,
  toast as t
};
