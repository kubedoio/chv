const TOKEN_KEY = 'chv_auth_token';

export type TokenStorage = 'session' | 'memory';

let memoryToken: string | null = null;

export function getToken(storage: TokenStorage = 'session'): string | null {
  if (storage === 'memory') {
    return memoryToken;
  }
  return sessionStorage.getItem(TOKEN_KEY);
}

export function setToken(token: string, storage: TokenStorage = 'session'): void {
  if (storage === 'memory') {
    memoryToken = token;
  } else {
    sessionStorage.setItem(TOKEN_KEY, token);
  }
}

export function clearToken(storage: TokenStorage = 'session'): void {
  if (storage === 'memory') {
    memoryToken = null;
  } else {
    sessionStorage.removeItem(TOKEN_KEY);
  }
}

export function hasToken(storage: TokenStorage = 'session'): boolean {
  return getToken(storage) !== null;
}

export function maskToken(token: string): string {
  if (token.length <= 8) {
    return '****';
  }
  return token.slice(0, 4) + '****' + token.slice(-4);
}
