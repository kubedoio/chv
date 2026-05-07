import { env } from '$env/dynamic/public';
import { goto } from '$app/navigation';
import { toast } from '$lib/stores/toast.svelte';
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
  InstallActionResponse,
  InstallStatusResponse,
  LoginResponse,
  Network,
  NodeWithResources,
  Operation,
  StoragePool,
  UserInfo,
  VM,
  VMTemplate,
  CreateVMTemplateInput,
  CloneFromTemplateInput,
  CloudInitTemplate,
  CreateCloudInitTemplateInput,
  RenderCloudInitTemplateInput,
  RenderCloudInitTemplateResponse,
  Quota,
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
      
      // TODO: integrate structured logger instead of console
      // eslint-disable-next-line no-console
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

      // TODO: integrate structured logger instead of console
      // eslint-disable-next-line no-console
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


  return {
    setToken(next: string) {
      token = next;
      storeToken(next);
    },
    clearToken() {
      token = '';
      clearToken();
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
    deleteVM(id: string) {
      return request<void>(`/api/v1/vms/${id}`, { method: 'DELETE' });
    },
    listEvents(query = '') {
      return request<Event[]>(`/api/v1/events${query}`);
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
    updateNode(nodeId: string, data: UpdateNodeInput) {
      return request<NodeWithResources>(`/api/v1/nodes/${nodeId}`, {
        method: 'PATCH',
        body: JSON.stringify(data)
      });
    },
    deleteNode(nodeId: string) {
      return request<void>(`/api/v1/nodes/${nodeId}`, { method: 'DELETE' });
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
    // Quota endpoints
    listQuotas() {
      return request<Quota[]>('/v1/quotas', { method: 'POST' });
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
      return request<UsageWithQuota>('/v1/usage', { method: 'POST' });
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
