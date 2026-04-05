import { getApiBaseUrl } from '@/lib/env';
import { getToken } from '@/lib/auth/token';
import { CHVError, normalizeError, isAuthError } from '@/lib/errors';

export interface RequestConfig extends RequestInit {
  timeout?: number;
}

const DEFAULT_TIMEOUT = 30000;

export async function http<T>(
  path: string,
  config: RequestConfig = {}
): Promise<T> {
  const { timeout = DEFAULT_TIMEOUT, ...fetchConfig } = config;
  const baseUrl = getApiBaseUrl();
  const token = getToken();

  const url = `${baseUrl}${path}`;

  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    'Accept': 'application/json',
  };

  if (fetchConfig.headers) {
    const configHeaders = fetchConfig.headers as Record<string, string>;
    Object.entries(configHeaders).forEach(([key, value]) => {
      headers[key] = value;
    });
  }

  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeout);

  try {
    const response = await fetch(url, {
      ...fetchConfig,
      headers,
      signal: controller.signal,
    });

    clearTimeout(timeoutId);

    // Parse response body
    let data: unknown;
    const contentType = response.headers.get('content-type');
    if (contentType?.includes('application/json')) {
      data = await response.json();
    } else {
      data = await response.text();
    }

    // Handle error responses
    if (!response.ok) {
      const error = normalizeError(data, response.status);
      
      // Handle auth errors specially
      if (isAuthError(error)) {
        // Dispatch auth error event for the app to handle
        window.dispatchEvent(new CustomEvent('chv:auth:error', { detail: error }));
      }
      
      throw error;
    }

    return data as T;
  } catch (error) {
    clearTimeout(timeoutId);

    // Handle abort/timeout
    if (error instanceof DOMException && error.name === 'AbortError') {
      throw new CHVError('Request timed out', 'TIMEOUT_ERROR', { retryable: true });
    }

    // Handle network errors
    if (error instanceof TypeError && error.message.includes('fetch')) {
      throw new CHVError(
        'Unable to connect to the server. Please check your network connection.',
        'NETWORK_ERROR',
        { retryable: true }
      );
    }

    // Re-throw CHV errors
    if (error instanceof CHVError) {
      throw error;
    }

    // Wrap unknown errors
    throw normalizeError(error);
  }
}

// HTTP method helpers
export function get<T>(path: string, config?: RequestConfig): Promise<T> {
  return http<T>(path, { ...config, method: 'GET' });
}

export function post<T>(path: string, body: unknown, config?: RequestConfig): Promise<T> {
  return http<T>(path, {
    ...config,
    method: 'POST',
    body: JSON.stringify(body),
  });
}

export function put<T>(path: string, body: unknown, config?: RequestConfig): Promise<T> {
  return http<T>(path, {
    ...config,
    method: 'PUT',
    body: JSON.stringify(body),
  });
}

export function patch<T>(path: string, body: unknown, config?: RequestConfig): Promise<T> {
  return http<T>(path, {
    ...config,
    method: 'PATCH',
    body: JSON.stringify(body),
  });
}

export function del<T>(path: string, config?: RequestConfig): Promise<T> {
  return http<T>(path, { ...config, method: 'DELETE' });
}
