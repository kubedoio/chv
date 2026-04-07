import type {
  APIErrorEnvelope,
  Image,
  InstallActionResponse,
  InstallStatusResponse,
  Network,
  Operation,
  StoragePool,
  VM
} from '$lib/api/types';

const DEFAULT_BASE_URL = 'http://localhost:8080/api/v1';
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

export function createAPIClient(options?: { baseUrl?: string; token?: string }) {
  const baseUrl = options?.baseUrl ?? DEFAULT_BASE_URL;
  let token = options?.token ?? getStoredToken() ?? '';

  async function request<T>(path: string, init?: RequestInit): Promise<T> {
    const headers = new Headers(init?.headers ?? {});
    headers.set('Content-Type', 'application/json');
    if (token) {
      headers.set('Authorization', `Bearer ${token}`);
    }

    const response = await fetch(`${baseUrl}${path}`, {
      ...init,
      headers
    });

    if (!response.ok) {
      let payload: APIErrorEnvelope | undefined;
      try {
        payload = (await response.json()) as APIErrorEnvelope;
      } catch {
        payload = undefined;
      }
      throw new Error(payload?.error.message ?? `Request failed with status ${response.status}`);
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
      return request<{ id: string; token: string; message: string }>('/tokens', {
        method: 'POST',
        body: JSON.stringify({ name })
      });
    },
    validateLogin() {
      return request<{ ok: boolean }>('/login/validate', { method: 'POST' });
    },
    getInstallStatus() {
      return request<InstallStatusResponse>('/install/status');
    },
    bootstrapInstall() {
      return request<InstallActionResponse>('/install/bootstrap', {
        method: 'POST',
        body: JSON.stringify({})
      });
    },
    repairInstall(body: Record<string, boolean>) {
      return request<InstallActionResponse>('/install/repair', {
        method: 'POST',
        body: JSON.stringify(body)
      });
    },
    listNetworks() {
      return request<Network[]>('/networks');
    },
    listStoragePools() {
      return request<StoragePool[]>('/storage-pools');
    },
    listImages() {
      return request<Image[]>('/images');
    },
    listVMs() {
      return request<VM[]>('/vms');
    },
    listOperations() {
      return request<Operation[]>('/operations');
    }
  };
}

