import { post } from '@/lib/api/http';
import type { Token, TokenCreateRequest } from '@/types';

const BASE_PATH = '/api/v1/tokens';

export const tokensApi = {
  create: (data: TokenCreateRequest) => post<Token>(BASE_PATH, data),
};
