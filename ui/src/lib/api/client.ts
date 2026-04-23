import { env } from '$env/dynamic/public';
import { goto } from '$app/navigation';
import { toast } from '$lib/stores/toast';
import type {
  APIErrorEnvelope,
  CreateNetworkInput,
  CreateStoragePoolInput,
  CreateVMInput,
  CreateNodeInput,
  CreateNodeResponse,
  UpdateNodeInput,
  Event,
  Image,
  ImportProgress,
  InstallActionResponse,
  InstallStatusResponse,
  LoginResponse,
  Network,
  Node,
  NodeWithResources,
  Operation,
  StoragePool,
  UserInfo,
  VM,
  VMMetrics,
  VMMetricsResponse,
  BulkVMResponse,
  VMSnapshot,
  VMTemplate,
  CreateVMTemplateInput,
  CloneFromTemplateInput,
  CloudInitTemplate,
  CreateCloudInitTemplateInput,
  RenderCloudInitTemplateInput,
  RenderCloudInitTemplateResponse,
  VLANNetwork,
  CreateVLANInput,
  DHCPServerConfig,
  ConfigureDHCPInput,
  DHCPLease,
  FirewallRule,
  CreateFirewallRuleInput,
  Quota,
  Usage,
  UsageWithQuota,
  CheckQuotaRequest,
  CheckQuotaResponse,
  SetQuotaInput,
  UpdateQuotaInput,
  BackupHistory,
  BackupJob,
  BackupJobResponse,
  CreateBackupJobInput,
} from '$lib/api/types';

const DEFAULT_BASE_URL = env.PUBLIC_CHV_API_BASE_URL || ''; // Empty string means same origin
const TOKEN_STORAGE_KEY = 'chv-api-token';

function canUseStorage(): boolean {
  return typeof localStorage !== 'undefined' && typeof localStorage.getItem === 'function';
}

export function getStoredToken(): string | null {
  if (!canUseStorage()) {
    return null;
  }
  return localStorage.getItem(TOKEN_STORAGE_KEY);
}

export function storeToken(token: string): void {
  if (canUseStorage() && typeof localStorage.setItem === 'function') {
    localStorage.setItem(TOKEN_STORAGE_KEY, token);
  }
}

export function clearToken(): void {
  if (canUseStorage() && typeof localStorage.removeItem === 'function') {
    localStorage.removeItem(TOKEN_STORAGE_KEY);
  }
}

/**
 * Decode the JWT role claim from the stored token.
 * Handles base64url encoding (RFC 7515) by normalising before decoding.
 * Returns the role string or null if the token is absent / malformed.
 */
export function getStoredRole(): string | null {
  try {
    const token = getStoredToken();
    if (!token) return null;
    const parts = token.split('.');
    if (parts.length < 3) return null;
    const segment = parts[1].replace(/-/g, '+').replace(/_/g, '/');
    const padded = segment.padEnd(Math.ceil(segment.length / 4) * 4, '=');
    const payload = JSON.parse(atob(padded));
    return payload.role ?? null;
  } catch {
    return null;
  }
}

/**
 * Custom API error class that preserves error details from the server.
 */
export class APIError extends Error {
  public readonly status: number;
  public readonly code: string;
  public readonly retryable: boolean;
  public readonly hint?: string;

  constructor(
    message: string,
    status: number,
    code: string,
    retryable: boolean = false,
    hint?: string
  ) {
    super(message);
    this.name = 'APIError';
    this.status = status;
    this.code = code;
    this.retryable = retryable;
    this.hint = hint;
  }
}

/**
 * Check if an error is a network error (fetch failed to connect).
 */
function isNetworkError(error: unknown): boolean {
  return error instanceof TypeError && 
    (error.message.includes('fetch') || 
     error.message.includes('Network') ||
     error.message.includes('Failed to fetch'));
}

/**
 * Get a user-friendly error message based on the error type.
 */
function getUserFriendlyMessage(error: unknown): string {
  if (error instanceof APIError) {
    // Return the server message for API errors
    return error.message;
  }
  
  if (error instanceof TypeError && isNetworkError(error)) {
    return 'Unable to connect to the server. Please check your network connection and try again.';
  }
  
  if (error instanceof Error) {
    return error.message;
  }
  
  return 'An unexpected error occurred. Please try again.';
}

function getHeader(response: Response, name: string): string | null {
  return response.headers?.get?.(name) ?? null;
}

function isJsonResponse(response: Response): boolean {
  const contentType = getHeader(response, 'content-type');
  return contentType?.toLowerCase().includes('application/json') ?? false;
}

async function parseJSONResponse<T>(response: Response, path: string): Promise<T> {
  if (response.status === 204 || getHeader(response, 'content-length') === '0') {
    return undefined as T;
  }

  if (!isJsonResponse(response)) {
    let bodyPrefix = '';
    try {
      bodyPrefix = (await response.text()).trim().slice(0, 64);
    } catch {
      bodyPrefix = '';
    }

    const contentType = getHeader(response, 'content-type') ?? 'unknown content-type';
    throw new APIError(
      `Expected JSON response from ${path} but received ${contentType}.`,
      response.status,
      'INVALID_RESPONSE',
      false,
      bodyPrefix ? `Response starts with "${bodyPrefix}"` : undefined
    );
  }

  try {
    return (await response.json()) as T;
  } catch {
    throw new APIError(
      `Failed to parse JSON response from ${path}.`,
      response.status,
      'INVALID_RESPONSE'
    );
  }
}

export function createAPIClient(options?: { baseUrl?: string; token?: string }) {
  const baseUrl = options?.baseUrl ?? DEFAULT_BASE_URL;
  let token = options?.token ?? getStoredToken() ?? '';

  async function request<T>(path: string, init?: RequestInit): Promise<T> {
    const headers = new Headers(init?.headers ?? {});
    headers.set('Content-Type', 'application/json');
    if (token) {
      headers.set('Authorization', `Bearer ${token}`);
    }

    let response: Response;
    
    try {
      response = await fetch(`${baseUrl}${path}`, {
        ...init,
        headers
      });
    } catch (fetchError) {
      // Network error (server unreachable, CORS, etc.)
      const message = getUserFriendlyMessage(fetchError);
      
      // Log for debugging
      console.error('API Network Error:', {
        path,
        error: fetchError,
        timestamp: new Date().toISOString()
      });
      
      // Show toast for network errors
      toast.error(message);
      
      throw new Error(message);
    }

    if (!response.ok) {
      let payload: APIErrorEnvelope | undefined;
      try {
        if (!isJsonResponse(response)) {
          throw new Error('non-json error response');
        }
        payload = (await response.json()) as APIErrorEnvelope;
      } catch {
        payload = undefined;
      }

      // Handle 401 Unauthorized - clear token and redirect to login
      if (response.status === 401) {
        clearToken();
        
        // Show toast notification
        toast.error('Session expired. Please log in again.');
        
        // Redirect to login page
        if (typeof window !== 'undefined') {
          // Use goto if we're in a Svelte context, otherwise window.location
          try {
            await goto('/login');
          } catch {
            window.location.href = '/login';
          }
        }
        
        throw new APIError(
          'Session expired. Please log in again.',
          401,
          'UNAUTHORIZED',
          false
        );
      }

      // Log error for debugging
      console.error('API Error:', {
        path,
        status: response.status,
        code: payload?.error.code,
        message: payload?.error.message,
        retryable: payload?.error.retryable,
        hint: payload?.error.hint,
        timestamp: new Date().toISOString()
      });

      // Create APIError with full details
      const error = new APIError(
        payload?.error.message ?? `Request failed with status ${response.status}`,
        response.status,
        payload?.error.code ?? 'UNKNOWN_ERROR',
        payload?.error.retryable ?? false,
        payload?.error.hint
      );

      // Show toast for server errors (5xx) and unexpected errors
      if (response.status >= 500) {
        toast.error('A server error occurred. Please try again later.');
      }

      throw error;
    }

    return parseJSONResponse<T>(response, path);
  }

  async function upload<T>(path: string, formData: FormData): Promise<T> {
    if (token) {
      // Note: Don't set Content-Type header manually for FormData,
      // the browser will do it automatically and include the boundary.
    }

    const headers = new Headers();
    if (token) {
      headers.set('Authorization', `Bearer ${token}`);
    }

    let response: Response;
    try {
      response = await fetch(`${baseUrl}${path}`, {
        method: 'POST',
        headers,
        body: formData
      });
    } catch (fetchError) {
      const message = getUserFriendlyMessage(fetchError);
      toast.error(message);
      throw new Error(message);
    }

    if (!response.ok) {
      // Same error handling as request function... (simplified for now)
      throw new Error(`Upload failed with status ${response.status}`);
    }

    return parseJSONResponse<T>(response, path);
  }

  return {
    setToken(next: string) {
      token = next;
      storeToken(next);
    },
    clearToken() {
      token = '';
      clearToken();
    },
    createToken(name: string) {
      return request<{ id: string; token: string; message: string }>('/api/v1/tokens', {
        method: 'POST',
        body: JSON.stringify({ name })
      });
    },
    validateLogin() {
      return request<{ ok: boolean }>('/api/v1/login/validate', { method: 'POST' });
    },
    getInstallStatus() {
      return request<InstallStatusResponse>('/api/v1/install/status');
    },
    bootstrapInstall() {
      return request<InstallActionResponse>('/api/v1/install/bootstrap', {
        method: 'POST',
        body: JSON.stringify({})
      });
    },
    repairInstall(body: Record<string, boolean>) {
      return request<InstallActionResponse>('/api/v1/install/repair', {
        method: 'POST',
        body: JSON.stringify(body)
      });
    },
    listNetworks() {
      return request<Network[]>('/api/v1/networks');
    },
    createNetwork(data: CreateNetworkInput) {
      return request<Network>('/api/v1/networks', {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    getNetwork(networkId: string) {
      return request<Network>(`/api/v1/networks/${networkId}`);
    },
    updateNetwork(networkId: string, data: Partial<CreateNetworkInput>) {
      return request<Network>(`/api/v1/networks/${networkId}`, {
        method: 'PATCH',
        body: JSON.stringify(data)
      });
    },
    deleteNetwork(networkId: string) {
      return request<void>(`/api/v1/networks/${networkId}`, { method: 'DELETE' });
    },
    listStoragePools() {
      return request<StoragePool[]>('/api/v1/storage-pools');
    },
    createStoragePool(data: CreateStoragePoolInput) {
      return request<StoragePool>('/api/v1/storage-pools', {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    listImages() {
      return request<Image[]>('/api/v1/images');
    },
    importImage(data: {
      name: string;
      source_url: string;
      checksum?: string;
      os_family?: string;
      architecture?: string;
      format?: string;
    }) {
      return request<Image>('/api/v1/images/import', {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    uploadImage(formData: FormData) {
      return upload<Image>('/api/v1/images/upload', formData);
    },
    listVMs() {
      return request<VM[]>('/api/v1/vms');
    },
    createVM(data: CreateVMInput) {
      return request<VM>('/api/v1/vms', {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    listOperations() {
      return request<Operation[]>('/api/v1/operations');
    },
    getVM(id: string) {
      return request<VM>(`/api/v1/vms/${id}`);
    },
    startVM(id: string) {
      return request<VM>(`/api/v1/vms/${id}/start`, { method: 'POST' });
    },
    stopVM(id: string) {
      return request<VM>(`/api/v1/vms/${id}/stop`, { method: 'POST' });
    },
    restartVM(id: string) {
      return request<{ message: string }>(`/api/v1/vms/${id}/restart`, { method: 'POST' });
    },
    deleteVM(id: string) {
      return request<void>(`/api/v1/vms/${id}`, { method: 'DELETE' });
    },
    listEvents(query = '') {
      return request<Event[]>(`/api/v1/events${query}`);
    },
    getVMMetrics(id: string) {
      return request<VMMetricsResponse>(`/api/v1/vms/${id}/metrics`);
    },
    bulkStartVMs(ids: string[]) {
      return request<BulkVMResponse>('/api/v1/vms/bulk/start', {
        method: 'POST',
        body: JSON.stringify({ ids })
      });
    },
    bulkStopVMs(ids: string[]) {
      return request<BulkVMResponse>('/api/v1/vms/bulk/stop', {
        method: 'POST',
        body: JSON.stringify({ ids })
      });
    },
    bulkDeleteVMs(ids: string[]) {
      return request<BulkVMResponse>('/api/v1/vms/bulk/delete', {
        method: 'POST',
        body: JSON.stringify({ ids })
      });
    },
    getVMConsoleURL(id: string) {
      return request<{ ws_url: string; message: string }>(`/api/v1/vms/${id}/console`);
    },
    getVMStatus(id: string) {
      return request<{
        id: string;
        actual_state: string;
        desired_state: string;
        pid: number;
        uptime: number;
        last_error: string;
        updated_at: string;
      }>(`/api/v1/vms/${id}/status`);
    },
    getImageProgress(id: string) {
      return request<ImportProgress>(`/api/v1/images/${id}/progress`);
    },
    login(username: string, password: string) {
      return request<LoginResponse>('/api/v1/auth/login', {
        method: 'POST',
        body: JSON.stringify({ username, password })
      });
    },
    logout() {
      return request<void>('/api/v1/auth/logout', { method: 'POST' });
    },
    getCurrentUser() {
      return request<UserInfo>('/api/v1/auth/me');
    },
    async listVMSnapshots(id: string) {
      const res = await request<{ items: VMSnapshot[] }>('/api/v1/vms/snapshots', {
        method: 'POST',
        body: JSON.stringify({ vm_id: id })
      });
      return res.items ?? [];
    },
    createVMSnapshot(id: string, data?: { name?: string; description?: string; includes_memory?: boolean }) {
      return request<VMSnapshot>('/api/v1/vms/snapshots/create', {
        method: 'POST',
        body: JSON.stringify({ vm_id: id, ...(data ?? {}) })
      });
    },
    restoreVMSnapshot(_vmId: string, snapId: string) {
      return request<{ status: string }>('/api/v1/vms/snapshots/restore', {
        method: 'POST',
        body: JSON.stringify({ snapshot_id: snapId })
      });
    },
    deleteVMSnapshot(_vmId: string, snapId: string) {
      return request<{ status: string }>('/api/v1/vms/snapshots/delete', {
        method: 'POST',
        body: JSON.stringify({ snapshot_id: snapId })
      });
    },
    // Node management endpoints
    listNodes() {
      return request<NodeWithResources[]>('/api/v1/nodes');
    },
    createNode(data: CreateNodeInput) {
      return request<CreateNodeResponse>('/api/v1/nodes', {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    getNode(nodeId: string) {
      return request<NodeWithResources>(`/api/v1/nodes/${nodeId}`);
    },
    getNodeMetrics(nodeId: string) {
      return request<any>(`/api/v1/nodes/${nodeId}/metrics`);
    },
    updateNode(nodeId: string, data: UpdateNodeInput) {
      return request<NodeWithResources>(`/api/v1/nodes/${nodeId}`, {
        method: 'PATCH',
        body: JSON.stringify(data)
      });
    },
    deleteNode(nodeId: string) {
      return request<void>(`/api/v1/nodes/${nodeId}`, { method: 'DELETE' });
    },
    setNodeMaintenance(nodeId: string, enabled: boolean) {
      return request<{ message: string; maintenance: boolean; status: string }>(`/api/v1/nodes/${nodeId}/maintenance`, {
        method: 'POST',
        body: JSON.stringify({ enabled })
      });
    },
    discoverNode(nodeId: string) {
      return request<{
        node_id: string;
        discovered: string[];
        added: string[];
        count: number;
      }>(`/api/v1/nodes/${nodeId}/discover`, { method: 'POST' });
    },
    // Node-scoped resource endpoints
    listNodeVMs(nodeId: string) {
      return request<{
        node_id: string;
        node_name: string;
        resources: VM[];
        count: number;
      }>(`/api/v1/nodes/${nodeId}/vms`);
    },
    listNodeImages(nodeId: string) {
      return request<{
        node_id: string;
        node_name: string;
        resources: Image[];
        count: number;
      }>(`/api/v1/nodes/${nodeId}/images`);
    },
    listNodeStoragePools(nodeId: string) {
      return request<{
        node_id: string;
        node_name: string;
        resources: StoragePool[];
        count: number;
      }>(`/api/v1/nodes/${nodeId}/storage`);
    },
    listNodeNetworks(nodeId: string) {
      return request<{
        node_id: string;
        node_name: string;
        resources: Network[];
        count: number;
      }>(`/api/v1/nodes/${nodeId}/networks`);
    },
    // VM Power Actions
    shutdownVM(id: string, timeout?: number) {
      const query = timeout ? `?timeout=${timeout}` : '';
      return request<{ message: string; timeout: number }>(`/api/v1/vms/${id}/shutdown${query}`, { method: 'POST' });
    },
    forceStopVM(id: string) {
      return request<{ message: string }>(`/api/v1/vms/${id}/force-stop`, { method: 'POST' });
    },
    resetVM(id: string) {
      return request<{ message: string }>(`/api/v1/vms/${id}/reset`, { method: 'POST' });
    },
    restartVMWithOptions(id: string, graceful?: boolean, timeout?: number) {
      const params = new URLSearchParams();
      if (graceful !== undefined) params.append('graceful', String(graceful));
      if (timeout !== undefined) params.append('timeout', String(timeout));
      const query = params.toString() ? `?${params.toString()}` : '';
      return request<{ message: string; graceful: boolean; timeout: number }>(`/api/v1/vms/${id}/restart${query}`, { method: 'POST' });
    },
    // VM Templates
    listVMTemplates() {
      return request<VMTemplate[]>('/v1/vm-templates');
    },
    createVMTemplate(data: CreateVMTemplateInput) {
      return request<VMTemplate>('/v1/vm-templates', {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    getVMTemplate(id: string) {
      return request<VMTemplate>(`/v1/vm-templates/${id}`);
    },
    deleteVMTemplate(id: string) {
      return request<void>(`/v1/vm-templates/${id}`, { method: 'DELETE' });
    },
    cloneFromTemplate(templateId: string, data: CloneFromTemplateInput) {
      return request<VM>(`/v1/vm-templates/${templateId}/clone`, {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    previewVMTemplate(id: string) {
      return request<VMTemplate>(`/v1/vm-templates/${id}/preview`);
    },
    // Cloud-init Templates
    listCloudInitTemplates() {
      return request<CloudInitTemplate[]>('/v1/cloud-init-templates');
    },
    getCloudInitTemplate(id: string) {
      return request<CloudInitTemplate>(`/v1/cloud-init-templates/${id}`);
    },
    createCloudInitTemplate(data: CreateCloudInitTemplateInput) {
      return request<CloudInitTemplate>('/v1/cloud-init-templates', {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    deleteCloudInitTemplate(id: string) {
      return request<void>(`/v1/cloud-init-templates/${id}`, { method: 'DELETE' });
    },
    renderCloudInitTemplate(templateId: string, data: RenderCloudInitTemplateInput) {
      return request<RenderCloudInitTemplateResponse>(`/v1/cloud-init-templates/${templateId}/render`, {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    applyCloudInit(vmId: string, data: { template_id?: string; variables?: Record<string, string>; user_data?: string }) {
      return request<{ message: string; vm_id: string; warning: string }>(`/api/v1/vms/${vmId}/cloud-init/apply`, {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    // VLAN endpoints
    listVLANs(networkId: string) {
      return request<VLANNetwork[]>(`/api/v1/networks/${networkId}/vlans`);
    },
    createVLAN(networkId: string, data: CreateVLANInput) {
      return request<VLANNetwork>(`/api/v1/networks/${networkId}/vlans`, {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    deleteVLAN(networkId: string, vlanId: string) {
      return request<{ success: boolean }>(`/api/v1/networks/${networkId}/vlans/${vlanId}`, {
        method: 'DELETE'
      });
    },
    // DHCP endpoints
    getDHCPStatus(networkId: string) {
      return request<DHCPServerConfig>(`/api/v1/networks/${networkId}/dhcp`);
    },
    configureDHCP(networkId: string, data: ConfigureDHCPInput) {
      return request<DHCPServerConfig>(`/api/v1/networks/${networkId}/dhcp`, {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    startDHCPServer(networkId: string) {
      return request<{ message: string; is_running: boolean }>(`/api/v1/networks/${networkId}/dhcp/start`, {
        method: 'POST'
      });
    },
    stopDHCPServer(networkId: string) {
      return request<{ message: string; is_running: boolean }>(`/api/v1/networks/${networkId}/dhcp/stop`, {
        method: 'POST'
      });
    },
    getDHCPLeases(networkId: string) {
      return request<DHCPLease[]>(`/api/v1/networks/${networkId}/dhcp/leases`);
    },
    // Firewall endpoints
    listFirewallRules(vmId: string) {
      return request<FirewallRule[]>(`/api/v1/vms/${vmId}/firewall/rules`);
    },
    createFirewallRule(vmId: string, data: CreateFirewallRuleInput) {
      return request<FirewallRule>(`/api/v1/vms/${vmId}/firewall/rules`, {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    deleteFirewallRule(vmId: string, ruleId: string) {
      return request<{ success: boolean }>(`/api/v1/vms/${vmId}/firewall/rules/${ruleId}`, {
        method: 'DELETE'
      });
    },
    // Backup Jobs
    listBackupJobs() {
      return request<BackupJobResponse[]>('/api/v1/backup-jobs');
    },
    createBackupJob(data: CreateBackupJobInput) {
      return request<BackupJob>('/api/v1/backup-jobs', {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    deleteBackupJob(id: string) {
      return request<{ success: boolean }>(`/api/v1/backup-jobs/${id}`, { method: 'DELETE' });
    },
    runBackupJob(id: string) {
      return request<BackupHistory>(`/api/v1/backup-jobs/${id}/run`, { method: 'POST' });
    },
    toggleBackupJob(id: string) {
      return request<{ success: boolean; enabled: boolean }>(`/api/v1/backup-jobs/${id}/toggle`, { method: 'POST' });
    },
    // VM Backups
    listVMBackups(vmId: string) {
      return request<BackupHistory[]>(`/api/v1/vms/${vmId}/backups`);
    },
    listBackupHistory() {
      return request<BackupHistory[]>('/api/v1/backup-history');
    },
    // Export/Import
    exportVM(vmId: string) {
      return request<{ export_id: string; filename: string; download_url: string }>(`/api/v1/vms/${vmId}/export`, { method: 'POST' });
    },
    downloadExport(exportId: string) {
      return fetch(`${baseUrl}/api/v1/exports/${exportId}/download`, {
        headers: token ? { 'Authorization': `Bearer ${token}` } : {}
      });
    },
    importVM(file: File, name: string) {
      const formData = new FormData();
      formData.append('file', file);
      formData.append('name', name);
      return upload<VM>('/api/v1/vms/import', formData);
    },
    // Quota endpoints
    listQuotas() {
      return request<Quota[]>('/v1/quotas');
    },
    createQuota(data: SetQuotaInput) {
      return request<Quota>('/v1/quotas/create', {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    getQuota(userId: string) {
      return request<Quota>(`/v1/quotas/${userId}`);
    },
    getMyQuota() {
      return request<Quota>('/v1/quotas/me');
    },
    updateQuota(userId: string, data: UpdateQuotaInput) {
      return request<Quota>(`/v1/quotas/${userId}`, {
        method: 'PATCH',
        body: JSON.stringify(data)
      });
    },
    getUsage() {
      return request<UsageWithQuota>('/v1/usage');
    },
    getUserUsage(userId: string) {
      return request<UsageWithQuota>(`/v1/quotas/${userId}/usage`);
    },
    checkQuota(data: CheckQuotaRequest) {
      return request<CheckQuotaResponse>('/v1/quotas/check', {
        method: 'POST',
        body: JSON.stringify(data)
      });
    },
    deleteQuota(userId: string) {
      return request<{ success: boolean }>(`/v1/quotas/${userId}`, {
        method: 'DELETE'
      });
    }
  };
}
