import type { APIError } from '@/types';

export class CHVError extends Error {
  public readonly code: string;
  public readonly resourceType?: string;
  public readonly resourceId?: string;
  public readonly retryable: boolean;
  public readonly hint?: string;
  public readonly statusCode?: number;

  constructor(
    message: string,
    code: string,
    options: {
      resourceType?: string;
      resourceId?: string;
      retryable?: boolean;
      hint?: string;
      statusCode?: number;
    } = {}
  ) {
    super(message);
    this.name = 'CHVError';
    this.code = code;
    this.resourceType = options.resourceType;
    this.resourceId = options.resourceId;
    this.retryable = options.retryable ?? false;
    this.hint = options.hint;
    this.statusCode = options.statusCode;
  }

  toJSON(): APIError {
    return {
      code: this.code,
      message: this.message,
      resource_type: this.resourceType,
      resource_id: this.resourceId,
      retryable: this.retryable,
      hint: this.hint,
    };
  }
}

export function isCHVError(error: unknown): error is CHVError {
  return error instanceof CHVError;
}

export function normalizeError(error: unknown, statusCode?: number): CHVError {
  // Already a CHVError
  if (isCHVError(error)) {
    return error;
  }

  // Standard Error
  if (error instanceof Error) {
    return new CHVError(error.message, 'UNKNOWN_ERROR', { statusCode });
  }

  // Backend error response
  if (typeof error === 'object' && error !== null) {
    const err = error as Record<string, unknown>;
    return new CHVError(
      String(err.message || 'An unknown error occurred'),
      String(err.code || 'UNKNOWN_ERROR'),
      {
        resourceType: err.resource_type as string | undefined,
        resourceId: err.resource_id as string | undefined,
        retryable: err.retryable as boolean | undefined,
        hint: err.hint as string | undefined,
        statusCode,
      }
    );
  }

  // Fallback
  return new CHVError('An unknown error occurred', 'UNKNOWN_ERROR', { statusCode });
}

export function isAuthError(error: unknown): boolean {
  if (!isCHVError(error)) return false;
  return error.statusCode === 401 || error.statusCode === 403 || error.code === 'UNAUTHORIZED';
}

export function isNotFoundError(error: unknown): boolean {
  if (!isCHVError(error)) return false;
  return error.statusCode === 404 || error.code === 'NOT_FOUND';
}

export function isConflictError(error: unknown): boolean {
  if (!isCHVError(error)) return false;
  return error.statusCode === 409 || error.code === 'CONFLICT';
}

export function isValidationError(error: unknown): boolean {
  if (!isCHVError(error)) return false;
  return error.statusCode === 400 || error.code === 'INVALID_REQUEST';
}
