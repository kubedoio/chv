import { env } from '$env/dynamic/public';
import { goto } from '$app/navigation';
import { toast } from '$lib/stores/toast';
import type {
  APIErrorEnvelope,
  CreateNetworkInput,
  CreateStoragePoolInput,
  CreateVMInput,
  Event,
  Image,
  ImportProgress,
  InstallActionResponse,
  InstallStatusResponse,
  LoginResponse,
  Network,
  Operation,
  StoragePool,
  UserInfo,
  VM,
  VMMetrics,
  VMMetricsResponse,
  BulkVMResponse,
  VMSnapshot,
} from '$lib/api/types';

const DEFAULT_BASE_URL = env.PUBLIC_CHV_API_BASE_URL || ''; // Empty string means same origin
const TOKEN_STORAGE_KEY = 'chv-api-token';

export function getStoredToken(): string | null {
  if (typeof localStorage === 'undefined') {
    return null;
  }
  return localStorage.getItem(TOKEN_STORAGE_KEY);
}

export function storeToken(token: string): void {
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(TOKEN_STORAGE_KEY, token);
  }
}

export function clearToken(): void {
  if (typeof localStorage !== 'undefined') {
    localStorage.removeItem(TOKEN_STORAGE_KEY);
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

    return (await response.json()) as T;
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

    return (await response.json()) as T;
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
    listVMSnapshots(id: string) {
      return request<VMSnapshot[]>(`/api/v1/vms/${id}/snapshots`);
    },
    createVMSnapshot(id: string) {
      return request<VMSnapshot>(`/api/v1/vms/${id}/snapshots`, { method: 'POST' });
    },
    restoreVMSnapshot(vmId: string, snapId: string) {
      return request<{ success: boolean }>(`/api/v1/vms/${vmId}/snapshots/${snapId}/restore`, { method: 'POST' });
    },
    deleteVMSnapshot(vmId: string, snapId: string) {
      return request<{ success: boolean }>(`/api/v1/vms/${vmId}/snapshots/${snapId}`, { method: 'DELETE' });
    }
  };
}
