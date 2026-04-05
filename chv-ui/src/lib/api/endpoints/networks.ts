import { get, post } from '@/lib/api/http';
import type { Network, NetworkCreateRequest, ListResponse } from '@/types';

const BASE_PATH = '/api/v1/networks';

export const networksApi = {
  list: () => get<ListResponse<Network>>(BASE_PATH),
  
  get: (id: string) => get<Network>(`${BASE_PATH}/${id}`),
  
  create: (data: NetworkCreateRequest) => post<Network>(BASE_PATH, data),
};
