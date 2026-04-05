// Environment configuration

// Support runtime config from window.ENV (for Docker deployments)
// Falls back to build-time env var, then default
const runtimeEnv = (typeof window !== 'undefined' && (window as unknown as { ENV?: { API_BASE_URL?: string } }).ENV) || {};

export const ENV = {
  API_BASE_URL: runtimeEnv.API_BASE_URL || import.meta.env.VITE_CHV_API_BASE_URL || 'http://localhost:8081',
} as const;

export function getApiBaseUrl(): string {
  return ENV.API_BASE_URL;
}
