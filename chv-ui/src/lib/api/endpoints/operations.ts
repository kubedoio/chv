import { get } from '@/lib/api/http';
import type { Operation, ListResponse } from '@/types';

const BASE_PATH = '/api/v1/operations';

export interface OperationFilters {
  resource_type?: string;
  resource_id?: string;
  status?: string;
  operation_type?: string;
  node_id?: string;
}

export const operationsApi = {
  list: (filters?: OperationFilters) => {
    const params = new URLSearchParams();
    if (filters) {
      Object.entries(filters).forEach(([key, value]) => {
        if (value) params.append(key, value);
      });
    }
    const query = params.toString();
    return get<ListResponse<Operation>>(`${BASE_PATH}${query ? `?${query}` : ''}`);
  },
  
  get: (id: string) => get<Operation>(`${BASE_PATH}/${id}`),
  
  getLogs: (id: string) => get<unknown[]>(`${BASE_PATH}/${id}/logs`),
};
