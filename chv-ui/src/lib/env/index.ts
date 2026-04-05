// Environment configuration

export const ENV = {
  API_BASE_URL: import.meta.env.VITE_CHV_API_BASE_URL || 'http://localhost:8081',
} as const;

export function getApiBaseUrl(): string {
  return ENV.API_BASE_URL;
}
