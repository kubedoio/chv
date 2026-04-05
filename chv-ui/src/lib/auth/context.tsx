import { createContext, useContext, useState, useCallback, useEffect } from 'react';
import { getToken, setToken, clearToken, hasToken, maskToken } from './token';
import { nodesApi } from '@/lib/api';
import { isAuthError, CHVError } from '@/lib/errors';

interface AuthState {
  isAuthenticated: boolean;
  isLoading: boolean;
  error: CHVError | null;
  tokenPreview: string | null;
}

interface AuthContextValue extends AuthState {
  login: (token: string) => Promise<void>;
  logout: () => void;
  clearError: () => void;
}

const AuthContext = createContext<AuthContextValue | null>(null);

interface AuthProviderProps {
  children: React.ReactNode;
}

export function AuthProvider({ children }: AuthProviderProps) {
  const [state, setState] = useState<AuthState>({
    isAuthenticated: hasToken(),
    isLoading: false,
    error: null,
    tokenPreview: hasToken() ? maskToken(getToken() || '') : null,
  });

  // Listen for auth errors from the API
  useEffect(() => {
    const handleAuthError = (event: CustomEvent<CHVError>) => {
      setState(prev => ({
        ...prev,
        isAuthenticated: false,
        error: event.detail,
        tokenPreview: null,
      }));
      clearToken();
    };

    window.addEventListener('chv:auth:error', handleAuthError as EventListener);
    return () => {
      window.removeEventListener('chv:auth:error', handleAuthError as EventListener);
    };
  }, []);

  const login = useCallback(async (token: string) => {
    setState(prev => ({ ...prev, isLoading: true, error: null }));

    try {
      // Store token temporarily
      setToken(token);

      // Validate token by fetching nodes
      await nodesApi.list();

      // Token is valid
      setState({
        isAuthenticated: true,
        isLoading: false,
        error: null,
        tokenPreview: maskToken(token),
      });
    } catch (error) {
      // Clear the invalid token
      clearToken();

      const authError = isAuthError(error)
        ? (error as CHVError)
        : new CHVError('Invalid token or unable to connect', 'AUTH_FAILED');

      setState({
        isAuthenticated: false,
        isLoading: false,
        error: authError,
        tokenPreview: null,
      });

      throw authError;
    }
  }, []);

  const logout = useCallback(() => {
    clearToken();
    setState({
      isAuthenticated: false,
      isLoading: false,
      error: null,
      tokenPreview: null,
    });
  }, []);

  const clearError = useCallback(() => {
    setState(prev => ({ ...prev, error: null }));
  }, []);

  const value: AuthContextValue = {
    ...state,
    login,
    logout,
    clearError,
  };

  return (
    <AuthContext.Provider value={value}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth(): AuthContextValue {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
}
